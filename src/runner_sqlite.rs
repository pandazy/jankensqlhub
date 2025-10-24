use crate::{
    QueryDefinitions, parameters,
    result::{JankenError, QueryResult, Result},
    str_utils::split_sql_statements,
};

// Import generic types for parameter decoupling
use parameters::ParameterValue;
use rusqlite::Connection;

// Implement trait for converting generic ParameterValue to SQLite-specific ToSql
impl From<ParameterValue> for Box<dyn rusqlite::ToSql> {
    fn from(param_value: ParameterValue) -> Self {
        match param_value {
            ParameterValue::String(s) => Box::new(s),
            ParameterValue::Integer(i) => Box::new(i),
            ParameterValue::Float(f) => Box::new(f),
            ParameterValue::Boolean(b) => Box::new(b as i32), // SQLite represents booleans as integers
            ParameterValue::Blob(bytes) => Box::new(bytes),
            ParameterValue::Null => Box::new(rusqlite::types::Value::Null),
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

/// Create a prepared statement from SQL using the generic parameter decoupling approach
/// This separates parameter analysis (generic) from database-specific conversions (SQLite-specific)
fn prepare_single_statement_sqlite(
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<PreparedStatement> {
    // Use the generic parameter preparation (database-agnostic)
    let mut generic_statement = parameters::prepare_parameter_statement_generic(
        statement_sql,
        all_parameters,
        request_params_obj,
    )?;

    // Convert @param placeholders to :param for SQLite
    for param_name in &parameters::extract_parameters_with_regex(
        &generic_statement.sql,
        &parameters::PARAMETER_REGEX,
    ) {
        generic_statement.sql = generic_statement
            .sql
            .replace(&format!("@{param_name}"), &format!(":{param_name}"));
    }

    // Convert generic parameters to SQLite-specific ToSql types
    let sqlite_params = generic_statement
        .parameters
        .into_iter()
        .map(|(name, param_value)| {
            // Parameter names from generic function are already converted to SQLite format
            let sqlite_name = format!(":{name}");
            let to_sql: Box<dyn rusqlite::ToSql> = param_value.into();
            (sqlite_name, to_sql)
        })
        .collect();

    Ok(PreparedStatement {
        sql: generic_statement.sql,
        named_params: sqlite_params,
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
    let prepared =
        prepare_single_statement_sqlite(statement_sql, all_parameters, request_params_obj)?;
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
        let individual_statements = split_sql_statements(&query.sql);

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
        let prepared =
            prepare_single_statement_sqlite(&query.sql, &query.parameters, request_params_obj)?;
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
        let prepared =
            prepare_single_statement_sqlite(&query.sql, &query.parameters, request_params_obj)?;
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
