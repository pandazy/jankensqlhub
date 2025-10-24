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
    once_cell::sync::Lazy::new(|| Regex::new(r"#\[(\w+)\]").unwrap());
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
/// Extracts normal parameters (@param), table name parameters (#\[table\]), and list parameters (:\[list\]) from the SQL.
/// Returns combined results. If a name is used for multiple parameter types, an error is returned.
/// For @params: type defaults to String but can be overridden by "args" JSON
/// For #\[table\] names: type is always TableName (auto-detected), only constraints from "args" JSON are applied
/// For :\[list\] parameters: type is always List (auto-detected), constraints from "args" JSON are applied
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
/// Keywords must be whole words (surrounded by whitespace or punctuation) to avoid
/// false positives from identifiers containing these substrings
pub fn contains_transaction_keywords(sql: &str) -> bool {
    let upper_sql = sql.to_uppercase();

    // Check for exact keyword matches using word boundaries
    regex::Regex::new(r"(?i)\b(BEGIN|COMMIT|ROLLBACK|START TRANSACTION|END TRANSACTION)\b")
        .unwrap()
        .is_match(&upper_sql)
}

// ============================================================================
// GENERIC PARAMETER VALUE TYPES
// ============================================================================

/// Generic representation of typed parameter values, database-agnostic
/// This allows sharing parameter preparation logic across different database backends
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Blob(Vec<u8>),
    Null,
}

/// Prepared statement with generic parameter values (database-agnostic)
#[derive(Debug)]
pub struct PreparedParameterStatement {
    pub sql: String,
    pub parameters: Vec<(String, ParameterValue)>,
}

/// Convert a JSON value to a generic ParameterValue
/// This handles the database-agnostic conversion from serde_json::Value to our internal types
pub fn json_value_to_parameter_value(
    value: &serde_json::Value,
    param_type: &ParameterType,
) -> Result<ParameterValue> {
    match param_type {
        ParameterType::String => {
            let s = value
                .as_str()
                .ok_or_else(|| JankenError::ParameterTypeMismatch {
                    expected: "string".to_string(),
                    got: value.to_string(),
                })?;
            Ok(ParameterValue::String(s.to_string()))
        }
        ParameterType::Integer => {
            let i = value
                .as_i64()
                .ok_or_else(|| JankenError::ParameterTypeMismatch {
                    expected: "integer".to_string(),
                    got: value.to_string(),
                })?;
            Ok(ParameterValue::Integer(i))
        }
        ParameterType::Float => {
            let f = value
                .as_f64()
                .ok_or_else(|| JankenError::ParameterTypeMismatch {
                    expected: "float".to_string(),
                    got: value.to_string(),
                })?;
            Ok(ParameterValue::Float(f))
        }
        ParameterType::Boolean => {
            let b = value
                .as_bool()
                .ok_or_else(|| JankenError::ParameterTypeMismatch {
                    expected: "boolean".to_string(),
                    got: value.to_string(),
                })?;
            Ok(ParameterValue::Boolean(b))
        }
        ParameterType::TableName => {
            let s = value
                .as_str()
                .ok_or_else(|| JankenError::ParameterTypeMismatch {
                    expected: "string (table name)".to_string(),
                    got: value.to_string(),
                })?;
            Ok(ParameterValue::String(s.to_string()))
        }
        ParameterType::List => {
            // List parameters are expanded separately, so we shouldn't convert them here
            Err(JankenError::ParameterTypeMismatch {
                expected: "non-list parameter".to_string(),
                got: "list parameter (should be expanded)".to_string(),
            })
        }
        ParameterType::Blob => {
            let bytes = value
                .as_array()
                .ok_or_else(|| JankenError::ParameterTypeMismatch {
                    expected: "array of byte values".to_string(),
                    got: value.to_string(),
                })?
                .iter()
                .map(|v| v.as_u64().unwrap_or(0) as u8)
                .collect();
            Ok(ParameterValue::Blob(bytes))
        }
    }
}

/// Generic parameter statement preparation (database-agnostic)
/// This handles SQL analysis, parameter validation, and SQL transformations without database-specific conversions
pub fn prepare_parameter_statement_generic(
    statement_sql: &str,
    all_parameters: &[Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<PreparedParameterStatement> {
    // Validate all parameters first to ensure consistency and prevent SQL injection
    for param_def in all_parameters {
        let value = request_params_obj
            .get(&param_def.name)
            .ok_or_else(|| JankenError::ParameterNotProvided(param_def.name.clone()))?;

        // Validate parameter constraints
        param_def.constraints.validate(
            value,
            &param_def.param_type,
            &param_def.name,
            request_params_obj,
        )?;
    }

    let mut prepared_sql = statement_sql.to_string();
    let mut parameters = Vec::new();

    // Convert JSON parameter values to generic ParameterValue types
    let statement_param_names = extract_parameters_with_regex(&prepared_sql, &PARAMETER_REGEX);
    for param_name in &statement_param_names {
        let param_value = request_params_obj
            .get(param_name)
            .ok_or_else(|| JankenError::ParameterNotProvided(param_name.clone()))?;

        let param_def = all_parameters
            .iter()
            .find(|p| p.name == *param_name)
            .ok_or_else(|| JankenError::ParameterNotProvided(param_name.clone()))?;

        let generic_value = json_value_to_parameter_value(param_value, &param_def.param_type)?;
        parameters.push((param_name.clone(), generic_value));
    }

    // Handle table name replacement (#\[table_name\])
    for cap in TABLE_NAME_REGEX.captures_iter(&prepared_sql.clone()) {
        if let Some(param_name_match) = cap.get(1) {
            let param_name = param_name_match.as_str();

            let table_name_value = request_params_obj
                .get(param_name)
                .ok_or_else(|| JankenError::ParameterNotProvided(param_name.to_string()))?;

            let table_name_str =
                table_name_value
                    .as_str()
                    .ok_or_else(|| JankenError::ParameterTypeMismatch {
                        expected: "string (table name)".to_string(),
                        got: table_name_value.to_string(),
                    })?;

            let valid_ident = crate::str_utils::quote_identifier(table_name_str);
            prepared_sql = TABLE_NAME_REGEX
                .replace(&prepared_sql, valid_ident)
                .to_string();
        }
    }

    // Handle list parameter expansion (:\[list\])
    for cap in LIST_PARAMETER_REGEX.captures_iter(&prepared_sql.clone()) {
        if let Some(_param_match) = cap.get(0) {
            if let Some(param_name_match) = cap.get(1) {
                let list_param_name = param_name_match.as_str();

                let list_value = request_params_obj.get(list_param_name).ok_or_else(|| {
                    JankenError::ParameterNotProvided(list_param_name.to_string())
                })?;

                let list_array =
                    list_value
                        .as_array()
                        .ok_or_else(|| JankenError::ParameterTypeMismatch {
                            expected: "array".to_string(),
                            got: list_value.to_string(),
                        })?;

                if list_array.is_empty() {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: "non-empty list".to_string(),
                        got: "empty array".to_string(),
                    });
                }

                // Create positional placeholders and values
                // For list items, we infer the type from the JSON value itself
                let mut placeholders = Vec::new();
                for (i, item) in list_array.iter().enumerate() {
                    let param_key = format!("{list_param_name}_{i}");
                    placeholders.push(format!("@{param_key}")); // Keep @ for consistency

                    // Convert JSON value to generic ParameterValue, handling different types
                    let generic_value = match item {
                        serde_json::Value::String(s) => ParameterValue::String(s.clone()),
                        serde_json::Value::Number(n) => {
                            if n.is_i64() {
                                ParameterValue::Integer(n.as_i64().unwrap())
                            } else {
                                ParameterValue::Float(n.as_f64().unwrap())
                            }
                        }
                        serde_json::Value::Bool(b) => ParameterValue::Boolean(*b),
                        serde_json::Value::Null => ParameterValue::Null,
                        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                            // Arrays and objects get converted to JSON strings
                            ParameterValue::String(serde_json::to_string(item).unwrap_or_default())
                        }
                    };
                    parameters.push((param_key, generic_value));
                }

                // Replace the :[param] with (positional placeholders joined by comma)
                let placeholder_str = placeholders.join(", ");
                prepared_sql = LIST_PARAMETER_REGEX
                    .replace(&prepared_sql, format!("({placeholder_str})"))
                    .to_string();
            }
        }
    }

    Ok(PreparedParameterStatement {
        sql: prepared_sql,
        parameters,
    })
}
