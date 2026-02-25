use crate::{
    QueryDefinitions, parameters,
    result::{JankenError, QueryResult},
    str_utils::split_sql_statements,
};

// Import generic types for parameter decoupling
use parameters::ParameterValue;
use tokio_postgres::Client;

// PostgreSQL type OIDs for all column types
const POSTGRES_TYPE_OID_BOOL: u32 = 16;
const POSTGRES_TYPE_OID_BYTEA: u32 = 17;
const POSTGRES_TYPE_OID_INT2: u32 = 21;
const POSTGRES_TYPE_OID_INT4: u32 = 23;
const POSTGRES_TYPE_OID_INT8: u32 = 20;
const POSTGRES_TYPE_OID_FLOAT4: u32 = 700;
const POSTGRES_TYPE_OID_FLOAT8: u32 = 701;
const POSTGRES_TYPE_OID_TEXT: u32 = 25;
const POSTGRES_TYPE_OID_VARCHAR: u32 = 1043;
const POSTGRES_TYPE_OID_BPCHAR: u32 = 1042;
const POSTGRES_TYPE_OID_JSON: u32 = 114;
const POSTGRES_TYPE_OID_JSONB: u32 = 3802;

/// Convert a generic ParameterValue directly to PostgreSQL ToSql trait object
/// This provides easier testability by being a direct function call instead of a trait implementation
fn parameter_value_to_postgresql_tosql(
    param_value: ParameterValue,
) -> Box<dyn tokio_postgres::types::ToSql + Sync> {
    match param_value {
        ParameterValue::String(s) => Box::new(s),
        ParameterValue::Integer(i) => Box::new(i as i32), // PostgreSQL typically uses i32 for integers
        ParameterValue::Float(f) => Box::new(f),
        ParameterValue::Boolean(b) => Box::new(b),
        ParameterValue::Blob(bytes) => Box::new(bytes),
    }
}

/// Create a prepared statement from SQL using the generic parameter decoupling approach
/// This separates parameter analysis (generic) from database-specific conversions (PostgreSQL-specific)
fn prepare_single_statement_postgresql(
    statement_sql: &str,
    all_parameters: &[crate::parameters::Parameter],
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<PreparedStatement> {
    // Use the generic parameter preparation (database-agnostic)
    let generic_statement = parameters::prepare_parameter_statement_generic(
        statement_sql,
        all_parameters,
        request_params_obj,
    )?;

    // Convert generic parameters to PostgreSQL-specific ToSql types using direct function call
    let pgsql_params = generic_statement
        .parameters
        .into_iter()
        .map(|(name, param_value)| {
            let to_sql = parameter_value_to_postgresql_tosql(param_value);
            (name, to_sql)
        })
        .collect();

    Ok(PreparedStatement {
        sql: generic_statement.sql,
        named_params: pgsql_params,
    })
}

fn to_json_value<T: serde::Serialize>(value: T) -> anyhow::Result<serde_json::Value> {
    serde_json::to_value(value).map_err(Into::into)
}

/// Convert a PostgreSQL column value based on the given type
/// This function handles the type-specific conversion to JSON using OID-based detection for stability
pub fn postgres_type_to_json_conversion(
    column_type: &tokio_postgres::types::Type,
    row: &tokio_postgres::Row,
    idx: usize,
) -> anyhow::Result<serde_json::Value> {
    let oid = column_type.oid();
    match oid {
        POSTGRES_TYPE_OID_BOOL => {
            let val: bool = row.try_get(idx)?;
            to_json_value(val)
        }
        POSTGRES_TYPE_OID_INT2 => {
            let val: i16 = row.try_get(idx)?;
            to_json_value(val)
        }
        POSTGRES_TYPE_OID_INT4 => {
            let val: i32 = row.try_get(idx)?;
            to_json_value(val)
        }
        POSTGRES_TYPE_OID_INT8 => {
            let val: i64 = row.try_get(idx)?;
            to_json_value(val)
        }
        POSTGRES_TYPE_OID_FLOAT4 => {
            let val: f32 = row.try_get(idx)?;
            to_json_value(val)
        }
        POSTGRES_TYPE_OID_FLOAT8 => {
            let val: f64 = row.try_get(idx)?;
            to_json_value(val)
        }
        POSTGRES_TYPE_OID_TEXT | POSTGRES_TYPE_OID_VARCHAR | POSTGRES_TYPE_OID_BPCHAR => {
            let val: String = row.try_get(idx)?;
            to_json_value(val)
        }
        POSTGRES_TYPE_OID_BYTEA => {
            let val: Vec<u8> = row.try_get(idx)?;
            to_json_value(val)
        }
        POSTGRES_TYPE_OID_JSON | POSTGRES_TYPE_OID_JSONB => {
            let json_val: serde_json::Value = row.try_get(idx)?;
            to_json_value(json_val)
        }
        _ => {
            // Fall back to string representation for unsupported types
            // Since tokio_postgres may not support all PostgreSQL types for String conversion,
            // we return a marker string to indicate the fallback was executed
            let val = format!("Unsupported PostgreSQL type OID: {oid}");
            to_json_value(val)
        }
    }
}

// Convert a single PostgreSQL row to a JSON object based on column types and field names
fn row_to_json_object(
    row: &tokio_postgres::Row,
    returns: &[String],
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let mut obj = serde_json::Map::new();

    // Get all columns from the row
    let columns = row.columns();

    for field_name in returns {
        // Find the column index by matching the column name
        let column_idx = columns.iter().position(|col| col.name() == field_name);

        let value = match column_idx {
            Some(idx) => {
                let col = &columns[idx];
                postgres_type_to_json_conversion(col.type_(), row, idx)
            }
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

    Ok(obj)
}

// Map PostgreSQL row data to JSON objects based on column types and field names
// This function converts database rows to structured JSON data for easier unit testing
pub fn map_rows_to_json_data(
    rows: Vec<tokio_postgres::Row>,
    returns: &[String],
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut result_data = Vec::new();

    for row in rows {
        let obj = row_to_json_object(&row, returns)?;
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
) -> anyhow::Result<String> {
    let prepared =
        prepare_single_statement_postgresql(statement_sql, all_parameters, request_params_obj)?;

    // Convert to positional parameters for PostgreSQL
    let (positional_sql, positional_params) = prepared.as_positional_params();

    // Execute with positional parameter values
    transaction
        .execute(&positional_sql, &positional_params)
        .await
        .map_err(anyhow::Error::from)?;

    Ok(positional_sql)
}

// Execute mutation query (INSERT/UPDATE/DELETE/etc.) - split and execute within transaction
async fn execute_mutation_query(
    query: &crate::query::QueryDef,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    transaction: &mut tokio_postgres::Transaction<'_>,
) -> anyhow::Result<Vec<String>> {
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

        let (pos_sql, pos_params) = prepared.as_positional_params();
        transaction
            .execute(&pos_sql, &pos_params)
            .await
            .map_err(anyhow::Error::from)?;
        sql_statements.push(pos_sql);
    }

    Ok(sql_statements)
}

/// Resolve the returns specification to actual field names
fn resolve_returns(
    returns_spec: &crate::query::ReturnsSpec,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<Vec<String>> {
    match returns_spec {
        crate::query::ReturnsSpec::Static(fields) => Ok(fields.clone()),
        crate::query::ReturnsSpec::Dynamic(param_name) => {
            // Get the comma_list parameter value
            let param_value = request_params_obj
                .get(param_name)
                .ok_or_else(|| JankenError::new_parameter_not_provided(param_name.clone()))?;

            let fields_array = param_value.as_array().ok_or_else(|| {
                JankenError::new_parameter_type_mismatch(
                    "array for comma_list parameter",
                    param_value.to_string(),
                )
            })?;

            // Convert to vector of strings
            let fields: Vec<String> = fields_array
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect();

            Ok(fields)
        }
    }
}

// Execute query with both read and mutation operations within a unified transaction
pub async fn execute_query_unified(
    query: &crate::query::QueryDef,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    transaction: &mut tokio_postgres::Transaction<'_>,
) -> anyhow::Result<QueryResult> {
    // Resolve returns specification to actual field names
    let returns_fields = resolve_returns(&query.returns, request_params_obj)?;

    if !returns_fields.is_empty() {
        // Query with returns specified - return structured data
        let prepared =
            prepare_single_statement_postgresql(&query.sql, &query.parameters, request_params_obj)?;

        let (positional_sql, positional_params) = prepared.as_positional_params();

        let rows = transaction
            .query(&positional_sql, &positional_params)
            .await
            .map_err(anyhow::Error::from)?;

        let result_data = map_rows_to_json_data(rows, &returns_fields)?;

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

/// Execute a query within a user-provided PostgreSQL transaction.
/// This allows the caller to manage the transaction lifecycle (begin/commit/rollback),
/// enabling multiple `query_run` calls within the same transaction.
pub async fn query_run_postgresql_with_transaction(
    transaction: &mut tokio_postgres::Transaction<'_>,
    queries: &QueryDefinitions,
    query_name: &str,
    request_params: &serde_json::Value,
) -> anyhow::Result<QueryResult> {
    let query = queries
        .definitions
        .get(query_name)
        .ok_or_else(|| JankenError::new_query_not_found(query_name))?;

    let request_params_obj = request_params
        .as_object()
        .ok_or_else(|| JankenError::new_parameter_type_mismatch("object", "not object"))?;

    execute_query_unified(query, request_params_obj, transaction).await
}

/// Execute queries with PostgreSQL backend.
/// This is the main entry point for PostgreSQL operations.
/// It creates a transaction internally, executes the query, and commits.
pub async fn query_run_postgresql(
    client: &mut Client,
    queries: &QueryDefinitions,
    query_name: &str,
    request_params: &serde_json::Value,
) -> anyhow::Result<QueryResult> {
    let mut transaction = client.transaction().await.map_err(anyhow::Error::from)?;

    let query_result = query_run_postgresql_with_transaction(
        &mut transaction,
        queries,
        query_name,
        request_params,
    )
    .await?;

    transaction.commit().await.map_err(anyhow::Error::from)?;
    Ok(query_result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_value_float_to_postgresql_conversion() {
        let float_param = crate::parameters::ParameterValue::Float(3.15);
        let _sql_float: Box<dyn tokio_postgres::types::ToSql + Sync> =
            parameter_value_to_postgresql_tosql(float_param);
        // The conversion function works correctly if it doesn't panic
    }

    #[test]
    fn test_parameter_value_boolean_to_postgresql_conversion() {
        let bool_param = crate::parameters::ParameterValue::Boolean(true);
        let _sql_bool: Box<dyn tokio_postgres::types::ToSql + Sync> =
            parameter_value_to_postgresql_tosql(bool_param);
        // The conversion function works correctly if it doesn't panic
    }

    #[test]
    fn test_parameter_value_blob_to_postgresql_conversion() {
        let blob_param = crate::parameters::ParameterValue::Blob(vec![1, 2, 3, 255]);
        let _sql_blob: Box<dyn tokio_postgres::types::ToSql + Sync> =
            parameter_value_to_postgresql_tosql(blob_param);
        // The conversion function works correctly if it doesn't panic
    }

    // Tests for resolve_returns function
    #[test]
    fn test_resolve_returns_static_multiple_fields() {
        let returns_spec = crate::query::ReturnsSpec::Static(vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
        ]);
        let params = serde_json::Map::new();

        let result = resolve_returns(&returns_spec, &params).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "id");
        assert_eq!(result[1], "name");
        assert_eq!(result[2], "email");
    }

    #[test]
    fn test_resolve_returns_static_single_field() {
        let returns_spec = crate::query::ReturnsSpec::Static(vec!["name".to_string()]);
        let params = serde_json::Map::new();

        let result = resolve_returns(&returns_spec, &params).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "name");
    }

    #[test]
    fn test_resolve_returns_static_empty_list() {
        let returns_spec = crate::query::ReturnsSpec::Static(vec![]);
        let params = serde_json::Map::new();

        let result = resolve_returns(&returns_spec, &params).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_resolve_returns_dynamic_valid_comma_list() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": ["name", "email", "age"]
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "name");
        assert_eq!(result[1], "email");
        assert_eq!(result[2], "age");
    }

    #[test]
    fn test_resolve_returns_dynamic_single_field() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("cols".to_string());
        let params_value = serde_json::json!({
            "cols": ["id"]
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "id");
    }

    #[test]
    fn test_resolve_returns_dynamic_empty_array() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": []
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_resolve_returns_dynamic_missing_parameter() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params = serde_json::Map::new();

        let result = resolve_returns(&returns_spec, &params);

        assert!(result.is_err());
        let err = result.unwrap_err();
        if let Some(janken_err) = err.downcast_ref::<JankenError>() {
            let data = crate::get_error_data(janken_err);
            let param_name = crate::error_meta(data, "parameter_name").unwrap();
            assert_eq!(param_name, "fields");
        } else {
            panic!("Expected JankenError for missing parameter");
        }
    }

    #[test]
    fn test_resolve_returns_dynamic_non_array_string() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": "name,email"
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params);

        assert!(result.is_err());
        let err = result.unwrap_err();
        if let Some(janken_err) = err.downcast_ref::<JankenError>() {
            let data = crate::get_error_data(janken_err);
            let expected = crate::error_meta(data, crate::M_EXPECTED).unwrap();
            assert!(expected.contains("array for comma_list parameter"));
        } else {
            panic!("Expected JankenError for type mismatch");
        }
    }

    #[test]
    fn test_resolve_returns_dynamic_non_array_number() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": 42
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params);

        assert!(result.is_err());
        let err = result.unwrap_err();
        if let Some(janken_err) = err.downcast_ref::<JankenError>() {
            let data = crate::get_error_data(janken_err);
            let expected = crate::error_meta(data, crate::M_EXPECTED).unwrap();
            assert!(expected.contains("array for comma_list parameter"));
        } else {
            panic!("Expected JankenError for type mismatch");
        }
    }

    #[test]
    fn test_resolve_returns_dynamic_non_array_object() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": {"name": "value"}
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params);

        assert!(result.is_err());
        let err = result.unwrap_err();
        if let Some(janken_err) = err.downcast_ref::<JankenError>() {
            let data = crate::get_error_data(janken_err);
            let expected = crate::error_meta(data, crate::M_EXPECTED).unwrap();
            assert!(expected.contains("array for comma_list parameter"));
        } else {
            panic!("Expected JankenError for type mismatch");
        }
    }

    #[test]
    fn test_resolve_returns_dynamic_array_with_non_string_values() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": ["name", 123, true, null, "email"]
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "name");
        assert_eq!(result[1], "email");
    }

    #[test]
    fn test_resolve_returns_dynamic_array_all_non_string_values() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": [123, true, null, {"key": "value"}]
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_resolve_returns_dynamic_order_preserved() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("columns".to_string());
        let params_value = serde_json::json!({
            "columns": ["age", "email", "name", "id"]
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params).unwrap();

        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "age");
        assert_eq!(result[1], "email");
        assert_eq!(result[2], "name");
        assert_eq!(result[3], "id");
    }

    #[test]
    fn test_resolve_returns_dynamic_duplicate_fields() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": ["name", "name", "email", "name"]
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params).unwrap();

        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "name");
        assert_eq!(result[1], "name");
        assert_eq!(result[2], "email");
        assert_eq!(result[3], "name");
    }

    #[test]
    fn test_resolve_returns_static_special_characters() {
        let returns_spec = crate::query::ReturnsSpec::Static(vec![
            "user_id".to_string(),
            "first-name".to_string(),
            "email.address".to_string(),
        ]);
        let params = serde_json::Map::new();

        let result = resolve_returns(&returns_spec, &params).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "user_id");
        assert_eq!(result[1], "first-name");
        assert_eq!(result[2], "email.address");
    }

    #[test]
    fn test_resolve_returns_dynamic_special_characters() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": ["user_id", "first-name", "email.address"]
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "user_id");
        assert_eq!(result[1], "first-name");
        assert_eq!(result[2], "email.address");
    }

    #[test]
    fn test_resolve_returns_dynamic_empty_strings() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": ["name", "", "email", ""]
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params).unwrap();

        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "name");
        assert_eq!(result[1], "");
        assert_eq!(result[2], "email");
        assert_eq!(result[3], "");
    }

    #[test]
    fn test_resolve_returns_static_unicode_field_names() {
        let returns_spec = crate::query::ReturnsSpec::Static(vec![
            "名前".to_string(),
            "メール".to_string(),
            "年齢".to_string(),
        ]);
        let params = serde_json::Map::new();

        let result = resolve_returns(&returns_spec, &params).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "名前");
        assert_eq!(result[1], "メール");
        assert_eq!(result[2], "年齢");
    }

    #[test]
    fn test_resolve_returns_dynamic_unicode_field_names() {
        let returns_spec = crate::query::ReturnsSpec::Dynamic("fields".to_string());
        let params_value = serde_json::json!({
            "fields": ["名前", "メール", "年齢"]
        });
        let params = params_value.as_object().unwrap();

        let result = resolve_returns(&returns_spec, params).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "名前");
        assert_eq!(result[1], "メール");
        assert_eq!(result[2], "年齢");
    }
}
