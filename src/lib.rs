pub mod parameter_constraints;
pub mod parameters;
pub mod query;
pub mod result;
pub mod runner;
pub mod runner_sqlite;
pub mod str_utils;

// PostgreSQL runner (now available in production builds)
pub mod runner_postgresql;

// Re-export PostgreSQL function for production use
pub use runner_postgresql::query_run_postgresql;

// Re-export types for convenience
pub use parameters::{Parameter, ParameterType};
pub use query::{QueryDef, QueryDefinitions};
pub use result::{
    // Error codes
    ERR_CODE_IO,
    ERR_CODE_JSON,
    ERR_CODE_PARAMETER_NAME_CONFLICT,
    ERR_CODE_PARAMETER_NOT_PROVIDED,
    ERR_CODE_PARAMETER_TYPE_MISMATCH,
    ERR_CODE_POSTGRES,
    ERR_CODE_QUERY_NOT_FOUND,
    ERR_CODE_REGEX,
    ERR_CODE_SQLITE,
    JankenError,
    M_COLUMN,
    M_CONFLICT_NAME,
    M_ERROR,
    M_ERROR_KIND,
    // Metadata field names
    M_EXPECTED,
    M_GOT,
    M_LINE,
    M_PARAM_NAME,
    M_QUERY_NAME,
    QueryResult,
    Result,
    error_meta,
    // Helper functions
    get_error_data,
    get_error_info,
};
pub use runner::query_run_sqlite;

// Re-export third-party types used in the public API to provide fallback for dependency conflicts
pub use serde_json::Value as JsonValue;

// Re-export third-party types used in the public API to provide fallback for dependency conflicts
pub use rusqlite::Connection as SqliteConnection;
