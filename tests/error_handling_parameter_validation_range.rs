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
fn test_parameter_validation_range() {
    let json_definitions = serde_json::json!({
        "select_with_range": {
            "query": "select * from source where id=@id",
            "returns": ["id", "name", "score"],
            "args": {
                "id": {
                    "type": "integer",
                    "range": [1, 100]
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();
    conn.execute("INSERT INTO source VALUES (50, 'Test', NULL)", [])
        .unwrap();

    let params = serde_json::json!({"id": 50});
    let result = query_run_sqlite(&mut conn, &queries, "select_with_range", &params);
    assert!(result.is_ok());

    let params = serde_json::json!({"id": 0});
    let err = query_run_sqlite(&mut conn, &queries, "select_with_range", &params).unwrap_err();
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("between 1 and 100"));
        assert_eq!(got, "0");
    } else {
        panic!("Expected ParameterTypeMismatch error");
    }
}

#[test]
fn test_parameter_validation_range_non_numeric() {
    let json_definitions = serde_json::json!({
        "select_with_range": {
            "query": "select * from source where id=@id",
            "args": {
                "id": {
                    "type": "integer",
                    "range": [1, 100]
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    let cases = vec![
        (serde_json::json!({"id": "not_int"}), "\"not_int\""),
        (serde_json::json!({"id": true}), "true"),
        (serde_json::json!({"id": null}), "null"),
    ];

    for (params, expected_got) in cases {
        let err = query_run_sqlite(&mut conn, &queries, "select_with_range", &params).unwrap_err();
        let err_str = format!("{err:?}");
        if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert_eq!(expected, "integer");
            assert_eq!(got, expected_got);
        } else {
            panic!("Expected ParameterTypeMismatch, got: {err_str}");
        }
    }
}

#[test]
fn test_parameter_validation_range_wrong_type() {
    let json_definitions = serde_json::json!({
        "select_with_range_string": {
            "query": "select * from source where name=@name",
            "args": {
                "name": {
                    "type": "string",
                    "range": [1, 100]
                }
            }
        },
        "select_with_range_bool": {
            "query": "select * from source where id=@id",
            "args": {
                "id": {
                    "type": "boolean",
                    "range": [0, 1]
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'test', NULL)", [])
        .unwrap();

    let params = serde_json::json!({"name": "test"});
    let err =
        query_run_sqlite(&mut conn, &queries, "select_with_range_string", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "numeric type or blob");
        assert_eq!(got, "string");
    } else {
        panic!("Expected ParameterTypeMismatch, got: {err_str}");
    }

    let params = serde_json::json!({"id": true});
    let err = query_run_sqlite(&mut conn, &queries, "select_with_range_bool", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "numeric type or blob");
        assert_eq!(got, "boolean");
    } else {
        panic!("Expected ParameterTypeMismatch, got: {err_str}");
    }
}
