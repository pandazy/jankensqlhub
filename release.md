# Release Notes v1.2.0

## 🎯 **Major Features**

### 1. ✨ Returns Field Mapping by Name

Changed the "returns" field implementation to map database columns by name instead of positional indexing, making queries more robust and flexible.

**Before v1.2.0 (Positional):**
```rust
// Mapped by index - fragile and order-dependent
for (idx, field_name) in returns.iter().enumerate() {
    let value = row.get(idx);  // Position-based
}
```

**After v1.2.0 (Name-based):**
```rust
// Mapped by column name - robust and order-independent
for field_name in returns {
    let column_idx = columns.iter().position(|col| col.name() == field_name);
    let value = row.get(column_idx);  // Name-based
}
```

**Benefits:**
- ✅ Results independent of column order in SELECT statements
- ✅ Works correctly with `SELECT *` queries
- ✅ Compatible with dynamic table/column names (`#[table_name]` syntax)
- ✅ Missing columns map to `null` instead of causing errors
- ✅ More SQL-like behavior (standard SQL references columns by name)

**Example:**
```json
{
  "query": "SELECT id, name, email FROM users WHERE id=@id",
  "returns": ["name", "email", "id"],  // Order doesn't matter anymore!
  "args": {"id": {"type": "integer"}}
}
```

---

### 2. 🆕 Comma List Parameter Feature

Added support for `~[param]` syntax to replace placeholders with comma-separated values, enabling dynamic field selection and table lists.

**Syntax:**
```sql
SELECT ~[fields] FROM users WHERE status='active'
```

**With runtime params:**
```json
{"fields": ["name", "email", "age"]}
```

**Becomes:**
```sql
SELECT name,email,age FROM users WHERE status='active'
```

**Key Features:**
- **Dynamic field selection**: `SELECT ~[fields] FROM table`
- **Dynamic table lists**: `SELECT * FROM ~[tables]`
- **Array validation**: Each element must be a string (table name format)
- **Constraint support**: Works with `enum` and `pattern` constraints
- **SQL injection prevention**: Validates alphanumeric + underscore only

**Example Definition:**
```json
{
  "select_fields": {
    "query": "SELECT ~[fields] FROM users WHERE status='active'",
    "returns": ["name", "email", "age"],
    "args": {
      "fields": {"enum": ["name", "email", "age"]}
    }
  }
}
```

**Security:**
- Table name validation (alphanumeric and underscores only)
- Prevents SQL injection through special characters
- Empty arrays rejected at runtime
- Non-string elements rejected

---

### 3. 🔒 Enhanced Args Validation

Strengthened validation for parameter definitions in the `args` object to ensure type safety and prevent silent failures.

**The Problem:**
```json
{
  "args": {
    "fields": ["name", "email"]  // ❌ Was silently ignored - no error!
  }
}
```

**The Solution:**
```json
{
  "args": {
    "fields": {"enum": ["name", "email"]}  // ✅ Proper constraint object
  }
}
```

**Validation Rules:**
- **✅ Parameter NOT in args** → Valid (no constraints applied)
- **✅ Parameter with object value** → Valid (e.g., `{"enum": [...]}`)
- **❌ Parameter with non-object value** → Error (e.g., `["value"]`, `"string"`, `42`)

**Benefits:**
- Early error detection at definition time
- Type safety for all parameter definitions
- Clear, specific error messages
- No more silent failures

**Error Examples:**
```
Expected: parameter definition to be an object with constraint fields
Got: ["name","email"] (type: array)
```

---

## 📝 **Documentation Updates**

- Clarified distinction between runtime parameter values vs definition constraints
- Added comma list examples with correct `args` format
- Updated README with proper constraint object syntax
- Enhanced error messages for better debugging

## 🧪 **Testing**

All 117 tests passing:
- ✅ 21 comma list parameter tests
- ✅ 12 query definition validation tests
- ✅ Complete coverage for returns mapping
- ✅ All SQLite and PostgreSQL integration tests

## 🔧 **Modified Files**

**Returns Mapping:**
- `src/runner_postgresql.rs` - Name-based mapping for PostgreSQL
- `src/runner_sqlite.rs` - Name-based mapping for SQLite

**Comma List Feature:**
- `src/parameters.rs` - Added `~[param]` parsing and validation
- `src/parameter_constraints.rs` - Comma list constraint validation
- `tests/comma_list_tests.rs` - Comprehensive test suite (21 tests)

**Args Validation:**
- `src/parameter_constraints.rs` - Enhanced `parse_constraints()` validation
- `tests/error_handling_query_definition.rs` - Args validation tests

## 🔄 **Migration Guide**

### For Non-Object Args Values:

**Before:**
```json
"args": {
  "fields": ["name", "email"],  // Will now error
  "status": "active"             // Will now error
}
```

**After (Option 1 - Add constraints):**
```json
"args": {
  "fields": {"enum": ["name", "email"]},
  "status": {"enum": ["active", "inactive"]}
}
```

**After (Option 2 - Remove if no constraints needed):**
```json
// Simply omit the args object or parameter
```

### For Returns Field Order:

No migration needed! Your existing queries will work correctly regardless of column order in SELECT statements.

---

**Version 1.2.0** - Name-based returns mapping, comma list parameters, and enhanced args validation
