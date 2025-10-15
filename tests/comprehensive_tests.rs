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
    assert!(result.unwrap().is_empty());

    let accounts = db_conn
        .query_run(&queries, "select_accounts", &serde_json::json!({}))
        .unwrap();
    assert_eq!(accounts.len(), 2);

    let alice_balance = accounts
        .iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some("Alice"))
        .unwrap()
        .get("balance")
        .and_then(|b| b.as_i64());
    assert_eq!(alice_balance, Some(900));

    let bob_balance = accounts
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
    assert!(result.unwrap().is_empty());

    let accounts = db_conn
        .query_run(&queries, "select_accounts2", &serde_json::json!({}))
        .unwrap();
    assert_eq!(accounts.len(), 2);

    let alice_balance = accounts
        .iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some("Alice"))
        .unwrap()
        .get("balance")
        .and_then(|b| b.as_i64());
    assert_eq!(alice_balance, Some(1700));

    let bob_balance = accounts
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
    assert_eq!(accounts.len(), 0);
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
        .len();

    let params = serde_json::json!({"name": sql_injection_attempt});
    db_conn
        .query_run(&queries, "insert_single", &params)
        .unwrap();

    let params = serde_json::json!({"id": 1, "name": "TestUser"});
    let result = db_conn.query_run(&queries, "my_list", &params).unwrap();
    assert!(!result.is_empty());

    let final_result = db_conn
        .query_run(&queries, "select_all", &serde_json::json!({}))
        .unwrap();
    assert_eq!(final_result.len(), initial_count + 1);
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
    assert!(result.is_empty());

    let params = serde_json::json!({"id": 100});
    let result = db_conn
        .query_run(&queries, "select_by_id", &params)
        .unwrap();
    assert_eq!(result.len(), 1);
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
    let params = str_utils::extract_parameters_in_statement(stmt);
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
    assert!(insert_result.is_empty());

    let result = db_conn
        .query_run(&queries, "select_multiple", &params)
        .unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn test_sqlite_row_error_handling() {
    let conn = setup_db();
    conn.execute(
        "CREATE TABLE test_wide (a INTEGER, b INTEGER, c INTEGER, d INTEGER)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO test_wide VALUES (1, 2, 3, 4)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    let json_definitions = serde_json::json!({
        "select_wide": {
            "query": "SELECT a, b FROM test_wide",
            "returns": ["a", "b", "c", "d", "nonexistent_field"],
            "args": {}
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({});
    let result = db_conn.query_run(&queries, "select_wide", &params).unwrap();

    assert_eq!(result.len(), 1);
    let obj = &result[0];
    assert_eq!(obj.get("a"), Some(&serde_json::json!(1)));
    assert_eq!(obj.get("b"), Some(&serde_json::json!(2)));
    assert_eq!(obj.get("c"), Some(&serde_json::json!(null)));
    assert_eq!(obj.get("d"), Some(&serde_json::json!(null)));
    assert_eq!(obj.get("nonexistent_field"), Some(&serde_json::json!(null)));
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

    assert_eq!(result.len(), 2);
    // Check exact response values
    assert_eq!(result[0], serde_json::json!({"value": null})); // Infinity -> null
    assert_eq!(result[1], serde_json::json!({"value": "invalid"}));
}
