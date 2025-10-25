use thiserror::Error;

/// Common error data structure for all Janken errors
#[derive(Debug, Clone)]
pub struct ErrorData {
    pub code: u16,
    pub metadata: Option<String>, // Stringified JSON
}

/// Main error type for the Janken SQL library
#[derive(Error, Debug)]
pub enum JankenError {
    #[error("Query not found")]
    QueryNotFound { data: ErrorData },
    #[error("Parameter not provided")]
    ParameterNotProvided { data: ErrorData },
    #[error("Parameter type mismatch")]
    ParameterTypeMismatch { data: ErrorData },
    #[error("Parameter name conflict")]
    ParameterNameConflict { data: ErrorData },
}

/// Type alias for Results using JankenError
pub type Result<T> = std::result::Result<T, JankenError>;

/// Result of executing a query, containing both the executed SQL statements and the result data
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    pub sql_statements: Vec<String>,
    pub data: Vec<serde_json::Value>,
}

/// Common metadata field names (short constants, ≥4 chars where applicable)
pub const M_EXPECTED: &str = "expected";
pub const M_GOT: &str = "got";
pub const M_QUERY_NAME: &str = "query_name";
pub const M_PARAM_NAME: &str = "parameter_name";
pub const M_CONFLICT_NAME: &str = "conflicting_name";
pub const M_ERROR_KIND: &str = "error_kind";
pub const M_LINE: &str = "line";
pub const M_COLUMN: &str = "column";
pub const M_ERROR: &str = "error";

/// Error codes for JankenError variants
pub const ERR_CODE_QUERY_NOT_FOUND: u16 = 2000;
pub const ERR_CODE_PARAMETER_NOT_PROVIDED: u16 = 2010;
pub const ERR_CODE_PARAMETER_TYPE_MISMATCH: u16 = 2020;
pub const ERR_CODE_PARAMETER_NAME_CONFLICT: u16 = 2030;

/// Implementation for creating structured errors
impl JankenError {
    pub fn new_query_not_found(query_name: impl Into<String>) -> Self {
        let query_name = query_name.into();
        JankenError::QueryNotFound {
            data: ErrorData {
                code: ERR_CODE_QUERY_NOT_FOUND,
                metadata: Some(
                    serde_json::json!({
                        M_QUERY_NAME: query_name
                    })
                    .to_string(),
                ),
            },
        }
    }

    pub fn new_parameter_not_provided(param_name: impl Into<String>) -> Self {
        let param_name = param_name.into();
        JankenError::ParameterNotProvided {
            data: ErrorData {
                code: ERR_CODE_PARAMETER_NOT_PROVIDED,
                metadata: Some(
                    serde_json::json!({
                        M_PARAM_NAME: param_name
                    })
                    .to_string(),
                ),
            },
        }
    }

    pub fn new_parameter_type_mismatch(
        expected: impl Into<String>,
        got: impl Into<String>,
    ) -> Self {
        let expected = expected.into();
        let got = got.into();
        JankenError::ParameterTypeMismatch {
            data: ErrorData {
                code: ERR_CODE_PARAMETER_TYPE_MISMATCH,
                metadata: Some(
                    serde_json::json!({
                        M_EXPECTED: expected,
                        M_GOT: got
                    })
                    .to_string(),
                ),
            },
        }
    }

    pub fn new_parameter_name_conflict(param_name: impl Into<String>) -> Self {
        let param_name = param_name.into();
        JankenError::ParameterNameConflict {
            data: ErrorData {
                code: ERR_CODE_PARAMETER_NAME_CONFLICT,
                metadata: Some(
                    serde_json::json!({
                        M_CONFLICT_NAME: param_name
                    })
                    .to_string(),
                ),
            },
        }
    }
}

/// Error code mappings and descriptions
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub code: u16,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
}

pub const ERROR_MAPPINGS: &[ErrorInfo] = &[
    ErrorInfo {
        code: ERR_CODE_QUERY_NOT_FOUND,
        name: "QUERY_NOT_FOUND",
        category: "Query",
        description: "Requested query definition was not found",
    },
    ErrorInfo {
        code: ERR_CODE_PARAMETER_NOT_PROVIDED,
        name: "PARAMETER_NOT_PROVIDED",
        category: "Parameter",
        description: "Required parameter was not provided",
    },
    ErrorInfo {
        code: ERR_CODE_PARAMETER_TYPE_MISMATCH,
        name: "PARAMETER_TYPE_MISMATCH",
        category: "Parameter",
        description: "Parameter value does not match expected type",
    },
    ErrorInfo {
        code: ERR_CODE_PARAMETER_NAME_CONFLICT,
        name: "PARAMETER_NAME_CONFLICT",
        category: "Parameter",
        description: "Parameter name conflicts with table name",
    },
];

/// Helper function to get error data from any JankenError variant
pub fn get_error_data(err: &JankenError) -> &ErrorData {
    match err {
        JankenError::QueryNotFound { data } => data,
        JankenError::ParameterNotProvided { data } => data,
        JankenError::ParameterTypeMismatch { data } => data,
        JankenError::ParameterNameConflict { data } => data,
    }
}

/// Get error information by code
pub fn get_error_info(code: u16) -> Option<&'static ErrorInfo> {
    ERROR_MAPPINGS.iter().find(|info| info.code == code)
}

/// Helper function to extract metadata field from error data as string
pub fn error_meta(data: &ErrorData, field: &str) -> Option<String> {
    data.metadata.as_ref().and_then(|metadata_str| {
        serde_json::from_str::<serde_json::Value>(metadata_str)
            .ok()
            .and_then(|metadata| metadata.get(field)?.as_str().map(|s| s.to_string()))
    })
}
