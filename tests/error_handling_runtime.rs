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
fn test_io_error() {
    // Test IO error for invalid file path
    let result = QueryDefinitions::from_file("non_existent_file.json");

    match result {
        Err(err) => {
            // IO errors are now returned as native std::io::Error wrapped in anyhow
            if err.downcast_ref::<std::io::Error>().is_some() {
                // Successfully downcast to io::Error - test passes
            } else {
                panic!("Expected IO error, got: {err:?}");
            }
        }
        Ok(_) => panic!("Expected error for non-existent file"),
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
    let mut conn = setup_db();

    // This should fail with SQLite error due to invalid SQL syntax
    let params = serde_json::json!({});
    let result = query_run_sqlite(&mut conn, &queries, "bad_query", &params);
    assert!(result.is_err());

    // SQLite errors are now returned as native rusqlite::Error wrapped in anyhow
    // We just verify that an error occurred, not the specific variant
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    // The error should be downcastable and contain SQLite-related error info
    if let Ok(rusqlite_err) = err.downcast::<rusqlite::Error>() {
        // Should be a SQLite error
        assert!(
            format!("{rusqlite_err}").contains("syntax")
                || format!("{rusqlite_err}").contains("SQLITE")
                || format!("{rusqlite_err}").contains("error")
        );
    } else {
        panic!("Expected SQLite error, got: {err_str}");
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

    let mut conn = setup_db();

    // Now try to run it with a parameter that would trigger regex validation
    let params = serde_json::json!({"pattern": "test_value"});

    let result = query_run_sqlite(&mut conn, &queries.unwrap(), "regex_query", &params);
    // This should fail because regex compilation fails during validation
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        // Should fail with regex validation error
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert!(expected.contains("regex"));
    } else {
        panic!("Expected ParameterTypeMismatch for invalid regex, got: {err_str}");
    }
}

#[test]
fn test_table_name_parameter_security_and_validation() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE safe_table (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO safe_table VALUES (1, 'safe')", [])
        .unwrap();

    let json_definitions = serde_json::json!({
        "table_injection_test": {
            "query": "SELECT * FROM #[table_name] WHERE id=@id",
            "returns": ["id", "name"],
            "args": {
                "id": { "type": "integer" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Test type mismatches: wrong data types for table_name parameter
    let type_mismatch_cases = vec![
        (
            serde_json::json!({"table_name": 123, "id": 1}),
            "table_name",
            "123",
        ),
        (
            serde_json::json!({"table_name": true, "id": 1}),
            "table_name",
            "true",
        ),
        (
            serde_json::json!({"table_name": null, "id": 1}),
            "table_name",
            "null",
        ),
        (
            serde_json::json!({"table_name": ["table"], "id": 1}),
            "table_name",
            "[\"table\"]",
        ),
        (
            serde_json::json!({"table_name": {"nested": "value"}, "id": 1}),
            "table_name",
            "{\"nested\":\"value\"}",
        ),
    ];

    for (params, expected_type, expected_got) in type_mismatch_cases {
        let err =
            query_run_sqlite(&mut conn, &queries, "table_injection_test", &params).unwrap_err();
        let err_str = format!("{err:?}");
        if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert_eq!(
                expected, expected_type,
                "Expected type mismatch for {expected_got}"
            );
            assert_eq!(got, expected_got);
        } else {
            panic!("Expected ParameterTypeMismatch for {expected_got}, got: {err_str}");
        }
    }

    // Test various malicious table name attempts and invalid formats
    let malicious_and_invalid_params = vec![
        ("'; DROP TABLE safe_table; --", "SQL injection: classic"),
        (
            "safe_table; DROP TABLE safe_table; --",
            "SQL injection: stacked queries",
        ),
        (
            "safe_table'; DROP TABLE safe_table; --",
            "SQL injection: truncated",
        ),
        (
            "'; SELECT * FROM secret_table; --",
            "SQL injection: information disclosure",
        ),
        (
            "safe_table'; UPDATE safe_table SET name='hacked'; --",
            "SQL injection: data modification",
        ),
        ("table with spaces", "Invalid characters: spaces"),
        ("table-with-dashes", "Invalid characters: dashes"),
        ("table@special", "Invalid characters: special chars"),
        ("", "Invalid format: empty string"),
        // Remove this as it appears our validation doesn't reject uppercase - table names can contain uppercase letters
    ];

    for (malicious_table_name, description) in malicious_and_invalid_params {
        let params = serde_json::json!({"table_name": malicious_table_name, "id": 1});
        let result = query_run_sqlite(&mut conn, &queries, "table_injection_test", &params);
        assert!(
            result.is_err(),
            "Expected error for {description}: '{malicious_table_name}' should be rejected"
        );
    }

    // Test valid table names should work
    let valid_table_name = "safe_table";
    let params = serde_json::json!({"table_name": valid_table_name, "id": 1});
    let result = query_run_sqlite(&mut conn, &queries, "table_injection_test", &params);
    assert!(
        result.is_ok(),
        "Expected valid table name '{valid_table_name}' to work"
    );

    let data = result.unwrap();
    assert_eq!(data.data.len(), 1);
    assert_eq!(data.data[0], serde_json::json!({"id": 1, "name": "safe"}));
}

#[test]
fn test_table_name_validation_error() {
    // Test that invalid table names trigger ParameterTypeMismatch error
    let json_definitions = serde_json::json!({
        "table_query": {
            "query": "SELECT * FROM #[table_name]",
            "returns": ["id"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Test invalid table names should fail with validation error
    let invalid_names = vec![
        "",
        "table-with-dashes",
        "table@special",
        "table with spaces",
    ];

    for table_name in invalid_names {
        let mut conn = Connection::open_in_memory().unwrap();

        let params = serde_json::json!({"table_name": table_name});
        let err = query_run_sqlite(&mut conn, &queries, "table_query", &params).unwrap_err();
        let err_str = format!("{err:?}");
        if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert_eq!(
                expected,
                "valid table name (alphanumeric and underscores only)"
            );
            assert_eq!(got, table_name);
        } else {
            panic!(
                "Expected ParameterTypeMismatch for invalid table name '{table_name}', got: {err_str}"
            );
        }
    }

    // Test valid table name should work
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE valid_name (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    conn.execute("INSERT INTO valid_name VALUES (42)", [])
        .unwrap();
    let params = serde_json::json!({"table_name": "valid_name"});
    let result = query_run_sqlite(&mut conn, &queries, "table_query", &params);
    assert!(result.is_ok(), "Valid table name should work");
    let data = result.unwrap();
    assert_eq!(data.data.len(), 1);
    assert_eq!(data.data[0], serde_json::json!({"id": 42}));
}
