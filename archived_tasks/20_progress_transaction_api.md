# Task: Provide `query_run_` methods accepting transaction objects directly

## Status: ✅ Complete

## Summary
Added new public methods `query_run_sqlite_with_transaction` and `query_run_postgresql_with_transaction` that accept user-provided transaction objects directly, giving users flexibility to use them within their own transaction management. Refactored existing `query_run_sqlite` and `query_run_postgresql` to delegate to the new methods internally.

## Changes Made

### `src/runner_sqlite.rs`
- Added `query_run_sqlite_with_transaction(tx: &rusqlite::Transaction, queries, query_name, request_params)` — executes a query within a user-provided SQLite transaction without committing
- Refactored `query_run_sqlite` to create a transaction, delegate to `query_run_sqlite_with_transaction`, then commit

### `src/runner_postgresql.rs`
- Added `query_run_postgresql_with_transaction(transaction: &mut tokio_postgres::Transaction<'_>, queries, query_name, request_params)` — executes a query within a user-provided PostgreSQL transaction without committing
- Refactored `query_run_postgresql` to create a transaction, delegate to `query_run_postgresql_with_transaction`, then commit

### `src/lib.rs`
- Exported both new functions: `query_run_sqlite_with_transaction` and `query_run_postgresql_with_transaction`

## Test Results
- All 145 tests passed (including SQLite and PostgreSQL integration tests)
- `cargo clippy --fix --allow-dirty` — clean
- `cargo fmt` — clean