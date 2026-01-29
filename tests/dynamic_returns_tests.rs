use jankensqlhub::{
    M_EXPECTED, M_GOT, QueryDefinitions, error_meta, get_error_data, query_run_sqlite,
};
use rusqlite::Connection;
use serde_json::json;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER, name TEXT, email TEXT, age INTEGER)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO users VALUES (1, 'Alice', 'alice@test.com', 25), (2, 'Bob', 'bob@test.com', 30)", []).unwrap();
    conn
}

#[test]
fn test_dynamic_returns_basic() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "select_dynamic": {
            "query": "SELECT ~[fields] FROM users WHERE id = 1",
            "returns": "~[fields]",
            "args": {
                "fields": {"enum": ["name", "email", "age"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({"fields": ["name", "email"]});
    let result = query_run_sqlite(&mut conn, &queries, "select_dynamic", &params).unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], "Alice");
    assert_eq!(result.data[0]["email"], "alice@test.com");
    // Age should not be in the result
    assert!(!result.data[0].as_object().unwrap().contains_key("age"));
}

#[test]
fn test_dynamic_returns_all_fields() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "select_all_dynamic": {
            "query": "SELECT ~[fields] FROM users",
            "returns": "~[fields]",
            "args": {
                "fields": {"enum": ["id", "name", "email", "age"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({"fields": ["id", "name", "email", "age"]});
    let result = query_run_sqlite(&mut conn, &queries, "select_all_dynamic", &params).unwrap();

    assert_eq!(result.data.len(), 2);
    assert_eq!(result.data[0]["id"], 1);
    assert_eq!(result.data[0]["name"], "Alice");
    assert_eq!(result.data[0]["email"], "alice@test.com");
    assert_eq!(result.data[0]["age"], 25);
}

#[test]
fn test_dynamic_returns_single_field() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "select_single": {
            "query": "SELECT ~[fields] FROM users",
            "returns": "~[fields]"
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({"fields": ["name"]});
    let result = query_run_sqlite(&mut conn, &queries, "select_single", &params).unwrap();

    assert_eq!(result.data.len(), 2);
    assert_eq!(result.data[0]["name"], "Alice");
    assert_eq!(result.data[1]["name"], "Bob");
    assert_eq!(result.data[0].as_object().unwrap().len(), 1);
}

#[test]
fn test_dynamic_returns_with_filter() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "select_filtered": {
            "query": "SELECT ~[fields] FROM users WHERE age > @min_age",
            "returns": "~[fields]",
            "args": {
                "fields": {"enum": ["name", "age"]},
                "min_age": {"type": "integer"}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({
        "fields": ["name", "age"],
        "min_age": 26
    });
    let result = query_run_sqlite(&mut conn, &queries, "select_filtered", &params).unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], "Bob");
    assert_eq!(result.data[0]["age"], 30);
}

#[test]
fn test_dynamic_returns_error_not_comma_list() {
    // Test that returns reference must point to a comma_list parameter
    let json_definitions = json!({
        "invalid_ref": {
            "query": "SELECT name FROM users WHERE id = @id",
            "returns": "~[id]",
            "args": {
                "id": {"type": "integer"}
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("comma_list parameter"),
            "Expected error about comma_list parameter, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_dynamic_returns_error_param_not_found() {
    // Test that returns reference must point to an existing parameter
    let json_definitions = json!({
        "param_not_found": {
            "query": "SELECT name FROM users",
            "returns": "~[non_existent]"
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let got = error_meta(data, M_GOT).unwrap();
        assert!(
            got.contains("not found"),
            "Expected error about parameter not found, got: {}",
            got
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_dynamic_returns_error_invalid_format() {
    // Test that returns string must be in ~[param] format
    let json_definitions = json!({
        "invalid_format": {
            "query": "SELECT name FROM users",
            "returns": "not_valid_format"
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("~[param_name]"),
            "Expected error about ~[param_name] format, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_dynamic_returns_error_extra_characters() {
    // Test that returns string must be exactly ~[param], no extra characters
    let json_definitions = json!({
        "extra_chars": {
            "query": "SELECT ~[fields] FROM users",
            "returns": "~[fields] extra",
            "args": {
                "fields": {"enum": ["name"]}
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("~[param_name]"),
            "Expected error about ~[param_name] format, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_static_returns_still_works() {
    // Ensure static array returns still work
    let mut conn = setup_db();

    let json_definitions = json!({
        "static_returns": {
            "query": "SELECT id, name, email FROM users WHERE id = 1",
            "returns": ["name", "email"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({});
    let result = query_run_sqlite(&mut conn, &queries, "static_returns", &params).unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], "Alice");
    assert_eq!(result.data[0]["email"], "alice@test.com");
}

#[test]
fn test_empty_returns_still_works() {
    // Ensure empty returns (mutation queries) still work
    let mut conn = setup_db();

    let json_definitions = json!({
        "insert_user": {
            "query": "INSERT INTO users (name, email) VALUES (@name, @email)",
            "args": {
                "name": {"type": "string"},
                "email": {"type": "string"}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({
        "name": "Charlie",
        "email": "charlie@test.com"
    });
    let result = query_run_sqlite(&mut conn, &queries, "insert_user", &params).unwrap();

    assert_eq!(result.data.len(), 0);
    assert!(!result.sql_statements.is_empty());
}

#[test]
fn test_dynamic_returns_different_param_values() {
    // Test that different parameter values produce different result structures
    let mut conn = setup_db();

    let json_definitions = json!({
        "flexible_select": {
            "query": "SELECT ~[fields] FROM users WHERE id = 1",
            "returns": "~[fields]",
            "args": {
                "fields": {"enum": ["id", "name", "email", "age"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // First request: only name
    let params1 = json!({"fields": ["name"]});
    let result1 = query_run_sqlite(&mut conn, &queries, "flexible_select", &params1).unwrap();
    assert_eq!(result1.data[0].as_object().unwrap().len(), 1);
    assert!(result1.data[0].as_object().unwrap().contains_key("name"));

    // Second request: name and email
    let params2 = json!({"fields": ["name", "email"]});
    let result2 = query_run_sqlite(&mut conn, &queries, "flexible_select", &params2).unwrap();
    assert_eq!(result2.data[0].as_object().unwrap().len(), 2);
    assert!(result2.data[0].as_object().unwrap().contains_key("name"));
    assert!(result2.data[0].as_object().unwrap().contains_key("email"));

    // Third request: all fields
    let params3 = json!({"fields": ["id", "name", "email", "age"]});
    let result3 = query_run_sqlite(&mut conn, &queries, "flexible_select", &params3).unwrap();
    assert_eq!(result3.data[0].as_object().unwrap().len(), 4);
}

#[test]
fn test_dynamic_returns_with_constraint_validation() {
    // Test that enum constraints on the comma_list parameter are enforced
    let mut conn = setup_db();

    let json_definitions = json!({
        "constrained_select": {
            "query": "SELECT ~[fields] FROM users",
            "returns": "~[fields]",
            "args": {
                "fields": {"enum": ["name", "email"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Valid fields
    let params = json!({"fields": ["name", "email"]});
    let result = query_run_sqlite(&mut conn, &queries, "constrained_select", &params);
    assert!(result.is_ok());

    // Invalid field
    let params_invalid = json!({"fields": ["name", "age"]});
    let result_invalid =
        query_run_sqlite(&mut conn, &queries, "constrained_select", &params_invalid);
    assert!(result_invalid.is_err());
}

#[test]
fn test_dynamic_returns_runtime_missing_param() {
    // Test that if the comma_list parameter is not provided at runtime, we get an error
    let mut conn = setup_db();

    let json_definitions = json!({
        "missing_param": {
            "query": "SELECT ~[fields] FROM users",
            "returns": "~[fields]"
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({});

    let result = query_run_sqlite(&mut conn, &queries, "missing_param", &params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let param_name = error_meta(data, "parameter_name").unwrap();
        assert_eq!(param_name, "fields");
    } else {
        panic!("Expected JankenError for missing parameter, got: {:?}", err);
    }
}
