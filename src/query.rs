use crate::{
    parameters::Parameter,
    result::{JankenError, Result},
};
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::str::FromStr;

/// Represents a parsed SQL query with parameters
#[derive(Debug)]
pub struct QueryDef {
    pub sql: String,
    pub parameters: Vec<Parameter>,
    pub returns: Vec<String>,
}

impl QueryDef {
    /// Create a new QueryDef from SQL string and an optional args object, parsing parameters and preparing for execution
    pub fn from_sql(
        sql: &str,
        args: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> Result<Self> {
        // Prevent explicit transaction control keywords (BEGIN, COMMIT, etc.) because they conflict with rusqlite's transaction handling
        if crate::parameters::contains_transaction_keywords(sql) {
            return Err(JankenError::ParameterTypeMismatch {
                expected: "SQL without explicit transaction keywords".to_string(),
                got:
                    "Query contains BEGIN, COMMIT, ROLLBACK, START TRANSACTION, or END TRANSACTION"
                        .to_string(),
            });
        }

        // Parse parameters while respecting quote boundaries (only extracts names)
        let mut parameters = crate::parameters::parse_parameters_with_quotes(sql)?;

        // Validate parameter types and constraints from "args"
        // If parameters exist, they MUST be defined in args
        for param in &mut parameters {
            if let Some(args_map) = args {
                if let Some(arg_def) = args_map.get(&param.name) {
                    // Parse type from args
                    if let Some(type_val) = arg_def.get("type") {
                        if let Some(type_str) = type_val.as_str() {
                            let param_type = crate::parameters::ParameterType::from_str(type_str)
                                .map_err(|_| JankenError::ParameterTypeMismatch {
                                expected: "integer, string, float, or boolean".to_string(),
                                got: type_str.to_string(),
                            })?;
                            param.param_type = param_type;
                        }
                    }

                    // Parse constraints
                    if let Some(range_val) = arg_def.get("range") {
                        if let Some(range_array) = range_val.as_array() {
                            let range: Vec<f64> =
                                range_array.iter().filter_map(|v| v.as_f64()).collect();
                            param.constraints.range = Some(range);
                        }
                    }

                    if let Some(pattern_val) = arg_def.get("pattern") {
                        if let Some(pattern_str) = pattern_val.as_str() {
                            param.constraints.pattern = Some(pattern_str.to_string());
                        }
                    }

                    if let Some(enum_val) = arg_def.get("enum") {
                        if let Some(enum_array) = enum_val.as_array() {
                            param.constraints.enum_values = Some(enum_array.clone());
                        }
                    }
                } else {
                    // Parameter found in SQL but not defined in args - this is an error
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: "parameter definition in args".to_string(),
                        got: format!("parameter '{}' not defined in args object", param.name),
                    });
                }
            } else {
                // No args provided but parameters found in SQL
                return Err(JankenError::ParameterTypeMismatch {
                    expected: "args object with parameter definitions".to_string(),
                    got: format!(
                        "parameter '{}' found in SQL but no args object provided",
                        param.name
                    ),
                });
            }
        }

        Ok(QueryDef {
            sql: sql.to_string(),
            parameters,
            returns: Vec::new(),
        })
    }
}

/// Collection of parsed SQL query definitions loaded from JSON configuration
#[derive(Debug)]
pub struct QueryDefinitions {
    /// Named query definitions keyed by their identifying name
    pub definitions: HashMap<String, QueryDef>,
}

impl QueryDefinitions {
    /// Load query definitions from a JSON file
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let json: serde_json::Value = serde_json::from_str(&content)?;
        Self::from_json(json)
    }

    /// Load query definitions from a serde_json::Value object
    pub fn from_json(json: serde_json::Value) -> Result<Self> {
        let json_map = json
            .as_object()
            .ok_or_else(|| JankenError::ParameterTypeMismatch {
                expected: "object".to_string(),
                got: json.to_string(),
            })?;

        let mut definitions = HashMap::new();
        for (name, value) in json_map {
            let map = value
                .as_object()
                .ok_or_else(|| JankenError::ParameterTypeMismatch {
                    expected: "object".to_string(),
                    got: format!("{name}: {value}"),
                })?;

            let sql = map.get("query").and_then(|q| q.as_str()).ok_or_else(|| {
                JankenError::ParameterTypeMismatch {
                    expected: "required 'query' field with string value".to_string(),
                    got: format!("{name}: missing 'query' field"),
                }
            })?;

            let args = map.get("args").and_then(|a| a.as_object());
            let mut query_def = QueryDef::from_sql(sql, args)?;

            // Parse returns field
            if let Some(returns_val) = map.get("returns") {
                if let Some(returns_array) = returns_val.as_array() {
                    let returns: Vec<String> = returns_array
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();
                    // Deduplicate using a set but maintain order
                    let mut seen = std::collections::HashSet::new();
                    let unique_returns: Vec<String> = returns
                        .into_iter()
                        .filter(|item| seen.insert(item.clone()))
                        .collect();
                    query_def.returns = unique_returns;
                } else {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: "array of strings".to_string(),
                        got: returns_val.to_string(),
                    });
                }
            } else {
                // No returns specified - empty array
                query_def.returns = Vec::new();
            }

            definitions.insert(name.clone(), query_def);
        }
        Ok(QueryDefinitions { definitions })
    }
}
