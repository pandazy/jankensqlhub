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
fn test_parameter_validation_enum() {
    let json_definitions = serde_json::json!({
        "select_with_enum_string": {
            "query": "select * from source where name=@status",
            "returns": ["id", "name", "score"],
            "args": {
                "status": { "enum": ["active", "inactive", "pending"] }
            }
        },
        "select_with_enum_int": {
            "query": "select * from source where id=@level",
            "returns": ["id", "name", "score"],
            "args": {
                "level": {
                    "type": "integer",
                    "enum": [1, 2, 3, 4, 5]
                }
            }
        },
        "select_with_enum_table": {
            "query": "SELECT * FROM #[table_name]",
            "returns": ["id", "name", "score"],
            "args": {
                "table_name": {
                    "enum": ["users", "products", "orders"]
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'active', NULL)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (3, 'test', NULL)", [])
        .unwrap();

    // Test string enum - should work
    let params = serde_json::json!({"status": "active"});
    let result = query_run_sqlite(&mut conn, &queries, "select_with_enum_string", &params);
    assert!(result.is_ok());

    let params = serde_json::json!({"status": "unknown"});
    let err =
        query_run_sqlite(&mut conn, &queries, "select_with_enum_string", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert!(expected.contains("active"));
            assert!(expected.contains("inactive"));
            assert!(expected.contains("pending"));
            assert_eq!(got, "\"unknown\"");
        }
        _ => panic!("Wrong error type: {err:?}"),
    }

    // Test integer enum - should work
    let params = serde_json::json!({"level": 3});
    let result = query_run_sqlite(&mut conn, &queries, "select_with_enum_int", &params);
    assert!(result.is_ok());

    let params = serde_json::json!({"level": 10}); // Not in enum [1,2,3,4,5]
    let err = query_run_sqlite(&mut conn, &queries, "select_with_enum_int", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert!(expected.contains("1"));
            assert!(expected.contains("2"));
            assert!(expected.contains("3"));
            assert!(expected.contains("4"));
            assert!(expected.contains("5"));
            assert_eq!(got, "10");
        }
        _ => panic!("Wrong error type: {err:?}"),
    }

    // Test table_name enum - should validate enum values but also pass normal table_name validation
    let mut conn2 = Connection::open_in_memory().unwrap();
    conn2
        .execute(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, score REAL)",
            [],
        )
        .unwrap();
    conn2
        .execute("INSERT INTO users VALUES (1, 'John', 95.0)", [])
        .unwrap();

    // Should work with enum value that's a valid table name
    let params = serde_json::json!({"table_name": "users"});
    let result = query_run_sqlite(&mut conn2, &queries, "select_with_enum_table", &params);
    assert!(result.is_ok());

    // Should fail with enum value that's not in the allowed list
    let params = serde_json::json!({"table_name": "admin"}); // "admin" is not in enum ["users", "products", "orders"]
    let err =
        query_run_sqlite(&mut conn2, &queries, "select_with_enum_table", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert!(expected.contains("users"));
            assert!(expected.contains("products"));
            assert!(expected.contains("orders"));
            assert_eq!(got, "\"admin\"");
        }
        _ => panic!("Wrong error type: {err:?}"),
    }
}
