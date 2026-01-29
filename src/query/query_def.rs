use crate::{
    ParameterType,
    parameter_constraints::parse_constraints,
    parameters::{self, Parameter},
    result::{JankenError, Result},
};
use std::str::FromStr;

/// Specification for which fields to return from a query
#[derive(Debug, Clone)]
pub enum ReturnsSpec {
    /// Static list of field names specified at definition time
    Static(Vec<String>),
    /// Dynamic reference to a comma_list parameter: ~[param_name]
    Dynamic(String), // Stores the parameter name (without ~[])
}

/// Represents a parsed SQL query with parameters
#[derive(Debug)]
pub struct QueryDef {
    pub sql: String,
    pub parameters: Vec<Parameter>,
    pub returns: ReturnsSpec,
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
            returns: ReturnsSpec::Static(Vec::new()),
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
        // Skip parameters that are not String type (i.e., TableName, List, CommaList are auto-detected)
        let skip_types = [
            ParameterType::TableName,
            ParameterType::List,
            ParameterType::CommaList,
        ];
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
        if param.param_type == ParameterType::TableName
            || param.param_type == ParameterType::List
            || param.param_type == ParameterType::CommaList
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
