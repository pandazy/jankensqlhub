# Janken SQL Hub - Database Query Management Library

A high-performance, modular Rust library for parameterizable SQL query management that prevents SQL injection through prepared statements and supports multiple database backends (currently SQLite, PostgreSQL planned).

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
-- Basic parameter syntax - @params default to string type if no args specified
SELECT * FROM users WHERE id=@user_id AND name=@user_name

-- Dynamic identifier parameters - #[xxx] syntax for table names, column names, etc. (always table_name type)
SELECT * FROM #[table_name] WHERE id=@user_id
SELECT #[column_name] FROM users ORDER BY #[column_name]

-- List parameters for IN clauses - always list type with item type validation
SELECT * FROM users WHERE id IN :[user_ids] AND status IN :[statuses]

-- Parameters in quoted strings (treated as literals)
SELECT * FROM users WHERE name='@literal_text'
```

### Architecture Design Principles

**Janken SQL Hub** serves as a **server-side query adapter**, bridging the gap between web API endpoints and database operations:

- **QueryDef**: Pre-defined, validated SQL queries stored on the server
- **query_run()**: Web request handler that maps JSON parameters to prepared statements
- **Security First**: Query templates prevent SQL injection while retaining SQL's efficiency
- **No ORM Abstraction**: Direct SQL usage avoids inefficient query builders and ORMs

```rust
// Web API Workflow:
// 1. Client sends JSON payload: {"user_id": 123}
// 2. Server uses query_name (not SQL) to identify predefined query
// 3. Parameters are validated and injected into prepared statement
// 4. Result returned as JSON

let params = serde_json::json!({"user_id": 123, "status": "active"});
let result = conn.query_run(&queries, "find_user", &params)?;
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
use janken_sql_hub::{DatabaseConnection, QueryDefinitions};

// Load from JSON file
let queries = QueryDefinitions::from_file("queries.json")?;

// Or load from JSON object
let json = serde_json::json!({...});
let queries = QueryDefinitions::from_json(json)?;
```

### 3. Execute Queries
```rust
// Setup SQLite connection
let sqlite_conn = DatabaseConnection::SQLite(Connection::open_in_memory()?);
let mut conn = DatabaseConnection::SQLite(conn);

// Get user by ID (returns QueryResult with JSON data and SQL execution details)
let params = serde_json::json!({"user_id": 42});
let query_result = conn.query_run(&queries, "get_user", &params)?;
// Access JSON results: query_result.data
// Access executed SQL statements: query_result.sql_statements (for debugging)

// Create new user
let params = serde_json::json!({"name": "Alice", "email": "alice@example.com"});
let query_result = conn.query_run(&queries, "create_user", &params)?;

// Query from dynamic table
let params = serde_json::json!({"source": "accounts", "id": 1, "name": "John"});
let query_result = conn.query_run(&queries, "query_from_table", &params)?;

// Insert into dynamic table
let params = serde_json::json!({"dest_table": "users", "name": "Bob"});
let query_result = conn.query_run(&queries, "insert_into_dynamic_table", &params)?;
```

### 4. Parameter Types and Constraints Supported

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

**Validation Logic:**
1. The conditional parameter value must match one of the defined conditions
2. If multiple conditional parameters are specified, they're evaluated alphabetically by parameter name
3. The first matching condition (alphabetically) determines the allowed values
4. Parameter values must be in the allowed array for the matching condition

**Example:**
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
  }
}
```

## ⚡ Performance Characteristics

- **Regex Compilation**: One-time lazy static initialization
- **Parameter Parsing**: O(n) where n = SQL length
- **Query Execution**: Database-dependent (SQLite ~2-3x slower prepared vs raw, PostgreSQL similar)
- **Memory Usage**: Minimal (regex + parameter vectors)
- **Zero-Copy**: Parameter values passed by reference where possible

## 🧪 Quality Assurance

- **Test Coverage**: 100% coverage
- **Zero Warnings**: `cargo clippy -- -D warnings` clean
- **Memory Safety**: Rust ownership system guarantees
- **Type Safety**: Compile-time parameter validation
- **SQL Injection**: Automatic prepared statements prevent attacks

## 📈 Roadmap

### Planned Enhancements
- [ ] PostgreSQL native connection implementation

### Database Backend Priorities
1. ✅ SQLite (complete)
2.  PostgreSQL (planned implementation)

## 🐘 PostgreSQL Testing

**JankenSQLHub** supports PostgreSQL testing through experimental modules for future multi-backend support development. See the [operational guide](op.md) for detailed setup instructions for contributors.

## 📦 Installation & Links

**Install from Crates.io:**
```bash
cargo add jankensqlhub
```

**Links:**
- [📦 Crates.io](https://crates.io/crates/jankensqlhub)
- [📚 Documentation](https://docs.rs/jankensqlhub)
- [🏠 Repository](https://github.com/pandazy/jankensqlhub)

---

**Built with ❤️ in Rust for type-safe, performant database query management.**
