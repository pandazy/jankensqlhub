//! Data type conversion PostgreSQL integration tests for JankenSQLHub
//!
//! Tests data type mapping and conversion from PostgreSQL to JSON.

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
async fn test_postgres_comprehensive_column_types() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let test_table = "test_comprehensive_column_types";

    // Clean up and create test table with comprehensive column types
    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {test_table}"), &[])
        .await;

    client
        .execute(
            &format!(
                r#"
            CREATE TABLE {test_table} (
                id SERIAL PRIMARY KEY,
                bool_col BOOLEAN,
                int2_col SMALLINT,
                int4_col INTEGER,
                int8_col BIGINT,
                float4_col REAL,
                float8_col DOUBLE PRECISION,
                text_col TEXT,
                varchar_col VARCHAR(50),
                bytea_col BYTEA
            )
            "#
            ),
            &[],
        )
        .await
        .expect("Failed to create test table");

    // Insert test data with supported types
    client
        .execute(
            &format!("INSERT INTO {test_table} (bool_col, int2_col, int4_col, int8_col, float4_col, float8_col, text_col, varchar_col, bytea_col) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"),
            &[
                &true,
                &32767i16,               // SMALLINT max
                &42i32,                  // INTEGER
                &9223372036854775807i64, // BIGINT max
                &42.5f32,                // REAL
                &123.456f64,             // DOUBLE PRECISION
                &"Hello World",
                &"VARCHAR string",
                &vec![1u8, 2u8, 3u8, 255u8],
            ],
        )
        .await
        .expect("Failed to insert test data");

    let json_definitions = serde_json::json!({
        "select_comprehensive": {
            "query": format!("SELECT id, bool_col, int2_col, int4_col, int8_col, float4_col, float8_col, text_col, varchar_col, bytea_col FROM {test_table} ORDER BY id"),
            "returns": ["id", "bool_col", "int2_col", "int4_col", "int8_col", "float4_col", "float8_col", "text_col", "varchar_col", "bytea_col"],
            "args": {}
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    // Execute the query and verify all column types are properly handled
    let params = serde_json::json!({});
    let result = query_run_postgresql(&mut client, &queries, "select_comprehensive", &params)
        .await
        .unwrap();

    assert_eq!(result.data.len(), 1, "Should have one row");

    let row = &result.data[0];
    let obj = row.as_object().unwrap();

    // Test BOOL column
    assert_eq!(obj.get("bool_col"), Some(&serde_json::json!(true)));

    // Test SMALLINT column
    assert_eq!(obj.get("int2_col"), Some(&serde_json::json!(32767)));

    // Test INTEGER column
    assert_eq!(obj.get("int4_col"), Some(&serde_json::json!(42)));

    // Test BIGINT column
    assert_eq!(
        obj.get("int8_col"),
        Some(&serde_json::json!(9223372036854775807i64))
    );

    // Test REAL/FLOAT4 column (should be close to 42.5)
    let float4_val = obj.get("float4_col").unwrap();
    let float4_num = float4_val.as_f64().unwrap();
    assert!((float4_num - 42.5).abs() < 0.001);

    // Test DOUBLE PRECISION/FLOAT8 column
    assert_eq!(obj.get("float8_col"), Some(&serde_json::json!(123.456)));

    // Test TEXT column
    assert_eq!(obj.get("text_col"), Some(&serde_json::json!("Hello World")));

    // Test VARCHAR column
    assert_eq!(
        obj.get("varchar_col"),
        Some(&serde_json::json!("VARCHAR string"))
    );

    // Test BYTEA column (should be array of byte values)
    let bytea_val = obj.get("bytea_col").unwrap();
    assert!(
        bytea_val.is_array(),
        "BYTEA should be converted to JSON array"
    );
    let bytea_arr = bytea_val.as_array().unwrap();
    assert_eq!(
        bytea_arr,
        &vec![
            serde_json::json!(1),
            serde_json::json!(2),
            serde_json::json!(3),
            serde_json::json!(255)
        ]
    );
}

#[tokio::test]
async fn test_map_rows_to_json_data_int8_column() {
    use jankensqlhub::runner_postgresql::map_rows_to_json_data;

    let Some(client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let test_table = "test_int8";

    // Clean up and create test table
    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {test_table}"), &[])
        .await;

    client
        .execute(
            &format!(
                r#"
            CREATE TABLE {test_table} (
                id SERIAL PRIMARY KEY,
                int8_col BIGINT
            )
            "#
            ),
            &[],
        )
        .await
        .expect("Failed to create test table");

    // Insert test data with BIGINT maximum value
    client
        .execute(
            &format!("INSERT INTO {test_table} (int8_col) VALUES ($1)"),
            &[&9223372036854775807i64],
        )
        .await
        .expect("Failed to insert test data");

    // Query the data using raw PostgreSQL to get actual Row objects
    let rows = client
        .query(
            &format!("SELECT id, int8_col FROM {test_table} ORDER BY id"),
            &[],
        )
        .await
        .expect("Failed to query data");

    // Test the map_rows_to_json_data function directly
    let field_names = vec!["id".to_string(), "int8_col".to_string()];

    let result = map_rows_to_json_data(rows, &field_names);

    let json_objects = result.unwrap();
    assert_eq!(json_objects.len(), 1, "Should have one row");

    let obj = json_objects[0].as_object().unwrap();

    // Check int8_col - BIGINT should map to i64 and then to Number
    let int8_val = obj.get("int8_col").unwrap();
    assert_eq!(
        int8_val,
        &serde_json::json!(9223372036854775807i64),
        "int8_col should be the maximum i64 value"
    );
}

#[tokio::test]
async fn test_map_rows_to_json_data_comprehensive_column_types() {
    use jankensqlhub::runner_postgresql::map_rows_to_json_data;

    let Some(client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let test_table = "test_comprehensive_types";

    // Clean up and create test table
    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {test_table}"), &[])
        .await;

    client
        .execute(
            &format!(
                r#"
            CREATE TABLE {test_table} (
                id SERIAL PRIMARY KEY,
                bool_col BOOLEAN,
                int_col INTEGER,
                float32_col REAL,
                float64_col DOUBLE PRECISION,
                text_col TEXT,
                bytea_col BYTEA
            )
            "#
            ),
            &[],
        )
        .await
        .expect("Failed to create test table");

    // Insert test data with supported postgres types (exclude JSON/JSONB to avoid complex setup)
    client
        .execute(
            &format!("INSERT INTO {test_table} (bool_col, int_col, float32_col, float64_col, text_col, bytea_col) VALUES ($1, $2, $3, $4, $5, $6)"),
            &[&true, &42, &42.5f32, &123.456, &"Hello World", &vec![1u8, 2u8, 3u8, 255u8]],
        )
        .await
        .expect("Failed to insert test data");

    // Query the data using raw PostgreSQL to get actual Row objects
    let rows = client
        .query(
            &format!(
                "SELECT id, bool_col, int_col, float32_col, float64_col, text_col, bytea_col FROM {test_table} ORDER BY id"
            ),
            &[],
        )
        .await
        .expect("Failed to query data");

    // Test the map_rows_to_json_data function directly - note we're requesting one extra field that doesn't exist
    let field_names = vec![
        "id".to_string(),
        "bool_col".to_string(),
        "int_col".to_string(),
        "float32_col".to_string(),
        "float64_col".to_string(),
        "text_col".to_string(),
        "bytea_col".to_string(),
        "nonexistent_col".to_string(), // This field doesn't exist in the row
    ];

    let result = map_rows_to_json_data(rows, &field_names);

    let json_objects = result.unwrap();
    assert_eq!(json_objects.len(), 1, "Should have one row");

    let first_obj = &json_objects[0];
    assert!(first_obj.is_object(), "Result should be a JSON object");

    let obj = first_obj.as_object().unwrap();

    // Check id field (should be some positive integer from SERIAL)
    assert!(obj.contains_key("id"), "Should contain id field");
    let id_val = obj.get("id").unwrap();
    assert!(id_val.is_number(), "id should be a number");

    // Check bool_col
    assert_eq!(
        obj.get("bool_col"),
        Some(&serde_json::json!(true)),
        "bool_col should be true"
    );

    // Check int_col
    assert_eq!(
        obj.get("int_col"),
        Some(&serde_json::json!(42)),
        "int_col should be 42"
    );

    // Check float32_col (REAL/float4 becomes f64, should be close to 42.5)
    let float32_val = obj.get("float32_col").unwrap();
    let float32_num = float32_val.as_f64().unwrap();
    assert!(
        (float32_num - 42.5).abs() < 0.001,
        "float32_col should be approximately 42.5, got {float32_num}"
    );

    // Check float64_col (DOUBLE PRECISION/float8)
    let float64_val = obj.get("float64_col").unwrap();
    let float64_num = float64_val.as_f64().unwrap();
    assert!(
        (float64_num - 123.456).abs() < 0.001,
        "float64_col should be approximately 123.456, got {float64_num}"
    );

    // Check text_col
    assert_eq!(
        obj.get("text_col"),
        Some(&serde_json::json!("Hello World")),
        "text_col should be 'Hello World'"
    );

    // Check bytea_col - should be array of numbers
    let bytea_val = obj.get("bytea_col").unwrap();
    assert!(bytea_val.is_array(), "bytea_col should be an array");
    let bytea_arr = bytea_val.as_array().unwrap();
    assert_eq!(bytea_arr.len(), 4, "bytea array should have 4 elements");
    assert_eq!(bytea_arr[0], serde_json::json!(1), "First byte should be 1");
    assert_eq!(
        bytea_arr[1],
        serde_json::json!(2),
        "Second byte should be 2"
    );
    assert_eq!(bytea_arr[2], serde_json::json!(3), "Third byte should be 3");
    assert_eq!(
        bytea_arr[3],
        serde_json::json!(255),
        "Fourth byte should be 255"
    );

    // Check nonexistent_col - should be null for missing column (covers the None case)
    assert_eq!(
        obj.get("nonexistent_col"),
        Some(&serde_json::json!(null)),
        "nonexistent_col should be null for missing columns"
    );
}

/// Integration test for unsupported type fallback behavior
#[tokio::test]
async fn test_postgres_unsupported_type_fallback() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let test_table = "test_unsupported_type_fallback";

    // Clean up and create test table with an unsupported type (TIMESTAMPTZ)
    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {test_table}"), &[])
        .await;

    client
        .execute(
            &format!(
                r#"
            CREATE TABLE {test_table} (
                id SERIAL PRIMARY KEY,
                timestamp_col TIMESTAMPTZ
            )
            "#
            ),
            &[],
        )
        .await
        .expect("Failed to create test table");

    // Insert test data with a timestamp with timezone (use PostgreSQL's NOW() function)
    client
        .execute(
            &format!("INSERT INTO {test_table} (timestamp_col) VALUES (NOW())"),
            &[],
        )
        .await
        .expect("Failed to insert test data");

    let json_definitions = serde_json::json!({
        "select_timestamp": {
            "query": format!("SELECT id, timestamp_col FROM {test_table} ORDER BY id"),
            "returns": ["id", "timestamp_col"],
            "args": {}
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    // Execute the query and verify timestamp is converted to string via unsupported type fallback
    let params = serde_json::json!({});
    let result = query_run_postgresql(&mut client, &queries, "select_timestamp", &params)
        .await
        .unwrap();

    assert_eq!(result.data.len(), 1, "Should have one row");

    let row = &result.data[0];
    let obj = row.as_object().unwrap();

    // Check that the timestamp was handled via the unsupported type fallback (converted to string)
    let timestamp_val = obj.get("timestamp_col").unwrap();

    assert!(
        timestamp_val.is_string(),
        "TIMESTAMPTZ (unsupported type OID) should be converted to JSON string via fallback, got: {timestamp_val:?}"
    );

    // The string should be the marker indicating unsupported type with OID
    let timestamp_str = timestamp_val.as_str().unwrap();
    assert!(
        timestamp_str.starts_with("Unsupported PostgreSQL type OID:"),
        "String should indicate unsupported type, got: {timestamp_str}"
    );
}
