use crate::{
    ParameterType, QueryDefinitions, parameters,
    result::{JankenError, QueryResult, Result},
    str_utils::{self, quote_identifier},
};
use rusqlite::Connection;

/// Type alias for parameter preparation result to reduce type complexity
/// SQLite-specific due to rusqlite::ToSql trait
type PreparedParametersResult = (String, Vec<(String, Box<dyn rusqlite::ToSql>)>);

/// Convert a serde_json Value to a rusqlite ToSql type for list items
/// This is SQLite-specific due to rusqlite::ToSql trait
fn json_value_to_sql(value: &serde_json::Value) -> Box<dyn rusqlite::ToSql> {
    match value {
        serde_json::Value::String(s) => Box::new(s.clone()),
        serde_json::Value::Number(n) => {
            if n.is_i64() {
                Box::new(n.as_i64().unwrap())
            } else {
                Box::new(n.as_f64().unwrap())
            }
        }
        serde_json::Value::Bool(b) => Box::new(*b as i32),
        serde_json::Value::Null => Box::new(rusqlite::types::Value::Null),
        serde_json::Value::Array(a) => Box::new(format!("{a:?}")),
        serde_json::Value::Object(o) => Box::new(format!("{o:?}")),
    }
}

/// Convert a JSON value to rusqlite ToSql based on parameter type definition
/// This is SQLite-specific due to rusqlite::ToSql trait
fn parameter_value_to_sql(
    param_value: &serde_json::Value,
    param_type: &ParameterType,
) -> Box<dyn rusqlite::ToSql> {
    match param_type {
        ParameterType::String => Box::new(param_value.as_str().unwrap().to_string()),
        ParameterType::Integer => Box::new(param_value.as_i64().unwrap()),
        ParameterType::Float => Box::new(param_value.as_f64().unwrap()),
        ParameterType::Boolean => Box::new(param_value.as_bool().unwrap() as i32),
        ParameterType::TableName => Box::new(param_value.as_str().unwrap().to_string()),
        ParameterType::List => {
            // List parameters are handled separately in list expansion
            Box::new(String::new()) // Placeholder
        }
        ParameterType::Blob => {
            // Convert array of byte values to Vec<u8>
            let bytes: Vec<u8> = param_value
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            Box::new(bytes)
        }
    }
}

/// Result of preparing a single SQL statement with parameter conversion
/// SQLite-specific due to rusqlite named parameter format
struct PreparedStatement {
    /// The SQL with placeholders ready for execution
    sql: String,
    /// Named parameters ready for rusqlite execution: (:name, value)
    named_params: Vec<(String, Box<dyn rusqlite::ToSql>)>,
}

/// Method to convert parameters to Rusqlite named params format
/// SQLite-specific due to rusqlite trait
impl PreparedStatement {
    /// Get parameters in rusqlite named parameter format
    fn as_named_params(&self) -> Vec<(&str, &dyn rusqlite::ToSql)> {
        self.named_params
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_ref()))
            .collect()
    }
}

/// Replace @parameters with :parameters and collect their values as rusqlite types (excluding table names)
/// SQLite-specific due to rusqlite::ToSql trait usage
fn prepare_statement_parameters(
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<PreparedParametersResult> {
    let statement_param_names =
        parameters::extract_parameters_with_regex(statement_sql, &parameters::PARAMETER_REGEX);
    let mut prepared_sql = statement_sql.to_string();
    let mut named_params = Vec::new();

    for param_name in &statement_param_names {
        prepared_sql = prepared_sql.replace(&format!("@{param_name}"), &format!(":{param_name}"));
        // Get the parameter value from request params
        let param_value = request_params_obj
            .get(param_name)
            .ok_or_else(|| JankenError::ParameterNotProvided(param_name.clone()))?;

        // Find the parameter definition for type validation
        let param_def = all_parameters
            .iter()
            .find(|p| p.name == *param_name)
            .ok_or_else(|| JankenError::ParameterNotProvided(param_name.clone()))?;

        // Convert JSON value to rusqlite::ToSql based on parameter type (validation already done upstream)
        let to_sql: Box<dyn rusqlite::ToSql> =
            parameter_value_to_sql(param_value, &param_def.param_type);
        named_params.push((format!(":{param_name}"), to_sql));
    }

    Ok((prepared_sql, named_params))
}

/// Create a prepared statement from SQL with proper parameter replacement
/// This is mostly DB-independent logic, but relies on SQLite-specific parameter preparation
/// Returns the prepared statement with SQL and named parameters for execution
fn prepare_single_statement(
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<PreparedStatement> {
    // Validate parameters first to ensure consistency and prevent SQL injection
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

    let (mut prepared_sql, mut named_params) =
        prepare_statement_parameters(statement_sql, all_parameters, request_params_obj)?;

    // Replace #table_name parameters with direct table name values from request_params_obj
    for cap in parameters::TABLE_NAME_REGEX.captures_iter(&prepared_sql.clone()) {
        if let Some(param_name_match) = cap.get(1) {
            let param_name = param_name_match.as_str();

            // Parameters are already validated at the beginning, so this will always succeed
            let table_name_value = request_params_obj.get(param_name).unwrap();
            let table_name_str = table_name_value.as_str().unwrap();
            // Validate as identifier
            let valid_ident = quote_identifier(table_name_str);
            prepared_sql = parameters::TABLE_NAME_REGEX
                .replace(&prepared_sql, valid_ident)
                .to_string();
        }
    }

    // Replace :[list] parameters with expanded named parameters (:list_0, :list_1, etc.)
    for cap in parameters::LIST_PARAMETER_REGEX.captures_iter(&prepared_sql.clone()) {
        if let Some(_param_match) = cap.get(0) {
            if let Some(param_name_match) = cap.get(1) {
                let list_param_name = param_name_match.as_str();

                // Get the list parameter value from request params
                let list_value = request_params_obj.get(list_param_name).unwrap();
                let list_array = list_value.as_array().unwrap();

                if list_array.is_empty() {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: "non-empty list".to_string(),
                        got: "empty array".to_string(),
                    });
                }

                // Create named placeholders and values
                // Since ParameterConstraints::validate already validates the array type upstream,
                // we can safely unwrap values here to avoid redundant type checking calculations
                let mut placeholders = Vec::new();
                for (i, item) in list_array.iter().enumerate() {
                    let param_key = format!(":{list_param_name}_{i}");
                    placeholders.push(param_key.clone());

                    let to_sql: Box<dyn rusqlite::ToSql> = json_value_to_sql(item);
                    named_params.push((param_key, to_sql));
                }

                // Replace the :[param] with (:param_0, :param_1, ...)
                let placeholder_str = placeholders.join(", ");
                prepared_sql = parameters::LIST_PARAMETER_REGEX
                    .replace(&prepared_sql, format!("({placeholder_str})"))
                    .to_string();
            }
        }
    }

    Ok(PreparedStatement {
        sql: prepared_sql,
        named_params,
    })
}

/// Execute a single SQL statement with its appropriate parameters
/// SQLite-specific due to rusqlite::Transaction
fn execute_single_statement(
    tx: &rusqlite::Transaction,
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<String> {
    let prepared = prepare_single_statement(statement_sql, all_parameters, request_params_obj)?;
    let named_params = prepared.as_named_params();

    // Now execute with the named parameter values
    let mut stmt = tx.prepare(&prepared.sql)?;
    stmt.execute(&named_params[..])
        .map_err(JankenError::Sqlite)?;
    Ok(prepared.sql)
}

/// Execute mutation query (INSERT/UPDATE/DELETE/etc.) - split and execute within transaction
/// This logic is mostly DB-independent but uses SQLite-specific transaction
fn execute_mutation_query(
    query: &crate::query::QueryDef,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    tx: &rusqlite::Transaction,
) -> Result<Vec<String>> {
    let mut sql_statements = Vec::new();
    if query.sql.contains(';') {
        // Has parameters - split into individual statements and execute each one
        let individual_statements = str_utils::split_sql_statements(&query.sql);

        for statement_sql in individual_statements {
            // Execute each statement with the appropriate parameters
            let sql = execute_single_statement(
                tx,
                &statement_sql,
                &query.parameters,
                request_params_obj,
            )?;
            sql_statements.push(sql);
        }
    } else {
        // Single-statement mutation - prepare and execute normally with all parameters
        let prepared = prepare_single_statement(&query.sql, &query.parameters, request_params_obj)?;
        let named_params = prepared.as_named_params();
        let mut stmt = tx.prepare(&prepared.sql)?;
        stmt.execute(&named_params[..])
            .map_err(JankenError::Sqlite)?;
        sql_statements.push(prepared.sql);
    }

    Ok(sql_statements)
}

/// Execute query with both read and mutation operations within a unified transaction
/// This logic is mostly DB-independent but uses SQLite-specific transaction
pub fn execute_query_unified(
    query: &crate::query::QueryDef,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    tx: &rusqlite::Transaction,
) -> Result<QueryResult> {
    if !query.returns.is_empty() {
        // Query with returns specified - return structured data
        let prepared = prepare_single_statement(&query.sql, &query.parameters, request_params_obj)?;
        let mut stmt = tx.prepare(&prepared.sql)?;
        let named_params = prepared.as_named_params();
        let rows = stmt.query_map(&named_params[..], |row| {
            let mut obj = serde_json::Map::new();
            for (idx, field_name) in query.returns.iter().enumerate() {
                let value: rusqlite::Result<serde_json::Value> = match row.get_ref(idx) {
                    Ok(rusqlite::types::ValueRef::Integer(i)) => {
                        Ok(serde_json::Value::Number(i.into()))
                    }
                    Ok(rusqlite::types::ValueRef::Real(r)) => Ok(serde_json::Value::from(r)),
                    Ok(rusqlite::types::ValueRef::Text(s)) => Ok(serde_json::Value::String(
                        String::from_utf8_lossy(s).to_string(),
                    )),
                    Ok(rusqlite::types::ValueRef::Blob(b)) => Ok(serde_json::Value::Array(
                        b.iter()
                            .map(|&byte| serde_json::Value::Number(byte.into()))
                            .collect(),
                    )),
                    Ok(rusqlite::types::ValueRef::Null) => Ok(serde_json::Value::Null),
                    Err(e) => Err(e),
                };
                match value {
                    Ok(val) => {
                        obj.insert(field_name.clone(), val);
                    }
                    Err(_) => {
                        obj.insert(field_name.clone(), serde_json::Value::Null);
                    }
                }
            }
            Ok(serde_json::Value::Object(obj))
        })?;
        let result = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(QueryResult {
            sql_statements: vec![prepared.sql],
            data: result,
        })
    } else {
        // Mutation query (INSERT/UPDATE/DELETE/etc.) - split and execute within transaction
        let sql_statements = execute_mutation_query(query, request_params_obj, tx)?;
        Ok(QueryResult {
            sql_statements,
            data: vec![],
        })
    }
}

/// Execute queries with SQLite backend
/// This is the main entry point for SQLite operations
pub fn query_run_sqlite(
    conn: &mut Connection,
    queries: &QueryDefinitions,
    query_name: &str,
    request_params: &serde_json::Value,
) -> Result<QueryResult> {
    let query = queries
        .definitions
        .get(query_name)
        .ok_or_else(|| JankenError::QueryNotFound(query_name.to_string()))?;

    let request_params_obj =
        request_params
            .as_object()
            .ok_or_else(|| JankenError::ParameterTypeMismatch {
                expected: "object".to_string(),
                got: "not object".to_string(),
            })?;

    let tx = conn.transaction().map_err(JankenError::Sqlite)?;

    // Handle all queries uniformly within transactions
    let query_result = execute_query_unified(query, request_params_obj, &tx)?;

    // Always commit the transaction (for both single and multi-statement queries)
    tx.commit().map_err(JankenError::Sqlite)?;
    Ok(query_result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_value_to_sql() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();

        // Test String
        let string_result = json_value_to_sql(&serde_json::Value::String("hello".to_string()));
        let val: String = conn
            .query_row("SELECT ?", [string_result.as_ref()], |r| r.get(0))
            .unwrap();
        assert_eq!(val, "hello");

        // Test Integer (i64)
        let int_result =
            json_value_to_sql(&serde_json::Value::Number(serde_json::Number::from(42)));
        let val: i64 = conn
            .query_row("SELECT ?", [int_result.as_ref()], |r| r.get(0))
            .unwrap();
        assert_eq!(val, 42);

        // Test Float (f64)
        let float_result = json_value_to_sql(&serde_json::Value::Number(
            serde_json::Number::from_f64(1.23).unwrap(),
        ));
        let val: f64 = conn
            .query_row("SELECT ?", [float_result.as_ref()], |r| r.get(0))
            .unwrap();
        assert!(val > 1.22 && val < 1.24);

        // Test Boolean (true -> 1)
        let bool_result = json_value_to_sql(&serde_json::Value::Bool(true));
        let val: i32 = conn
            .query_row("SELECT ?", [bool_result.as_ref()], |r| r.get(0))
            .unwrap();
        assert_eq!(val, 1);

        // Test Boolean (false -> 0)
        let bool_result = json_value_to_sql(&serde_json::Value::Bool(false));
        let val: i32 = conn
            .query_row("SELECT ?", [bool_result.as_ref()], |r| r.get(0))
            .unwrap();
        assert_eq!(val, 0);

        // Test Null
        let null_result = json_value_to_sql(&serde_json::Value::Null);
        let is_null: i32 = conn
            .query_row("SELECT ? IS NULL", [null_result.as_ref()], |r| r.get(0))
            .unwrap();
        assert_eq!(is_null, 1);

        // Test Array (string representation)
        let array_result = json_value_to_sql(&serde_json::Value::Array(vec![
            serde_json::Value::Number(1.into()),
            serde_json::Value::String("test".to_string()),
        ]));
        let val: String = conn
            .query_row("SELECT ?", [array_result.as_ref()], |r| r.get(0))
            .unwrap();
        assert_eq!(val, "[Number(1), String(\"test\")]");

        // Test Object (string representation)
        let mut obj = serde_json::Map::new();
        obj.insert(
            "key".to_string(),
            serde_json::Value::String("value".to_string()),
        );
        let obj_result = json_value_to_sql(&serde_json::Value::Object(obj));
        let val: String = conn
            .query_row("SELECT ?", [obj_result.as_ref()], |r| r.get(0))
            .unwrap();
        assert_eq!(val, "{\"key\": String(\"value\")}");
    }

    #[test]
    fn test_prepare_statement_parameters_edge_cases() {
        // Test edge cases to cover parameter type conversion functions
        // These represent "practically impossible" cases but needed for 100% test coverage

        // Test parameter_value_to_sql with actual SQL execution
        let conn = rusqlite::Connection::open_in_memory().unwrap();

        // Test List type conversion (should return empty string placeholder)
        let list_result = parameter_value_to_sql(
            &serde_json::Value::Array(vec![serde_json::Value::String("test".to_string())]),
            &ParameterType::List,
        );
        let list_count: i32 = conn
            .query_row("SELECT ? = ''", [list_result.as_ref()], |row| row.get(0))
            .unwrap();
        assert_eq!(
            list_count, 1,
            "List type should return empty string placeholder"
        );

        // Test TableName type conversion (should preserve the string)
        let table_result = parameter_value_to_sql(
            &serde_json::Value::String("my_table".to_string()),
            &ParameterType::TableName,
        );
        let table_count: i32 = conn
            .query_row("SELECT ? = 'my_table'", [table_result.as_ref()], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(
            table_count, 1,
            "TableName type should preserve the string value"
        );
    }
}
