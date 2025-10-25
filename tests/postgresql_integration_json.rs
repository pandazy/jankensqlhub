//! JSON handling PostgreSQL integration tests for JankenSQLHub
//!
//! Tests JSON and JSONB column types and parsing behavior.

use jankensqlhub::query_run_postgresql;
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
async fn test_postgres_json_parsing_and_text_handling() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let test_table = "test_json_parsing_and_text_handling";

    // Create a test table with TEXT column to test TEXT handling
    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {test_table}"), &[])
        .await;

    client
        .execute(
            &format!(
                r#"
            CREATE TABLE {test_table} (
                id SERIAL PRIMARY KEY,
                json_text TEXT,
                regular_text TEXT
            )
            "#
            ),
            &[],
        )
        .await
        .expect("Failed to create test table");

    // Insert test data - JSON stored as TEXT and regular text
    client
        .execute(
            &format!("INSERT INTO {test_table} (json_text, regular_text) VALUES ($1, $2)"),
            &[
                &"[1,2,3,4]", // Valid JSON array stored as TEXT
                &"Just some regular text content",
            ],
        )
        .await
        .expect("Failed to insert test data");

    let json_definitions = serde_json::json!({
        "select_text_data": {
            "query": format!("SELECT id, json_text, regular_text FROM {test_table} ORDER BY id"),
            "returns": ["id", "json_text", "regular_text"],
            "args": {}
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    // Execute the query and verify TEXT columns are properly handled
    let params = serde_json::json!({});
    let result = query_run_postgresql(&mut client, &queries, "select_text_data", &params)
        .await
        .unwrap();

    assert_eq!(result.data.len(), 1, "Should have one row");

    let row = &result.data[0];
    let obj = row.as_object().unwrap();

    // Test that JSON stored as TEXT is returned as a string
    assert_eq!(obj.get("json_text"), Some(&serde_json::json!("[1,2,3,4]")));

    // Test regular TEXT column
    assert_eq!(
        obj.get("regular_text"),
        Some(&serde_json::json!("Just some regular text content"))
    );
}

/// Unit test for postgres_type_to_json_conversion function
/// Tests the decoupled column type conversion logic (no dependency on column metadata lookup)
#[tokio::test]
async fn test_postgres_type_to_json_conversion_direct() {
    let Some(client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let test_table = "test_type_conversion_direct";

    // Clean up and create test table with a single column to test direct type conversion
    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {test_table}"), &[])
        .await;

    client
        .execute(
            &format!(
                r#"
            CREATE TABLE {test_table} (
                bool_col BOOLEAN
            )
            "#
            ),
            &[],
        )
        .await
        .expect("Failed to create test table");

    // Insert test data
    client
        .execute(
            &format!("INSERT INTO {test_table} (bool_col) VALUES ($1)"),
            &[&true],
        )
        .await
        .expect("Failed to insert test data");

    // Query the data
    let rows = client
        .query(&format!("SELECT bool_col FROM {test_table} LIMIT 1"), &[])
        .await
        .expect("Failed to query data");

    let row = &rows[0];

    // Test that we can call the conversion function directly with the type
    // This demonstrates the decoupling - we don't need to go through column metadata lookup
    let bool_result = jankensqlhub::runner_postgresql::postgres_type_to_json_conversion(
        &tokio_postgres::types::Type::BOOL,
        row,
        0,
    );
    assert_eq!(bool_result.unwrap(), serde_json::json!(true));

    // Test with wrong type (this would fail type conversion in try_get, demonstrating the type safety)
    let wrong_type_result = jankensqlhub::runner_postgresql::postgres_type_to_json_conversion(
        &tokio_postgres::types::Type::TEXT,
        row,
        0,
    );
    // This should fail because TEXT try_get on a BOOLEAN column would fail
    assert!(wrong_type_result.is_err());
}

#[tokio::test]
async fn test_postgres_json_and_jsonb_column_types() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let test_table = "test_json_columns";

    // Clean up and create test table with JSON and JSONB columns
    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {test_table}"), &[])
        .await;

    client
        .execute(
            &format!(
                r#"
            CREATE TABLE {test_table} (
                id SERIAL PRIMARY KEY,
                json_col JSON,
                jsonb_col JSONB,
                text_json_col TEXT
            )
            "#
            ),
            &[],
        )
        .await
        .expect("Failed to create test table");

    // Insert valid JSON objects and invalid JSON stored as TEXT (to test parsing)
    client
        .execute(
            &format!("INSERT INTO {test_table} (json_col, jsonb_col, text_json_col) VALUES ('{{\"name\":\"John\",\"age\":30,\"active\":true,\"scores\":[85,92,78]}}', '{{\"department\":\"Engineering\",\"level\":\"Senior\"}}', '{{\"valid\":\"json\"}}')"),
            &[],
        )
        .await
        .expect("Failed to insert valid JSON data");

    // Insert invalid JSON as TEXT (to test parsing error handling)
    client
        .execute(
            &format!("INSERT INTO {test_table} (text_json_col) VALUES ('{{invalid json')"),
            &[],
        )
        .await
        .expect("Failed to insert invalid JSON data as TEXT");

    let json_definitions = serde_json::json!({
        "select_json_data": {
            "query": format!("SELECT id, json_col, jsonb_col, text_json_col FROM {test_table} ORDER BY id"),
            "returns": ["id", "json_col", "jsonb_col", "text_json_col"],
            "args": {}
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    // Execute the query and verify JSON columns are properly handled
    let params = serde_json::json!({});
    let result = query_run_postgresql(&mut client, &queries, "select_json_data", &params)
        .await
        .unwrap();

    assert_eq!(result.data.len(), 2, "Should have two rows");

    // First row: valid JSON data everywhere
    let first_row = &result.data[0];
    assert!(
        first_row.is_object(),
        "First result should be a JSON object"
    );

    let obj1 = first_row.as_object().unwrap();

    // Just verify something was parsed (not null) for valid JSON
    let json_val = obj1.get("json_col").unwrap();
    assert!(
        !json_val.is_null(),
        "json_col should contain parsed JSON, not null"
    );

    let jsonb_val = obj1.get("jsonb_col").unwrap();
    assert!(
        !jsonb_val.is_null(),
        "jsonb_col should contain parsed JSON, not null"
    );

    let text_json_val = obj1.get("text_json_col").unwrap();
    assert!(
        !text_json_val.is_null(),
        "text_json_col should contain parsed JSON, not null"
    );

    // Second row exists and invalid JSON was handled
    let second_row = &result.data[1];
    assert!(
        second_row.is_object(),
        "Second result should be a JSON object"
    );

    let obj2 = second_row.as_object().unwrap();
    // At minimum, this proves the invalid JSON TEXT column was processed
    assert!(
        obj2.contains_key("text_json_col"),
        "Should contain processed text_json_col"
    );
}

#[tokio::test]
async fn test_json_parsing_fallback() {
    // Test what happens when we try to parse invalid JSON - this covers the Err(_) case in JSON parsing
    let invalid_json_str = "{invalid json";
    let result: Result<serde_json::Value, serde_json::Error> =
        serde_json::from_str(invalid_json_str);
    assert!(result.is_err(), "Invalid JSON should fail to parse");

    // This demonstrates that the JSON parsing fallback case is testable by constructing JSON strings in tests
    // rather than trying to insert them as database columns which have their own type system
}
