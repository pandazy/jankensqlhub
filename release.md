# Release Notes v0.6.0

## 🚨 **Breaking Change: Parameter Syntax Updated**

### Table Name Parameter Syntax Change
- **CHANGED**: Table name parameter syntax updated from `#table_name` to `#[table_name]`
- **Reason**: The old syntax (`#table_name`) failed with concatenated table names due to parsing ambiguities from underscores
- **Benefit**: Enables concatenation like `rel_#[table_a]_#[table_b]` for relationship table names
- **Migration Required**: Update all existing queries using `#table` syntax

### Backward Compatibility
- **Breaking**: Existing queries using `#table_name` syntax will no longer be recognized
- **Migration**: Change all `#identifier` to `#[identifier]` in query definitions
- **Validation**: All tests have been updated and verified to work with new syntax

### Examples of Updated Syntax

**Before (v0.5.0 and earlier):**
```json
{
  "query_dynamic_table": {
    "query": "SELECT * FROM #table_name WHERE id=@id",
    "args": {
      "table_name": {"enum": ["users", "accounts"]}
    }
  }
}
```

**After (v0.6.0 and later):**
```json
{
  "query_dynamic_table": {
    "query": "SELECT * FROM #[table_name] WHERE id=@id",
    "args": {
      "table_name": {"enum": ["users", "accounts"]}
    }
  }
}
```

### Concatenation Support Enabled
With the new syntax, you can now create queries with concatenated table parameters:

```json
{
  "query_related_tables": {
    "query": "SELECT * FROM rel_#[parent_table]_#[child_table] WHERE parent_id=@id",
    "returns": ["id", "data"],
    "args": {
      "parent_table": {"enum": ["users", "companies"]},
      "child_table": {"enum": ["orders", "products"]},
      "id": {"type": "integer"}
    }
  }
}
```

---
**Version 0.6.0** - Parameter syntax updated to support table name concatenation</parameter>
