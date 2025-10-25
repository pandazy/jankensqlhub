use jankensqlhub::{JankenError, QueryDefinitions, query_run_sqlite};
use rusqlite::Connection;

#[test]
fn test_blob_parameter_validation() {
    // Test blob parameter with range constraints for size limits

    // Create query with blob parameter and size range constraint
    let json_definitions = serde_json::json!({
        "insert_blob": {
            "query": "INSERT INTO blob_test VALUES (@id, @data)",
            "args": {
                "id": { "type": "integer" },
                "data": { "type": "blob", "range": [1, 100] }  // Blob size must be between 1 and 100 bytes
            }
        },
        "select_blob": {
            "query": "SELECT data FROM blob_test WHERE id=@id",
            "returns": ["data"],
            "args": {
                "id": { "type": "integer" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE blob_test (id INTEGER, data BLOB)", [])
        .unwrap();

    // Test valid blob data - convert text to within size range
    let valid_text = "Hello"; // 5 bytes when UTF-8 encoded
    let valid_blob_data: Vec<serde_json::Value> = valid_text
        .as_bytes()
        .iter()
        .map(|&b| serde_json::json!(b))
        .collect();
    let params = serde_json::json!({"id": 1, "data": valid_blob_data});
    let result = query_run_sqlite(&mut conn, &queries, "insert_blob", &params);
    assert!(result.is_ok(), "Valid blob within size range should work");

    // Test blob that's too small (0 bytes)
    let empty_blob = serde_json::json!([]);
    let params = serde_json::json!({"id": 2, "data": empty_blob});
    let err = query_run_sqlite(&mut conn, &queries, "insert_blob", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("blob size between 1 and 100 bytes"));
            assert_eq!(got, "0 bytes");
        }
        _ => panic!("Expected ParameterTypeMismatch for too small blob, got: {err:?}"),
    }

    // Test blob that's too large (over 100 bytes)
    let large_blob: Vec<u8> = (0..=100).collect(); // 101 bytes
    let large_blob_json: Vec<serde_json::Value> =
        large_blob.iter().map(|&b| serde_json::json!(b)).collect();
    let params = serde_json::json!({"id": 3, "data": large_blob_json});
    let err = query_run_sqlite(&mut conn, &queries, "insert_blob", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("blob size between 1 and 100 bytes"));
            assert_eq!(got, "101 bytes");
        }
        _ => panic!("Expected ParameterTypeMismatch for too large blob, got: {err:?}"),
    }

    // Test invalid blob format - not an array
    let params = serde_json::json!({"id": 4, "data": "not_an_array"});
    let err = query_run_sqlite(&mut conn, &queries, "insert_blob", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "blob");
            assert_eq!(got, "\"not_an_array\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid blob format, got: {err:?}"),
    }

    // Test invalid blob data - array with non-byte values (over 255)
    let invalid_bytes = serde_json::json!([300, 400]); // Values over 255
    let params = serde_json::json!({"id": 5, "data": invalid_bytes});
    let err = query_run_sqlite(&mut conn, &queries, "insert_blob", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("byte values (0-255) at index 0"));
            assert_eq!(got, "300");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid byte values, got: {err:?}"),
    }

    // Test invalid blob data - array with non-numbers
    let invalid_bytes = serde_json::json!(["not", "numbers"]);
    let params = serde_json::json!({"id": 6, "data": invalid_bytes});
    let err = query_run_sqlite(&mut conn, &queries, "insert_blob", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("byte values (0-255) at index 0"));
            assert_eq!(got, "\"not\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for non-number values, got: {err:?}"),
    }

    // Test with more realistic binary data - converting text to UTF-8 bytes
    let text_content = "Hello World! 👋 UTF-8 中文";
    let text_bytes: Vec<u8> = text_content.as_bytes().to_vec();
    let text_bytes_json: Vec<serde_json::Value> =
        text_bytes.iter().map(|&b| serde_json::json!(b)).collect();

    let params = serde_json::json!({"id": 7, "data": text_bytes_json});
    let result = query_run_sqlite(&mut conn, &queries, "insert_blob", &params);
    assert!(result.is_ok(), "UTF-8 text converted to bytes should work");

    // Verify retrieval - should round-trip correctly
    let params = serde_json::json!({"id": 7});
    let result = query_run_sqlite(&mut conn, &queries, "select_blob", &params).unwrap();
    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["data"], serde_json::json!(text_bytes_json));

    // Test that valid blob can be retrieved and is returned as array of bytes
    let params = serde_json::json!({"id": 1});
    let result = query_run_sqlite(&mut conn, &queries, "select_blob", &params).unwrap();
    assert_eq!(result.data.len(), 1);
    // The blob should be returned as an array of numbers representing bytes
    assert_eq!(
        result.data[0]["data"],
        serde_json::json!([72, 101, 108, 108, 111])
    ); // "Hello" bytes
}
