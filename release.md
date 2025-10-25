# Release Notes v0.8.0

## 🐘 **PostgreSQL Support Added**

### Multi-Database Support
- **PRODUCTION POSTGRESQL**: First-class PostgreSQL support alongside SQLite
- **ASYNC EXECUTION**: Leverages tokio-postgres for high-performance async operations
- **ACID TRANSACTIONS**: All PostgreSQL queries executed within transactions with automatic rollback
- **TYPE MAPPING**: Complete JSON-to-PostgreSQL data type conversion
- **PARAMETER PREPARED STATEMENTS**: Automatic conversion to PostgreSQL `$1, $2, ...` format

### API Extensions
- **`query_run_postgresql()`**: New async function for PostgreSQL query execution
- **Dual Backend Support**: Same API and parameter syntax across SQLite and PostgreSQL
- **Production Dependencies**: Moved tokio and tokio-postgres to regular dependencies
- **Error Handling**: PostgreSQL-specific error types added to `JankenError`

### Implementation Features
- **Transaction-Wrapped Execution**: All queries run in transactions for consistency
- **SQL Injection Protection**: Prepared statements prevent attacks in PostgreSQL
- **List Parameters**: Full support for `:[list_param]` syntax in PostgreSQL
- **Dynamic Table Names**: `#[table_name]` parameter validation works with PostgreSQL
- **Integration Tests**: Comprehensive test suite covering all PostgreSQL features

### Architecture Continuity
- **Same Query Syntax**: All parameter types (`@param`, `#[table]`, `:[list]`) work identically
- **Same Constraints**: Range, enum, pattern validation works across both backends
- **Unified API**: Developers can switch between SQLite and PostgreSQL seamlessly

### Testing & Quality
- **PostgreSQL Integration Tests**: Test suite runs when `POSTGRES_CONNECTION_STRING` env var is set
- **Docker Compose Setup**: Easy PostgreSQL testing environment
- **Zero Breaking Changes**: All existing SQLite functionality unchanged
- **Code Quality**: Passes clippy and rustfmt, maintains high standards

---
**Version 0.8.0** - Production-ready PostgreSQL support
