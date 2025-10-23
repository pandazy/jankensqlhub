pub mod connection;
pub mod parameter_constraints;
pub mod parameters;
pub mod query;
pub mod result;
pub mod runner;
pub mod runner_sqlite;
pub mod str_utils;

// Re-export result, parameter, query, connection and runner types for convenience
pub use connection::DatabaseConnection;
pub use parameters::{Parameter, ParameterType};
pub use query::{QueryDef, QueryDefinitions};
pub use result::{JankenError, QueryResult, Result};
pub use runner::{QueryRunner, query_run_sqlite};

// Re-export third-party types used in the public API to provide fallback for dependency conflicts
pub use serde_json::Value as JsonValue;

// Re-export third-party types used in the public API to provide fallback for dependency conflicts
pub use rusqlite::Connection as SqliteConnection;
