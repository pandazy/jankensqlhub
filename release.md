# Release Notes v0.7.0

## 🔄 **Major Refactoring: Database Backend Separation**

### Architecture Restructuring
- **MODULAR EXECUTION**: Split `runner.rs` into DB-independent and DB-specific modules
- **CLEAN SEPARATION**: Database logic now cleanly separated by backend
- **MULTI-DB PREPARED**: Architecture ready for PostgreSQL and other database backends

### Module Structure Changes
**Before:**
```
src/runner.rs  # Mixed concerns: common + SQLite logic
```

**After:**
```
src/runner.rs           # Common interface, QueryRunner trait, re-exports
src/runner_sqlite.rs    # SQLite-specific implementations
src/runner_postgres.rs  # Future PostgreSQL implementations
```

### Benefits
- **Maintainability**: Database-specific logic isolated per backend
- **Extensibility**: Easy to add new database backends
- **Clean APIs**: Each backend has dedicated implementation

### Implementation Details
- **`src/runner.rs`**: `QueryRunner` trait, common utilities, module coordination
- **`src/runner_sqlite.rs`**: SQLite type conversions, prepared statements, transaction handling
- **`src/str_utils.rs`**: Moved `quote_identifier()` utility function
- **Direct API**: Simplified to use `query_run_sqlite()` directly

### Backward Compatibility
- **Zero Breaking Changes**: All existing code continues to work
- **Same Public API**: `query_run_sqlite()`, `QueryRunner` trait unchanged
- **Seamless Upgrade**: Transparent architectural improvement

---
**Version 0.7.0** - Database backend separation and modular execution architecture
