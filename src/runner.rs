use crate::{
    QueryDefinitions,
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
            crate::parameters::ParameterType::Integer => {
                let int_val = value
                    .as_i64()
                    .ok_or_else(|| JankenError::ParameterTypeMismatch {
                        expected: "integer".to_string(),
                        got: value.to_string(),
                    })?;
                request_param_values.push(Box::new(int_val));
            }
            crate::parameters::ParameterType::String => {
                let str_val = value
                    .as_str()
                    .ok_or_else(|| JankenError::ParameterTypeMismatch {
                        expected: "string".to_string(),
                        got: value.to_string(),
                    })?;
                request_param_values.push(Box::new(str_val.to_string()));
            }
            crate::parameters::ParameterType::Float => {
                let float_val =
                    value
                        .as_f64()
                        .ok_or_else(|| JankenError::ParameterTypeMismatch {
                            expected: "float".to_string(),
                            got: value.to_string(),
                        })?;
                request_param_values.push(Box::new(float_val));
            }
            crate::parameters::ParameterType::Boolean => {
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
        // SELECT query - prepare and execute, return data
        let mut stmt = tx.prepare(&query.sqlite_prepared)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            let id: i64 = row.get(0)?;
            Ok(serde_json::Value::String(id.to_string()))
        })?;
        let result = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(result)
    } else {
        // Any mutation query (INSERT/UPDATE/DELETE/etc.) - split and execute within transaction
        execute_mutation_query(query, &query.sqlite_prepared, params, tx)?;
        Ok(vec![])
    }
}

fn execute_mutation_query(
    query: &crate::query::QueryDef,
    prepared_sql: &str,
    params: &[Box<dyn rusqlite::ToSql>],
    tx: &rusqlite::Transaction,
) -> Result<()> {
    if prepared_sql.contains(';') {
        if !prepared_sql.contains('@') {
            // No parameters - can use execute_batch() for efficiency
            tx.execute_batch(prepared_sql)?;
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
        let mut stmt = tx.prepare(prepared_sql)?;
        stmt.execute(rusqlite::params_from_iter(params))
            .map_err(JankenError::Sqlite)?;
    }

    Ok(())
}

/// Execute a single SQL statement with its appropriate parameters
fn execute_single_statement(
    tx: &rusqlite::Transaction,
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    all_param_values: &[Box<dyn rusqlite::ToSql>],
) -> Result<()> {
    let statement_param_names = str_utils::extract_parameters_in_statement(statement_sql);
    let mut statement_params: Vec<&Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut placeholder_sql = statement_sql.to_string();

    // Create placeholders for this statement's parameters in order
    for param_name in statement_param_names {
        // Find this parameter in the global parameter list and get its value
        if let Some(global_index) = all_parameters
            .iter()
            .position(|global_param| global_param.name == param_name)
        {
            statement_params.push(&all_param_values[global_index]);
            // Replace @param_name with ? placeholder
            placeholder_sql = placeholder_sql.replace(&format!("@{param_name}"), "?");
        }
    }

    // Now execute with the correct parameter values for this statement
    let mut stmt = tx.prepare(&placeholder_sql)?;
    stmt.execute(rusqlite::params_from_iter(statement_params))
        .map_err(JankenError::Sqlite)?;
    Ok(())
}
