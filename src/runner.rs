use crate::{
    ParameterType, QueryDefinitions,
    result::{JankenError, Result},
    str_utils,
};
use regex::Regex;
use rusqlite::Connection;

/// Regex for table name parameters (#table_name syntax)
static TABLE_NAME_REGEX: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"#(\w+)").unwrap());

/// Validate table name format (alphanumeric and underscores only)
fn is_valid_table_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Quote an identifier properly for SQL (for table names)
fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace("\"", "\"\""))
}

/// Trait for executing parameterized SQL queries against different database backends
pub trait QueryRunner {
    fn query_run(
        &mut self,
        queries: &QueryDefinitions,
        query_name: &str,
        params: &serde_json::Value,
    ) -> Result<Vec<serde_json::Value>>;
}

pub fn query_run_sqlite(
    conn: &mut Connection,
    queries: &QueryDefinitions,
    query_name: &str,
    request_params: &serde_json::Value,
) -> Result<Vec<serde_json::Value>> {
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
    let result = execute_query_unified(query, request_params_obj, &tx)?;

    // Always commit the transaction (for both single and multi-statement queries)
    tx.commit().map_err(JankenError::Sqlite)?;
    Ok(result)
}

pub fn execute_query_unified(
    query: &crate::query::QueryDef,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    tx: &rusqlite::Transaction,
) -> Result<Vec<serde_json::Value>> {
    if !query.returns.is_empty() {
        // Query with returns specified - return structured data
        let (prepared_sql, _) =
            prepare_single_statement(&query.sql, &query.parameters, request_params_obj, &|idx| {
                format!("?{idx}")
            })?;
        let mut stmt = tx.prepare(&prepared_sql)?;
        let request_param_values = convert_params_to_sqlite(request_params_obj, &query.parameters)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(&request_param_values), |row| {
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
        Ok(result)
    } else {
        // Mutation query (INSERT/UPDATE/DELETE/etc.) - split and execute within transaction
        execute_mutation_query(query, request_params_obj, tx)?;
        Ok(vec![])
    }
}

fn execute_mutation_query(
    query: &crate::query::QueryDef,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    tx: &rusqlite::Transaction,
) -> Result<()> {
    if query.sql.contains(';') {
        if !query.sql.contains('@') {
            // No parameters - can use execute_batch() for efficiency
            tx.execute_batch(&query.sql)?;
        } else {
            // Has parameters - split into individual statements and execute each one
            let individual_statements = str_utils::split_sql_statements(&query.sql);

            for statement_sql in individual_statements {
                // Execute each statement with the appropriate parameters
                execute_single_statement(
                    tx,
                    &statement_sql,
                    &query.parameters,
                    request_params_obj,
                )?;
            }
        }
    } else {
        // Single-statement mutation - prepare and execute normally with all parameters
        let request_param_values = convert_params_to_sqlite(request_params_obj, &query.parameters)?;
        let (prepared_sql, _) =
            prepare_single_statement(&query.sql, &query.parameters, request_params_obj, &|idx| {
                format!("?{idx}")
            })?;
        let mut stmt = tx.prepare(&prepared_sql)?;
        stmt.execute(rusqlite::params_from_iter(&request_param_values))
            .map_err(JankenError::Sqlite)?;
    }

    Ok(())
}

/// Convert request parameters map to Box<dyn rusqlite::ToSql> vector for SQLite execution
/// This is called just before query execution to minimize time spent in rigid rusqlite::ToSql format
fn convert_params_to_sqlite(
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    param_defs: &[crate::parameters::Parameter],
) -> Result<Vec<Box<dyn rusqlite::ToSql>>> {
    // Check and collect param values in the SAME order as they appear in the query definition
    // This ensures parameter positions match the prepared statement's placeholders
    let mut request_param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    for param_def in param_defs {
        let value = request_params_obj
            .get(&param_def.name)
            .ok_or_else(|| JankenError::ParameterNotProvided(param_def.name.clone()))?;

        // Validate parameter constraints
        param_def
            .constraints
            .validate(value, &param_def.param_type)?;

        match &param_def.param_type {
            ParameterType::String => {
                let str_val = value
                    .as_str()
                    .ok_or_else(|| JankenError::ParameterTypeMismatch {
                        expected: "string".to_string(),
                        got: value.to_string(),
                    })?;
                request_param_values.push(Box::new(str_val.to_string()));
            }
            ParameterType::Integer => {
                let int_val = value
                    .as_i64()
                    .ok_or_else(|| JankenError::ParameterTypeMismatch {
                        expected: "integer".to_string(),
                        got: value.to_string(),
                    })?;
                request_param_values.push(Box::new(int_val));
            }
            ParameterType::Float => {
                let float_val =
                    value
                        .as_f64()
                        .ok_or_else(|| JankenError::ParameterTypeMismatch {
                            expected: "float".to_string(),
                            got: value.to_string(),
                        })?;
                request_param_values.push(Box::new(float_val));
            }
            ParameterType::Boolean => {
                let bool_val =
                    value
                        .as_bool()
                        .ok_or_else(|| JankenError::ParameterTypeMismatch {
                            expected: "boolean".to_string(),
                            got: value.to_string(),
                        })?;
                // Convert boolean to integer for SQLite (true=1, false=0)
                let int_val = if bool_val { 1 } else { 0 };
                request_param_values.push(Box::new(int_val));
            }
            ParameterType::TableName => {
                let table_name =
                    value
                        .as_str()
                        .ok_or_else(|| JankenError::ParameterTypeMismatch {
                            expected: "string (table_name)".to_string(),
                            got: value.to_string(),
                        })?;
                // Validate table name format
                if !is_valid_table_name(table_name) {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: "valid table name (alphanumeric and underscores only)"
                            .to_string(),
                        got: table_name.to_string(),
                    });
                }
                request_param_values.push(Box::new(table_name.to_string()));
            }
        }
    }
    Ok(request_param_values)
}

/// Create a prepared statement from SQL with proper parameter replacement
/// Returns the prepared SQL and the parameter indices that should be used
fn prepare_single_statement(
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    placeholder_gen: &dyn Fn(usize) -> String,
) -> Result<(String, Vec<usize>)> {
    let statement_param_names = str_utils::extract_parameters_in_statement(statement_sql);
    let mut prepared_sql = statement_sql.to_string();
    let mut param_positions = Vec::new();
    let mut next_placeholder_idx = 1;

    // Replace @parameters first
    for param_name in statement_param_names {
        if let Some(global_index) = all_parameters
            .iter()
            .position(|global_param| global_param.name == param_name)
        {
            param_positions.push(global_index);
            prepared_sql = prepared_sql.replace(
                &format!("@{param_name}"),
                &placeholder_gen(next_placeholder_idx),
            );
            next_placeholder_idx += 1;
        }
    }

    // Replace #table_name parameters with direct table name values from request_params_obj
    for cap in TABLE_NAME_REGEX.captures_iter(&prepared_sql.clone()) {
        if let Some(param_name_match) = cap.get(1) {
            let param_name = param_name_match.as_str();

            // Get the table name value from request parameters
            if let Some(table_name_value) = request_params_obj.get(param_name) {
                if let Some(table_name_str) = table_name_value.as_str() {
                    // Validate as identifier
                    let valid_ident = quote_identifier(table_name_str);
                    prepared_sql = TABLE_NAME_REGEX
                        .replace(&prepared_sql, valid_ident)
                        .to_string();
                } else {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: "string (table_name)".to_string(),
                        got: table_name_value.to_string(),
                    });
                }
            } else {
                return Err(JankenError::ParameterNotProvided(param_name.to_string()));
            }
        }
    }

    Ok((prepared_sql, param_positions))
}

/// Execute a single SQL statement with its appropriate parameters
fn execute_single_statement(
    tx: &rusqlite::Transaction,
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let request_param_values = convert_params_to_sqlite(request_params_obj, all_parameters)?;
    let (placeholder_sql, param_positions) =
        prepare_single_statement(statement_sql, all_parameters, request_params_obj, &|idx| {
            format!("?{idx}")
        })?;

    // Collect the parameter values for this statement in the correct order
    let statement_params: Vec<&Box<dyn rusqlite::ToSql>> = param_positions
        .iter()
        .map(|&idx| &request_param_values[idx])
        .collect();

    // Now execute with the correct parameter values for this statement
    let mut stmt = tx.prepare(&placeholder_sql)?;
    stmt.execute(rusqlite::params_from_iter(statement_params))
        .map_err(JankenError::Sqlite)?;
    Ok(())
}
