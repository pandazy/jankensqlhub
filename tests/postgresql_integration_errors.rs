//! Error handling PostgreSQL integration tests for JankenSQLHub
//!
//! Tests parameter validation errors and error handling.

use jankensqlhub::{JankenError, query_run_postgresql};
use tokio_postgres::NoTls;

// Helper function to get PostgreSQL connection string from environment
fn get_postgres_connection_string() -> Option<String> {
    std::env::var("POSTGRES_CONNECTION_STRING").ok()
}

// Helper function to establish PostgreSQL connection for tests
async fn setup_postgres_connection() -> Option<tokio_postgres::Client> {
    let connection_string = get_postgres_connection_string()?;
    let (client, connection) = tokio_postgres::connect(&connection_string, NoTls)
        .await
        .ok()?;

    // Run the connection in the background
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {e}");
        }
    });

    Some(client)
}

#[tokio::test]
async fn test_postgres_non_object_request_params_error() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let json_definitions = serde_json::json!({
        "simple_select": {
            "query": "SELECT 1 as dummy",
            "returns": ["dummy"],
            "args": {}
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    // Test with string parameter instead of object
    let invalid_params = serde_json::Value::String("not an object".to_string());
    let result =
        query_run_postgresql(&mut client, &queries, "simple_select", &invalid_params).await;
    assert!(result.is_err());

    // Check that the error is the expected ParameterTypeMismatch
    let err = result.unwrap_err();
    assert!(matches!(err, JankenError::ParameterTypeMismatch { .. }));
    if let JankenError::ParameterTypeMismatch { expected, got } = err {
        assert_eq!(expected, "object");
        assert_eq!(got, "not object");
    }

    // Test with array parameter instead of object
    let invalid_params = serde_json::json!(["not", "an", "object"]);
    let result =
        query_run_postgresql(&mut client, &queries, "simple_select", &invalid_params).await;
    assert!(result.is_err());

    // Check that the error is the expected ParameterTypeMismatch
    let err = result.unwrap_err();
    assert!(matches!(err, JankenError::ParameterTypeMismatch { .. }));
    if let JankenError::ParameterTypeMismatch { expected, got } = err {
        assert_eq!(expected, "object");
        assert_eq!(got, "not object");
    }

    // Test with number parameter instead of object
    let invalid_params = serde_json::json!(42);
    let result =
        query_run_postgresql(&mut client, &queries, "simple_select", &invalid_params).await;
    assert!(result.is_err());

    // Check that the error is the expected ParameterTypeMismatch
    let err = result.unwrap_err();
    assert!(matches!(err, JankenError::ParameterTypeMismatch { .. }));
    if let JankenError::ParameterTypeMismatch { expected, got } = err {
        assert_eq!(expected, "object");
        assert_eq!(got, "not object");
    }
}

#[tokio::test]
async fn test_postgres_empty_list_error() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let json_definitions = serde_json::json!({
        "select_empty_list": {
            "query": "SELECT 1 as dummy WHERE 1 IN :[ids]",
            "returns": ["dummy"],
            "args": {
                "ids": { "itemtype": "integer" }
            }
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    // Empty list should result in error (no table needed for this parameter validation)
    let params = serde_json::json!({"ids": []});
    let result = query_run_postgresql(&mut client, &queries, "select_empty_list", &params).await;
    assert!(result.is_err());
}
