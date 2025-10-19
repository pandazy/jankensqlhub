use crate::{
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
            _ => Err(JankenError::ParameterTypeMismatch {
                expected: "integer, string, float, boolean, table_name or list".to_string(),
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
        };
        write!(f, "{s}")
    }
}

fn constraint_mismatch_error(param_type: &ParameterType, value: &serde_json::Value) -> JankenError {
    JankenError::ParameterTypeMismatch {
        expected: param_type.to_string(),
        got: value.to_string(),
    }
}

/// Parameter constraints for validation
#[derive(Debug, Clone, Default)]
pub struct ParameterConstraints {
    pub range: Option<Vec<f64>>, // For numeric types: [min, max]
    pub pattern: Option<String>, // For string types: regex pattern
    pub enum_values: Option<Vec<serde_json::Value>>, // For any type: allowed values
    pub item_type: Option<ParameterType>, // For list types: the type of each item
}

impl ParameterConstraints {
    /// Validate basic type (without any constraints)
    fn validate_basic_type(value: &serde_json::Value, param_type: &ParameterType) -> Result<()> {
        match param_type {
            ParameterType::String => {
                if !value.is_string() {
                    return Err(constraint_mismatch_error(param_type, value));
                }
            }
            ParameterType::Integer => {
                if !value.is_number() || value.as_number().unwrap().as_i64().is_none() {
                    return Err(constraint_mismatch_error(param_type, value));
                }
            }
            ParameterType::Float => {
                if !value.is_number() || value.as_number().unwrap().as_f64().is_none() {
                    return Err(constraint_mismatch_error(param_type, value));
                }
            }
            ParameterType::Boolean => {
                if !value.is_boolean() {
                    return Err(constraint_mismatch_error(param_type, value));
                }
            }
            ParameterType::TableName => {
                if !value.is_string() {
                    return Err(constraint_mismatch_error(param_type, value));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Validate parameter constraints (range, pattern, enum) assuming basic type validation is already done
    fn validate_constraints(
        &self,
        value: &serde_json::Value,
        param_type: &ParameterType,
    ) -> Result<()> {
        // First validate basic type
        Self::validate_basic_type(value, param_type)?;
        // Then validate the constraint rules
        self.validate_constraint_rules(value, param_type)
    }

    /// Validate constraint rules (range, pattern, enum) only, assuming basic type is already validated
    fn validate_constraint_rules(
        &self,
        value: &serde_json::Value,
        param_type: &ParameterType,
    ) -> Result<()> {
        // Check that range is only specified for numeric types
        if self.range.is_some()
            && !matches!(param_type, ParameterType::Integer | ParameterType::Float)
        {
            return Err(JankenError::ParameterTypeMismatch {
                expected: "numeric type".to_string(),
                got: param_type.to_string(),
            });
        }

        // Check range for numeric types
        if let Some(range) = &self.range {
            // Validated upfront that param_type is Integer or Float, so value is number
            let num_val = value.as_f64().unwrap();

            if let (Some(&min), Some(&max)) = (range.first(), range.get(1)) {
                if num_val < min || num_val > max {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: format!("value between {min} and {max}"),
                        got: num_val.to_string(),
                    });
                }
            }
        }

        // Check pattern for string types
        if let Some(pattern) = &self.pattern {
            if let Some(string_val) = value.as_str() {
                let regex =
                    Regex::new(pattern).map_err(|_| JankenError::ParameterTypeMismatch {
                        expected: "valid regex pattern".to_string(),
                        got: pattern.clone(),
                    })?;
                if !regex.is_match(string_val) {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: format!("string matching pattern '{pattern}'"),
                        got: string_val.to_string(),
                    });
                }
            } else {
                return Err(JankenError::ParameterTypeMismatch {
                    expected: ParameterType::String.to_string().to_lowercase(),
                    got: value.to_string(),
                });
            }
        }

        // Check enum values
        if let Some(enum_values) = &self.enum_values {
            if !enum_values.contains(value) {
                let enum_str = enum_values
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(JankenError::ParameterTypeMismatch {
                    expected: format!("one of [{enum_str}]"),
                    got: value.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Validate a parameter value against these constraints
    pub fn validate(&self, value: &serde_json::Value, param_type: &ParameterType) -> Result<()> {
        if param_type == &ParameterType::List {
            if !value.is_array() {
                return Err(constraint_mismatch_error(param_type, value));
            }

            // Validate each item in the list if item_type is specified
            // Note: item_type validation is already done during constraint parsing at definition time
            if let Some(item_type) = &self.item_type {
                let array = value.as_array().unwrap(); // already verified at the beginning;
                for (index, item) in array.iter().enumerate() {
                    // Validate basic type and constraints for each item
                    if Self::validate_basic_type(item, item_type).is_err() {
                        return Err(JankenError::ParameterTypeMismatch {
                            expected: format!("{item_type} at index {index}"),
                            got: item.to_string(),
                        });
                    }
                    self.validate_constraint_rules(item, item_type)?;
                }
            }
            // For Lists, constraints (range, pattern, enum) apply to items if item_type is set,
            // but not to the list itself, so we don't call validate_constraints after this match
            return Ok(());
        }

        self.validate_constraints(value, param_type)?;

        if param_type == &ParameterType::TableName {
            // value cannot be a non-string here since basic type validation has been done
            let table_name_str = value.as_str().unwrap();
            if table_name_str.is_empty()
                || !table_name_str
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_')
            {
                return Err(JankenError::ParameterTypeMismatch {
                    expected: "valid table name (alphanumeric and underscores only)".to_string(),
                    got: table_name_str.to_string(),
                });
            }
        }

        Ok(())
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
