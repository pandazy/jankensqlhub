use jankensqlhub::{
    DatabaseConnection, JankenError, QueryDefinitions, QueryRunner, query_run_sqlite,
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
fn test_query_not_found() {
    // Test QueryNotFound error for non-existent query names
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Load valid queries from inline JSON
    let queries_json = serde_json::json!({
        "my_list": {
            "query": "select * from source where id=@id and name=@name",
            "args": {
                "id": { "type": "integer" },
                "name": { "type": "string" }
            }
        },
        "select_all": {
            "query": "select * from source"
        }
    });
    let queries = QueryDefinitions::from_json(queries_json).unwrap();

    // Try to run a query that doesn't exist
    let params = serde_json::json!({});
    let err = db_conn
        .query_run(&queries, "non_existent_query", &params)
        .unwrap_err();

    match err {
        JankenError::QueryNotFound(name) => {
            assert_eq!(name, "non_existent_query");
        }
        _ => panic!("Expected QueryNotFound error, got: {err:?}"),
    }
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
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let params = serde_json::json!({"id": "not_int"}); // id should be integer but got string
    let err = db_conn
        .query_run(&queries, "test_select", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "integer");
            assert_eq!(got, "\"not_int\"");
        }
        _ => panic!("Wrong error type: {err:?}"),
    }
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
    let conn = setup_db();
    conn.execute("INSERT INTO source VALUES (50, 'Test', NULL)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    let params = serde_json::json!({"id": 50});
    let result = db_conn.query_run(&queries, "select_with_range", &params);
    assert!(result.is_ok());

    let params = serde_json::json!({"id": 0});
    let err = db_conn
        .query_run(&queries, "select_with_range", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("between 1 and 100"));
            assert_eq!(got, "0");
        }
        _ => panic!("Wrong error type: {err:?}"),
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
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let cases = vec![
        (serde_json::json!({"id": "not_int"}), "\"not_int\""),
        (serde_json::json!({"id": true}), "true"),
        (serde_json::json!({"id": null}), "null"),
    ];

    for (params, expected_got) in cases {
        let err = db_conn
            .query_run(&queries, "select_with_range", &params)
            .unwrap_err();
        match err {
            JankenError::ParameterTypeMismatch { expected, got } => {
                assert_eq!(expected, "number (integer/float)");
                assert_eq!(got, expected_got);
            }
            _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
        }
    }
}

#[test]
fn test_parameter_validation_enum() {
    let json_definitions = serde_json::json!({
        "select_with_enum": {
            "query": "select * from source where name=@status",
            "returns": ["id", "name", "score"],
            "args": {
                "status": {
                    "type": "string",
                    "enum": ["active", "inactive", "pending"]
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'active', NULL)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    let params = serde_json::json!({"status": "active"});
    let result = db_conn.query_run(&queries, "select_with_enum", &params);
    assert!(result.is_ok());

    let params = serde_json::json!({"status": "unknown"});
    let err = db_conn
        .query_run(&queries, "select_with_enum", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("active"));
            assert!(expected.contains("inactive"));
            assert!(expected.contains("pending"));
            assert_eq!(got, "\"unknown\"");
        }
        _ => panic!("Wrong error type: {err:?}"),
    }
}

#[test]
fn test_io_error() {
    // Test Io error for invalid file path
    let result = QueryDefinitions::from_file("non_existent_file.json");

    match result {
        Err(JankenError::Io(_)) => {} // Should be Io error
        _ => panic!("Expected Io error, got: {result:?}"),
    }
}

#[test]
fn test_sqlite_sql_syntax_error() {
    // Create a query definition with invalid SQL to trigger SQLite syntax error
    let json_definitions = serde_json::json!({
        "bad_query": {
            "query": "INVALID SQL SYNTAX THAT WILL FAIL"
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // This should fail with SQLite error due to invalid SQL syntax
    let params = serde_json::json!({});
    let result = db_conn.query_run(&queries, "bad_query", &params);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        crate::JankenError::Sqlite(_) => {} // SQLite error as expected
        _ => panic!("Expected Sqlite error, got: {err:?}"),
    }
}

#[test]
fn test_invalid_parameter_syntax() {
    // Test that parameters found in SQL must have args definitions
    // Now that we don't parse inline types, parameters without args definitions cause errors
    let json_invalid = serde_json::json!({
        "bad_param": {
            "query": "SELECT * FROM table WHERE id=@param",
            "args": {}
        }
    });

    let result = QueryDefinitions::from_json(json_invalid);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            // Should require args object to define parameter 'param'
            assert_eq!(expected, "parameter definition in args");
            assert!(got.contains("parameter 'param' not defined in args object"));
        }
        _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
    }
}

#[test]
fn test_invalid_parameter_type_in_query_definition() {
    // Test that invalid parameter types in args give the correct error message
    let json_definitions = serde_json::json!({
        "bad_query": {
            "query": "select * from source where id=@id",
            "args": {
                "id": { "type": "invalidtype" }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            // Should show all supported types
            assert_eq!(expected, "integer, string, float, or boolean");
            assert_eq!(got, "invalidtype");
        }
        _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
    }
}

#[test]
fn test_transaction_keywords_rejected() {
    // Test that explicit transaction keywords are rejected
    let json_definitions = serde_json::json!({
        "bad_transaction_query": {
            "query": "BEGIN; INSERT INTO accounts (name, balance) VALUES ('Alice', 1000); COMMIT;"
        },
        "bad_rollback_query": {
            "query": "START TRANSACTION; INSERT INTO accounts (name, balance) VALUES ('Bob', 1000); ROLLBACK;"
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "SQL without explicit transaction keywords");
            assert_eq!(
                got,
                "Query contains BEGIN, COMMIT, ROLLBACK, START TRANSACTION, or END TRANSACTION"
            );
        }
        _ => panic!("Expected ParameterTypeMismatch for transaction keywords, got: {err:?}"),
    }
}

#[test]
fn test_regex_error() {
    // Test that invalid regex patterns in parameter validation are handled appropriately
    // The current implementation attempts to compile the regex and returns ParameterTypeMismatch if invalid
    // Since this happens during ParameterConstraints::validate (not during query definition creation),
    // we need to create the query definition first, then try to run it with a value that triggers validation.

    let json_definitions = serde_json::json!({
        "regex_query": {
            "query": "select * from source where name = @pattern",
            "args": {
                "pattern": {
                    "type": "string",
                    "pattern": "[invalid regex("  // Invalid regex - unclosed bracket
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions);
    // The regex validation happens during constraint validation, not during query creation,
    // so the query definition will be created successfully
    assert!(queries.is_ok());

    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Now try to run it with a parameter that would trigger regex validation
    let params = serde_json::json!({"pattern": "test_value"});

    let result = db_conn.query_run(&queries.unwrap(), "regex_query", &params);
    // This should fail because regex compilation fails during validation
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, .. } => {
            // Should fail with regex validation error
            assert!(expected.contains("regex"));
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid regex, got: {err:?}"),
    }
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
    let conn = setup_db();
    conn.execute(
        "INSERT INTO source VALUES (1, 'test@example.com', NULL)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO source VALUES (2, '555-123-4567', NULL)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    let params = serde_json::json!({"email": "user@domain.com"});
    let result = db_conn.query_run(&queries, "email_query", &params);
    assert!(result.is_ok());

    let params = serde_json::json!({"phone": "555-123-4567"});
    let result = db_conn.query_run(&queries, "phone_query", &params);
    assert!(result.is_ok());

    let params = serde_json::json!({"email": "invalid-email"});
    let err = db_conn
        .query_run(&queries, "email_query", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("string matching pattern"));
            assert_eq!(got, "invalid-email");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid email pattern, got: {err:?}"),
    }

    let params = serde_json::json!({"phone": "invalid-phone"});
    let err = db_conn
        .query_run(&queries, "phone_query", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("string matching pattern"));
            assert_eq!(got, "invalid-phone");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid phone pattern, got: {err:?}"),
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
    let conn = setup_db();
    conn.execute("INSERT INTO source VALUES (123, 'test', NULL)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Integer parameter with pattern constraint should fail with "string" error
    let params = serde_json::json!({"id": 123});
    let err = db_conn
        .query_run(&queries, "select_with_pattern_int", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "string");
            assert_eq!(got, "123");
        }
        _ => {
            panic!("Expected ParameterTypeMismatch for non-string pattern validation, got: {err:?}")
        }
    }

    // Boolean parameter with pattern constraint should fail with "string" error
    let params = serde_json::json!({"active": true});
    let err = db_conn
        .query_run(&queries, "select_with_pattern_bool", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "string");
            assert_eq!(got, "true");
        }
        _ => {
            panic!("Expected ParameterTypeMismatch for non-string pattern validation, got: {err:?}")
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
    let conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'test', NULL)", [])
        .unwrap();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let params = serde_json::json!({"name": "test"});
    let err = db_conn
        .query_run(&queries, "select_with_range_string", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "numeric type");
            assert_eq!(got, "string");
        }
        _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
    }

    let params = serde_json::json!({"id": true});
    let err = db_conn
        .query_run(&queries, "select_with_range_bool", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "numeric type");
            assert_eq!(got, "boolean");
        }
        _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
    }
}

#[test]
fn test_parameter_parsing_with_valid_parameters() {
    // Test normal parameter parsing works and indirectly tests the regex capture
    // We test valid parameter parsing to ensure no errors occur in the normal case
    use jankensqlhub::parameters::parse_parameters_with_quotes;

    // Test parsing parameters from a normal SQL query
    let sql = "SELECT * FROM users WHERE id=@user_id AND name=@user_name AND age=@user_age";
    let parameters = parse_parameters_with_quotes(sql).unwrap();

    // Verify we captured all parameters correctly
    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[0].name, "user_id");
    assert_eq!(parameters[1].name, "user_name");
    assert_eq!(parameters[2].name, "user_age");

    // Verify all parameters default to string type and have no constraints
    for param in &parameters {
        assert_eq!(param.param_type.to_string(), "string");
        assert!(param.constraints.range.is_none());
        assert!(param.constraints.pattern.is_none());
        assert!(param.constraints.enum_values.is_none());
    }
}

#[test]
fn test_no_args_provided_for_parameter_in_sql() {
    // Test that parameters in SQL require an args object to be provided
    use jankensqlhub::QueryDef;

    let sql = "SELECT * FROM source WHERE id=@param";
    let result = QueryDef::from_sql(sql, None);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "args object with parameter definitions");
            assert!(got.contains("parameter 'param' found in SQL but no args object provided"));
        }
        _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
    }
}

#[test]
fn test_from_json_invalid_returns_field() {
    // Test that QueryDefinitions::from_json fails when returns field is not an array
    let json_definitions = serde_json::json!({
        "bad_query": {
            "query": "SELECT * FROM test",
            "returns": "not an array"
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "array of strings");
            assert!(got.contains("not an array"));
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid returns field, got: {err:?}"),
    }
}

#[test]
fn test_from_json_non_object_input() {
    // Test that QueryDefinitions::from_json fails with expected error when input is not an object
    use jankensqlhub::QueryDefinitions;

    // Test with string value
    let json_string = serde_json::json!("not an object");
    let result = QueryDefinitions::from_json(json_string);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "object");
            assert_eq!(got, "\"not an object\"");
        }
        _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
    }

    // Test with array value
    let json_array = serde_json::json!(["not", "an", "object"]);
    let result = QueryDefinitions::from_json(json_array);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(expected, "object");
        }
        _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
    }

    // Test with number value
    let json_number = serde_json::json!(42);
    let result = QueryDefinitions::from_json(json_number);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(expected, "object");
        }
        _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
    }
}

#[test]
fn test_from_json_missing_query_field() {
    // Test that QueryDefinitions::from_json fails when 'query' field is missing
    use jankensqlhub::QueryDefinitions;

    let json_definitions = serde_json::json!({
        "missing_query": {
            "args": {
                "id": {"type": "integer"}
            },
            "returns": ["id", "name"]
        },
        "missing_query2": {
            // completely empty object
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "required 'query' field with string value");
            assert_eq!(got, "missing_query: missing 'query' field");
        }
        _ => panic!("Expected ParameterTypeMismatch for missing query field, got: {err:?}"),
    }
}

#[test]
fn test_from_json_query_definition_not_object() {
    // Test that QueryDefinitions::from_json fails when query definition is not an object
    use jankensqlhub::QueryDefinitions;

    let json_definitions = serde_json::json!({
        "invalid_query_def": "not an object"
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "object");
            assert_eq!(got, "invalid_query_def: \"not an object\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for non-object query definition, got: {err:?}"),
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
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Test non-JSON object request_params (covers line ~20)
    let mut conn = setup_db();
    let request_params_string = serde_json::json!("not an object");
    let result = query_run_sqlite(&mut conn, &queries, "int_test", &request_params_string);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "object");
            assert_eq!(got, "not object");
        }
        _ => panic!("Expected ParameterTypeMismatch for non-object, got: {err:?}"),
    }

    // Test integer parameter with wrong type (covers line ~58)
    let mut conn = setup_db();
    let request_params = serde_json::json!({"id": "not_int"});
    let result = query_run_sqlite(&mut conn, &queries, "int_test", &request_params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "integer");
            assert_eq!(got, "\"not_int\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for integer validation, got: {err:?}"),
    }

    // Test string parameter with wrong type (covers line ~67)
    let mut conn = setup_db();
    let request_params = serde_json::json!({"name": 123});
    let result = query_run_sqlite(&mut conn, &queries, "str_test", &request_params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "string");
            assert_eq!(got, "123");
        }
        _ => panic!("Expected ParameterTypeMismatch for string validation, got: {err:?}"),
    }

    // Test float parameter with wrong type (covers line ~74)
    let mut conn = setup_db();
    let request_params = serde_json::json!({"score": "not_a_number"});
    let result = query_run_sqlite(&mut conn, &queries, "float_test", &request_params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "float");
            assert_eq!(got, "\"not_a_number\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for float validation, got: {err:?}"),
    }

    // Test boolean parameter with wrong type (covers line ~84)
    let mut conn = setup_db();
    let request_params = serde_json::json!({"active": []});
    let result = query_run_sqlite(&mut conn, &queries, "bool_test", &request_params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "boolean");
            assert_eq!(got, "[]");
        }
        _ => {
            panic!("Expected ParameterTypeMismatch for boolean validation with array, got: {err:?}")
        }
    }
}
