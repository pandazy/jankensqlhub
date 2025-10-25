pub mod parameter_constraints;
pub mod parameters;
pub mod query;
pub mod result;
#[cfg(feature = "sqlite")]
pub mod runner_sqlite;
pub mod str_utils;

// PostgreSQL runner (now available in production builds)
#[cfg(feature = "postgresql")]
pub mod runner_postgresql;

// Re-export PostgreSQL function for production use
#[cfg(feature = "postgresql")]
pub use runner_postgresql::query_run_postgresql;

#[cfg(feature = "sqlite")]
pub use runner_sqlite::query_run_sqlite;

// Re-export types for convenience
pub use parameters::{Parameter, ParameterType};
pub use query::{QueryDef, QueryDefinitions};
pub use result::{
    // Error codes
    ERR_CODE_PARAMETER_NAME_CONFLICT,
    ERR_CODE_PARAMETER_NOT_PROVIDED,
    ERR_CODE_PARAMETER_TYPE_MISMATCH,
    ERR_CODE_QUERY_NOT_FOUND,
    JankenError,
    M_CONFLICT_NAME,
    M_ERROR,
    // Metadata field names
    M_EXPECTED,
    M_GOT,
    M_PARAM_NAME,
    M_QUERY_NAME,
    QueryResult,
    Result,
    error_meta,
    // Helper functions
    get_error_data,
    get_error_info,
};

// Re-export third-party types used in the public API to provide fallback for dependency conflicts
pub use serde_json::Value as JsonValue;

// Re-export third-party types used in the public API to provide fallback for dependency conflicts
#[cfg(feature = "sqlite")]
pub use rusqlite::Connection as SqliteConnection;

pub use anyhow;
