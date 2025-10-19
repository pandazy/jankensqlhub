use crate::{
    ParameterType,
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
        Self::check_transaction_keywords(sql)?;

        let mut parameters = crate::parameters::parse_parameters_with_quotes(sql)?;
        // Create augmented args with defaults for @params not specified in input args
        let augmented_args = Self::create_augmented_args(&parameters, args);

        for param in &mut parameters {
            Self::process_parameter_with_args(param, &augmented_args)?;
        }

        Ok(QueryDef {
            sql: sql.to_string(),
            parameters,
            returns: Vec::new(),
        })
    }

    fn check_transaction_keywords(sql: &str) -> Result<()> {
        if crate::parameters::contains_transaction_keywords(sql) {
            Err(JankenError::ParameterTypeMismatch {
                expected: "SQL without explicit transaction keywords".to_string(),
                got:
                    "Query contains BEGIN, COMMIT, ROLLBACK, START TRANSACTION, or END TRANSACTION"
                        .to_string(),
            })
        } else {
            Ok(())
        }
    }

    /// Create augmented args by adding default "type": "string" for @params not specified in input args
    fn create_augmented_args(
        parameters: &[Parameter],
        args: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut augmented_args = match args {
            Some(existing_args) => existing_args.clone(),
            None => serde_json::Map::new(),
        };

        // For each parameter that doesn't have an arg definition, add default string type
        // Skip parameters that are not String type (i.e., TableName, List are auto-detected)
        let skip_types = [ParameterType::TableName, ParameterType::List];
        for param in parameters {
            if !skip_types.contains(&param.param_type) && !augmented_args.contains_key(&param.name)
            {
                // Only add default string type for String parameters without args
                let default_arg = serde_json::json!({ "type": "string" });
                augmented_args.insert(param.name.clone(), default_arg);
            }
        }
        augmented_args
    }

    fn process_parameter_with_args(
        param: &mut Parameter,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        if param.param_type == crate::parameters::ParameterType::TableName
            || param.param_type == crate::parameters::ParameterType::List
        {
            Self::process_automatic_parameter(param, args)?;
        } else {
            Self::process_regular_parameter(param, args)?;
        }
        Ok(())
    }

    fn process_automatic_parameter(
        param: &mut Parameter,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        if let Some(arg_def) = args.get(&param.name) {
            Self::parse_constraints(&mut param.constraints, arg_def)?;
        }
        Ok(())
    }

    fn process_regular_parameter(
        param: &mut Parameter,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        // Due to augmented args creation, we know this parameter must exist in args
        let arg_def = args.get(&param.name).unwrap();
        Self::parse_regular_parameter_type(param, arg_def)?;
        Self::parse_constraints(&mut param.constraints, arg_def)?;

        Ok(())
    }

    fn parse_regular_parameter_type(
        param: &mut Parameter,
        arg_def: &serde_json::Value,
    ) -> Result<()> {
        if let Some(type_val) = arg_def.get("type") {
            if let Some(type_str) = type_val.as_str() {
                let new_param_type = ParameterType::from_str(type_str)?;

                // Only assign if the type is different from the current parameter type
                if new_param_type != param.param_type {
                    param.param_type = new_param_type;
                }
            }
        }
        Ok(())
    }

    fn parse_constraints(
        constraints: &mut crate::parameters::ParameterConstraints,
        arg_def: &serde_json::Value,
    ) -> Result<()> {
        if let Some(range_val) = arg_def.get("range") {
            if let Some(range_array) = range_val.as_array() {
                let range: Vec<f64> = range_array.iter().filter_map(|v| v.as_f64()).collect();
                constraints.range = Some(range);
            }
        }

        if let Some(pattern_val) = arg_def.get("pattern") {
            if let Some(pattern_str) = pattern_val.as_str() {
                constraints.pattern = Some(pattern_str.to_string());
            }
        }

        if let Some(enum_val) = arg_def.get("enum") {
            if let Some(enum_array) = enum_val.as_array() {
                constraints.enum_values = Some(enum_array.clone());
            }
        }

        if let Some(itemtype_val) = arg_def.get("itemtype") {
            if let Some(itemtype_str) = itemtype_val.as_str() {
                let item_type = ParameterType::from_str(itemtype_str)?;
                // Validate item type - TableName and List are not allowed as item types
                match item_type {
                    ParameterType::TableName | ParameterType::List => {
                        return Err(JankenError::ParameterTypeMismatch {
                            expected: "item_type for list items cannot be TableName or List"
                                .to_string(),
                            got: item_type.to_string(),
                        });
                    }
                    _ => {}
                }
                constraints.item_type = Some(item_type);
            }
        }

        Ok(())
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
        let json_map = match json.as_object() {
            Some(map) => map,
            None => {
                return Err(JankenError::ParameterTypeMismatch {
                    expected: "object".to_string(),
                    got: json.to_string(),
                });
            }
        };

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
