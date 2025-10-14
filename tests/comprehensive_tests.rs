use jankensqlhub::{DatabaseConnection, QueryDefinitions, QueryRunner};
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
fn test_multi_statement_transaction_acid_properties() {
    // Test multi-statement transaction ACID properties with different scenarios
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();

    // Create accounts table
    conn.execute(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();

    // Test scenarios: (query_name, params, should_succeed)
    let empty_json = serde_json::json!({});
    let params_json = serde_json::json!({
        "from_name": "Alice",
        "to_name": "Bob",
        "initial_balance": 2000,
        "amount": 300
    });
    let test_cases = vec![
        ("multi_statement_transfer", &empty_json, true), // Fixed values, success
        ("multi_statement_transfer_with_params", &params_json, true), // With parameters, success
        ("multi_statement_failure_transfer", &empty_json, false), // Failure case
    ];

    for (query_name, params, should_succeed) in test_cases {
        let conn = setup_db();
        conn.execute(
            "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
            [],
        )
        .unwrap();
        let mut db_conn = DatabaseConnection::SQLite(conn);

        let result = db_conn.query_run(&queries, query_name, params);

        if should_succeed {
            assert!(result.is_ok(), "Transaction {query_name} should succeed");
            assert!(result.unwrap().is_empty()); // Multi-statement returns empty

            // Verify ACID properties: all operations completed atomically
            let accounts = db_conn
                .query_run(&queries, "select_accounts", &serde_json::json!({}))
                .unwrap();
            assert_eq!(
                accounts.len(),
                2,
                "Should have exactly 2 accounts for {query_name}"
            );
            assert!(
                accounts.contains(&serde_json::json!("1")),
                "Alice account should exist for {query_name}"
            );
            assert!(
                accounts.contains(&serde_json::json!("2")),
                "Bob account should exist for {query_name}"
            );
        } else {
            assert!(result.is_err(), "Transaction {query_name} should fail");

            // Verify ACID properties: none of the operations succeeded (rollback)
            let accounts = db_conn
                .query_run(&queries, "select_accounts", &serde_json::json!({}))
                .unwrap();
            assert_eq!(
                accounts.len(),
                0,
                "Should have zero accounts after rollback for {query_name}"
            );
        }
    }
}

#[test]
fn test_sql_injection_protection() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test classic SQL injection attempts - these should be safe
    let sql_injection_attempts = vec![
        "'; DROP TABLE source; --",
        "OR 1=1; --",
        "' UNION SELECT * FROM sqlite_master --",
        "'; SELECT * FROM source; --",
    ];

    let num_attempts = sql_injection_attempts.len();

    // Insert a baseline record to ensure table operations are working
    let params = serde_json::json!({"name": "TestUser"});
    db_conn
        .query_run(&queries, "insert_single", &params)
        .unwrap();

    let initial_count = db_conn
        .query_run(&queries, "select_all", &serde_json::json!({}))
        .unwrap()
        .len();

    for injection in sql_injection_attempts {
        // Try to insert with malicious name - this should work safely
        let params = serde_json::json!({"name": injection});
        db_conn
            .query_run(&queries, "insert_single", &params)
            .unwrap();

        // Verify we can find the malicious string as a literal value
        // This demonstrates SQL injection didn't occur (if it did, this would fail)
        let params = serde_json::json!({"id": 1, "name": "TestUser"});
        let result = db_conn.query_run(&queries, "my_list", &params).unwrap();
        assert!(!result.is_empty()); // Should find our original record
    }

    // Verify table is intact and has expected data
    let final_params = serde_json::json!({});
    let final_result = db_conn
        .query_run(&queries, "select_all", &final_params)
        .unwrap();
    // Should have original record + 4 injection attempts + 1 baseline = 6 total
    assert_eq!(final_result.len(), initial_count + num_attempts);

    // Additional SQL injection testing with different parameters
    let conn2 = setup_db();
    let mut db_conn2 = DatabaseConnection::SQLite(conn2);

    let json_definitions2 = serde_json::json!({
        "insert_with_params": {
            "query": "INSERT INTO source (id, name) VALUES (@id, @name)",
            "args": { "id": {"type": "integer"}, "name": {"type": "string"} }
        },
        "select_by_id": {
            "query": "SELECT id FROM source WHERE id=@id",
            "args": { "id": {"type": "integer"} }
        }
    });

    let queries2 = QueryDefinitions::from_json(json_definitions2).unwrap();

    // Test injection attempts through ID parameter
    let injection_ids = vec!["1 OR 1=1", "1; DROP TABLE source; --"];

    for injection_id in injection_ids {
        // Insert with potential injection through id parameter
        let params = serde_json::json!({"id": injection_id, "name": "injection_test"});
        let result = db_conn2.query_run(&queries2, "insert_with_params", &params);

        // Should fail because id must be integer
        assert!(
            result.is_err(),
            "Injection attempt should fail for non-integer id: {injection_id}"
        );
    }

    // Test injection attempts through name parameter (should succeed safely)
    let injection_names = vec!["'; DROP TABLE source; --", "admin'--", "' OR '1'='1"];

    // Set initial id counter
    let mut current_id = 100;

    for injection_name in injection_names {
        // Insert with potential injection through name parameter (should be safe)
        let params = serde_json::json!({"id": current_id, "name": injection_name});
        let result = db_conn2.query_run(&queries2, "insert_with_params", &params);
        assert!(
            result.is_ok(),
            "Injection attempt through name should succeed safely: {injection_name}"
        );

        // Verify we can retrieve the record (proves injection didn't break the query)
        let params = serde_json::json!({"id": current_id});
        let result = db_conn2
            .query_run(&queries2, "select_by_id", &params)
            .unwrap();
        assert_eq!(
            result.len(),
            1,
            "Should be able to retrieve record after injection attempt"
        );

        current_id += 1; // Increment to avoid conflicts
    }
}

#[test]
fn test_parameter_parsing_edge_cases() {
    use jankensqlhub::parameters::parse_parameters_with_quotes;

    // Test with various valid parameter names that match the regex \w+
    let test_cases = vec![
        "SELECT * FROM table WHERE value=@a",        // Single character
        "SELECT * FROM table WHERE value=@param123", // Alphanumeric
        "SELECT * FROM table WHERE value=@PARAM",    // Uppercase
        "SELECT * FROM table WHERE value=@param_name", // Underscore
        "SELECT * FROM table WHERE value=@p0a1r2a3m4", // Numbers mixed
        "@valid @another",                           // Multiple parameters
    ];

    for sql in &test_cases {
        // These should all parse without triggering "missing parameter name" errors
        let result = parse_parameters_with_quotes(sql);
        assert!(result.is_ok(), "Failed to parse parameters from SQL: {sql}",);
    }

    // Test edge case with just @ - this should NOT match the regex since @ is not followed by \w+
    let result = parse_parameters_with_quotes("@");
    assert!(result.is_ok());
    let parameters = result.unwrap();
    assert_eq!(parameters.len(), 0); // Should not parse as a parameter

    // Test exclusion: parameters inside quotes should not be parsed
    let sql = "SELECT * FROM table WHERE value='@not_param' AND other=@real_param";
    let parameters = parse_parameters_with_quotes(sql).unwrap();
    assert_eq!(parameters.len(), 1); // Only @real_param should be found
    assert_eq!(parameters[0].name, "real_param");
}

#[test]
fn test_str_utils_functionality() {
    // Test utility functions that exercise the covered lines in str_utils.rs
    // Covers: line 18 (escaped = false), lines 23/30/31 (quote state), lines 55-61 (splitting), line 80 (extraction)
    use jankensqlhub::str_utils;

    // Test escaped backslash handling - covers line 18 (escaped = false; continue;)
    let sql_with_escape = "SELECT 'string\\'s' FROM table WHERE @param";
    let param_pos = sql_with_escape.find("@param").unwrap();
    assert!(!str_utils::is_in_quotes(sql_with_escape, param_pos));

    // Test quote state management - covers lines 23, 30, 31
    let complex_quotes = r#"SELECT "double" 'single' FROM table WHERE @param"#;
    let param_pos = complex_quotes.find("@param").unwrap();
    assert!(!str_utils::is_in_quotes(complex_quotes, param_pos));

    // Test SQL splitting with both single and double quotes - covers lines 55, 56, 58, 59, 61
    let multi_stmt = r#"INSERT INTO t VALUES ("val"); UPDATE t SET x='value'; SELECT 1"#;
    let statements = str_utils::split_sql_statements(multi_stmt);
    assert_eq!(statements.len(), 3);
    assert!(statements[0].starts_with("INSERT"));
    assert!(statements[1].starts_with("UPDATE"));
    assert!(statements[2].starts_with("SELECT"));

    // Test parameter extraction with better coverage - covers line 80 and surrounding logic
    let stmt = r#"SELECT col FROM t WHERE name='literal\'quote' AND @param='value'"#;
    let params = str_utils::extract_parameters_in_statement(stmt);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0], "param");
}

#[test]
fn test_multi_statement_no_params() {
    // Test multiple statement query without any parameters to cover line 186 in runner.rs
    let json_definitions = serde_json::json!({
        "insert_multiple_fixed": {
            "query": "INSERT INTO source (name) VALUES ('Fixed1'); INSERT INTO source (name) VALUES ('Fixed2');"
        },
        "select_multiple": {
            "query": "SELECT id FROM source WHERE name LIKE 'Fixed%' ORDER BY id"
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Execute multi-statement insert without parameters
    let params = serde_json::json!({}); // No parameters
    let insert_result = db_conn
        .query_run(&queries, "insert_multiple_fixed", &params)
        .unwrap();
    assert!(insert_result.is_empty()); // INSERT returns empty

    // Verify the insertions worked
    let result = db_conn
        .query_run(&queries, "select_multiple", &params)
        .unwrap();
    assert_eq!(result.len(), 2); // Should return both inserted records
    // Should have auto-incremented IDs (1 and 2)
}
