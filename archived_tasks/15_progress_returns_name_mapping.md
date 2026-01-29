# Progress: Returns Field Mapping by Name

## Task Description
Changed the "returns" definition implementation to map fields by matching database column names instead of using positional indexing.

## Problem
The original implementation mapped `returns` fields by position (index 0 to first column, index 1 to second column, etc.). This was fragile and didn't work well with:
- Dynamic column selection using `SELECT *`
- Dynamic table/column names with `#[table_name]` syntax
- SQL queries where column order might vary

## Solution Implemented

### 1. PostgreSQL Runner (`src/runner_postgresql.rs`)
Modified `row_to_json_object()` function:
```rust
// OLD: Positional mapping
for (idx, field_name) in returns.iter().enumerate() {
    let value = match row.columns().get(idx) { ... }
}

// NEW: Name-based mapping
let columns = row.columns();
for field_name in returns {
    let column_idx = columns.iter().position(|col| col.name() == field_name);
    let value = match column_idx {
        Some(idx) => {
            let col = &columns[idx];
            postgres_type_to_json_conversion(col.type_(), row, idx)
        }
        None => Ok(serde_json::Value::Null),
    };
}
```

### 2. SQLite Runner (`src/runner_sqlite.rs`)
Modified `execute_query_unified()` function:
```rust
// Extract column names from prepared statement
let column_names: Vec<String> = stmt
    .column_names()
    .iter()
    .map(|name| name.to_string())
    .collect();

// Map by name instead of position
for field_name in &query.returns {
    let column_idx = column_names.iter().position(|name| name == field_name);
    let value: rusqlite::Result<serde_json::Value> = match column_idx {
        Some(idx) => match row.get_ref(idx) { ... },
        None => Ok(serde_json::Value::Null),
    };
}
```

## Test Fixes

### 1. `tests/mvp_tests.rs` - `test_table_name_column_syntax`
**Issue**: Test used `returns: ["column_value"]` but actual column was `name`
**Fix**: Changed to `returns: ["name"]` to match the actual database column name

### 2. `test_json/resource_queries.json` - `select_related_entities`
**Issue**: Query returned columns with dynamic names like `show_id` or `song_id`, but `returns` expected `source_id` and `target_id`
**Fix**: Added SQL aliases to map dynamic columns to expected names:
```json
"query": "select r.id as rel_id, r.#[source_fk] as source_id, r.#[target_fk] as target_id, ..."
```

## Benefits

1. **More Robust**: Results are independent of column order in SELECT statements
2. **More Flexible**: Can use `SELECT *` and extract only needed columns via `returns`
3. **Better Compatibility**: Works correctly with dynamic table/column names
4. **Safer**: Missing columns map to `null` instead of causing index errors
5. **More SQL-like**: Matches standard SQL behavior where columns are referenced by name

## Testing

All tests pass successfully:
- ✅ 19 unit tests
- ✅ All SQLite integration tests (including mvp_tests, resource_queries_tests)
- ✅ All PostgreSQL integration tests (types, transactions, errors, JSON)
- ✅ All parameter validation tests
- ✅ All enumif tests
- ✅ All injection protection tests

Verified with: `sh run-local-tests.sh`

## Code Quality

- ✅ `cargo clippy --fix --allow-dirty` - No warnings
- ✅ `cargo fmt` - Code formatted

## Files Modified

1. `src/runner_postgresql.rs` - Changed `row_to_json_object()` to use name-based mapping
2. `src/runner_sqlite.rs` - Changed `execute_query_unified()` to use name-based mapping
3. `tests/mvp_tests.rs` - Fixed `test_table_name_column_syntax` returns field
4. `test_json/resource_queries.json` - Added SQL aliases in `select_related_entities` query

## Status
✅ **COMPLETED** - All changes implemented, tested, and verified
