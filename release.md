# Release Notes v0.4.1

## ✨ **Improved Parameter Validation**

### Range Constraint Definition-Time Validation
- Range constraints now validated at query definition time (during `QueryDefinitions::from_json()`)
- Range must be an array with exactly 2 numbers: `[min, max]`
- Non-array values, arrays with wrong element counts, or non-numeric elements are rejected early
- Provides clearer error messages and prevents runtime failures from malformed range definitions

**Before (v0.4.0)**: Invalid range definitions would be accepted and fail at runtime
**After (v0.4.1)**: Invalid range definitions are caught during query parsing with helpful error messages

### Examples of Improved Validation

```rust
// ✅ Valid range definitions accepted
{"range": [1, 100]}              // integer range
{"range": [0.0, 1000.0]}         // float range

// ❌ Invalid range definitions now rejected at definition time
{"range": "not_an_array"}        // Error: not an array
{"range": [1]}                   // Error: array with 1 element (needs 2)
{"range": [1, 2, 3]}             // Error: array with 3 elements (needs exactly 2)
{"range": ["min", "max"]}        // Error: non-numeric elements
```

## 🐛 **Bug Fixes**

### Enhanced Error Messages
- More specific error messages for range constraint definition errors
- Distinguishes between different types of range validation failures
- Maintains consistency with existing parameter validation error patterns

---
**Version 0.4.1** - Enhanced parameter validation with definition-time range constraint checking