use crate::{
    ParameterType,
    parameter_constraints::parse_constraints,
    parameters::{self, Parameter},
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

        let mut parameters = parameters::parse_parameters_with_quotes(sql)?;
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
        let got = "Query contains BEGIN, COMMIT, ROLLBACK, START TRANSACTION, or END TRANSACTION";
        if parameters::contains_transaction_keywords(sql) {
            let expected = "SQL without explicit transaction keywords";
            Err(JankenError::new_parameter_type_mismatch(expected, got))
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
        if param.param_type == ParameterType::TableName || param.param_type == ParameterType::List {
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
            parse_constraints(&mut param.constraints, arg_def)?;
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
        parse_constraints(&mut param.constraints, arg_def)?;

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
                let err = JankenError::new_parameter_type_mismatch("object", json.to_string());
                return Err(err);
            }
        };

        let mut definitions = HashMap::new();
        for (name, value) in json_map {
            let map = value.as_object().ok_or_else(|| {
                JankenError::new_parameter_type_mismatch("object", format!("{name}: {value}"))
            })?;

            let sql = map.get("query").and_then(|q| q.as_str()).ok_or_else(|| {
                JankenError::new_parameter_type_mismatch(
                    "required 'query' field with string value",
                    format!("{name}: missing 'query' field"),
                )
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
                    return Err(JankenError::new_parameter_type_mismatch(
                        "array of strings",
                        returns_val.to_string(),
                    ));
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
