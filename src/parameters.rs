use crate::{
    result::{JankenError, Result},
    str_utils::is_in_quotes,
};
use regex::Regex;
use std::str::FromStr;

// Regex compiled once as a lazy static for performance
static PARAMETER_REGEX: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"@(\w+)").unwrap());
static TABLE_NAME_REGEX: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"#(\w+)").unwrap());

/// Parameter type enums for database operations
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterType {
    Integer,
    String,
    Float,
    Boolean,
    TableName,
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
            _ => Err(JankenError::ParameterTypeMismatch {
                expected: "integer, string, float, boolean or table_name".to_string(),
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
        };
        write!(f, "{s}")
    }
}

/// Parameter constraints for validation
#[derive(Debug, Clone, Default)]
pub struct ParameterConstraints {
    pub range: Option<Vec<f64>>, // For numeric types: [min, max]
    pub pattern: Option<String>, // For string types: regex pattern
    pub enum_values: Option<Vec<serde_json::Value>>, // For any type: allowed values
}

impl ParameterConstraints {
    /// Validate a parameter value against these constraints
    pub fn validate(&self, value: &serde_json::Value, param_type: &ParameterType) -> Result<()> {
        // Check range for numeric types
        if let Some(range) = &self.range {
            let num_val =
                if param_type.to_string() == "integer" || param_type.to_string() == "float" {
                    if let Some(num) = value.as_f64() {
                        num
                    } else {
                        return Err(JankenError::ParameterTypeMismatch {
                            expected: "number (integer/float)".to_string(),
                            got: value.to_string(),
                        });
                    }
                } else {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: "numeric type".to_string(),
                        got: format!("{param_type:?}").to_lowercase(),
                    });
                };

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

        if param_type == &ParameterType::TableName {
            if let Some(table_name_str) = value.as_str() {
                if table_name_str.is_empty()
                    || !table_name_str
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_')
                {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: "valid table name (alphanumeric and underscores only)"
                            .to_string(),
                        got: table_name_str.to_string(),
                    });
                }
            } else {
                return Err(JankenError::ParameterTypeMismatch {
                    expected: "string (table_name)".to_string(),
                    got: value.to_string(),
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
/// Extracts both normal parameters (@param) and table name parameters (#table) from the SQL.
/// Returns combined results. If a name is used for both parameter types, an error is returned.
/// For @params: type defaults to String but can be overridden by "args" JSON
/// For #table names: type is always TableName (auto-detected), only constraints from "args" JSON are applied
pub fn parse_parameters_with_quotes(sql: &str) -> Result<Vec<Parameter>> {
    let param_names = extract_parameters_with_regex(sql, &PARAMETER_REGEX);
    let table_names = extract_parameters_with_regex(sql, &TABLE_NAME_REGEX);

    // Check for conflicts between parameter names and table names
    for table_name in &table_names {
        if param_names.contains(table_name) {
            return Err(JankenError::ParameterNameConflict(table_name.clone()));
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

    Ok(parameters)
}

/// Extract parameter names from a single statement
pub fn extract_parameters_in_statement(statement: &str) -> Vec<String> {
    extract_parameters_with_regex(statement, &PARAMETER_REGEX)
}

/// Helper function to extract unique parameter names with regex, respecting quotes
/// Returns in order of first appearance in the SQL
fn extract_parameters_with_regex(statement: &str, regex: &Regex) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_type_display_table_name() {
        // Test that ParameterType::TableName displays correctly
        // Since TableName is auto-detected and not set directly by users,
        // we need a simple test to cover this display path
        let param = Parameter {
            name: "test_table".to_string(),
            param_type: ParameterType::TableName,
            constraints: ParameterConstraints::default(),
        };
        assert_eq!(param.param_type.to_string(), "table_name");
    }
}
