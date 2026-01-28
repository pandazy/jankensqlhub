# Release Notes v1.1.0

## ✨ **New Feature: Enumif Fuzzy Matching**

### Overview
Added fuzzy matching support to `enumif` constraints, enabling flexible pattern-based conditional validation. This powerful feature allows condition keys to use patterns for matching values, making enumif constraints more versatile and reducing configuration verbosity.

### Pattern Types

Enumif conditions now support three fuzzy matching patterns:

1. **`start:pattern`** - Matches values starting with the pattern
   ```json
   "start:admin": ["read_all", "write_all", "delete_all"]
   ```
   Matches: "admin_super", "admin_level2", etc.

2. **`end:pattern`** - Matches values ending with the pattern
   ```json
   "end:_txt": ["edit", "view"]
   ```
   Matches: "file_txt", "document_txt", etc.

3. **`contain:pattern`** - Matches values containing the pattern
   ```json
   "contain:error": ["critical", "high"]
   ```
   Matches: "error_occurred", "system_error", etc.

### Example Usage

```json
{
  "user_search": {
    "query": "SELECT * FROM users WHERE role=@role AND permission=@permission",
    "args": {
      "role": {},
      "permission": {
        "enumif": {
          "role": {
            "start:admin": ["read_all", "write_all", "delete_all"],
            "start:user": ["read_own", "write_own"],
            "guest": ["read_public"]
          }
        }
      }
    }
  }
}
```

### Key Features

- **Mixed patterns**: Combine exact matches with fuzzy patterns in the same enumif definition
- **Deterministic behavior**: Alphabetically sorted condition keys ensure consistent matching order
- **Security maintained**: Pattern validation at definition time prevents malformed configurations
- **Case-sensitive**: All matching is case-sensitive for security and precision

### Pattern Validation

- **Fuzzy patterns** (`start:`, `end:`, `contain:`): Pattern names must be alphanumeric with underscores (e.g., `start:admin_role`)
- **Exact matches**: Any string that doesn't contain ':' is treated as an exact match with no character restrictions (e.g., "user-role", "status.active")
- Invalid match types or empty patterns are rejected at definition time

### Technical Details

- Alphabetical precedence: When multiple patterns match, the first one alphabetically is used
- Backward compatible: Existing enumif configurations work without changes
- Comprehensive test coverage: 8 new test cases covering all fuzzy matching scenarios

---

**Version 1.1.0** - Added fuzzy matching support to enumif constraints with `start:`, `end:`, and `contain:` patterns
