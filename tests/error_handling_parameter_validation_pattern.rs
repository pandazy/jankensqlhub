use jankensqlhub::{
    JankenError, M_EXPECTED, M_GOT, QueryDefinitions, error_meta, query_run_sqlite,
};
use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE source (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, score REAL)",
        [],
    )
    .unwrap();
    conn
}

#[test]
fn test_parameter_validation_pattern() {
    let json_definitions = serde_json::json!({
        "email_query": {
            "query": "select * from source where name = @email",
            "returns": ["id", "name", "score"],
            "args": {
                "email": {
                    "type": "string",
                    "pattern": "\\S+@\\S+\\.\\S+"
                }
            }
        },
        "phone_query": {
            "query": "select * from source where name = @phone",
            "returns": ["id", "name", "score"],
            "args": {
                "phone": {
                    "type": "string",
                    "pattern": "\\d{3}-\\d{3}-\\d{4}"
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();
    conn.execute(
        "INSERT INTO source VALUES (1, 'test@example.com', NULL)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO source VALUES (2, '555-123-4567', NULL)", [])
        .unwrap();

    let params = serde_json::json!({"email": "user@domain.com"});
    let result = query_run_sqlite(&mut conn, &queries, "email_query", &params);
    assert!(result.is_ok());

    let params = serde_json::json!({"phone": "555-123-4567"});
    let result = query_run_sqlite(&mut conn, &queries, "phone_query", &params);
    assert!(result.is_ok());

    let params = serde_json::json!({"email": "invalid-email"});
    let err = query_run_sqlite(&mut conn, &queries, "email_query", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("string matching pattern"));
        assert_eq!(got, "invalid-email");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid email pattern, got: {err_str}");
    }

    let params = serde_json::json!({"phone": "invalid-phone"});
    let err = query_run_sqlite(&mut conn, &queries, "phone_query", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("string matching pattern"));
        assert_eq!(got, "invalid-phone");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid phone pattern, got: {err_str}");
    }
}

#[test]
fn test_parameter_validation_pattern_non_string() {
    // Test that pattern validation fails for non-string parameter types
    let json_definitions = serde_json::json!({
        "select_with_pattern_int": {
            "query": "select * from source where id = @id",
            "args": {
                "id": {
                    "type": "integer",
                    "pattern": "\\d+"  // Pattern constraint on integer type should fail (expects string)
                }
            }
        },
        "select_with_pattern_bool": {
            "query": "select * from source where id = @active",
            "args": {
                "active": {
                    "type": "boolean",
                    "pattern": "true|false"  // Pattern constraint on boolean type should fail (expects string)
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();
    conn.execute("INSERT INTO source VALUES (123, 'test', NULL)", [])
        .unwrap();

    // Integer parameter with pattern constraint should fail with "string" error
    let params = serde_json::json!({"id": 123});
    let err =
        query_run_sqlite(&mut conn, &queries, "select_with_pattern_int", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "string");
        assert_eq!(got, "123");
    } else {
        panic!("Expected ParameterTypeMismatch for non-string pattern validation, got: {err_str}")
    }

    // Boolean parameter with pattern constraint should fail with "string" error
    let params = serde_json::json!({"active": true});
    let err =
        query_run_sqlite(&mut conn, &queries, "select_with_pattern_bool", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "string");
        assert_eq!(got, "true");
    } else {
        panic!("Expected ParameterTypeMismatch for non-string pattern validation, got: {err_str}")
    }
}

#[test]
fn test_parameter_validation_pattern_table_name() {
    // Test that pattern validation works for table_name parameters
    let json_definitions = serde_json::json!({
        "table_pattern_query": {
            "query": "SELECT * FROM #[table_name]",
            "returns": ["id", "name", "score"],
            "args": {
                "table_name": {
                    "pattern": "^test_\\w+$"  // Must start with "test_" followed by word characters
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE test_users (id INTEGER PRIMARY KEY, name TEXT, score REAL)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE test_products (id INTEGER PRIMARY KEY, name TEXT, score REAL)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO test_users VALUES (1, 'user1', 85.0)", [])
        .unwrap();
    conn.execute("INSERT INTO test_products VALUES (2, 'product1', 95.5)", [])
        .unwrap();

    // Test valid table names that match the pattern
    let valid_params = vec![
        serde_json::json!({"table_name": "test_users"}),
        serde_json::json!({"table_name": "test_products"}),
    ];

    for params in valid_params {
        let result = query_run_sqlite(&mut conn, &queries, "table_pattern_query", &params);
        assert!(result.is_ok(), "Expected success for valid pattern match");
    }

    // Test invalid table names that don't match the pattern
    let invalid_cases = vec![
        (serde_json::json!({"table_name": "users"}), "users"), // Doesn't start with "test_"
        (
            serde_json::json!({"table_name": "test-table"}),
            "test-table",
        ), // Contains dash (not word char)
        (
            serde_json::json!({"table_name": "test users"}),
            "test users",
        ), // Contains space (not word char)
        (
            serde_json::json!({"table_name": "prefix_test"}),
            "prefix_test",
        ), // Wrong prefix
    ];

    for (params, expected_got) in invalid_cases {
        let err =
            query_run_sqlite(&mut conn, &queries, "table_pattern_query", &params).unwrap_err();
        let err_str = format!("{err:?}");
        if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert!(
                expected.contains("string matching pattern"),
                "Expected pattern validation error"
            );
            assert_eq!(got, expected_got);
        } else {
            panic!("Expected ParameterTypeMismatch for invalid pattern, got: {err_str}");
        }
    }
}
