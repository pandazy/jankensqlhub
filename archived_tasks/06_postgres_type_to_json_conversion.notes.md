# PostgreSQL Type to JSON Conversion Findings

## Summary
Investigation and fix for PostgreSQL JSON and JSONB column handling in `postgres_type_to_json_conversion` function.

## Issues Identified

### 1. Type Detection Methodology
**Problem**: Initial attempts to match `tokio_postgres::types::Type::JSON` and `Type::JSONB` failed to match actual column types.

**Solution**: Use OID-based detection instead of enum pattern matching.
- JSON OID: 114
- JSONB OID: 3802

**Why OID?**: OID-based detection is more robust because:
- PostgreSQL OIDs are stable identifiers that don't change between tokio_postgres versions
- Pattern matching against enum variants can fail if the enum representation changes
- OIDs are directly available from column metadata without additional type conversion
- This approach works reliably across different tokio_postgres versions and build configurations

### 2. JSON Deserialization Approach
**Problem**: Initial approach tried to retrieve JSON columns as `String` and then parse them, but this resulted in "WrongType" errors because tokio_postgres requires the correct Rust type for deserialization.

**Solution**: Enable the `with-serde_json-1` feature for tokio_postgres and retrieve JSON columns directly as `serde_json::Value`.

### 3. Feature Requirements
**Problem**: Direct `serde_json::Value` deserialization from PostgreSQL JSON columns requires the `with-serde_json-1` feature flag on tokio_postgres.

**Solution**: Added `"with-serde_json-1"` to the tokio_postgres features in Cargo.toml.

## Implementation Details

### Type Detection Strategy
```rust
// PostgreSQL type OIDs for JSON column detection
const POSTGRES_TYPE_OID_JSON: u32 = 114;
const POSTGRES_TYPE_OID_JSONB: u32 = 3802;

// Usage in conversion function
let oid = column_type.oid();
if oid == POSTGRES_TYPE_OID_JSON || oid == POSTGRES_TYPE_OID_JSONB {
    let json_val: serde_json::Value = row.try_get(idx)?;
    Ok(json_val)
}
```

### Dependencies
- `tokio-postgres = { version = "0.7", features = ["with-serde_json-1"] }`

## Test Verification
The fix enables proper handling of JSON/JSONB columns as evidenced by:
- `test_postgres_json_and_jsonb_column_types` now passes
- All PostgreSQL integration tests pass (18/18)
- Full test suite passes (68 tests)

## Key Lessons

1. **OID-based Detection**: PostgreSQL type OIDs are more reliable than enum pattern matching for type detection.
2. **Direct Deserialization**: When tokio_postgres supports direct deserialization to target types, use those instead of intermediate string conversion.
3. **Feature Flags**: PostgreSQL-specific JSON support requires explicit feature activation.
4. **Error Debugging**: Running tests in non-captured mode with debug output is crucial for understanding deserialization failures.
