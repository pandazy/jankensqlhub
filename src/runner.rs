//! Database execution runners for different SQL backends
//!
//! This module contains DB-independent runner logic and specific implementations
//! for different database backends (SQLite, PostgreSQL, etc.).

use crate::{
    QueryDefinitions,
    result::{QueryResult, Result},
};

/// Common functions are now in separate modules (runner_sqlite.rs, runner_postgres.rs, etc.)
/// Trait for executing parameterized SQL queries against different database backends
/// This provides the common interface that all DB implementations must satisfy
pub trait QueryRunner {
    fn query_run(
        &mut self,
        queries: &QueryDefinitions,
        query_name: &str,
        params: &serde_json::Value,
    ) -> Result<QueryResult>;
}

// =============================================================================
// RE-EXPORT DB-SPECIFIC FUNCTIONS
// =============================================================================

// Re-export SQLite functions (the actual implementation moved to runner_sqlite.rs)
pub use crate::runner_sqlite::*;

// =============================================================================
// POSTGRESQL IMPLEMENTATION (PLACEHOLDER)
// =============================================================================

// PostgreSQL implementation will be in a separate module when implemented
// For now, we have a placeholder to show the intended structure

// # Postgresql module would be:
// mod runner_postgres;
// pub use runner_postgres::*;
