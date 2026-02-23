---
name: using-jankensqlhub
description: Guide for using the jankensqlhub Rust crate. Use when writing code with jankensqlhub, creating JSON query definitions, executing SQL queries via SQLite or PostgreSQL, handling JankenError, or debugging parameter validation failures.
---

# Using JankenSQLHub

## Overview

[JankenSQLHub](https://github.com/pandazy/jankensqlhub) is a Rust library for parameterized SQL query management. It replaces manual SQL construction and field validation with JSON-configured query definitions that enforce type safety and prevent SQL injection.

## Core Concept

```
JSON Query Definition → Prepared Statement → Database → JSON Response
```

All queries are defined as JSON, validated at runtime via constraints, and executed as prepared statements. No ORM — direct SQL with safety guarantees.

## Installation

```bash
cargo add jankensqlhub                    # Both SQLite + PostgreSQL
cargo add jankensqlhub --features sqlite  # SQLite only
```

## Parameter Syntax

| Syntax | Type | Purpose | Example |
|--------|------|---------|---------|
| `@param` | Value parameter | Prepared statement binding | `WHERE name=@user_name` |
| `#[param]` | Identifier | Safe table/column names | `SELECT * FROM #[table]` |
| `:[param]` | List | IN clause arrays | `WHERE id IN :[ids]` |
| `~[param]` | Comma list | Dynamic field selection | `SELECT ~[fields] FROM users` |

## Query Definition Structure

```json
{
  "query_name": {
    "query": "SQL with parameter placeholders",
    "returns": ["col1", "col2"],
    "args": {
      "param_name": { "type": "string", "constraints..." }
    }
  }
}
```

- `query` (required): SQL statement with parameter placeholders
- `returns` (optional): Column names for SELECT (enables JSON response mapping)
- `args` (optional): Parameter type overrides and constraints

## Type System

| Type | Default for | Constraint Options |
|------|-------------|-------------------|
| `string` | `@param` | `pattern`, `enum`, `enumif`, `range` (char count) |
| `integer` | — | `range`, `enum` |
| `float` | — | `range`, `enum` |
| `boolean` | — | `enum` |
| `table_name` | `#[param]` | `enum` (required), `range` (char count) |
| `list` | `:[param]` | `itemtype`, `range` (array size) |
| `comma_list` | `~[param]` | `enum`, `enumif`, `range` (array size) |

**Key rule:** `@param` defaults to `string` type — no need for `{"type": "string"}`.

## Constraints

### `enum` — Static whitelist

```json
{
  "table": {"enum": ["artist", "show", "song"]}
}
```

### `enumif` — Conditional whitelist based on another parameter

```json
{
  "fields": {
    "enumif": {
      "table": {
        "artist": ["id", "name", "status"],
        "show": ["id", "name", "vintage"],
        "song": ["id", "name", "artist_id"]
      }
    }
  }
}
```

When `table` = `"artist"`, `fields` must only contain values from `["id", "name", "status"]`.

### `enumif` fuzzy matching patterns

| Pattern | Description | Example |
|---------|-------------|---------|
| `"value"` | Exact match | `"admin"` |
| `"start:prefix"` | Starts with | `"start:admin"` matches `"admin_user"` |
| `"end:suffix"` | Ends with | `"end:txt"` matches `"readme.txt"` |
| `"contain:str"` | Contains | `"contain:error"` matches `"system_error"` |

### `range` — Value/size bounds

```json
{
  "age": {"type": "integer", "range": [0, 150]},
  "username": {"type": "string", "range": [3, 50]},
  "ids": {"itemtype": "integer", "range": [1, 100]}
}
```

Range semantics vary by type: value bounds for numbers, character count for strings, array size for lists.

### `pattern` — Regex validation

```json
{
  "email": {"pattern": "\\S+@\\S+\\.\\S+"}
}
```

## Dynamic Returns with `~[fields]`

Map return columns dynamically using comma_list:

```json
{
  "search": {
    "query": "SELECT ~[fields] FROM #[table] WHERE name=@name",
    "returns": "~[fields]",
    "args": {
      "table": {"enum": ["artist", "show"]},
      "fields": {
        "enumif": {
          "table": {
            "artist": ["id", "name"],
            "show": ["id", "name", "vintage"]
          }
        }
      }
    }
  }
}
```

When `returns` is `"~[fields]"`, the JSON response uses the actual field names passed in.

## Execution

### SQLite

```rust
use jankensqlhub::{QueryDefinitions, query_run_sqlite};
use rusqlite::Connection;
use serde_json::json;

let queries = QueryDefinitions::from_json(json!({
    "get_user": {
        "query": "SELECT ~[fields] FROM users WHERE id=@id",
        "returns": "~[fields]",
        "args": {
            "fields": {"enum": ["id", "name", "email"]},
            "id": {"type": "integer"}
        }
    }
}))?;

let mut conn = Connection::open("db.sqlite")?;
let params = json!({"id": 42, "fields": ["name", "email"]});
let result = query_run_sqlite(&mut conn, &queries, "get_user", &params)?;
// result.data is Vec<serde_json::Value>
```

### PostgreSQL (async)

```rust
use jankensqlhub::{QueryDefinitions, query_run_postgresql};

let result = query_run_postgresql(&mut client, &queries, "get_user", &params).await?;
```

## Error Handling

JankenSQLHub produces structured errors:

| Code | Error Type | Description |
|------|------------|-------------|
| 2000 | `QueryNotFound` | Query name not in definitions |
| 2010 | `ParameterNotProvided` | Required parameter missing |
| 2020 | `ParameterTypeMismatch` | Value fails type/constraint validation |
| 2030 | `ParameterNameConflict` | Parameter name conflicts with table name |

### Reading error details

```rust
use jankensqlhub::{JankenError, get_error_data, get_error_info, error_meta};
use jankensqlhub::{M_EXPECTED, M_GOT, M_PARAM_NAME};

if let Some(janken_err) = error.downcast_ref::<JankenError>() {
    let data = get_error_data(janken_err);
    if let Some(info) = get_error_info(data.code) {
        eprintln!("{} ({}) - {}", info.name, data.code, info.description);
    }
    // Extract metadata
    let param = error_meta(&data, M_PARAM_NAME);
    let expected = error_meta(&data, M_EXPECTED);
    let got = error_meta(&data, M_GOT);
}
```

### Preserving error metadata through custom error types

When wrapping JankenSQLHub errors in a custom error enum, use `From<anyhow::Error>` with `downcast_ref` to preserve structured metadata instead of losing it with `.to_string()`:

```rust
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        if let Some(janken_err) = err.downcast_ref::<JankenError>() {
            let data = get_error_data(janken_err);
            let param = error_meta(&data, M_PARAM_NAME).unwrap_or_default();
            let got = error_meta(&data, M_GOT).unwrap_or_default();
            let info_name = get_error_info(data.code)
                .map(|i| i.name.to_string())
                .unwrap_or_else(|| err.to_string());
            return AppError::Internal(format!("{info_name}: param={param}, got={got}"));
        }
        AppError::Internal(err.to_string())
    }
}
```

**Critical:** Use `.map_err(AppError::from)` on `query_run_sqlite` calls, NOT `.map_err(|e| AppError::Internal(e.to_string()))` — the latter discards JankenSQLHub metadata.

## Important Rules

1. **Null values are rejected** — all parameter values must be non-null
2. **`#[param]` requires `enum`** — identifier parameters must always have an `enum` or `enumif` constraint
3. **`@param` defaults to string** — omit `{"type": "string"}` for string params
4. **Multiple queries per definition** — a single `QueryDefinitions` can hold many named queries
5. **`from_json()` for runtime, `from_file()` for static** — both produce the same `QueryDefinitions`