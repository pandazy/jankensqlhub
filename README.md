# Janken SQL Hub - Database Query Management Library

A Rust library for parameterizable SQL query management that prevents SQL injection through prepared statements and supports multiple database backends (SQLite and PostgreSQL).

## Table of Contents

- [Overview](#-overview)
- [Claude Code Skill](#-claude-code-skill)
- [Quick Start](#-quick-start)
- [Parameter Syntax Reference](#-parameter-syntax-reference)
- [Usage Guide](#-usage-guide)
- [Advanced Features](#-advanced-features)
- [Error Handling](#-error-handling)
- [PostgreSQL Support](#-postgresql-support)
- [Installation](#-installation)
- [Architecture](#architecture)
- [Acknowledgments](#-acknowledgments)

---

## 🎯 Overview

**Janken SQL Hub** enables developers to define SQL queries with parameters in a database-agnostic way, automatically generating prepared statements for different database backends while preventing SQL injection attacks.

### Why JSON-Configured Queries?

Common CRUD operations often become scattered across codebases, mixed with business logic, making them hard to audit and maintain. **Janken SQL Hub** solves this by:

- **Centralizing query definitions** - All SQL in portable JSON files, not buried in code
- **Co-locating SQL with constraints** - Query logic and validation rules live together
- **Enabling easy auditing** - Review all database operations in one place
- **Simplifying maintenance** - Update queries without touching application code

```json
{
  "update_user_status": {
    "query": "UPDATE users SET status=@status WHERE id=@user_id",
    "args": {
      "user_id": {"type": "integer"},
      "status": {"enum": ["active", "inactive", "suspended"]}
    }
  }
}
```
*SQL and its constraints are cohesive, clear, and reviewable.*

### Non-Invasive Design

**Janken SQL Hub** is a focused utility, not a framework:

- **No coding restrictions** - Use it for what it's good at, use something else for the rest
- **Coexists with existing code** - Works alongside raw SQL, ORMs, or any other database access pattern
- **Simple utility functions** - `query_run_sqlite()` and `query_run_postgresql()` wrap your existing connections
- **Gradual adoption** - Start with a few queries, expand as needed

```rust
// JankenSQLHub handles configured queries
let result = query_run_sqlite(&mut conn, &queries, "get_user", &params)?;

// Your existing code continues to work unchanged
conn.execute("DROP TABLE temp_data", [])?;
```

### Core Capabilities

| Capability | Description |
|------------|-------------|
| **Parameterizable SQL** | `@param_name` syntax with automatic prepared statement generation |
| **Dynamic Identifiers** | `#[identifier]` syntax for safe table/column names |
| **List Parameters** | `:[list_param]` syntax for IN clauses |
| **Comma Lists** | `~[param]` syntax for comma-separated field lists |
| **Type Safety** | Parameter validation with constraints (range, pattern, enum) |
| **Multi-Backend** | SQLite and PostgreSQL support with identical API |

---

## 🤖 Claude Code Skill

This repository includes a [Claude Code skill](https://code.claude.com/docs/en/skills) at `.claude/skills/using-jankensqlhub/SKILL.md` that provides AI-assisted guidance when working with JankenSQLHub. When using [Claude Code](https://code.claude.com), the skill is automatically discovered and gives Claude knowledge of:

- Parameter syntax and query definition structure
- Type system and constraint configuration
- SQLite and PostgreSQL execution patterns
- Structured error handling with `JankenError`

This enables Claude to generate correct JankenSQLHub code, debug parameter validation issues, and follow library conventions without needing to re-read the documentation each time.

---

## 🚀 Quick Start

### 1. Define a Query (JSON)

```json
{
  "get_user": {
    "query": "SELECT id, name, email FROM users WHERE id=@user_id",
    "returns": ["id", "name", "email"],
    "args": {
      "user_id": {"type": "integer"}
    }
  }
}
```

### 2. Execute the Query (Rust)

```rust
use janken_sql_hub::{QueryDefinitions, query_run_sqlite};
use rusqlite::Connection;

// Load queries and connect to database
let queries = QueryDefinitions::from_file("queries.json")?;
let mut conn = Connection::open("mydb.sqlite")?;

// Execute with JSON parameters
let params = serde_json::json!({"user_id": 42});
let result = query_run_sqlite(&mut conn, &queries, "get_user", &params)?;
// result.data contains the JSON response
```

That's it! The library handles prepared statements and SQL injection prevention automatically.

---

## 📖 Parameter Syntax Reference

| Syntax | Type | Description | Example |
|--------|------|-------------|---------|
| `@param` | string (default) | Basic parameter placeholder | `WHERE name=@user_name` |
| `@param` | any type | Override type in args | `"user_id": {"type": "integer"}` |
| `#[param]` | table_name | Dynamic identifier (validated) | `SELECT * FROM #[table_name]` |
| `:[param]` | list | Array for IN clauses | `WHERE id IN :[user_ids]` |
| `~[param]` | comma_list | Comma-separated values | `SELECT ~[fields] FROM users` |

### Quick Examples

```sql
-- Basic parameters (default to string, can override type)
SELECT * FROM users WHERE id=@user_id AND name=@user_name

-- Dynamic table/column names (always validated against enum)
SELECT * FROM #[table_name] WHERE id=@user_id

-- List parameters for IN clauses
SELECT * FROM users WHERE id IN :[user_ids]

-- Comma list for dynamic field selection
SELECT ~[fields] FROM users WHERE status='active'
-- With {"fields": ["name", "email"]} becomes: SELECT name,email FROM users
```

---

## 📚 Usage Guide

### Query Definition Structure

Each query definition supports these fields:

| Field | Required | Description |
|-------|----------|-------------|
| `query` | ✅ | SQL statement with parameter placeholders |
| `returns` | Optional | Column names for SELECT queries (JSON response structure) |
| `args` | Optional | Parameter type overrides and constraints |

### Basic Examples

**SELECT with parameters:**
```json
{
  "search_users": {
    "query": "SELECT id, name FROM users WHERE age > @min_age",
    "returns": ["id", "name"],
    "args": {
      "min_age": {"type": "integer"}
    }
  }
}
```

**INSERT:**
```json
{
  "create_user": {
    "query": "INSERT INTO users (name, email) VALUES (@name, @email)"
  }
}
```
*Note: `@name` and `@email` default to string type, so args can be omitted.*

**Dynamic table:**
```json
{
  "query_from_table": {
    "query": "SELECT * FROM #[source] WHERE id=@id",
    "returns": ["id", "name"],
    "args": {
      "source": {"enum": ["users", "accounts"]},
      "id": {"type": "integer"}
    }
  }
}
```

**List parameter (IN clause):**
```json
{
  "get_users_by_ids": {
    "query": "SELECT id, name FROM users WHERE id IN :[user_ids]",
    "returns": ["id", "name"],
    "args": {
      "user_ids": {"itemtype": "integer"}
    }
  }
}
```

### Executing Queries

```rust
use janken_sql_hub::{QueryDefinitions, query_run_sqlite};
use rusqlite::Connection;

let queries = QueryDefinitions::from_file("queries.json")?;
let mut conn = Connection::open_in_memory()?;

// Basic parameter
let params = serde_json::json!({"user_id": 42});
let result = query_run_sqlite(&mut conn, &queries, "get_user", &params)?;

// Dynamic table
let params = serde_json::json!({"source": "accounts", "id": 1});
let result = query_run_sqlite(&mut conn, &queries, "query_from_table", &params)?;

// List parameter
let params = serde_json::json!({"user_ids": [1, 2, 3, 4, 5]});
let result = query_run_sqlite(&mut conn, &queries, "get_users_by_ids", &params)?;
```

### Important: Null Values Not Supported

**JSON null values are rejected.** All parameter values must be non-null (strings, numbers, booleans, arrays, objects).

*Rationale: null acts as a super-passport that circumvents type validation, leading to weaker type safety and potential security issues.*

---

## ⚙️ Advanced Features

### Parameter Types and Constraints

**Supported Types:**

| Type | Description | Constraint Options |
|------|-------------|-------------------|
| `string` | Text (default for `@param`) | `pattern`, `enum`, `range` (char count) |
| `integer` | Whole numbers | `range`, `enum` |
| `float` | Decimal numbers | `range`, `enum` |
| `boolean` | true/false | `enum` |
| `blob` | Binary data | `range` (size in bytes) |
| `table_name` | Auto-assigned to `#[param]` | `enum` (required), `range` (char count) |
| `list` | Auto-assigned to `:[param]` | `itemtype`, `range` (array size) |
| `comma_list` | Auto-assigned to `~[param]` | `enum`, `range` (array size) |

**Constraint Examples:**

```json
{
  "args": {
    "age": {"type": "integer", "range": [0, 150]},
    "email": {"pattern": "\\S+@\\S+\\.\\S+"},
    "status": {"enum": ["active", "inactive", "pending"]},
    "data": {"type": "blob", "range": [1, 1048576]},
    "user_ids": {"itemtype": "integer", "range": [1, 100]},
    "table": {"enum": ["users", "accounts"]},
    "fields": {"enum": ["name", "email", "age"], "range": [1, 3]},
    "username": {"type": "string", "range": [3, 50]}
  }
}
```

**Range Constraint Semantics:**

| Type | Range Meaning |
|------|---------------|
| `integer`, `float` | Value must be within [min, max] |
| `string`, `table_name` | Character count must be within [min, max] |
| `blob` | Size in bytes must be within [min, max] |
| `list`, `comma_list` | Array size (element count) must be within [min, max] |
| `boolean` | Range not supported |

### Dynamic Returns

Map return columns dynamically using the same comma_list parameter:

```json
{
  "dynamic_select": {
    "query": "SELECT ~[fields] FROM users",
    "returns": "~[fields]",
    "args": {
      "fields": {"enum": ["name", "email", "age"]}
    }
  }
}
```

### Conditional Enum Constraints (`enumif`)

Validate parameter values based on other parameters:

```json
{
  "args": {
    "media_source": {
      "enumif": {
        "media_type": {
          "song": ["artist", "album"],
          "show": ["channel", "episodes"]
        }
      }
    }
  }
}
```

With `media_type: "song"`, `media_source` must be "artist" or "album".

**Fuzzy Matching Patterns:**

| Pattern | Description | Example |
|---------|-------------|---------|
| `"value"` | Exact match | `"admin"` matches only "admin" |
| `"start:prefix"` | Starts with | `"start:admin"` matches "admin_user" |
| `"end:suffix"` | Ends with | `"end:txt"` matches "readme.txt" |
| `"contain:str"` | Contains | `"contain:error"` matches "system_error" |

```json
{
  "permission": {
    "enumif": {
      "role": {
        "start:admin": ["read_all", "write_all", "delete_all"],
        "start:user": ["read_own", "write_own"],
        "contain:guest": ["read_public"]
      }
    }
  }
}
```

*Note: When multiple patterns could match, the first alphabetically is used.*

---

## 🛡️ Error Handling

JankenSQLHub provides structured errors with unique codes and JSON metadata.

### Basic Usage

```rust
use jankensqlhub::{JankenError, get_error_data, get_error_info};

if let Some(janken_err) = error.downcast_ref::<JankenError>() {
    let data = get_error_data(janken_err);
    
    if let Some(info) = get_error_info(data.code) {
        eprintln!("{} ({}) - {}", info.name, data.code, info.description);
    }
}
```

### Error Code Reference

| Code | Error Type | Description |
|------|------------|-------------|
| 2000 | QUERY_NOT_FOUND | Query definition not found |
| 2010 | PARAMETER_NOT_PROVIDED | Required parameter missing |
| 2020 | PARAMETER_TYPE_MISMATCH | Value doesn't match expected type |
| 2030 | PARAMETER_NAME_CONFLICT | Parameter name conflicts with table name |

### Extracting Metadata

```rust
use jankensqlhub::{error_meta, M_EXPECTED, M_GOT, M_PARAM_NAME, M_QUERY_NAME};

match janken_err {
    JankenError::ParameterTypeMismatch { .. } => {
        let expected = error_meta(data, M_EXPECTED)?;
        let got = error_meta(data, M_GOT)?;
        eprintln!("Type mismatch: expected {}, got {}", expected, got);
    }
    JankenError::ParameterNotProvided { .. } => {
        let param_name = error_meta(data, M_PARAM_NAME)?;
        eprintln!("Missing parameter: {}", param_name);
    }
    _ => {}
}
```

---

## 🐘 PostgreSQL Support

PostgreSQL support shares the same API with async execution:

```rust
use jankensqlhub::{QueryDefinitions, query_run_postgresql};
use tokio_postgres::NoTls;

// Setup connection
let (client, connection) = tokio_postgres::connect(&connection_string, NoTls).await?;
tokio::spawn(async move { 
    if let Err(e) = connection.await { 
        eprintln!("connection error: {}", e); 
    } 
});

// Execute queries (same API as SQLite)
let params = serde_json::json!({"user_id": 42});
let result = query_run_postgresql(&mut client, &queries, "get_user", &params).await?;
```

### PostgreSQL Features

- **Async Execution**: Leverages tokio-postgres for high-performance operations
- **ACID Transactions**: Automatic transaction wrapping with rollback on failure
- **Prepared Statements**: Auto-conversion to `$1, $2, ...` format
- **JSON/JSONB Support**: Direct querying with automatic serde_json conversion

See the [operational guide](op.md) for testing setup.

---

## 📦 Installation

```bash
cargo add jankensqlhub
```

### Feature Flags

| Flag | Description |
|------|-------------|
| `all` (default) | Both SQLite and PostgreSQL |
| `sqlite` | SQLite only |
| `postgresql` | PostgreSQL only |

```bash
# SQLite only
cargo add jankensqlhub --features sqlite

# PostgreSQL only
cargo add jankensqlhub --features postgresql
```

### Links

- [📦 Crates.io](https://crates.io/crates/jankensqlhub)
- [📚 Documentation](https://docs.rs/jankensqlhub)
- [🏠 Repository](https://github.com/pandazy/jankensqlhub)

---

## Architecture

**Janken SQL Hub** serves as a **server-side query adapter**, bridging web API endpoints and database operations:

```
Client JSON → QueryDef (predefined) → Prepared Statement → Database → JSON Response
```

- **No ORM**: Direct SQL usage avoids query builder overhead
- **Security First**: Query templates prevent SQL injection
- **Type Safety**: Compile-time parameter validation

---

## 🙏 Acknowledgments

This project was developed with significant assistance from [Cline](https://cline.bot/) - an autonomous AI coding agent for VS Code that handles complex software engineering tasks.

---

**Built with ❤️ in Rust for type-safe, performant database query management.**
