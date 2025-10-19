# Release Notes v0.4.0

## ⚠️ **Breaking API Change**

### QueryResult Structure Enhancement
- `query_run()` methods now return `QueryResult` struct instead of `Vec<serde_json::Value>`
- `QueryResult` contains both JSON results and executed SQL statements for debugging
- **Breaking**: Access JSON data via `.data` field, SQL statements via `.sql_statements`

## ✨ **New Features**

### List Parameters Support
- New parameter syntax `:[list_name]` for IN clauses and array operations
- Example: `SELECT * FROM users WHERE id IN :[user_ids]`
- Automatic type assignment to "list" with optional item type validation
- Use `{"itemtype": "integer"}` in args to specify list item types

### Default Parameter Types
- `@parameter` placeholders now default to "string" type when no args specified
- No longer requires explicit type specification for string parameters
- Reduces configuration verbosity for common string parameters

## Before (v0.3.x)
```rust
let result: Vec<serde_json::Value> = conn.query_run(&queries, "my_query", &params)?;
for item in result {
    println!("Data: {:?}", item);
}
```

## After (v0.4.0)
```rust
let query_result: QueryResult = conn.query_run(&queries, "my_query", &params)?;
for item in &query_result.data {
    println!("Data: {:?}", item);
}
// New debugging feature
for sql in &query_result.sql_statements {
    println!("Executed SQL: {}", sql);
}
```

## Migration Notes
- Change all `let result` declarations to `let query_result`
- Update all direct vector operations to use `.data` field:
  - `result.len()` → `query_result.data.len()`
  - `result.iter()` → `query_result.data.iter()`
  - `result[0]` → `query_result.data[0]`
  - etc.
- **New Benefit**: Access executed SQL statements via `query_result.sql_statements` for debugging
- Existing query definitions and JSON responses remain unchanged
- All parameter validation and SQL injection protection features preserved

---
**Version 0.4.0** - Enhanced query result structure with SQL statement debugging, list parameters, and default type assignment
