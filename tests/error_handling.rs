use jankensqlhub::{DatabaseConnection, JankenError, QueryDefinitions, QueryRunner};
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
fn test_parameter_not_provided() {
    // Test ParameterNotProvided error when required parameter is missing
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Load queries that require parameters - using inline JSON for self-containment
    let queries_json = serde_json::json!({
        "my_list": {
            "query": "select * from source where id=@id and name=@name",
            "args": {
                "id": { "type": "integer" },
                "name": { "type": "string" }
            }
        }
    });
    let queries = QueryDefinitions::from_json(queries_json).unwrap();

    // Try to run my_list query which requires both id and name parameters, but omit name
    let params = serde_json::json!({"id": 1}); // Missing required "name" parameter

    let err = db_conn.query_run(&queries, "my_list", &params).unwrap_err();

    match err {
        JankenError::ParameterNotProvided(name) => {
            assert_eq!(name, "name");
        }
        _ => panic!("Expected ParameterNotProvided error, got: {err:?}"),
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
    // Test range validation for numeric parameters
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
    conn.execute("INSERT INTO source VALUES (50, 'Test', NULL)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Valid range value should work
    let params = serde_json::json!({"id": 50});
    let result = db_conn.query_run(&queries, "select_with_range", &params);
    assert!(result.is_ok());

    // Value below minimum should fail
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
fn test_parameter_validation_enum() {
    // Test enum validation for any parameter type
    let json_definitions = serde_json::json!({
        "select_with_enum": {
            "query": "select * from source where name=@status",
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

    // Valid enum value should work
    let params = serde_json::json!({"status": "active"});
    let result = db_conn.query_run(&queries, "select_with_enum", &params);
    assert!(result.is_ok());

    // Invalid enum value should fail
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
    // Test successful regex pattern matching for string validation
    let json_definitions = serde_json::json!({
        "email_query": {
            "query": "select * from source where name = @email",
            "args": {
                "email": {
                    "type": "string",
                    "pattern": "\\S+@\\S+\\.\\S+"
                }
            }
        },
        "phone_query": {
            "query": "select * from source where name = @phone",
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

    // Test valid email pattern
    let params = serde_json::json!({"email": "user@domain.com"});
    let result = db_conn.query_run(&queries, "email_query", &params);
    assert!(result.is_ok(), "Valid email should pass pattern validation");

    // Test valid phone pattern
    let params = serde_json::json!({"phone": "555-123-4567"});
    let result = db_conn.query_run(&queries, "phone_query", &params);
    assert!(
        result.is_ok(),
        "Valid phone number should pass pattern validation"
    );

    // Test invalid email pattern
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

    // Test invalid phone pattern
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
