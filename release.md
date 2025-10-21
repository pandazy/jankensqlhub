# Release Notes v0.5.0

## ✨ **BLOB Support Added**

### New BLOB Parameter Type
- Added support for `blob` parameter type for handling binary data
- BLOB parameters accept JSON arrays containing byte values (0-255)
- Automatic conversion to SQLite BLOB columns for database operations

### BLOB Range Constraints
- Range constraints now supported for BLOB parameters, representing size limits in bytes
- Define minimum and maximum allowed BLOB sizes using the existing range constraint syntax
- Example: `{"type": "blob", "range": [1, 1024]}` (1-1024 bytes)

### BLOB Parameter Validation
- Validates that BLOB values are arrays of numbers between 0-255
- Provides clear error messages for invalid BLOB data formats
- Size validation occurs both at parameter definition time and runtime

### Examples of BLOB Usage

```json
{
  "store_binary_data": {
    "query": "INSERT INTO files (name, data) VALUES (@name, @data)",
    "args": {
      "name": {"type": "string"},
      "data": {"type": "blob", "range": [1, 1048576]}  // 1 byte to 1MB
    }
  },
  "get_file_data": {
    "query": "SELECT name, data FROM files WHERE id=@id",
    "returns": ["name", "data"],
    "args": {
      "id": {"type": "integer"}
    }
  }
}
```

### API Usage
```rust
// Store binary data
let binary_data: Vec<u8> = vec![72, 101, 108, 108, 111]; // "Hello" as bytes
let params = serde_json::json!({
    "name": "hello.txt",
    "data": binary_data  // Will be stored as BLOB in database
});
conn.query_run(&queries, "store_binary_data", &params)?;
```

## 🧪 **Enhanced Type System**

### Updated Parameter Types
- Added `blob` to the list of supported parameter types
- Updated error messages and documentation to include BLOB support
- All existing parameter types (integer, string, float, boolean, table_name, list) remain unchanged

---
**Version 0.5.0** - Added BLOB parameter support for binary data handling
