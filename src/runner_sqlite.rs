use crate::{
    QueryDefinitions, parameters,
    result::{JankenError, QueryResult},
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
) -> anyhow::Result<PreparedStatement> {
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
) -> anyhow::Result<String> {
    let prepared =
        prepare_single_statement_sqlite(statement_sql, all_parameters, request_params_obj)?;
    let named_params = prepared.as_named_params();

    // Now execute with the named parameter values
    let mut stmt = tx.prepare(&prepared.sql)?;
    stmt.execute(&named_params[..])?;
    Ok(prepared.sql)
}

/// Execute mutation query (INSERT/UPDATE/DELETE/etc.) - split and execute within transaction
/// This logic is mostly DB-independent but uses SQLite-specific transaction
fn execute_mutation_query(
    query: &crate::query::QueryDef,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    tx: &rusqlite::Transaction,
) -> anyhow::Result<Vec<String>> {
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
        stmt.execute(&named_params[..])?;
        sql_statements.push(prepared.sql);
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

            // Safe unwrap: parameter type already validated as array at definition time
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

/// Execute query with both read and mutation operations within a unified transaction
/// This logic is mostly DB-independent but uses SQLite-specific transaction
pub fn execute_query_unified(
    query: &crate::query::QueryDef,
    request_params_obj: &serde_json::Map<String, serde_json::Value>,
    tx: &rusqlite::Transaction,
) -> anyhow::Result<QueryResult> {
    // Resolve returns specification to actual field names
    let returns_fields = resolve_returns(&query.returns, request_params_obj)?;

    if !returns_fields.is_empty() {
        // Query with returns specified - return structured data
        let prepared =
            prepare_single_statement_sqlite(&query.sql, &query.parameters, request_params_obj)?;
        let mut stmt = tx.prepare(&prepared.sql)?;
        let named_params = prepared.as_named_params();

        // Get column names from the prepared statement
        let column_names: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|name| name.to_string())
            .collect();

        let rows = stmt.query_map(&named_params[..], |row| {
            let mut obj = serde_json::Map::new();

            for field_name in &returns_fields {
                // Find the column index by matching the column name
                let column_idx = column_names.iter().position(|name| name == field_name);

                let value: serde_json::Value = match column_idx {
                    Some(idx) => match row.get_ref(idx).expect("column index should be valid") {
                        rusqlite::types::ValueRef::Integer(i) => {
                            serde_json::Value::Number(i.into())
                        }
                        rusqlite::types::ValueRef::Real(r) => serde_json::Value::from(r),
                        rusqlite::types::ValueRef::Text(s) => {
                            serde_json::Value::String(String::from_utf8_lossy(s).to_string())
                        }
                        rusqlite::types::ValueRef::Blob(b) => serde_json::Value::Array(
                            b.iter()
                                .map(|&byte| serde_json::Value::Number(byte.into()))
                                .collect(),
                        ),
                        rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                    },
                    None => serde_json::Value::Null,
                };

                obj.insert(field_name.clone(), value);
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
) -> anyhow::Result<QueryResult> {
    let query = queries
        .definitions
        .get(query_name)
        .ok_or_else(|| JankenError::new_query_not_found(query_name.to_string()))?;

    let request_params_obj = request_params
        .as_object()
        .ok_or_else(|| JankenError::new_parameter_type_mismatch("object", "not object"))?;

    let tx = conn.transaction()?;

    // Handle all queries uniformly within transactions
    let query_result = execute_query_unified(query, request_params_obj, &tx)?;

    // Always commit the transaction (for both single and multi-statement queries)
    tx.commit()?;
    Ok(query_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::ReturnsSpec;
    use serde_json::json;

    #[test]
    fn test_resolve_returns_static() {
        // Test static returns specification
        let returns_spec = ReturnsSpec::Static(vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
        ]);
        let params = json!({}).as_object().unwrap().clone();

        let result = resolve_returns(&returns_spec, &params).unwrap();
        assert_eq!(result, vec!["id", "name", "email"]);
    }

    #[test]
    fn test_resolve_returns_static_empty() {
        // Test empty static returns
        let returns_spec = ReturnsSpec::Static(vec![]);
        let params = json!({}).as_object().unwrap().clone();

        let result = resolve_returns(&returns_spec, &params).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_resolve_returns_dynamic_valid() {
        // Test dynamic returns with valid comma_list parameter
        let returns_spec = ReturnsSpec::Dynamic("fields".to_string());
        let params = json!({
            "fields": ["id", "name", "email"]
        })
        .as_object()
        .unwrap()
        .clone();

        let result = resolve_returns(&returns_spec, &params).unwrap();
        assert_eq!(result, vec!["id", "name", "email"]);
    }

    #[test]
    fn test_resolve_returns_dynamic_empty_array() {
        // Test dynamic returns with empty array
        let returns_spec = ReturnsSpec::Dynamic("fields".to_string());
        let params = json!({
            "fields": []
        })
        .as_object()
        .unwrap()
        .clone();

        let result = resolve_returns(&returns_spec, &params).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_resolve_returns_dynamic_missing_parameter() {
        // Test dynamic returns when parameter is not provided
        let returns_spec = ReturnsSpec::Dynamic("fields".to_string());
        let params = json!({}).as_object().unwrap().clone();

        let result = resolve_returns(&returns_spec, &params);
        assert!(result.is_err());

        let err = result.unwrap_err();
        if let Ok(JankenError::ParameterNotProvided { .. }) = err.downcast::<JankenError>() {
            // Test passed - got expected error type
        } else {
            panic!("Expected ParameterNotProvided error");
        }
    }

    #[test]
    fn test_resolve_returns_dynamic_non_array_value() {
        // Test dynamic returns with non-array value (defensive test)
        // This case should be unreachable in normal flow due to validation,
        // but we test it to ensure error handling works correctly
        let returns_spec = ReturnsSpec::Dynamic("fields".to_string());
        let params = json!({
            "fields": "not_an_array"
        })
        .as_object()
        .unwrap()
        .clone();

        let result = resolve_returns(&returns_spec, &params);
        assert!(result.is_err());

        let err = result.unwrap_err();
        if let Ok(JankenError::ParameterTypeMismatch { .. }) = err.downcast::<JankenError>() {
            // Test passed - got expected error type
        } else {
            panic!("Expected ParameterTypeMismatch error");
        }
    }

    #[test]
    fn test_resolve_returns_dynamic_non_array_number() {
        // Test dynamic returns with number instead of array (defensive test)
        let returns_spec = ReturnsSpec::Dynamic("fields".to_string());
        let params = json!({
            "fields": 123
        })
        .as_object()
        .unwrap()
        .clone();

        let result = resolve_returns(&returns_spec, &params);
        assert!(result.is_err());

        let err = result.unwrap_err();
        if let Ok(JankenError::ParameterTypeMismatch { .. }) = err.downcast::<JankenError>() {
            // Test passed - got expected error type
        } else {
            panic!("Expected ParameterTypeMismatch error");
        }
    }

    #[test]
    fn test_resolve_returns_dynamic_array_with_non_strings() {
        // Test that filter_map correctly handles non-string values in array
        let returns_spec = ReturnsSpec::Dynamic("fields".to_string());
        let params = json!({
            "fields": ["id", 123, "name", null, "email"]
        })
        .as_object()
        .unwrap()
        .clone();

        let result = resolve_returns(&returns_spec, &params).unwrap();
        // Only string values should be included
        assert_eq!(result, vec!["id", "name", "email"]);
    }
}
