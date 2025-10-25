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
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert_eq!(expected, "integer at index 1");
            assert_eq!(got, "\"invalid_string\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid list item type, got: {err:?}"),
    }

    // Test list with string pattern - invalid names should fail
    let params = serde_json::json!({"names": ["Alice", "lowercase_name", "123invalid"]});
    let err = query_run_sqlite(&mut conn, &queries, "list_string_pattern", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert!(expected.contains("string matching pattern"));
            assert_eq!(got, "lowercase_name");
        }
        _ => panic!("Expected ParameterTypeMismatch for pattern validation, got: {err:?}"),
    }

    // Test list with float range - out of range values should fail
    let params = serde_json::json!({"scores": [85.5, -5.0, 150.5, 92.0]});
    let err = query_run_sqlite(&mut conn, &queries, "list_float_range", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert!(expected.contains("value between 0 and 100"));
            assert_eq!(got, "-5");
        }
        _ => panic!("Expected ParameterTypeMismatch for range validation, got: {err:?}"),
    }

    // Test list with enum - invalid enum values should fail
    let params = serde_json::json!({"statuses": ["active", "unknown_status", "pending"]});
    let err = query_run_sqlite(&mut conn, &queries, "list_enum", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert!(expected.contains("active"));
            assert!(expected.contains("inactive"));
            assert!(expected.contains("pending"));
            assert_eq!(got, "\"unknown_status\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for enum validation, got: {err:?}"),
    }

    // Test empty list validation (should fail at runner level, not constraint level)
    let params = serde_json::json!({"ints": []});
    let err = query_run_sqlite(&mut conn, &queries, "list_int_constraints", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert_eq!(expected, "non-empty list");
            assert_eq!(got, "empty array");
        }
        _ => panic!("Expected ParameterTypeMismatch for empty list, got: {err:?}"),
    }

    // Test list with wrong basic type (pass non-array)
    let params = serde_json::json!({"ints": "not_an_array"});
    let err = query_run_sqlite(&mut conn, &queries, "list_int_constraints", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert_eq!(expected, "list");
            assert_eq!(got, "\"not_an_array\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for non-array list, got: {err:?}"),
    }
}
