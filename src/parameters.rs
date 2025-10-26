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
            _ => Err(JankenError::new_parameter_type_mismatch(
                "integer, string, float, boolean, table_name, list or blob",
                s,
            )),
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
            return Err(JankenError::new_parameter_name_conflict(table_name.clone()));
        }
    }
    for list_name in &list_names {
        if param_names.contains(list_name) {
            return Err(JankenError::new_parameter_name_conflict(list_name.clone()));
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
}

/// Prepared statement with generic parameter values (database-agnostic)
#[derive(Debug)]
pub struct PreparedParameterStatement {
    pub sql: String,
    pub parameters: Vec<(String, ParameterValue)>,
}

/// Convert a JSON value to a generic ParameterValue with type inference
/// Used for list items where the type is inferred from the JSON value itself
/// This handles: String->String, Number->Integer/Float, Bool->Boolean, Array/Object->JSON string
pub fn json_value_to_parameter_value_inferred(item: &serde_json::Value) -> Result<ParameterValue> {
    match item {
        serde_json::Value::String(s) => Ok(ParameterValue::String(s.clone())),
        serde_json::Value::Number(n) => {
            if n.is_i64() {
                // Safe unwrap: numbers are validated by constraints to be valid i64/f64
                Ok(ParameterValue::Integer(n.as_i64().unwrap()))
            } else {
                // Safe unwrap: numbers are validated by constraints to be valid i64/f64
                Ok(ParameterValue::Float(n.as_f64().unwrap()))
            }
        }
        serde_json::Value::Bool(b) => Ok(ParameterValue::Boolean(*b)),
        serde_json::Value::Null => Err(JankenError::new_parameter_type_mismatch(
            "non-null value",
            "null",
        )),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            // Arrays and objects get converted to JSON strings
            // Safe unwrap: serde_json::Value is always valid JSON and can always be serialized
            Ok(ParameterValue::String(serde_json::to_string(item).unwrap()))
        }
    }
}

/// Convert a JSON value to a generic ParameterValue
/// This handles the database-agnostic conversion from serde_json::Value to our internal types
pub fn json_value_to_parameter_value(
    value: &serde_json::Value,
    param_type: &ParameterType,
) -> Result<ParameterValue> {
    match param_type {
        ParameterType::String => {
            let s = value.as_str().ok_or_else(|| {
                JankenError::new_parameter_type_mismatch("string", value.to_string())
            })?;
            Ok(ParameterValue::String(s.to_string()))
        }
        ParameterType::Integer => {
            let i = value.as_i64().ok_or_else(|| {
                JankenError::new_parameter_type_mismatch("integer", value.to_string())
            })?;
            Ok(ParameterValue::Integer(i))
        }
        ParameterType::Float => {
            let f = value.as_f64().ok_or_else(|| {
                JankenError::new_parameter_type_mismatch("float", value.to_string())
            })?;
            Ok(ParameterValue::Float(f))
        }
        ParameterType::Boolean => {
            let b = value.as_bool().ok_or_else(|| {
                JankenError::new_parameter_type_mismatch("boolean", value.to_string())
            })?;
            Ok(ParameterValue::Boolean(b))
        }
        ParameterType::TableName => {
            let s = value.as_str().ok_or_else(|| {
                JankenError::new_parameter_type_mismatch("string (table name)", value.to_string())
            })?;
            Ok(ParameterValue::String(s.to_string()))
        }
        ParameterType::List => {
            // List parameters are expanded separately, so we shouldn't convert them here
            Err(JankenError::new_parameter_type_mismatch(
                "non-list parameter",
                "list parameter (should be expanded)",
            ))
        }
        ParameterType::Blob => {
            let bytes = value
                .as_array()
                .ok_or_else(|| {
                    JankenError::new_parameter_type_mismatch(
                        "array of byte values",
                        value.to_string(),
                    )
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
            .ok_or_else(|| JankenError::new_parameter_not_provided(param_def.name.clone()))?;

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
            .ok_or_else(|| JankenError::new_parameter_not_provided(param_name.clone()))?;

        let param_def = all_parameters
            .iter()
            .find(|p| p.name == *param_name)
            .ok_or_else(|| JankenError::new_parameter_not_provided(param_name.clone()))?;

        let generic_value = json_value_to_parameter_value(param_value, &param_def.param_type)?;
        parameters.push((param_name.clone(), generic_value));
    }

    // Handle table name replacement (#\[table_name\])
    for cap in TABLE_NAME_REGEX.captures_iter(&prepared_sql.clone()) {
        if let Some(param_name_match) = cap.get(1) {
            let param_name = param_name_match.as_str();

            // Safe unwrap: parameter presence already validated at function start
            let table_name_value = request_params_obj.get(param_name).unwrap();

            // Safe unwrap: parameter type already validated at function start
            let table_name_str = table_name_value.as_str().unwrap();
            prepared_sql = TABLE_NAME_REGEX
                .replace(&prepared_sql, table_name_str)
                .to_string();
        }
    }

    // Handle list parameter expansion (:\[list\])
    for cap in LIST_PARAMETER_REGEX.captures_iter(&prepared_sql.clone()) {
        if let Some(_param_match) = cap.get(0) {
            if let Some(param_name_match) = cap.get(1) {
                let list_param_name = param_name_match.as_str();

                // Safe unwrap: parameter presence already validated at function start
                let list_value = request_params_obj.get(list_param_name).unwrap();

                // Safe unwrap: parameter type already validated as list (array) at function start
                let list_array = list_value.as_array().unwrap();

                if list_array.is_empty() {
                    return Err(JankenError::new_parameter_type_mismatch(
                        "non-empty list",
                        "empty array",
                    ));
                }

                // Create positional placeholders and values
                // For list items, we infer the type from the JSON value itself
                let mut placeholders = Vec::new();
                for (i, item) in list_array.iter().enumerate() {
                    let param_key = format!("{list_param_name}_{i}");
                    placeholders.push(format!("@{param_key}")); // Keep @ for consistency

                    // Convert JSON value to generic ParameterValue with type inference
                    let generic_value = json_value_to_parameter_value_inferred(item)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_value_to_parameter_value_string() {
        let json_val = json!("hello world");
        let result = json_value_to_parameter_value(&json_val, &ParameterType::String).unwrap();
        assert_eq!(result, ParameterValue::String("hello world".to_string()));

        // Test invalid type
        let json_val = json!(123);
        let result = json_value_to_parameter_value(&json_val, &ParameterType::String);
        assert!(matches!(
            result,
            Err(JankenError::ParameterTypeMismatch { .. })
        ));
    }

    #[test]
    fn test_json_value_to_parameter_value_integer() {
        let json_val = json!(42);
        let result = json_value_to_parameter_value(&json_val, &ParameterType::Integer).unwrap();
        assert_eq!(result, ParameterValue::Integer(42));

        // Test invalid type
        let json_val = json!("not a number");
        let result = json_value_to_parameter_value(&json_val, &ParameterType::Integer);
        assert!(matches!(
            result,
            Err(JankenError::ParameterTypeMismatch { .. })
        ));
    }

    #[test]
    fn test_json_value_to_parameter_value_float() {
        let json_val = json!(3.15);
        let result = json_value_to_parameter_value(&json_val, &ParameterType::Float).unwrap();
        assert_eq!(result, ParameterValue::Float(3.15));

        // Test invalid type
        let json_val = json!("not a number");
        let result = json_value_to_parameter_value(&json_val, &ParameterType::Float);
        assert!(matches!(
            result,
            Err(JankenError::ParameterTypeMismatch { .. })
        ));
    }

    #[test]
    fn test_json_value_to_parameter_value_boolean() {
        let json_val = json!(true);
        let result = json_value_to_parameter_value(&json_val, &ParameterType::Boolean).unwrap();
        assert_eq!(result, ParameterValue::Boolean(true));

        // Test invalid type
        let json_val = json!(123);
        let result = json_value_to_parameter_value(&json_val, &ParameterType::Boolean);
        assert!(matches!(
            result,
            Err(JankenError::ParameterTypeMismatch { .. })
        ));
    }

    #[test]
    fn test_json_value_to_parameter_value_table_name() {
        let json_val = json!("users");
        let result = json_value_to_parameter_value(&json_val, &ParameterType::TableName).unwrap();
        assert_eq!(result, ParameterValue::String("users".to_string()));

        // Test invalid type
        let json_val = json!(123);
        let result = json_value_to_parameter_value(&json_val, &ParameterType::TableName);
        assert!(matches!(
            result,
            Err(JankenError::ParameterTypeMismatch { .. })
        ));
    }

    #[test]
    fn test_json_value_to_parameter_value_blob() {
        let json_val = json!([1, 2, 3, 255]);
        let result = json_value_to_parameter_value(&json_val, &ParameterType::Blob).unwrap();
        assert_eq!(result, ParameterValue::Blob(vec![1, 2, 3, 255]));

        // Test invalid type
        let json_val = json!("not an array");
        let result = json_value_to_parameter_value(&json_val, &ParameterType::Blob);
        assert!(matches!(
            result,
            Err(JankenError::ParameterTypeMismatch { .. })
        ));
    }

    #[test]
    fn test_json_value_to_parameter_value_list() {
        let json_val = json!([1, 2, 3]);
        let result = json_value_to_parameter_value(&json_val, &ParameterType::List);
        assert!(matches!(
            result,
            Err(JankenError::ParameterTypeMismatch { .. })
        ));
        match result {
            Err(JankenError::ParameterTypeMismatch { data }) => {
                let metadata: serde_json::Value =
                    serde_json::from_str(&data.metadata.unwrap_or("{}".to_string())).unwrap();
                let expected = metadata.get("expected").unwrap().as_str().unwrap();
                let got = metadata.get("got").unwrap().as_str().unwrap();
                assert_eq!(expected, "non-list parameter");
                assert_eq!(got, "list parameter (should be expanded)");
            }
            _ => panic!("Expected ParameterTypeMismatch error"),
        }
    }

    #[test]
    fn test_json_value_to_parameter_value_inferred() {
        // Test string
        let json_str = json!("hello");
        let result = json_value_to_parameter_value_inferred(&json_str).unwrap();
        assert_eq!(result, ParameterValue::String("hello".to_string()));

        // Test integer
        let json_int = json!(42);
        let result = json_value_to_parameter_value_inferred(&json_int).unwrap();
        assert_eq!(result, ParameterValue::Integer(42));

        // Test float
        let json_float = json!(3.15);
        let result = json_value_to_parameter_value_inferred(&json_float).unwrap();
        assert_eq!(result, ParameterValue::Float(3.15));

        // Test boolean
        let json_bool = json!(true);
        let result = json_value_to_parameter_value_inferred(&json_bool).unwrap();
        assert_eq!(result, ParameterValue::Boolean(true));

        // Test null - should now be rejected
        let json_null = json!(null);
        let result = json_value_to_parameter_value_inferred(&json_null);
        assert!(matches!(
            result,
            Err(JankenError::ParameterTypeMismatch { .. })
        ));

        // Test array (gets serialized as JSON string)
        let json_array = json!([1, 2, 3]);
        let result = json_value_to_parameter_value_inferred(&json_array).unwrap();
        assert_eq!(result, ParameterValue::String("[1,2,3]".to_string()));

        // Test object (gets serialized as JSON string)
        let json_obj = json!({"key": "value"});
        let result = json_value_to_parameter_value_inferred(&json_obj).unwrap();
        assert_eq!(
            result,
            ParameterValue::String("{\"key\":\"value\"}".to_string())
        );
    }

    #[test]
    fn test_prepare_parameter_statement_generic_basic_parameters() {
        let sql = "SELECT * FROM users WHERE id = @id AND name = @name";
        let parameters = vec![
            Parameter {
                name: "id".to_string(),
                param_type: ParameterType::Integer,
                constraints: ParameterConstraints::default(),
            },
            Parameter {
                name: "name".to_string(),
                param_type: ParameterType::String,
                constraints: ParameterConstraints::default(),
            },
        ];
        let request_params = json!({
            "id": 123,
            "name": "Alice"
        })
        .as_object()
        .unwrap()
        .clone();

        let result =
            prepare_parameter_statement_generic(sql, &parameters, &request_params).unwrap();

        assert_eq!(
            result.sql,
            "SELECT * FROM users WHERE id = @id AND name = @name"
        );
        assert_eq!(result.parameters.len(), 2);

        // Check that parameters are correctly converted
        let id_param = result
            .parameters
            .iter()
            .find(|(name, _)| name == "id")
            .unwrap();
        assert_eq!(id_param.1, ParameterValue::Integer(123));

        let name_param = result
            .parameters
            .iter()
            .find(|(name, _)| name == "name")
            .unwrap();
        assert_eq!(name_param.1, ParameterValue::String("Alice".to_string()));
    }

    #[test]
    fn test_prepare_parameter_statement_generic_table_name_parameters() {
        let sql = "SELECT * FROM #[table_name]";
        let parameters = vec![Parameter {
            name: "table_name".to_string(),
            param_type: ParameterType::TableName,
            constraints: ParameterConstraints::default(),
        }];
        let request_params = json!({"table_name": "users"}).as_object().unwrap().clone();

        let result =
            prepare_parameter_statement_generic(sql, &parameters, &request_params).unwrap();

        // Table name should be quoted and replaced
        assert_eq!(result.sql, "SELECT * FROM users");
        assert_eq!(result.parameters.len(), 0); // Table names don't create parameters
    }

    #[test]
    fn test_prepare_parameter_statement_generic_list_parameters() {
        let sql = "SELECT * FROM users WHERE id IN :[ids]";
        let parameters = vec![Parameter {
            name: "ids".to_string(),
            param_type: ParameterType::List,
            constraints: ParameterConstraints::default(),
        }];
        let request_params = json!({"ids": [1, 2, 3]}).as_object().unwrap().clone();

        let result =
            prepare_parameter_statement_generic(sql, &parameters, &request_params).unwrap();

        // List should be expanded to positional parameters
        assert_eq!(
            result.sql,
            "SELECT * FROM users WHERE id IN (@ids_0, @ids_1, @ids_2)"
        );
        assert_eq!(result.parameters.len(), 3);

        // Check parameter values
        assert_eq!(
            result.parameters[0],
            ("ids_0".to_string(), ParameterValue::Integer(1))
        );
        assert_eq!(
            result.parameters[1],
            ("ids_1".to_string(), ParameterValue::Integer(2))
        );
        assert_eq!(
            result.parameters[2],
            ("ids_2".to_string(), ParameterValue::Integer(3))
        );
    }

    #[test]
    fn test_prepare_parameter_statement_generic_mixed_parameter_types() {
        let sql = "SELECT * FROM #[table] WHERE id = @id AND active IN :[status]";
        let parameters = vec![
            Parameter {
                name: "table".to_string(),
                param_type: ParameterType::TableName,
                constraints: ParameterConstraints::default(),
            },
            Parameter {
                name: "id".to_string(),
                param_type: ParameterType::Integer,
                constraints: ParameterConstraints::default(),
            },
            Parameter {
                name: "status".to_string(),
                param_type: ParameterType::List,
                constraints: ParameterConstraints::default(),
            },
        ];
        let request_params = json!({
            "table": "users",
            "id": 123,
            "status": [true, false]
        })
        .as_object()
        .unwrap()
        .clone();

        let result =
            prepare_parameter_statement_generic(sql, &parameters, &request_params).unwrap();

        assert_eq!(
            result.sql,
            "SELECT * FROM users WHERE id = @id AND active IN (@status_0, @status_1)"
        );
        assert_eq!(result.parameters.len(), 3);

        // Check parameters
        assert_eq!(
            result.parameters[0],
            ("id".to_string(), ParameterValue::Integer(123))
        );
        assert_eq!(
            result.parameters[1],
            ("status_0".to_string(), ParameterValue::Boolean(true))
        );
        assert_eq!(
            result.parameters[2],
            ("status_1".to_string(), ParameterValue::Boolean(false))
        );
    }

    #[test]
    fn test_prepare_parameter_statement_generic_missing_parameter() {
        let sql = "SELECT * FROM users WHERE id = @id";
        let parameters = vec![Parameter {
            name: "id".to_string(),
            param_type: ParameterType::Integer,
            constraints: ParameterConstraints::default(),
        }];
        let request_params = json!({"name": "Alice"}).as_object().unwrap().clone(); // Missing 'id'

        let result = prepare_parameter_statement_generic(sql, &parameters, &request_params);
        assert!(
            matches!(result, Err(JankenError::ParameterNotProvided { data }) if {
                let metadata: serde_json::Value = serde_json::from_str(&data.metadata.clone().unwrap_or("{}".to_string())).unwrap();
                metadata.get("parameter_name").unwrap().as_str().unwrap() == "id"
            })
        );
    }

    #[test]
    fn test_prepare_parameter_statement_generic_empty_list_error() {
        let sql = "SELECT * FROM users WHERE id IN :[ids]";
        let parameters = vec![Parameter {
            name: "ids".to_string(),
            param_type: ParameterType::List,
            constraints: ParameterConstraints::default(),
        }];
        let request_params = json!({"ids": []}).as_object().unwrap().clone();

        let result = prepare_parameter_statement_generic(sql, &parameters, &request_params);
        assert!(matches!(
            result,
            Err(JankenError::ParameterTypeMismatch { .. })
        ));
    }

    #[test]
    fn test_prepare_parameter_statement_generic_list_parameter_type_error() {
        let sql = "SELECT * FROM users WHERE id IN :[ids]";
        let parameters = vec![Parameter {
            name: "ids".to_string(),
            param_type: ParameterType::List,
            constraints: ParameterConstraints::default(),
        }];
        let request_params = json!({"ids": "not an array"}).as_object().unwrap().clone();

        let result = prepare_parameter_statement_generic(sql, &parameters, &request_params);
        assert!(matches!(
            result,
            Err(JankenError::ParameterTypeMismatch { .. })
        ));
    }
}
