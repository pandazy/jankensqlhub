# Fix Test Compilation Failures - Progress

## Task Description
Fix all remaining test file compilation failures by updating error handling patterns to use `anyhow::Error` downcast instead of direct `JankenError` matching.

## Current Errors (from cargo check --tests)
- [x] **ALL FIXED** - All compilation failures have been resolved

## Fixed Issues

### E0308: Old match syntax (need to convert to downcast pattern)
- [x] `tests/enumif_tests_basic.rs` (4 matches - with special handling for QueryDefinitions errors)
- [x] `tests/enumif_tests_no_match.rs` (2 matches)
- [x] `tests/error_handling_parameter_validation_blob.rs` (2 remaining matches)
- [x] `tests/error_handling_query_definition.rs` (1 match - already fixed)

### E0382: Borrow of moved value (need to capture err_str before downcast)
- [x] `tests/error_handling_runtime.rs` (4 borrow issues)
- [x] `tests/error_handling_parameter_validation_list.rs` (6 borrow issues)
- [x] `tests/error_handling_parameter_validation_type.rs` (4 borrow issues + wrong downcast target)

### Other Issues
- [x] `tests/error_handling_utilities.rs` (needs fixes for removed JankenError::Sqlite)

## Pattern Applied

**For `query_run_sqlite` errors (anyhow::Error):**
```rust
let err = result.unwrap_err();
let err_str = format!("{err:?}");  // <-- THIS MUST COME BEFORE downcast()
if let Ok(JankenError::SomeVariant { data }) = err.downcast::<JankenError>() {
    // extract data and assert
} else {
    panic!("Expected SomeVariant, got: {err_str}");
}
```

**For `QueryDefinitions::from_json` errors (JankenError):**
```rust
let err = result.unwrap_err();
match err {
    JankenError::SomeVariant { data } => {
        // extract data and assert
    }
    _ => panic!("Expected SomeVariant, got: {err:?}"),
}
```

## Completed Steps
- [x] Fix error_handling_utilities.rs (removed sqlite references)
- [x] Fix error_handling_parameter_validation_type.rs (borrow issues and wrong downcast)
- [x] Fix error_handling_query_definition.rs (1 match)
- [x] Fix error_handling_parameter_validation_blob.rs (2 remaining matches)
- [x] Fix enumif_tests_basic.rs (4 matches)
- [x] Fix enumif_tests_no_match.rs (2 matches)
- [x] Run `cargo check --tests` to verify all fixes ✓ PASSED
- [x] Run `cargo clippy --fix --allow-dirty` ✓ PASSED
- [x] Run `cargo fmt` ✓ PASSED

All test compilation failures have been successfully fixed. The error handling patterns now correctly distinguish between `anyhow::Error` types (from runtime operations) that need downcast and direct `JankenError` types (from definition-time operations).
