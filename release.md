# Release Notes v0.3.0

## Major Feature Addition
- **table name parameter support** - Allow dynamic table names using `#table_name` syntax in SQL queries
- Table names can be parameterized with enum constraints for security and validation
- Enhanced flexibility for multi-tenant applications and dynamic schema operations

## Architecture Refactoring
- **Simplified inner structure** - Streamlined code organization for better maintainability
- Improved module separation and reduced complexity
- Enhanced parameter validation and processing pipeline
- Better error handling and constraint validation

## Examples
```rust
// Query with dynamic table name
let params = serde_json::json!({"source": "users", "id": 42});
let result = conn.query_run(&queries, "query_from_table", &params)?;

// Configuration with table name constraints
"query_from_table": {
  "query": "SELECT * FROM #source WHERE id=@id",
  "args": {
    "source": {"enum": ["users", "accounts"]},
    "id": {"type": "integer"}
  }
}
```

## Migration Notes
- Existing `@param` syntax continues to work unchanged
- No breaking changes to existing query definitions
- Table name parameters (`#table`) are validated using enum constraints only

---
**Version 0.3.0** - Enhanced parameter system with dynamic table name support
