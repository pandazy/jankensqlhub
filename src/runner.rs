use crate::{
    ParameterType, QueryDefinitions,
    result::{JankenError, Result},
    str_utils,
};
use rusqlite::Connection;

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

    // Check and collect param values in the SAME order as they appear in the query definition
    // This ensures parameter positions match the prepared statement's placeholders
    let mut request_param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    for param_def in &query.parameters {
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
        }
    }

    let tx = conn.transaction().map_err(JankenError::Sqlite)?;

    // Handle all queries uniformly within transactions
    let result = execute_query_unified(query, &request_param_values, &tx)?;

    // Always commit the transaction (for both single and multi-statement queries)
    tx.commit().map_err(JankenError::Sqlite)?;
    Ok(result)
}

pub fn execute_query_unified(
    query: &crate::query::QueryDef,
    params: &[Box<dyn rusqlite::ToSql>],
    tx: &rusqlite::Transaction,
) -> Result<Vec<serde_json::Value>> {
    if query.sql.to_lowercase().starts_with("select") && !query.sql.contains(';') {
        // SELECT query with returns specified - return structured data
        if !query.returns.is_empty() {
            let prepared_sql = prepare_statement_for_query(query, &|idx| format!("?{idx}"))?;
            let mut stmt = tx.prepare(&prepared_sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
                let mut obj = serde_json::Map::new();
                for (idx, field_name) in query.returns.iter().enumerate() {
                    let value: rusqlite::Result<serde_json::Value> = match row.get_ref(idx) {
                        Ok(rusqlite::types::ValueRef::Integer(i)) => {
                            Ok(serde_json::Value::Number(i.into()))
                        }
                        Ok(rusqlite::types::ValueRef::Real(r)) => {
                            if let Some(num) = serde_json::Number::from_f64(r) {
                                Ok(serde_json::Value::Number(num))
                            } else {
                                Ok(serde_json::Value::Null)
                            }
                        }
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
            // Legacy behavior: SELECT query - return array of first column as strings
            let prepared_sql = prepare_statement_for_query(query, &|idx| format!("?{idx}"))?;
            let mut stmt = tx.prepare(&prepared_sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
                let id: i64 = row.get(0)?;
                Ok(serde_json::Value::String(id.to_string()))
            })?;
            let result = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(result)
        }
    } else {
        // Any mutation query (INSERT/UPDATE/DELETE/etc.) - split and execute within transaction
        execute_mutation_query(query, params, tx)?;
        Ok(vec![])
    }
}

/// Prepare a statement for an entire query (single or multi-statement)
/// Returns the prepared SQL that can be executed
fn prepare_statement_for_query(
    query: &crate::query::QueryDef,
    placeholder_gen: &dyn Fn(usize) -> String,
) -> Result<String> {
    if query.sql.contains(';') {
        // Multi-statement: split and prepare each statement, then join them
        let individual_statements = str_utils::split_sql_statements(&query.sql);
        let mut prepared_statements = Vec::new();

        for statement_sql in individual_statements {
            let (prepared_sql, _) =
                prepare_single_statement(&statement_sql, &query.parameters, placeholder_gen)?;
            prepared_statements.push(prepared_sql);
        }

        Ok(prepared_statements.join("; "))
    } else {
        // Single statement: prepare directly
        let (prepared_sql, _) =
            prepare_single_statement(&query.sql, &query.parameters, placeholder_gen)?;
        Ok(prepared_sql)
    }
}

fn execute_mutation_query(
    query: &crate::query::QueryDef,
    params: &[Box<dyn rusqlite::ToSql>],
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
                execute_single_statement(tx, &statement_sql, &query.parameters, params)?;
            }
        }
    } else {
        // Single-statement mutation - prepare and execute normally with all parameters
        let prepared_sql = prepare_statement_for_query(query, &|idx| format!("?{idx}"))?;
        let mut stmt = tx.prepare(&prepared_sql)?;
        stmt.execute(rusqlite::params_from_iter(params))
            .map_err(JankenError::Sqlite)?;
    }

    Ok(())
}

/// Create a prepared statement from SQL with proper parameter replacement
/// Returns the prepared SQL and the parameter indices that should be used
fn prepare_single_statement(
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    placeholder_gen: &dyn Fn(usize) -> String,
) -> Result<(String, Vec<usize>)> {
    let statement_param_names = str_utils::extract_parameters_in_statement(statement_sql);
    let mut placeholder_sql = statement_sql.to_string();
    let mut param_positions = Vec::new();
    let mut next_placeholder_idx = 1;

    // Replace parameters in order and track their positions in the global parameter list
    for param_name in statement_param_names {
        // Find this parameter in the global parameter list
        if let Some(global_index) = all_parameters
            .iter()
            .position(|global_param| global_param.name == param_name)
        {
            param_positions.push(global_index);
            // Replace @param_name with placeholder format (e.g., ?1, $1, ?)
            placeholder_sql = placeholder_sql.replace(
                &format!("@{param_name}"),
                &placeholder_gen(next_placeholder_idx),
            );
            next_placeholder_idx += 1;
        }
    }

    Ok((placeholder_sql, param_positions))
}

/// Execute a single SQL statement with its appropriate parameters
fn execute_single_statement(
    tx: &rusqlite::Transaction,
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    all_param_values: &[Box<dyn rusqlite::ToSql>],
) -> Result<()> {
    let (placeholder_sql, param_positions) =
        prepare_single_statement(statement_sql, all_parameters, &|idx| format!("?{idx}"))?;

    // Collect the parameter values for this statement in the correct order
    let statement_params: Vec<&Box<dyn rusqlite::ToSql>> = param_positions
        .iter()
        .map(|&idx| &all_param_values[idx])
        .collect();

    // Now execute with the correct parameter values for this statement
    let mut stmt = tx.prepare(&placeholder_sql)?;
    stmt.execute(rusqlite::params_from_iter(statement_params))
        .map_err(JankenError::Sqlite)?;
    Ok(())
}
