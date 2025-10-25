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
fn test_parameter_type_mismatch() {
    // Create query with args specifying integer type for id
    let json_definitions = serde_json::json!({
        "test_select": {
            "query": "select * from source where id=@id",
            "args": {
                "id": { "type": "integer" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    let params = serde_json::json!({"id": "not_int"}); // id should be integer but got string
    let err = query_run_sqlite(&mut conn, &queries, "test_select", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "integer");
        assert_eq!(got, "\"not_int\"");
    } else {
        panic!("Wrong error type: {err_str}");
    }
}

#[test]
fn test_invalid_parameter_type_error() {
    // Test that invalid parameter types trigger ParameterTypeMismatch error
    let json_definitions = serde_json::json!({
        "invalid_type_query": {
            "query": "select * from source where id=@id",
            "args": {
                "id": { "type": "invalid_type" }  // Invalid type - not integer, string, float, or boolean
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(
            expected,
            "integer, string, float, boolean, table_name, list or blob"
        );
        assert_eq!(got, "invalid_type");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid parameter type, got: {err_str}");
    }
}

#[test]
fn test_sqlite_type_mismatch_errors() {
    // Test parameter type mismatch errors for all types and non-object request_params
    // This covers error handling in query_run_sqlite for parameter validation

    let json_definitions = serde_json::json!({
        "int_test": {
            "query": "select * from source where id=@id",
            "args": { "id": { "type": "integer" } }
        },
        "str_test": {
            "query": "select * from source where name=@name",
            "args": { "name": { "type": "string" } }
        },
        "float_test": {
            "query": "select * from source where score=@score",
            "args": { "score": { "type": "float" } }
        },
        "bool_test": {
            "query": "select * from source where active=@active",
            "args": { "active": { "type": "boolean" } }
        },
        "table_test": {
            "query": "SELECT * FROM #[table_name]",
            "returns": ["id", "name", "score"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let mut conn = setup_db();
    let request_params_string = serde_json::json!("not an object");
    let result = query_run_sqlite(&mut conn, &queries, "int_test", &request_params_string);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "object");
        assert_eq!(got, "not object");
    } else {
        panic!("Wrong error type: {err_str}");
    }

    let mut conn = setup_db();
    let request_params = serde_json::json!({"id": "not_int"});
    let result = query_run_sqlite(&mut conn, &queries, "int_test", &request_params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "integer");
        assert_eq!(got, "\"not_int\"");
    } else {
        panic!("Expected ParameterTypeMismatch for integer validation, got: {err_str}");
    }

    let mut conn = setup_db();
    let request_params = serde_json::json!({"name": 123});
    let result = query_run_sqlite(&mut conn, &queries, "str_test", &request_params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "string");
        assert_eq!(got, "123");
    } else {
        panic!("Expected ParameterTypeMismatch for string validation, got: {err_str}");
    }

    let mut conn = setup_db();
    let request_params = serde_json::json!({"score": "not_a_number"});
    let result = query_run_sqlite(&mut conn, &queries, "float_test", &request_params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "float");
        assert_eq!(got, "\"not_a_number\"");
    } else {
        panic!("Expected ParameterTypeMismatch for float validation, got: {err_str}");
    }

    let mut conn = setup_db();
    let request_params = serde_json::json!({"active": []});
    let result = query_run_sqlite(&mut conn, &queries, "bool_test", &request_params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "boolean");
        assert_eq!(got, "[]");
    } else {
        panic!("Expected ParameterTypeMismatch for boolean validation with array, got: {err_str}");
    }

    // Test table_name parameter type error that triggers the uncovered Err(_) branch in row processing
    let mut conn = Connection::open_in_memory().unwrap();
    // Create a table with some data
    conn.execute(
        "CREATE TABLE source (id INTEGER PRIMARY KEY, name TEXT, score REAL)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO source VALUES (1, 'test', 1.0)", [])
        .unwrap();

    let request_params = serde_json::json!({"table_name": 123}); // Pass number instead of string for table name
    let result = query_run_sqlite(&mut conn, &queries, "table_test", &request_params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "table_name");
        assert_eq!(got, "123");
    } else {
        panic!("Expected ParameterTypeMismatch for table_name parameter, got: {err_str}");
    }
}
