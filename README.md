# Janken SQL Hub - Database Query Management Library

A high-performance, modular Rust library for parameterizable SQL query management that prevents SQL injection through prepared statements and supports multiple database backends (SQLite and PostgreSQL).

## 🎯 Overview

**Janken SQL Hub** enables developers to define SQL queries with parameters in a database-agnostic way, automatically generating prepared statements for different database backends while preventing SQL injection attacks.

### Core Capabilities
- ✅ **Parameterizable SQL Templates** - `@param_name` syntax in queries, types defined separately
- ✅ **Dynamic Identifiers** - `#[identifier]` syntax for parameterizable table/column names and other SQL identifiers
- ✅ **List Parameter Support** - :[list_param] syntax for IN clauses with item type validation
- ✅ **Web API Integration** - Server-side query adapter mapping JSON requests to prepared statements
- ✅ **SQL Injection Protection** - Automatic prepared statement generation
- ✅ **Type Safety & Validation** - Parameter type validation with constraints (range, pattern, enum, table name validation)
- ✅ **Parameter Constraints** - Range limits, regex patterns, enumerated values, and table name validation

## 🚀 Quick Start

**Janken SQL Hub** enables developers to define SQL queries with parameters in a database-agnostic way, automatically generating prepared statements for different database backends while preventing SQL injection attacks.

## ✨ Key Features

### Parameter Syntax
```sql
-- Basic parameter syntax - @param_name parameters default to string type (can be overridden)
SELECT * FROM users WHERE id=@user_id AND name=@user_name

-- Dynamic identifier parameters - #[xxx] syntax for table names, column names, etc. (always table_name type)
SELECT * FROM #[table_name] WHERE id=@user_id
SELECT #[column_name] FROM users ORDER BY #[column_name]

-- List parameters for IN clauses - always list type with item type validation
SELECT * FROM users WHERE id IN :[user_ids] AND status IN :[statuses]

-- Comma list parameters - ~[param] syntax for comma-separated values (always comma_list type)
SELECT ~[fields] FROM users WHERE status='active'
-- With runtime params {"fields": ["name", "email", "age"]} becomes: SELECT name,email,age FROM users WHERE status='active'

-- Parameters in quoted strings (treated as literals)
SELECT * FROM users WHERE name='@literal_text'
```

### Architecture Design Principles

**Janken SQL Hub** serves as a **server-side query adapter**, bridging the gap between web API endpoints and database operations:

- **QueryDef**: Pre-defined, validated SQL queries stored on the server
- **query_run_sqlite() / query_run_postgresql()**: Database-specific query runners that map JSON parameters to prepared statements
- **Security First**: Query templates prevent SQL injection while retaining SQL's efficiency
- **No ORM Abstraction**: Direct SQL usage avoids inefficient query builders and ORMs

```rust
// Web API Workflow:
// 1. Client sends JSON payload: {"user_id": 123}
// 2. Server uses query_name (not SQL) to identify predefined query
// 3. Parameters are validated and injected into prepared statement
// 4. Result returned as JSON

let params = serde_json::json!({"user_id": 123});
let result = query_run_sqlite(&mut conn, &queries, "find_user", &params)?;
```

## 🚀 Usage Guide

### 1. Define Queries (JSON Configuration)

Each query definition contains:
- `"query"`: Required - The SQL statement with `@parameter` (`#[table_name]`) placeholders
- `"args"`: Optional - only needed to override default types or add constraints
- `"returns"`: Optional - Array of column names for SELECT queries (determines JSON response structure)

```json
{
  "get_user": {
    "query": "SELECT id, name, email FROM users WHERE id=@user_id",
    "returns": ["id", "name", "email"],
    "args": {
      "user_id": {"type": "integer"}
    }
  },
  "create_user": {
    "query": "INSERT INTO users (name, email) VALUES (@name, @email)",
    "args": {
      "name": {"type": "string"},
      "email": {"type": "string"}
    }
  },
  "search_users": {
    "query": "SELECT id, name FROM users WHERE age > @min_age AND age < @max_age",
    "returns": ["id", "name"],
    "args": {
      "min_age": {"type": "integer"},
      "max_age": {"type": "integer"}
    }
  },
  "get_user_by_status": {
    "query": "SELECT * FROM users WHERE status=@status",
    "returns": ["id", "name", "email", "status"],
    "args": {
      "status": {
        "type": "string",
        "enum": ["active", "inactive", "pending"]
      }
    }
  },
  "get_user_by_email": {
    "query": "SELECT * FROM users WHERE email LIKE @pattern",
    "returns": ["id", "name", "email"],
    "args": {
      "pattern": {
        "type": "string",
        "pattern": "\\S+@\\S+\\.\\S+"
      }
    }
  },
  "select_fields": {
    "query": "SELECT ~[fields] FROM users WHERE status='active'",
    "returns": ["name", "email", "age"],
    "args": {
      "fields": {"enum": ["name", "email", "age"]}
    }
  },
  "dynamic_select_fields": {
    "query": "SELECT ~[fields] FROM users WHERE status='active'",
    "returns": "~[fields]",
    "args": {
      "fields": {"enum": ["name", "email", "age"]}
    }
  },
  "query_from_table": {
    "query": "SELECT * FROM #[source] WHERE id=@id AND name=@name",
    "returns": ["id", "name"],
    "args": {
      "id": {"type": "integer"},
      "name": {"type": "string"},
      "source": {"enum": ["source"]}
    }
  },
  "insert_into_dynamic_table": {
    "query": "INSERT INTO #[dest_table] (name) VALUES (@name)",
    "args": {
      "dest_table": {"enum": ["accounts", "users"]},
      "name": {"type": "string"}
    }
  },
  "get_users_by_ids": {
    "query": "SELECT id, name FROM users WHERE id IN :[user_ids]",
    "returns": ["id", "name"],
    "args": {
      "user_ids": {"itemtype": "integer"}
    }
  },
  "select_column": {
    "query": "SELECT #[column_name] FROM #[table_name] ORDER BY #[column_name]",
    "returns": ["column_value"],
    "args": {
      "column_name": {"enum": ["id", "name", "score"]},
      "table_name": {"enum": ["users", "accounts"]}
    }
  },
  "store_file": {
    "query": "INSERT INTO files (name, data, size) VALUES (@name, @data, @size)",
    "args": {
      "name": {"type": "string"},
      "data": {"type": "blob", "range": [1, 1048576]},  // 1 byte to 1MB
      "size": {"type": "integer"}
    }
  }
}
```

### 2. Load Queries
```rust
use janken_sql_hub::{QueryDefinitions, query_run_sqlite};
use rusqlite::Connection;

// Load from JSON file
let queries = QueryDefinitions::from_file("queries.json")?;

// Or load from JSON object
let json = serde_json::json!({...});
let queries = QueryDefinitions::from_json(json)?;
```

### 3. Execute Queries
```rust
// Setup SQLite connection
let mut conn = Connection::open_in_memory()?;

// Get user by ID (returns QueryResult with JSON data and SQL execution details)
let params = serde_json::json!({"user_id": 42});
let query_result = query_run_sqlite(&mut conn, &queries, "get_user", &params)?;
// Access JSON results: query_result.data
// Access executed SQL statements: query_result.sql_statements (for debugging)

// Create new user
let params = serde_json::json!({"name": "Alice", "email": "alice@example.com"});
let query_result = query_run_sqlite(&mut conn, &queries, "create_user", &params)?;

// Query from dynamic table
let params = serde_json::json!({"source": "accounts", "id": 1, "name": "John"});
let query_result = query_run_sqlite(&mut conn, &queries, "query_from_table", &params)?;

// Insert into dynamic table
let params = serde_json::json!({"dest_table": "users", "name": "Bob"});
let query_result = query_run_sqlite(&mut conn, &queries, "insert_into_dynamic_table", &params)?;
```

### 4. Important Usage Notes

**JSON null values are not supported in requests and will be rejected.** All parameter values must be non-null JSON values (strings, numbers, booleans, arrays, objects).

*Despite the convenience null might provide, it acts as a super-passport that circumvents type validation - it implicitly "matches" almost all data types when explicit "required" validation isn't specified. This leads to weaker type safety and potential security issues, so JankenSQLHub rejects null values upfront to maintain strict type validation.*

### 5. Parameter Types and Constraints Supported

**Automatic Type Assignment:**
- `@param` parameters: Default to "string" type (can be overridden)
- `#[table_name]` parameters: Automatically assigned "table_name" type
- `:[list_param]` parameters: Automatically assigned "list" type

```rust
// User-specified parameter types (all case-insensitive)
"integer", "string", "float", "boolean", "blob"

// Automatically assigned parameter types (cannot be overridden)
"table_name"  // Assigned to parameters using #[table] syntax
"list"        // Assigned to parameters using :[list] syntax
"comma_list"  // Assigned to parameters using ~[param] syntax

// Constraint types
"range": [min, max]     // For numeric types (integer/float) and blob sizes
"pattern": "regex"      // For string types (e.g., email validation)
"enum": [value1, ...]   // For any type (allowed values). Table names support enum only.
"enumif": {...}         // For conditional enum constraints based on other parameters
"itemtype": "type"      // For list types: specifies the type of each item in the list

// Examples in args object
"id": {"type": "integer"}                                                 // Basic integer (overridden from default string)
"balance": {"type": "float", "range": [0.0, 1000000.0]}                   // Float with range
"status": {"enum": ["active", "inactive", "pending"]}  // String enum
"email": { "pattern": "\\S+@\\S+\\.\\S+"}                                // String with regex
"user_ids": {"itemtype": "integer"}                                     // List of integers for IN clauses
"names": {"type": "boolean"}                                              // Explicit string type (same as default)
"source": {"enum": ["users", "accounts"]}                               // Table name enum (table_name type auto-assigned)
"tags": {                                                               // Conditional enum based on media_type
  "enumif": {
    "media_type": {
      "song": ["artist", "album", "title"],
      "show": ["channel", "category", "episodes"]
    }
  }
}
```

### Conditional Enum Constraints (`enumif`)

The `enumif` constraint allows parameter validation based on the values of other parameters, enabling conditional enums. The conditional parameter (the one referenced in `enumif`) can be any primitive type (string, number, boolean) - not just enum values.

**Structure:**
```json
{
  "parameter_with_enumif": {
    "enumif": {
      "conditional_parameter": {
        "conditional_value1": ["allowed", "values", "for", "this", "condition"],
        "conditional_value2": ["different", "allowed", "values", "here"]
      }
    }
  }
}
```

**Fuzzy Matching Patterns:**

Condition keys support both exact matches and fuzzy matching patterns:
- **Exact match**: `"value"` - matches the value exactly
- **Start pattern**: `"start:prefix"` - matches values starting with the pattern (e.g., `"start:admin"` matches `"admin_user"`, `"admin_super"`)
- **End pattern**: `"end:suffix"` - matches values ending with the pattern (e.g., `"end:txt"` matches `"document.txt"`, `"readme.txt"`)
- **Contain pattern**: `"contain:substring"` - matches values containing the pattern (e.g., `"contain:error"` matches `"error_log"`, `"system_error"`)

Note: Fuzzy match patterns must be alphanumeric with underscores only. Exact matches allow any string value.

**Validation Logic:**
1. The conditional parameter value must match one of the defined conditions (exact or fuzzy)
2. If multiple conditional parameters are specified, they're evaluated alphabetically by parameter name
3. Condition keys within each conditional parameter are also evaluated alphabetically
4. **Conflict Resolution**: When multiple patterns could match the same value (e.g., `"contain:test"` and `"start:test"` both matching `"test123"`), the first match in alphabetical order is used. In this example, `"contain:test"` would be selected since 'c' comes before 's' alphabetically.
5. The first matching condition (alphabetically) determines the allowed values
6. Parameter values must be in the allowed array for the matching condition

**Examples:**
```json
{
  "media_source": {
    "enumif": {
      "media_type": {
        "song": ["artist", "album"],
        "show": ["channel", "episodes"]
      }
    }
  },
  "priority_level": {
    "enumif": {
      "severity": {
        "high": ["urgent", "immediate"],
        "low": ["optional"]
      }
    }
  },
  "permission": {
    "enumif": {
      "role": {
        "start:admin": ["read_all", "write_all", "delete_all"],
        "start:user": ["read_own", "write_own"],
        "contain:guest": ["read_public"]
      }
    }
  },
  "action": {
    "enumif": {
      "filename": {
        "end:txt": ["read_text", "edit_text"],
        "end:jpg": ["view_image", "resize_image"]
      }
    }
  }
}
```

## 🛡️ Flexible Error Handling

**Janken SQL Hub** provides structured error handling with unique error codes and JSON metadata for better debugging and customization. Each error includes:

- **Unique Error Code**: u16 identifier for programmatic error identification
- **Structured Metadata**: JSON string containing relevant contextual error details
- **Helper Functions**: Extract metadata fields without parsing JSON
- **Error Information**: Look up comprehensive error descriptions by code

### Programmatic Error Handling
```rust
use jankensqlhub::{JankenError, error_meta, get_error_data, get_error_info, M_EXPECTED, M_GOT, M_PARAM_NAME, M_QUERY_NAME};
use std::error::Error;

// Check if this is a structured JankenError (for query/parameter validation issues)
if let Some(janken_err) = error.downcast_ref::<JankenError>() {
    // Extract error data from the JankenError variant
    let data = get_error_data(janken_err);

    // Look up comprehensive error information
    if let Some(info) = get_error_info(data.code) {
        eprintln!("{} ({}) - {}", info.name, data.code, info.description);
    }

    // Handle specific JankenError variants
    match janken_err {
        JankenError::ParameterTypeMismatch { .. } => {
            let expected = error_meta(data, M_EXPECTED)?;
            let got = error_meta(data, M_GOT)?;
            eprintln!("Type mismatch: expected {}, got {}", expected, got);
        }
        JankenError::ParameterNotProvided { .. } => {
            let param_name = error_meta(data, M_PARAM_NAME)?;
            eprintln!("Missing required parameter: {}", param_name);
        }
        JankenError::QueryNotFound { .. } => {
            let query_name = error_meta(data, M_QUERY_NAME)?;
            eprintln!("Query not found: {}", query_name);
        }
    }
} else {
    // Handle other errors (IO, JSON parsing, database connection issues, etc. from anyhow)
    eprintln!("Error: {}", error);
}
```

### Error Code Reference
| Code | Error Type | Category | Description |
|------|------------|----------|-------------|
| 2000 | QUERY_NOT_FOUND | Query | Requested query definition was not found |
| 2010 | PARAMETER_NOT_PROVIDED | Parameter | Required parameter was not provided |
| 2020 | PARAMETER_TYPE_MISMATCH | Parameter | Parameter value does not match expected type |
| 2030 | PARAMETER_NAME_CONFLICT | Parameter | Parameter name conflicts with table name |

### Example Error Metadata
- **Parameter Type Mismatch**: `{"expected": "integer", "got": "\"not_int\""}`
- **Query Not Found**: `{"query_name": "find_user_by_id"}`
- **Parameter Not Provided**: `{"parameter_name": "user_id"}`
- **Parameter Name Conflict**: `{"conflicting_name": "users"}`

## ⚡ Performance Characteristics

- **Regex Compilation**: One-time lazy static initialization
- **Parameter Parsing**: O(n) where n = SQL length
- **Query Execution**: Database-dependent (SQLite ~2-3x slower prepared vs raw, PostgreSQL similar)
- **Memory Usage**: Minimal (regex + parameter vectors)
- **Zero-Copy**: Parameter values passed by reference where possible

## 🧪 Quality Assurance

- **Test Coverage**: 100% coverage
- **Zero Warnings**: Clean clippy warnings
- **Memory Safety**: Rust ownership system guarantees
- **Type Safety**: Compile-time parameter validation
- **SQL Injection**: Automatic prepared statements prevent attacks

## 📈 Roadmap

### Planned Enhancements
- [ ] TBD

### Database Backend Priorities
1. ✅ SQLite (complete)
2. ✅ PostgreSQL (complete)

## 🐘 PostgreSQL Support

**JankenSQLHub** provides production-ready PostgreSQL support alongside SQLite. Both backends share the same API and parameter syntax, ensuring consistent behavior across database systems.

```rust
use jankensqlhub::{QueryDefinitions, query_run_postgresql};
use tokio_postgres::NoTls;

// Setup PostgreSQL connection
let (client, connection) = tokio_postgres::connect(&connection_string, NoTls).await?;
tokio::spawn(async move { if let Err(e) = connection.await { eprintln!("connection error: {}", e); } });

// Execute queries with PostgreSQL
let params = serde_json::json!({"user_id": 42});
let result = query_run_postgresql(&mut client, &queries, "get_user", &params).await?;
```

### Key Features
- **Async Execution**: Leverages tokio-postgres for high-performance async operations
- **ACID Transactions**: All query execution wrapped in transactions with automatic rollback on failure
- **Prepared Statements**: Automatic conversion to PostgreSQL `$1, $2, ...` parameter format
- **Type Safety**: Full type mapping between JSON and PostgreSQL data types including JSON/JSONB columns
- **JSON/JSONB Support**: Direct query of PostgreSQL JSON and JSONB column types with automatic serde_json conversion
- **Integration Tests**: Comprehensive test suite covering all features

See the [operational guide](op.md) for testing setup and development instructions.

## 📦 Installation & Links

**Install from Crates.io:**
```bash
cargo add jankensqlhub
```

### Feature Flags

JankenSQLHub supports feature flags to include only the database backends you need:

- **`all`** (default): Enable both SQLite and PostgreSQL support
- **`sqlite`**: Enable only SQLite support
- **`postgresql`**: Enable only PostgreSQL support

**Examples:**
```bash
# Default (both SQLite and PostgreSQL)
cargo add jankensqlhub

# SQLite only
cargo add jankensqlhub --features sqlite

# PostgreSQL only
cargo add jankensqlhub --features postgresql
```

**Links:**
- [📦 Crates.io](https://crates.io/crates/jankensqlhub)
- [📚 Documentation](https://docs.rs/jankensqlhub)
- [🏠 Repository](https://github.com/pandazy/jankensqlhub)

---

**Built with ❤️ in Rust for type-safe, performant database query management.**
