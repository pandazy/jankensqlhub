use crate::{
    ParameterType, QueryDefinitions, parameters,
    result::{JankenError, QueryResult, Result},
    str_utils::{self, quote_identifier},
};
use tokio_postgres::Client;

// Type alias for parameter preparation result to reduce type complexity
// PostgreSQL-specific due to tokio_postgres::types::ToSql trait
type PreparedParametersResult = (
    String,
    Vec<(String, Box<dyn tokio_postgres::types::ToSql + Sync>)>,
);

// Convert a serde_json Value to a tokio_postgres ToSql type for list items
// This is PostgreSQL-specific due to tokio_postgres::types::ToSql trait
fn json_value_to_postgres(
    value: &serde_json::Value,
) -> Box<dyn tokio_postgres::types::ToSql + Sync> {
    match value {
        serde_json::Value::String(s) => Box::new(s.clone()),
        serde_json::Value::Number(n) => {
            if n.is_i64() {
                Box::new(n.as_i64().unwrap() as i32) // PostgreSQL integers are typically i32
            } else {
                Box::new(n.as_f64().unwrap())
            }
        }
        serde_json::Value::Bool(b) => Box::new(*b),
        serde_json::Value::Null => Box::new(Option::<String>::None), // Represent null as None
        serde_json::Value::Array(a) => {
            Box::new(serde_json::to_string(&serde_json::Value::Array(a.clone())).unwrap())
        } // JSON as string
        serde_json::Value::Object(o) => {
            Box::new(serde_json::to_string(&serde_json::Value::Object(o.clone())).unwrap())
        } // JSON as string
    }
}

// Convert a JSON value to tokio_postgres ToSql based on parameter type definition
// This is PostgreSQL-specific due to tokio_postgres::types::ToSql trait
fn parameter_value_to_postgres(
    param_value: &serde_json::Value,
    param_type: &ParameterType,
) -> Box<dyn tokio_postgres::types::ToSql + Sync> {
    match param_type {
        ParameterType::String => Box::new(param_value.as_str().unwrap().to_string()),
        ParameterType::Integer => Box::new(param_value.as_i64().unwrap() as i32),
        ParameterType::Float => Box::new(param_value.as_f64().unwrap()),
        ParameterType::Boolean => Box::new(param_value.as_bool().unwrap()),
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

// Replace @parameters with @names and collect their values as tokio_postgres types (excluding table names)
fn prepare_statement_parameters(
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<PreparedParametersResult> {
    let statement_param_names =
        parameters::extract_parameters_with_regex(statement_sql, &parameters::PARAMETER_REGEX);
    let prepared_sql = statement_sql.to_string();
    let mut named_params = Vec::new();

    for param_name in &statement_param_names {
        // prepared_sql remains unchanged (still has @param)
        // Get the parameter value from request params
        let param_value = request_params_obj
            .get(param_name)
            .ok_or_else(|| JankenError::ParameterNotProvided(param_name.clone()))?;

        // Find the parameter definition for type validation
        let param_def = all_parameters
            .iter()
            .find(|p| p.name == *param_name)
            .ok_or_else(|| JankenError::ParameterNotProvided(param_name.clone()))?;

        // Convert JSON value to tokio_postgres::types::ToSql based on parameter type (validation already done upstream)
        let to_sql: Box<dyn tokio_postgres::types::ToSql + Sync> =
            parameter_value_to_postgres(param_value, &param_def.param_type);
        named_params.push((param_name.clone(), to_sql));
    }

    Ok((prepared_sql, named_params))
}

// Create a prepared statement from SQL with proper parameter replacement
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

    // Replace :[list] parameters with expanded positional parameters ($1, $2, etc.)
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

                // Create positional placeholders and values
                let mut placeholders = Vec::new();
                for (i, item) in list_array.iter().enumerate() {
                    let param_key = format!("{list_param_name}_{i}");
                    placeholders.push(format!("@{param_key}")); // Keep @ for consistency

                    let to_sql: Box<dyn tokio_postgres::types::ToSql + Sync> =
                        json_value_to_postgres(item);
                    named_params.push((param_key, to_sql));
                }

                // Replace the :[param] with (positional placeholders joined by comma)
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

// Execute a single SQL statement with its appropriate parameters
async fn execute_single_statement(
    transaction: &mut tokio_postgres::Transaction<'_>,
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<String> {
    let prepared = prepare_single_statement(statement_sql, all_parameters, request_params_obj)?;

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
        let individual_statements = str_utils::split_sql_statements(&query.sql);

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
        let prepared = prepare_single_statement(&query.sql, &query.parameters, request_params_obj)?;

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
        let prepared = prepare_single_statement(&query.sql, &query.parameters, request_params_obj)?;

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
