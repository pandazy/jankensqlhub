# Release Notes v1.2.0

## 🎯 **Major Features**

### 1. ✨ Returns Field Mapping by Name

Changed the "returns" field implementation to map database columns by name instead of positional indexing, making queries more robust and flexible.

**Benefits:**
- ✅ Results independent of column order in SELECT statements
- ✅ Works correctly with `SELECT *` queries
- ✅ Compatible with dynamic table/column names (`#[table_name]` syntax)
- ✅ Missing columns map to `null` instead of causing errors

**Example:**
```json
{
  "query": "SELECT id, name, email FROM users WHERE id=@id",
  "returns": ["name", "email", "id"],  // Order doesn't matter!
  "args": {"id": {"type": "integer"}}
}
```

---

### 2. 🆕 Comma List Parameter Feature (`~[param]`)

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
- Dynamic field selection: `SELECT ~[fields] FROM table`
- Dynamic table lists: `SELECT * FROM ~[tables]`
- Constraint support: Works with `enum` and `pattern` constraints
- SQL injection prevention: Validates alphanumeric + underscore only

---

### 3. 🔄 Dynamic Returns Specification

Added support for `~[param_name]` syntax in the "returns" field, allowing return fields to be determined at runtime from a comma_list parameter.

**Static Returns (traditional):**
```json
{
  "query": "SELECT name, email, age FROM users",
  "returns": ["name", "email"]
}
```

**Dynamic Returns (new):**
```json
{
  "query": "SELECT ~[fields] FROM users WHERE status='active'",
  "returns": "~[fields]",
  "args": {
    "fields": {"enum": ["name", "email", "age"]}
  }
}
```

**Runtime Flexibility:**
- Same query definition can return different fields per request
- Request 1: `{"fields": ["name"]}` → returns only name
- Request 2: `{"fields": ["name", "email"]}` → returns name and email

**Validation:**
- Invalid format → clear error message
- Non-existent parameter → clear error message  
- Parameter not CommaList type → clear error message
- Missing at runtime → clear error message

---

## 🔒 **Enhanced Validation**

### Args Validation
Strengthened validation for parameter definitions in the `args` object:

- **✅ Parameter NOT in args** → Valid (no constraints applied)
- **✅ Parameter with object value** → Valid (e.g., `{"enum": [...]}`)
- **❌ Parameter with non-object value** → Error (e.g., `["value"]`, `"string"`)

---

## 🧪 **Testing**

All 172 tests passing:
- ✅ 21 comma list parameter tests
- ✅ 13 dynamic returns tests  
- ✅ 48 unit tests for resolve_returns and parameter handling
- ✅ Complete coverage for returns mapping
- ✅ All SQLite and PostgreSQL integration tests

## 🔧 **Files Modified**

**Returns Mapping:**
- `src/runner_postgresql.rs` - Name-based mapping for PostgreSQL
- `src/runner_sqlite.rs` - Name-based mapping for SQLite

**Comma List Feature:**
- `src/parameters.rs` - Added `~[param]` parsing and validation
- `src/parameter_constraints.rs` - Comma list constraint validation

**Dynamic Returns:**
- `src/query/query_def.rs` - Added `ReturnsSpec` enum
- `src/query/query_definitions.rs` - Enhanced parsing logic
- `tests/dynamic_returns_tests.rs` - Comprehensive test suite

---

**Version 1.2.0** - Name-based returns mapping, comma list parameters, and dynamic returns specification
