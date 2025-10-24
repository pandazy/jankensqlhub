use crate::{
    QueryDefinitions, parameters,
    result::{JankenError, QueryResult, Result},
    str_utils::split_sql_statements,
};

// Import generic types for parameter decoupling
use parameters::ParameterValue;
use tokio_postgres::Client;

// Implement trait for converting generic ParameterValue to PostgreSQL-specific ToSql
impl From<ParameterValue> for Box<dyn tokio_postgres::types::ToSql + Sync> {
    fn from(param_value: ParameterValue) -> Self {
        match param_value {
            ParameterValue::String(s) => Box::new(s),
            ParameterValue::Integer(i) => Box::new(i as i32), // PostgreSQL typically uses i32 for integers
            ParameterValue::Float(f) => Box::new(f),
            ParameterValue::Boolean(b) => Box::new(b),
            ParameterValue::Blob(bytes) => Box::new(bytes),
            ParameterValue::Null => Box::new(Option::<String>::None), // Represent null as None
        }
    }
}

/// Create a prepared statement from SQL using the generic parameter decoupling approach
/// This separates parameter analysis (generic) from database-specific conversions (PostgreSQL-specific)
fn prepare_single_statement_postgresql(
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<PreparedStatement> {
    // Use the generic parameter preparation (database-agnostic)
    let generic_statement = parameters::prepare_parameter_statement_generic(
        statement_sql,
        all_parameters,
        request_params_obj,
    )?;

    // Convert generic parameters to PostgreSQL-specific ToSql types
    let pgsql_params = generic_statement
        .parameters
        .into_iter()
        .map(|(name, param_value)| {
            let to_sql: Box<dyn tokio_postgres::types::ToSql + Sync> = param_value.into();
            (name, to_sql)
        })
        .collect();

    Ok(PreparedStatement {
        sql: generic_statement.sql,
        named_params: pgsql_params,
    })
}

// Map PostgreSQL row data to JSON objects based on column types and field names
// This function converts database rows to structured JSON data for easier unit testing
fn map_rows_to_json_data(
    rows: Vec<tokio_postgres::Row>,
    returns: &[String],
) -> Result<Vec<serde_json::Value>> {
    let mut result_data = Vec::new();

    for row in rows {
        let mut obj = serde_json::Map::new();
        for (idx, field_name) in returns.iter().enumerate() {
            // PostgreSQL row indexing starts at 0
            let value: Result<serde_json::Value> = match row.columns().get(idx) {
                Some(col) => match col.type_() {
                    &tokio_postgres::types::Type::BOOL => {
                        let val: bool = row.try_get(idx)?;
                        Ok(serde_json::Value::Bool(val))
                    }
                    &tokio_postgres::types::Type::INT2 | &tokio_postgres::types::Type::INT4 => {
                        let val: i32 = row.try_get(idx)?;
                        Ok(serde_json::Value::Number(val.into()))
                    }
                    &tokio_postgres::types::Type::INT8 => {
                        let val: i64 = row.try_get(idx)?;
                        Ok(serde_json::Value::Number(val.into()))
                    }
                    &tokio_postgres::types::Type::FLOAT4 | &tokio_postgres::types::Type::FLOAT8 => {
                        let val: f64 = row.try_get(idx)?;
                        Ok(serde_json::Value::Number(
                            serde_json::Number::from_f64(val).unwrap(),
                        ))
                    }
                    &tokio_postgres::types::Type::TEXT | &tokio_postgres::types::Type::VARCHAR => {
                        let val: String = row.try_get(idx)?;
                        Ok(serde_json::Value::String(val))
                    }
                    &tokio_postgres::types::Type::BYTEA => {
                        let val: Vec<u8> = row.try_get(idx)?;
                        Ok(serde_json::Value::Array(
                            val.iter()
                                .map(|&b| serde_json::Value::Number(b.into()))
                                .collect(),
                        ))
                    }
                    &tokio_postgres::types::Type::JSON | &tokio_postgres::types::Type::JSONB => {
                        let json_str: String = row.try_get(idx)?;
                        match serde_json::from_str(&json_str) {
                            Ok(val) => Ok(val),
                            Err(_) => Ok(serde_json::Value::Null),
                        }
                    }
                    _ => {
                        // Fall back to string representation for unsupported types
                        let val: String = row.try_get(idx)?;
                        Ok(serde_json::Value::String(val))
                    }
                },
                None => Ok(serde_json::Value::Null),
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
        result_data.push(serde_json::Value::Object(obj));
    }

    Ok(result_data)
}

// Prepared statement with PostgreSQL parameter handling
struct PreparedStatement {
    sql: String,
    // For PostgreSQL, we'll collect parameters as named values to be converted to positional
    named_params: Vec<(String, Box<dyn tokio_postgres::types::ToSql + Sync>)>,
}

// Convert named parameters to positional for PostgreSQL ($1, $2, etc.)
impl PreparedStatement {
    fn as_positional_params(&self) -> (String, Vec<&(dyn tokio_postgres::types::ToSql + Sync)>) {
        let mut positional_sql = self.sql.clone();
        let mut positional_params = Vec::new();

        // Sort parameters by name for consistent ordering (important for replacement)
        let mut sorted_params: Vec<_> = self.named_params.iter().collect();
        sorted_params.sort_by_key(|(name, _)| name.clone());

        for (index, (_, value)) in sorted_params.iter().enumerate() {
            let placeholder = format!("${}", index + 1);
            // Replace the first occurrence of the parameter name with positional placeholder
            // This assumes parameters are uniquely named in practice
            let param_pattern = format!("@{}", sorted_params[index].0);
            positional_sql = positional_sql.replace(&param_pattern, &placeholder);
            positional_params.push(value.as_ref());
        }

        (positional_sql, positional_params)
    }
}

// Execute a single SQL statement with its appropriate parameters
async fn execute_single_statement(
    transaction: &mut tokio_postgres::Transaction<'_>,
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<String> {
    let prepared =
        prepare_single_statement_postgresql(statement_sql, all_parameters, request_params_obj)?;

    // Convert to positional parameters for PostgreSQL
    let (positional_sql, positional_params) = prepared.as_positional_params();

    // Execute with positional parameter values
    transaction
        .execute(&positional_sql, &positional_params)
        .await
        .map_err(JankenError::Postgres)?;

    Ok(positional_sql)
}

// Execute mutation query (INSERT/UPDATE/DELETE/etc.) - split and execute within transaction
async fn execute_mutation_query(
    query: &crate::query::QueryDef,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    transaction: &mut tokio_postgres::Transaction<'_>,
) -> Result<Vec<String>> {
    let mut sql_statements = Vec::new();
    if query.sql.contains(';') {
        // Has parameters - split into individual statements and execute each one
        let individual_statements = split_sql_statements(&query.sql);

        for statement_sql in individual_statements {
            // Execute each statement with the appropriate parameters
            let sql = execute_single_statement(
                transaction,
                &statement_sql,
                &query.parameters,
                request_params_obj,
            )
            .await?;
            sql_statements.push(sql);
        }
    } else {
        // Single-statement mutation - prepare and execute normally with all parameters
        let prepared =
            prepare_single_statement_postgresql(&query.sql, &query.parameters, request_params_obj)?;

        let (positional_sql, positional_params) = prepared.as_positional_params();
        transaction
            .execute(&positional_sql, &positional_params)
            .await
            .map_err(JankenError::Postgres)?;
        sql_statements.push(positional_sql);
    }

    Ok(sql_statements)
}

// Execute query with both read and mutation operations within a unified transaction
pub async fn execute_query_unified(
    query: &crate::query::QueryDef,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    transaction: &mut tokio_postgres::Transaction<'_>,
) -> Result<QueryResult> {
    if !query.returns.is_empty() {
        // Query with returns specified - return structured data
        let prepared =
            prepare_single_statement_postgresql(&query.sql, &query.parameters, request_params_obj)?;

        let (positional_sql, positional_params) = prepared.as_positional_params();

        let rows = transaction
            .query(&positional_sql, &positional_params)
            .await
            .map_err(JankenError::Postgres)?;

        let result_data = map_rows_to_json_data(rows, &query.returns)?;

        Ok(QueryResult {
            sql_statements: vec![positional_sql],
            data: result_data,
        })
    } else {
        // Mutation query (INSERT/UPDATE/DELETE/etc.) - split and execute within transaction
        let sql_statements = execute_mutation_query(query, request_params_obj, transaction).await?;
        Ok(QueryResult {
            sql_statements,
            data: vec![],
        })
    }
}

// Execute queries with PostgreSQL backend
pub async fn query_run_postgresql(
    client: &mut Client,
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

    // Start transaction - always use transactions for consistency and ACID properties
    let mut transaction = client.transaction().await.map_err(JankenError::Postgres)?;

    // Handle all queries uniformly within transactions
    let query_result = execute_query_unified(query, request_params_obj, &mut transaction).await?;

    // Always commit the transaction (for both single and multi-statement queries)
    transaction.commit().await.map_err(JankenError::Postgres)?;
    Ok(query_result)
}
