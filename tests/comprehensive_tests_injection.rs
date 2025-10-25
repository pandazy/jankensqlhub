use jankensqlhub::{
    JankenError, M_EXPECTED, M_GOT, QueryDefinitions, error_meta, query_run_sqlite,
};
use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE source (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, score REAL, active BOOLEAN)",
        [],
    )
    .unwrap();
    conn
}

#[test]
fn test_sql_injection_protection_name_parameter() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let mut conn = setup_db();

    let sql_injection_attempt = "'; DROP TABLE source; --";

    let params = serde_json::json!({"name": "TestUser"});
    query_run_sqlite(&mut conn, &queries, "insert_single", &params).unwrap();

    let initial_count = query_run_sqlite(&mut conn, &queries, "select_all", &serde_json::json!({}))
        .unwrap()
        .data
        .len();

    let params = serde_json::json!({"name": sql_injection_attempt});
    query_run_sqlite(&mut conn, &queries, "insert_single", &params).unwrap();

    let params = serde_json::json!({"id": 1, "name": "TestUser", "source": "source"});
    let result = query_run_sqlite(&mut conn, &queries, "my_list", &params).unwrap();
    assert!(!result.data.is_empty());

    let final_result =
        query_run_sqlite(&mut conn, &queries, "select_all", &serde_json::json!({})).unwrap();
    assert_eq!(final_result.data.len(), initial_count + 1);
}

#[test]
fn test_sql_injection_protection_id_parameter() {
    let json_definitions = serde_json::json!({
        "insert_with_params": {
            "query": "INSERT INTO source (id, name) VALUES (@id, @name)",
            "args": { "id": {"type": "integer"}, "name": {"type": "string"} }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    let injection_id = "1 OR 1=1";
    let params = serde_json::json!({"id": injection_id, "name": "injection_test"});
    let result = query_run_sqlite(&mut conn, &queries, "insert_with_params", &params);
    assert!(result.is_err());
}

#[test]
fn test_sql_injection_protection_safe_name_parameter() {
    let json_definitions = serde_json::json!({
        "insert_with_params": {
            "query": "INSERT INTO source (id, name) VALUES (@id, @name)",
            "args": { "id": {"type": "integer"}, "name": {"type": "string"} }
        },
        "select_by_id": {
            "query": "SELECT id FROM source WHERE id=@id",
            "returns": ["id"],
            "args": { "id": {"type": "integer"} }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    let injection_name = "'; DROP TABLE source; --";

    let params = serde_json::json!({"id": 100, "name": injection_name});
    let result = query_run_sqlite(&mut conn, &queries, "insert_with_params", &params).unwrap();
    assert!(result.data.is_empty());

    let params = serde_json::json!({"id": 100});
    let result = query_run_sqlite(&mut conn, &queries, "select_by_id", &params).unwrap();
    assert_eq!(result.data.len(), 1);
}

#[test]
fn test_sql_injection_protection_list_parameters() {
    // Create queries with proper constraints for list parameters
    let json_definitions = serde_json::json!({
        "safe_int_list": {
            "query": "SELECT * FROM source WHERE id IN :[targets]",
            "returns": ["id", "name", "score"],
            "args": {
                "targets": { "itemtype": "integer" }
            }
        },
        "safe_string_list": {
            "query": "SELECT * FROM source WHERE name IN :[names]",
            "returns": ["id", "name", "score"],
            "args": {
                "names": { "itemtype": "string" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Create test table with safe data
    let mut conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'Alice', 95.0, 1)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (5, 'Bob', 87.5, 0)", [])
        .unwrap();

    // Test that safe integer list works
    let params = serde_json::json!({"targets": [1, 5]});
    let result = query_run_sqlite(&mut conn, &queries, "safe_int_list", &params).unwrap();
    assert_eq!(result.data.len(), 2);

    // Test SQL injection attempt through integer list - invalid type should fail
    let params = serde_json::json!({"targets": ["1'; DROP TABLE source; --", 5]});
    let err = query_run_sqlite(&mut conn, &queries, "safe_int_list", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "integer at index 0");
        assert_eq!(got, "\"1'; DROP TABLE source; --\"");
    } else {
        panic!("Expected ParameterTypeMismatch, got: {err_str}");
    }

    // Test SQL injection attempt through string list
    let params = serde_json::json!({"names": ["Alice'; DROP TABLE source; --", "Bob"]});
    let result = query_run_sqlite(&mut conn, &queries, "safe_string_list", &params).unwrap();
    // SQL injection should be blocked by prepared statements - no rows should match the malicious name
    assert_eq!(result.data.len(), 1); // Only "Bob" should match

    // Test that safe string values work in list
    let params = serde_json::json!({"names": ["Alice", "Bob"]});
    let result = query_run_sqlite(&mut conn, &queries, "safe_string_list", &params).unwrap();
    assert_eq!(result.data.len(), 2);

    // Verify table still exists and data is safe
    let params = serde_json::json!({"targets": [1, 5]});
    let result = query_run_sqlite(&mut conn, &queries, "safe_int_list", &params).unwrap();
    assert_eq!(result.data.len(), 2);
}
