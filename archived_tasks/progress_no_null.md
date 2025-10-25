---

## Summary
Remove the Null variant from ParameterValue enum to prevent dangerous null values that bypass type barriers. Each data type should use its own empty values instead.

## Current Status
- Initialized progress tracking

## Next Steps
- [x] Remove ParameterValue::Null from enum definition
- [x] Update json_value_to_parameter_value_inferred to reject/null convert Null values from JSON
- [x] Remove Null handling from runner_postgresql.rs parameter conversion
- [x] Remove Null handling from runner_sqlite.rs parameter conversion
- [x] Update tests to remove Null usage
- [x] Run comprehensive tests to verify functionality
- [x] Apply clippy fixes and fmt
- [x] Verify tests pass after sh run-local-tests.sh
