use crate::{
    parameter_constraints::ParameterConstraints,
    result::{JankenError, Result},
    str_utils::is_in_quotes,
};
use regex::Regex;
use std::str::FromStr;

// Regex compiled once as a lazy static for performance
pub static PARAMETER_REGEX: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"@(\w+)").unwrap());
pub static TABLE_NAME_REGEX: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"#(\w+)").unwrap());
pub static LIST_PARAMETER_REGEX: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r":\[(\w+)\]").unwrap());

/// Parameter type enums for database operations
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterType {
    Integer,
    String,
    Float,
    Boolean,
    TableName,
    List,
    Blob,
}

impl FromStr for ParameterType {
    type Err = JankenError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "integer" => Ok(ParameterType::Integer),
            "string" => Ok(ParameterType::String),
            "float" => Ok(ParameterType::Float),
            "boolean" => Ok(ParameterType::Boolean),
            "table_name" => Ok(ParameterType::TableName),
            "list" => Ok(ParameterType::List),
            "blob" => Ok(ParameterType::Blob),
            _ => Err(JankenError::ParameterTypeMismatch {
                expected: "integer, string, float, boolean, table_name, list or blob".to_string(),
                got: s.to_string(),
            }),
        }
    }
}

impl std::fmt::Display for ParameterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ParameterType::Integer => "integer",
            ParameterType::String => "string",
            ParameterType::Float => "float",
            ParameterType::Boolean => "boolean",
            ParameterType::TableName => "table_name",
            ParameterType::List => "list",
            ParameterType::Blob => "blob",
        };
        write!(f, "{s}")
    }
}

/// Parameter definition for SQL queries with validation constraints
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub param_type: ParameterType,
    pub constraints: ParameterConstraints,
}

/// Parse parameters from SQL while respecting quote boundaries
/// Extracts normal parameters (@param), table name parameters (#table), and list parameters (:[list]) from the SQL.
/// Returns combined results. If a name is used for multiple parameter types, an error is returned.
/// For @params: type defaults to String but can be overridden by "args" JSON
/// For #table names: type is always TableName (auto-detected), only constraints from "args" JSON are applied
/// For :[list] parameters: type is always List (auto-detected), constraints from "args" JSON are applied
pub fn parse_parameters_with_quotes(sql: &str) -> Result<Vec<Parameter>> {
    let param_names = extract_parameters_with_regex(sql, &PARAMETER_REGEX);
    let table_names = extract_parameters_with_regex(sql, &TABLE_NAME_REGEX);
    let list_names = extract_parameters_with_regex(sql, &LIST_PARAMETER_REGEX);

    // Check for conflicts between parameter names
    for table_name in &table_names {
        if param_names.contains(table_name) || list_names.contains(table_name) {
            return Err(JankenError::ParameterNameConflict(table_name.clone()));
        }
    }
    for list_name in &list_names {
        if param_names.contains(list_name) {
            return Err(JankenError::ParameterNameConflict(list_name.clone()));
        }
    }

    let mut parameters = Vec::new();

    // Add normal parameters (@param)
    for name in &param_names {
        parameters.push(Parameter {
            name: name.clone(),
            param_type: ParameterType::String, // Default type - will be overridden by "args" JSON if present
            constraints: ParameterConstraints::default(),
        });
    }

    // Add table name parameters (#table)
    for name in &table_names {
        parameters.push(Parameter {
            name: name.clone(),
            param_type: ParameterType::TableName,
            constraints: ParameterConstraints::default(),
        });
    }

    // Add list parameters (:[list])
    for name in &list_names {
        parameters.push(Parameter {
            name: name.clone(),
            param_type: ParameterType::List,
            constraints: ParameterConstraints::default(),
        });
    }

    Ok(parameters)
}

/// Extract parameter names from a single statement
pub fn extract_parameters_in_statement(statement: &str) -> Vec<String> {
    extract_parameters_with_regex(statement, &PARAMETER_REGEX)
}

/// Helper function to extract unique parameter names with regex, respecting quotes
/// Returns in order of first appearance in the SQL
pub fn extract_parameters_with_regex(statement: &str, regex: &Regex) -> Vec<String> {
    let mut params = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in regex.captures_iter(statement) {
        if let Some(named_match) = cap.get(0) {
            if !is_in_quotes(statement, named_match.start()) {
                let name = cap.get(1).unwrap().as_str().to_string();
                if seen.insert(name.clone()) {
                    params.push(name);
                }
            }
        }
    }

    params
}

/// Check if SQL contains transaction control keywords that conflict with rusqlite
pub fn contains_transaction_keywords(sql: &str) -> bool {
    let upper_sql = sql.to_uppercase();
    upper_sql.contains("BEGIN")
        || upper_sql.contains("COMMIT")
        || upper_sql.contains("ROLLBACK")
        || upper_sql.contains("START TRANSACTION")
        || upper_sql.contains("END TRANSACTION")
}
