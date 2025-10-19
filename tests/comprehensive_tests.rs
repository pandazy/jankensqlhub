use jankensqlhub::{DatabaseConnection, JankenError, QueryDefinitions, QueryRunner, parameters};
use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE source (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, score REAL, active BOOLEAN)",
        [],
    )
    .unwrap();
    conn
}

#[test]
fn test_multi_statement_transaction_fixed_transfer() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let params = serde_json::json!({});
    let result = db_conn.query_run(&queries, "multi_statement_transfer", &params);
    assert!(result.is_ok());
    assert!(result.unwrap().data.is_empty());

    let accounts = db_conn
        .query_run(&queries, "select_accounts", &serde_json::json!({}))
        .unwrap();
    assert_eq!(accounts.data.len(), 2);

    let alice_balance = accounts
        .data
        .iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some("Alice"))
        .unwrap()
        .get("balance")
        .and_then(|b| b.as_i64());
    assert_eq!(alice_balance, Some(900));

    let bob_balance = accounts
        .data
        .iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some("Bob"))
        .unwrap()
        .get("balance")
        .and_then(|b| b.as_i64());
    assert_eq!(bob_balance, Some(1100));
}

#[test]
fn test_multi_statement_transaction_with_params() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE accounts2 (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let params = serde_json::json!({"from_name": "Alice", "to_name": "Bob", "initial_balance": 2000, "amount": 300});
    let result = db_conn.query_run(&queries, "multi_statement_transfer_with_params", &params);
    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(result.data.is_empty());

    // Verify that the SQL statements are properly transformed with named parameters
    assert_eq!(
        result.sql_statements.len(),
        4,
        "Should have 4 SQL statements"
    );

    // Check each statement contains the expected parts and named parameters instead of @param
    assert!(
        result.sql_statements[0].contains(
            "INSERT INTO accounts2 (name, balance) VALUES (:from_name, :initial_balance)"
        ),
        "First statement should contain proper named parameters: {}",
        &result.sql_statements[0]
    );
    assert!(
        !result.sql_statements[0].contains("@from_name"),
        "First statement still contains @param instead of :param"
    );
    assert!(
        !result.sql_statements[0].contains("@initial_balance"),
        "First statement still contains @param"
    );

    assert!(
        result.sql_statements[1]
            .contains("INSERT INTO accounts2 (name, balance) VALUES (:to_name, :initial_balance)"),
        "Second statement should contain proper named parameters: {}",
        &result.sql_statements[1]
    );
    assert!(
        !result.sql_statements[1].contains("@to_name"),
        "Second statement still contains @param"
    );
    assert!(
        !result.sql_statements[1].contains("@initial_balance"),
        "Second statement still contains @param"
    );

    assert!(
        result.sql_statements[2]
            .contains("UPDATE accounts2 SET balance = balance - :amount WHERE name = :from_name"),
        "Third statement should contain proper named parameters: {}",
        &result.sql_statements[2]
    );
    assert!(
        !result.sql_statements[2].contains("@amount"),
        "Third statement still contains @param"
    );
    assert!(
        !result.sql_statements[2].contains("@from_name"),
        "Third statement still contains @param"
    );

    assert!(
        result.sql_statements[3]
            .contains("UPDATE accounts2 SET balance = balance + :amount WHERE name = :to_name"),
        "Fourth statement should contain proper named parameters: {}",
        &result.sql_statements[3]
    );
    assert!(
        !result.sql_statements[3].contains("@amount"),
        "Fourth statement still contains @param"
    );
    assert!(
        !result.sql_statements[3].contains("@to_name"),
        "Fourth statement still contains @param"
    );

    let accounts = db_conn
        .query_run(&queries, "select_accounts2", &serde_json::json!({}))
        .unwrap();
    assert_eq!(accounts.data.len(), 2);

    let alice_balance = accounts
        .data
        .iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some("Alice"))
        .unwrap()
        .get("balance")
        .and_then(|b| b.as_i64());
    assert_eq!(alice_balance, Some(1700));

    let bob_balance = accounts
        .data
        .iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some("Bob"))
        .unwrap()
        .get("balance")
        .and_then(|b| b.as_i64());
    assert_eq!(bob_balance, Some(2300));
}

#[test]
fn test_multi_statement_transaction_failure() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let result = db_conn.query_run(
        &queries,
        "multi_statement_failure_transfer",
        &serde_json::json!({}),
    );
    assert!(result.is_err());

    let accounts = db_conn
        .query_run(&queries, "select_accounts", &serde_json::json!({}))
        .unwrap();
    assert_eq!(accounts.data.len(), 0);
}

#[test]
fn test_sql_injection_protection_name_parameter() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let sql_injection_attempt = "'; DROP TABLE source; --";

    let params = serde_json::json!({"name": "TestUser"});
    db_conn
        .query_run(&queries, "insert_single", &params)
        .unwrap();

    let initial_count = db_conn
        .query_run(&queries, "select_all", &serde_json::json!({}))
        .unwrap()
        .data
        .len();

    let params = serde_json::json!({"name": sql_injection_attempt});
    db_conn
        .query_run(&queries, "insert_single", &params)
        .unwrap();

    let params = serde_json::json!({"id": 1, "name": "TestUser", "source": "source"});
    let result = db_conn.query_run(&queries, "my_list", &params).unwrap();
    assert!(!result.data.is_empty());

    let final_result = db_conn
        .query_run(&queries, "select_all", &serde_json::json!({}))
        .unwrap();
    assert_eq!(final_result.data.len(), initial_count + 1);
}

#[test]
fn test_sql_injection_protection_id_parameter() {
    let json_definitions = serde_json::json!({
        "insert_with_params": {
            "query": "INSERT INTO source (id, name) VALUES (@id, @name)",
            "args": { "id": {"type": "integer"}, "name": {"type": "string"} }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let injection_id = "1 OR 1=1";
    let params = serde_json::json!({"id": injection_id, "name": "injection_test"});
    let result = db_conn.query_run(&queries, "insert_with_params", &params);
    assert!(result.is_err());
}

#[test]
fn test_sql_injection_protection_safe_name_parameter() {
    let json_definitions = serde_json::json!({
        "insert_with_params": {
            "query": "INSERT INTO source (id, name) VALUES (@id, @name)",
            "args": { "id": {"type": "integer"}, "name": {"type": "string"} }
        },
        "select_by_id": {
            "query": "SELECT id FROM source WHERE id=@id",
            "returns": ["id"],
            "args": { "id": {"type": "integer"} }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let injection_name = "'; DROP TABLE source; --";

    let params = serde_json::json!({"id": 100, "name": injection_name});
    let result = db_conn
        .query_run(&queries, "insert_with_params", &params)
        .unwrap();
    assert!(result.data.is_empty());

    let params = serde_json::json!({"id": 100});
    let result = db_conn
        .query_run(&queries, "select_by_id", &params)
        .unwrap();
    assert_eq!(result.data.len(), 1);
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
    use jankensqlhub::str_utils;

    let sql_with_escape = "SELECT 'string\\'s' FROM table WHERE @param";
    let param_pos = sql_with_escape.find("@param").unwrap();
    assert!(!str_utils::is_in_quotes(sql_with_escape, param_pos));

    let complex_quotes = r#"SELECT "double" 'single' FROM table WHERE @param"#;
    let param_pos = complex_quotes.find("@param").unwrap();
    assert!(!str_utils::is_in_quotes(complex_quotes, param_pos));

    let multi_stmt = r#"INSERT INTO t VALUES ("val"); UPDATE t SET x='value'; SELECT 1"#;
    let statements = str_utils::split_sql_statements(multi_stmt);
    assert_eq!(statements.len(), 3);
    assert!(statements[0].starts_with("INSERT"));
    assert!(statements[1].starts_with("UPDATE"));
    assert!(statements[2].starts_with("SELECT"));

    let stmt = r#"SELECT col FROM t WHERE name='literal\'quote' AND @param='value'"#;
    let params = parameters::extract_parameters_in_statement(stmt);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0], "param");
}

#[test]
fn test_multi_statement_no_params() {
    let json_definitions = serde_json::json!({
        "insert_multiple_fixed": {
            "query": "INSERT INTO source (name) VALUES ('Fixed1'); INSERT INTO source (name) VALUES ('Fixed2');"
        },
        "select_multiple": {
            "query": "SELECT id FROM source WHERE name LIKE 'Fixed%' ORDER BY id",
            "returns": ["id"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let params = serde_json::json!({});
    let insert_result = db_conn
        .query_run(&queries, "insert_multiple_fixed", &params)
        .unwrap();
    assert!(insert_result.data.is_empty());

    let result = db_conn
        .query_run(&queries, "select_multiple", &params)
        .unwrap();
    assert_eq!(result.data.len(), 2);
}

#[test]
fn test_sqlite_row_error_handling() {
    let conn = setup_db();
    conn.execute(
        "CREATE TABLE test_wide (a INTEGER, b REAL, c TEXT, d BLOB, e INTEGER)",
        [],
    )
    .unwrap();

    // Insert test data with various types including NULL
    conn.execute(
        "INSERT INTO test_wide VALUES (1, 3.145, 'hello', X'DEADBEEF', NULL)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    let json_definitions = serde_json::json!({
        "select_wide_with_types": {
            "query": "SELECT a, b, c, d, e FROM test_wide",
            "returns": ["a", "b", "c", "d", "e", "missing_field"],
            "args": {}
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({});
    let result = db_conn
        .query_run(&queries, "select_wide_with_types", &params)
        .unwrap();

    assert_eq!(result.data.len(), 1);
    let obj = &result.data[0];

    assert_eq!(obj.get("a"), Some(&serde_json::json!(1))); // Integer
    assert_eq!(obj.get("b"), Some(&serde_json::json!(3.145))); // Real/Float
    assert_eq!(obj.get("c"), Some(&serde_json::json!("hello"))); // Text
    // BLOB data - stored as array of byte values
    assert!(obj.get("d").is_some()); // BLOB gets converted to byte array
    assert_eq!(obj.get("e"), Some(&serde_json::json!(null))); // NULL
    assert_eq!(obj.get("missing_field"), Some(&serde_json::json!(null))); // Missing field
}

#[test]
fn test_sqlite_real_nan_handling() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE test_float (id INTEGER, value REAL)", [])
        .unwrap();
    // Insert Infinity (1.0/0.0) and invalid text that SQLite can't convert to float
    conn.execute(
        "INSERT INTO test_float VALUES (1, 1.0 / 0.0), (2, 'invalid')",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    let json_definitions = serde_json::json!({
        "select_float": {
            "query": "SELECT value FROM test_float ORDER BY id",
            "returns": ["value"],
            "args": {}
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({});
    let result = db_conn
        .query_run(&queries, "select_float", &params)
        .unwrap();

    assert_eq!(result.data.len(), 2);
    // Check exact response values
    assert_eq!(result.data[0], serde_json::json!({"value": null})); // Infinity -> null
    assert_eq!(result.data[1], serde_json::json!({"value": "invalid"}));
}

#[test]
fn test_multi_table_name_parameters() {
    let conn = Connection::open_in_memory().unwrap();

    // Create three tables with test data
    conn.execute(
        "CREATE TABLE table1 (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE table2 (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE table3 (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();

    conn.execute("INSERT INTO table1 VALUES (1, 'Alice')", [])
        .unwrap();
    conn.execute("INSERT INTO table2 VALUES (2, 'Bob')", [])
        .unwrap();
    conn.execute("INSERT INTO table3 VALUES (3, 'Charlie')", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test query with three table name parameters
    let json_definitions = serde_json::json!({
        "multi_table_test": {
            "query": "SELECT DISTINCT t1.id AS id1, t1.name AS name1, t2.id AS id2, t2.name AS name2, t3.id AS id3, t3.name AS name3 FROM #table1 t1, #table2 t2, #table3 t3 WHERE t1.id = 1 AND t2.id = 2 AND t3.id = 3",
            "returns": ["id1", "name1", "id2", "name2", "id3", "name3"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({"table1": "table1", "table2": "table2", "table3": "table3"});
    let result = db_conn
        .query_run(&queries, "multi_table_test", &params)
        .unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(
        result.data[0],
        serde_json::json!({"id1": 1, "name1": "Alice", "id2": 2, "name2": "Bob", "id3": 3, "name3": "Charlie"})
    );
}

#[test]
fn test_multi_statement_table_name_parameters() {
    let conn = Connection::open_in_memory().unwrap();

    // Create tables for multi-statement test
    conn.execute(
        "CREATE TABLE source_table (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE dest_table (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();

    conn.execute("INSERT INTO source_table VALUES (1, 'Alice')", [])
        .unwrap();
    conn.execute("INSERT INTO source_table VALUES (2, 'Bob')", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test multi-statement query with table name parameters
    let json_definitions = serde_json::json!({
        "multi_statement_table_transfer": {
            "query": "INSERT INTO #dest_table SELECT * FROM #source_table WHERE id <= @limit; UPDATE #dest_table SET name = UPPER(name);",
            "args": {
                "limit": { "type": "integer" }
            }
        },
        "select_dest_table": {
            "query": "SELECT * FROM #dest_table ORDER BY id",
            "returns": ["id", "name"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Run multi-statement transfer
    let params =
        serde_json::json!({"source_table": "source_table", "dest_table": "dest_table", "limit": 2});
    let transfer_result = db_conn
        .query_run(&queries, "multi_statement_table_transfer", &params)
        .unwrap();
    assert!(transfer_result.data.is_empty());

    // Verify the data was transferred and modified
    let params = serde_json::json!({"dest_table": "dest_table"});
    let result = db_conn
        .query_run(&queries, "select_dest_table", &params)
        .unwrap();

    assert_eq!(result.data.len(), 2);
    assert_eq!(
        result.data[0],
        serde_json::json!({"id": 1, "name": "ALICE"})
    );
    assert_eq!(result.data[1], serde_json::json!({"id": 2, "name": "BOB"}));
}

#[test]
fn test_sql_injection_protection_list_parameters() {
    // Create queries with proper constraints for list parameters
    let json_definitions = serde_json::json!({
        "safe_int_list": {
            "query": "SELECT * FROM source WHERE id IN :[targets]",
            "returns": ["id", "name", "score"],
            "args": {
                "targets": { "itemtype": "integer" }
            }
        },
        "safe_string_list": {
            "query": "SELECT * FROM source WHERE name IN :[names]",
            "returns": ["id", "name", "score"],
            "args": {
                "names": { "itemtype": "string" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Create test table with safe data
    let conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'Alice', 95.0, 1)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (5, 'Bob', 87.5, 0)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test that safe integer list works
    let params = serde_json::json!({"targets": [1, 5]});
    let result = db_conn
        .query_run(&queries, "safe_int_list", &params)
        .unwrap();
    assert_eq!(result.data.len(), 2);

    // Test SQL injection attempt through integer list - invalid type should fail
    let params = serde_json::json!({"targets": ["1'; DROP TABLE source; --", 5]});
    let err = db_conn
        .query_run(&queries, "safe_int_list", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "integer at index 0");
            assert_eq!(got, "\"1'; DROP TABLE source; --\"");
        }
        _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
    }

    // Test SQL injection attempt through string list
    let params = serde_json::json!({"names": ["Alice'; DROP TABLE source; --", "Bob"]});
    let result = db_conn
        .query_run(&queries, "safe_string_list", &params)
        .unwrap();
    // SQL injection should be blocked by prepared statements - no rows should match the malicious name
    assert_eq!(result.data.len(), 1); // Only "Bob" should match

    // Test that safe string values work in list
    let params = serde_json::json!({"names": ["Alice", "Bob"]});
    let result = db_conn
        .query_run(&queries, "safe_string_list", &params)
        .unwrap();
    assert_eq!(result.data.len(), 2);

    // Verify table still exists and data is safe
    let params = serde_json::json!({"targets": [1, 5]});
    let result = db_conn
        .query_run(&queries, "safe_int_list", &params)
        .unwrap();
    assert_eq!(result.data.len(), 2);
}

#[test]
fn test_list_parameter_functionality() {
    let queries = QueryDefinitions::from_file("test_json/crud.json").unwrap();

    // Create test table with several rows
    let conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'Alice', 95.0, 1)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (5, 'Bob', 87.5, 0)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (10, 'Charlie', 92.0, 1)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (15, 'David', 88.5, 0)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (20, 'Eve', 91.0, 1)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);
    let params = serde_json::json!({"table": "source", "targets": [1, 5, 10]});

    let result = db_conn.query_run(&queries, "read", &params).unwrap();

    // Should return 3 rows matching the ids [1, 5, 10]
    assert_eq!(result.data.len(), 3);

    let names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"Alice".to_string()));
    assert!(names.contains(&"Bob".to_string()));
    assert!(names.contains(&"Charlie".to_string()));

    // Test with different array - should only return IDs 5 and 15
    let params = serde_json::json!({"table": "source", "targets": [5, 15]});
    let result = db_conn.query_run(&queries, "read", &params).unwrap();
    assert_eq!(result.data.len(), 2);

    let names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"Bob".to_string()));
    assert!(names.contains(&"David".to_string()));

    // Test empty list - should fail
    let params = serde_json::json!({"table": "source", "targets": []});
    let result = db_conn.query_run(&queries, "read", &params);
    assert!(result.is_err());

    // Test multiple list parameters
    let params = serde_json::json!({"table": "source", "ids": [1, 5], "scores": [95.0, 87.5]});
    let result = db_conn.query_run(&queries, "multi_list", &params).unwrap();

    // Should return records where id IN [1, 5] AND score IN [95.0, 87.5]
    // This matches Alice (id=1, score=95.0) and Bob (id=5, score=87.5)
    assert_eq!(result.data.len(), 2);

    let names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"Alice".to_string()));
    assert!(names.contains(&"Bob".to_string()));

    // Test string list parameters
    let params = serde_json::json!({"table": "source", "names": ["Alice", "Charlie", "Eve"]});
    let result = db_conn.query_run(&queries, "string_list", &params).unwrap();

    // Should return 3 rows matching the names ["Alice", "Charlie", "Eve"]
    assert_eq!(result.data.len(), 3);

    let returned_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(returned_names.contains(&"Alice".to_string()));
    assert!(returned_names.contains(&"Charlie".to_string()));
    assert!(returned_names.contains(&"Eve".to_string()));

    // Test with different string array - should only return Alice and Eve
    let params = serde_json::json!({"table": "source", "names": ["Alice", "Eve"]});
    let result = db_conn.query_run(&queries, "string_list", &params).unwrap();
    assert_eq!(result.data.len(), 2);

    let returned_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(returned_names.contains(&"Alice".to_string()));
    assert!(returned_names.contains(&"Eve".to_string()));

    // Test boolean list parameters
    let params = serde_json::json!({"table": "source", "statuses": [true, false]});
    let result = db_conn
        .query_run(&queries, "boolean_list", &params)
        .unwrap();

    // Should return all 5 rows since active contains both true and false values
    assert_eq!(result.data.len(), 5);

    // Test with only true values
    let params = serde_json::json!({"table": "source", "statuses": [true]});
    let result = db_conn
        .query_run(&queries, "boolean_list", &params)
        .unwrap();

    // Should return 3 rows with active=true (Alice=1, Charlie=1, Eve=1)
    assert_eq!(result.data.len(), 3);

    let returned_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(returned_names.contains(&"Alice".to_string()));
    assert!(returned_names.contains(&"Charlie".to_string()));
    assert!(returned_names.contains(&"Eve".to_string()));

    // Test with only false values
    let params = serde_json::json!({"table": "source", "statuses": [false]});
    let result = db_conn
        .query_run(&queries, "boolean_list", &params)
        .unwrap();

    // Should return 2 rows with active=false (Bob=0, David=0)
    assert_eq!(result.data.len(), 2);

    let returned_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(returned_names.contains(&"Bob".to_string()));
    assert!(returned_names.contains(&"David".to_string()));
}
