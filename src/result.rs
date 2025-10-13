use thiserror::Error;

/// Main error type for the Janken SQL library
#[derive(Error, Debug)]
pub enum JankenError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Query not found: {0}")]
    QueryNotFound(String),
    #[error("Parameter not provided: {0}")]
    ParameterNotProvided(String),
    #[error("Parameter type mismatch: expected {expected}, got {got}")]
    ParameterTypeMismatch { expected: String, got: String },
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}

/// Type alias for Results using JankenError
pub type Result<T> = std::result::Result<T, JankenError>;
