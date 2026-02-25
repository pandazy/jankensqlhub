# Release Notes v1.4.0

## 🚀 **New Feature: User-managed Transaction API**

### Added `_with_transaction` methods for both database backends

New public methods that accept user-provided transaction objects directly, giving full control over transaction lifecycle (begin/commit/rollback). This enables executing multiple JankenSQLHub queries within a single transaction.

#### SQLite: `query_run_sqlite_with_transaction`

```rust
use jankensqlhub::{QueryDefinitions, query_run_sqlite_with_transaction};

let tx = conn.transaction()?;
query_run_sqlite_with_transaction(&tx, &queries, "insert_user", &params1)?;
query_run_sqlite_with_transaction(&tx, &queries, "insert_profile", &params2)?;
tx.commit()?; // caller controls commit/rollback
```

#### PostgreSQL: `query_run_postgresql_with_transaction`

```rust
use jankensqlhub::{QueryDefinitions, query_run_postgresql_with_transaction};

let mut tx = client.transaction().await?;
query_run_postgresql_with_transaction(&mut tx, &queries, "insert_user", &params1).await?;
query_run_postgresql_with_transaction(&mut tx, &queries, "insert_profile", &params2).await?;
tx.commit().await?;
```

### Refactored existing methods

The existing `query_run_sqlite` and `query_run_postgresql` methods now internally delegate to the new `_with_transaction` variants, maintaining full backward compatibility while reducing code duplication.

---

**Version 1.4.0** - Added user-managed transaction API for SQLite and PostgreSQL