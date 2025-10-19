# Release Notes v0.4.0

## Major Feature Addition
- **Optional parameter arguments** - `@param` can now be used without explicit `args` specification in query definitions
- Unspecified `@param` automatically defaults to string type with no constraints for cleaner APIs
- Simplifies query definitions by reducing boilerplate for string parameters
- Maintains full backward compatibility - existing `args` specifications continue to work unchanged

## Examples

### Traditional Syntax (still supported)
```json
{
  "find_user": {
    "query": "SELECT * FROM users WHERE name=@username",
    "args": {
      "username": {"type": "string"}
    }
  }
}
```

### New Simplified Syntax
```json
{
  "find_user": {
    "query": "SELECT * FROM users WHERE name=@username"
    // No args needed - @username defaults to string type
  }
}
```

### Mixed Usage
```json
{
  "complex_query": {
    "query": "SELECT * FROM #table WHERE id=@id AND status=@status",
    "args": {
      "id": {"type": "integer"},
      "table": {"enum": ["users", "orders"]}
      // @status not specified, defaults to string type
    }
  }
}
```

## Migration Notes
- Existing query definitions require no changes - fully backward compatible
- New query definitions can be simplified by omitting `args` for string parameters
- Table name parameters (`#table`) still require `args` for constraint validation
- No performance impact on queries with explicit args

---
**Version 0.4.0** - Streamlined parameter syntax with automatic string defaults
