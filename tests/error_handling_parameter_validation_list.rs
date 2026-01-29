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
fn test_list_parameter_constraint_validation_errors() {
    // Test list parameter constraint validation errors

    // Test list with item_type integer - validation errors for incorrect item types
    let json_definitions = serde_json::json!({
        "list_int_constraints": {
            "query": "SELECT * FROM source WHERE id IN :[ints]",
            "args": {
                "ints": { "itemtype": "integer" }
            }
        },
        "list_string_pattern": {
            "query": "SELECT * FROM source WHERE name IN :[names]",
            "args": {
                "names": { "itemtype": "string", "pattern": "^[A-Z][a-z]+$" }
            }
        },
        "list_float_range": {
            "query": "SELECT * FROM source WHERE score IN :[scores]",
            "args": {
                "scores": { "itemtype": "float", "range": [0.0, 100.0] }
            }
        },
        "list_enum": {
            "query": "SELECT * FROM source WHERE status IN :[statuses]",
            "args": {
                "statuses": { "itemtype": "string", "enum": ["active", "inactive", "pending"] }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    // Test list with mixed item types - should fail at first invalid item (index 1)
    let params = serde_json::json!({"ints": [1, "invalid_string", 3.0, true]});
    let err = query_run_sqlite(&mut conn, &queries, "list_int_constraints", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "integer at index 1");
        assert_eq!(got, "\"invalid_string\"");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid list item type, got: {err_str}");
    }

    // Test list with string pattern - invalid names should fail
    let params = serde_json::json!({"names": ["Alice", "lowercase_name", "123invalid"]});
    let err = query_run_sqlite(&mut conn, &queries, "list_string_pattern", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("string matching pattern"));
        assert_eq!(got, "lowercase_name");
    } else {
        panic!("Expected ParameterTypeMismatch for pattern validation, got: {err_str}");
    }

    // Test list with float range - out of range values should fail
    let params = serde_json::json!({"scores": [85.5, -5.0, 150.5, 92.0]});
    let err = query_run_sqlite(&mut conn, &queries, "list_float_range", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("value between 0 and 100"));
        assert_eq!(got, "-5");
    } else {
        panic!("Expected ParameterTypeMismatch for range validation, got: {err_str}");
    }

    // Test list with enum - invalid enum values should fail
    let params = serde_json::json!({"statuses": ["active", "unknown_status", "pending"]});
    let err = query_run_sqlite(&mut conn, &queries, "list_enum", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("active"));
        assert!(expected.contains("inactive"));
        assert!(expected.contains("pending"));
        assert_eq!(got, "\"unknown_status\"");
    } else {
        panic!("Expected ParameterTypeMismatch for enum validation, got: {err_str}");
    }

    // Test empty list validation (should fail at runner level, not constraint level)
    let params = serde_json::json!({"ints": []});
    let err = query_run_sqlite(&mut conn, &queries, "list_int_constraints", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "non-empty list");
        assert_eq!(got, "empty array");
    } else {
        panic!("Expected ParameterTypeMismatch for empty list, got: {err_str}");
    }

    // Test list with wrong basic type (pass non-array)
    let params = serde_json::json!({"ints": "not_an_array"});
    let err = query_run_sqlite(&mut conn, &queries, "list_int_constraints", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "list");
        assert_eq!(got, "\"not_an_array\"");
    } else {
        panic!("Expected ParameterTypeMismatch for non-array list, got: {err_str}");
    }
}

#[test]
fn test_list_parameter_enumif_validation_errors() {
    // Test list parameter with enumif constraint validation errors
    // enumif allows list item values to depend on another parameter's value

    let json_definitions = serde_json::json!({
        "list_enumif_string_condition": {
            "query": "SELECT * FROM media WHERE type=@media_type AND sources IN :[sources]",
            "args": {
                "media_type": { "enum": ["song", "show"] },
                "sources": {
                    "itemtype": "string",
                    "enumif": {
                        "media_type": {
                            "song": ["artist", "album", "title"],
                            "show": ["channel", "category", "episodes"]
                        }
                    }
                }
            }
        },
        "list_enumif_number_condition": {
            "query": "SELECT * FROM metrics WHERE level=@level AND severities IN :[severities]",
            "args": {
                "level": { "type": "integer" },
                "severities": {
                    "itemtype": "string",
                    "enumif": {
                        "level": {
                            "0": ["info", "debug"],
                            "1": ["warning"],
                            "2": ["error", "critical"]
                        }
                    }
                }
            }
        },
        "list_enumif_boolean_condition": {
            "query": "SELECT * FROM users WHERE is_admin=@is_admin AND permissions IN :[permissions]",
            "args": {
                "is_admin": { "type": "boolean" },
                "permissions": {
                    "itemtype": "string",
                    "enumif": {
                        "is_admin": {
                            "true": ["read", "write", "admin", "delete"],
                            "false": ["read", "write"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    // Additional tables for enumif tests
    conn.execute(
        "CREATE TABLE media (id INTEGER, type TEXT, sources TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE metrics (id INTEGER, level INTEGER, severities TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER, is_admin BOOLEAN, permissions TEXT)",
        [],
    )
    .unwrap();

    // Test list with enumif - string conditional parameter
    // Valid case: all items match the condition
    let params = serde_json::json!({"media_type": "song", "sources": ["artist", "album", "title"]});
    let result = query_run_sqlite(&mut conn, &queries, "list_enumif_string_condition", &params);
    assert!(result.is_ok(), "All valid enumif values should work");

    // Invalid case: list contains value not allowed for "song"
    let params =
        serde_json::json!({"media_type": "song", "sources": ["artist", "channel", "album"]});
    let err =
        query_run_sqlite(&mut conn, &queries, "list_enumif_string_condition", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("artist"));
        assert!(expected.contains("album"));
        assert!(expected.contains("title"));
        assert_eq!(got, "\"channel\"");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid enumif list item, got: {err_str}");
    }

    // Test with different conditional value
    let params = serde_json::json!({"media_type": "show", "sources": ["channel", "episodes"]});
    let result = query_run_sqlite(&mut conn, &queries, "list_enumif_string_condition", &params);
    assert!(result.is_ok(), "Valid show sources should work");

    // Invalid case: using song values with show type
    let params = serde_json::json!({"media_type": "show", "sources": ["channel", "artist"]});
    let err =
        query_run_sqlite(&mut conn, &queries, "list_enumif_string_condition", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("channel"));
        assert!(expected.contains("category"));
        assert!(expected.contains("episodes"));
        assert_eq!(got, "\"artist\"");
    } else {
        panic!("Expected ParameterTypeMismatch for wrong conditional context, got: {err_str}");
    }

    // Test list with enumif - number conditional parameter
    let params = serde_json::json!({"level": 0, "severities": ["info", "debug"]});
    let result = query_run_sqlite(&mut conn, &queries, "list_enumif_number_condition", &params);
    assert!(result.is_ok(), "Level 0 with info/debug should work");

    // Invalid case: warning not allowed for level 0
    let params = serde_json::json!({"level": 0, "severities": ["info", "warning"]});
    let err =
        query_run_sqlite(&mut conn, &queries, "list_enumif_number_condition", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("info"));
        assert!(expected.contains("debug"));
        assert_eq!(got, "\"warning\"");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid number condition, got: {err_str}");
    }

    // Test with undefined condition value
    let params = serde_json::json!({"level": 5, "severities": ["info"]});
    let err =
        query_run_sqlite(&mut conn, &queries, "list_enumif_number_condition", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(
            expected,
            "conditional parameter value that matches a defined condition"
        );
        assert!(
            got.contains("value not covered by any enumif condition for parameter severities"),
            "got: {}",
            got
        );
    } else {
        panic!("Expected ParameterTypeMismatch for undefined condition, got: {err_str}");
    }

    // Test list with enumif - boolean conditional parameter
    let params = serde_json::json!({"is_admin": true, "permissions": ["read", "write", "admin"]});
    let result = query_run_sqlite(
        &mut conn,
        &queries,
        "list_enumif_boolean_condition",
        &params,
    );
    assert!(result.is_ok(), "Admin with all permissions should work");

    // Invalid case: non-admin trying to use admin permission
    let params = serde_json::json!({"is_admin": false, "permissions": ["read", "admin"]});
    let err = query_run_sqlite(
        &mut conn,
        &queries,
        "list_enumif_boolean_condition",
        &params,
    )
    .unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("read"));
        assert!(expected.contains("write"));
        assert!(!expected.contains("admin"));
        assert_eq!(got, "\"admin\"");
    } else {
        panic!("Expected ParameterTypeMismatch for boolean condition violation, got: {err_str}");
    }

    // Test empty list with enumif (should fail at runner level, not enumif validation)
    let params = serde_json::json!({"media_type": "song", "sources": []});
    let err =
        query_run_sqlite(&mut conn, &queries, "list_enumif_string_condition", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "non-empty list");
        assert_eq!(got, "empty array");
    } else {
        panic!("Expected ParameterTypeMismatch for empty list, got: {err_str}");
    }
}
