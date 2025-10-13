use crate::{
    result::{JankenError, Result},
    str_utils::is_in_quotes,
};
use regex::Regex;
use std::str::FromStr;

// Regex compiled once as a lazy static for performance
static PARAMETER_REGEX: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"@(\w+)").unwrap());

/// Parameter type enums for database operations
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterType {
    Integer,
    String,
    Float,
    Boolean,
}

impl FromStr for ParameterType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "integer" => Ok(ParameterType::Integer),
            "string" => Ok(ParameterType::String),
            "float" => Ok(ParameterType::Float),
            "boolean" => Ok(ParameterType::Boolean),
            _ => Err(format!("Unknown parameter type: {s}")),
        }
    }
}

impl ParameterType {
    pub fn to_string(&self) -> &'static str {
        match self {
            ParameterType::Integer => "integer",
            ParameterType::String => "string",
            ParameterType::Float => "float",
            ParameterType::Boolean => "boolean",
        }
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
            let num_val = match param_type {
                ParameterType::Integer => {
                    if let Some(int_val) = value.as_i64() {
                        int_val as f64
                    } else {
                        return Err(JankenError::ParameterTypeMismatch {
                            expected: "number".to_string(),
                            got: value.to_string(),
                        });
                    }
                }
                ParameterType::Float => {
                    if let Some(float_val) = value.as_f64() {
                        float_val
                    } else {
                        return Err(JankenError::ParameterTypeMismatch {
                            expected: "number".to_string(),
                            got: value.to_string(),
                        });
                    }
                }
                _ => {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: "numeric type".to_string(),
                        got: param_type.to_string().to_string(),
                    });
                }
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
            } else if *param_type == ParameterType::String {
                return Err(JankenError::ParameterTypeMismatch {
                    expected: "string".to_string(),
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
}

/// Parameter definition for SQL queries with validation constraints
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub param_type: ParameterType,
    pub constraints: ParameterConstraints,
}

/// Parse parameters from SQL while respecting quote boundaries
/// Note: This now only extracts unique parameter names. Types and constraints are defined in the "args" JSON object.
pub fn parse_parameters_with_quotes(sql: &str) -> Result<Vec<Parameter>> {
    let mut param_names = std::collections::HashSet::new();
    let mut parameters = Vec::new();

    // Process all captures while respecting quotes
    for cap in PARAMETER_REGEX.captures_iter(sql) {
        // Check if this parameter is inside quotes
        if let Some(named_match) = cap.get(0) {
            if is_in_quotes(sql, named_match.start()) {
                // Skip parameters inside quotes - they're literal text
                continue;
            }
        }

        // Parse the parameter name only
        let name = cap
            .get(1)
            .ok_or_else(|| JankenError::ParameterTypeMismatch {
                expected: "valid parameter name".to_string(),
                got: "missing parameter name in regex capture".to_string(),
            })?
            .as_str()
            .to_string();

        // Skip duplicates - each parameter should only be added once
        if param_names.contains(&name) {
            continue;
        }

        param_names.insert(name.clone());

        // Create parameter with default type - will be overridden by "args" JSON if present
        parameters.push(Parameter {
            name,
            param_type: ParameterType::String, // Default type
            constraints: ParameterConstraints::default(),
        });
    }

    Ok(parameters)
}

/// Create prepared statement by replacing parameters with placeholders
/// Takes a closure that generates the placeholder format for each parameter index
pub fn create_prepared_statement<F>(
    sql: &str,
    parameters: &[Parameter],
    placeholder_gen: F,
) -> Result<String>
where
    F: Fn(usize) -> String,
{
    let mut result = sql.to_string();
    let offsets = compute_parameter_offsets(sql, parameters)?;

    // Replace in reverse order to maintain correct positions
    for (param_idx, offset) in offsets.iter().enumerate().rev() {
        let placeholder = placeholder_gen(param_idx + 1); // 1-indexed
        result.replace_range(offset.start..offset.end, &placeholder);
    }

    Ok(result)
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

/// Compute parameter offsets in SQL, excluding quoted parameters
pub fn compute_parameter_offsets(
    sql: &str,
    parameters: &[Parameter],
) -> Result<Vec<std::ops::Range<usize>>> {
    let mut offsets = Vec::new();
    let mut param_iter = parameters.iter();

    for mat in PARAMETER_REGEX.find_iter(sql) {
        // Check if this parameter is inside quotes
        if is_in_quotes(sql, mat.start()) {
            // Skip parameters inside quotes
            continue;
        }

        // This is a parameter we're keeping
        if let Some(_param) = param_iter.next() {
            offsets.push(mat.range());
        }
    }

    Ok(offsets)
}
