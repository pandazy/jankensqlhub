//! Database execution runners for different SQL backends
//!
//! This module contains specific implementations for different database backends
//! (SQLite, PostgreSQL, etc.).

// ============================================================================
// RE-EXPORT DB-SPECIFIC FUNCTIONS
// ============================================================================

// Re-export SQLite functions (the main implementation)
pub use crate::runner_sqlite::*;

// ============================================================================
// POSTGRESQL IMPLEMENTATION (PLACEHOLDER)
// ============================================================================

// PostgreSQL implementation will be in a separate module when implemented
// For now, we have a placeholder to show the intended structure

// # Postgresql module would be:
// mod runner_postgres;
// pub use runner_postgres::*;
