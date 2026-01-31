# Release Notes v1.3.0

## ✨ **New Features**

### Extended Range Constraint Support

Range constraints now work on all parameter types except boolean:

| Type | Range Meaning |
|------|---------------|
| `integer`, `float` | Value must be within [min, max] |
| `string`, `table_name` | Character count must be within [min, max] |
| `blob` | Size in bytes must be within [min, max] |
| `list`, `comma_list` | Array size (element count) must be within [min, max] |

**Example Usage:**

```json
{
  "args": {
    "user_ids": {"itemtype": "integer", "range": [1, 100]},
    "fields": {"enum": ["name", "email", "age"], "range": [1, 3]},
    "username": {"type": "string", "range": [3, 50]}
  }
}
```

This allows you to:
- Limit the number of items in list parameters (e.g., max 100 IDs in an IN clause)
- Enforce minimum/maximum selections for comma_list fields
- Validate string length constraints
- Continue using range for numeric values and blob sizes

---

## 🔧 **Code Improvements**

### DRY Array Size Validation

Refactored array size range validation to share logic between `blob`, `list`, and `comma_list` types with customizable type names and units in error messages.

---

## 🧪 **Testing**

- Added comprehensive tests for comma_list range constraints
- Added tests for string length range validation
- Updated existing range tests for new behavior

---

**Version 1.3.0** - Extended range constraint support for all types except boolean
