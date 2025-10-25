use jankensqlhub::{QueryDefinitions, query_run_sqlite};
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
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();

    let params = serde_json::json!({});
    let result = query_run_sqlite(&mut conn, &queries, "multi_statement_transfer", &params);
    assert!(result.is_ok());
    assert!(result.unwrap().data.is_empty());

    let accounts = query_run_sqlite(
        &mut conn,
        &queries,
        "select_accounts",
        &serde_json::json!({}),
    )
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
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE accounts2 (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();

    let params = serde_json::json!({"from_name": "Alice", "to_name": "Bob", "initial_balance": 2000, "amount": 300});
    let result = query_run_sqlite(
        &mut conn,
        &queries,
        "multi_statement_transfer_with_params",
        &params,
    );
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

    let accounts = query_run_sqlite(
        &mut conn,
        &queries,
        "select_accounts2",
        &serde_json::json!({}),
    )
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
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();

    let result = query_run_sqlite(
        &mut conn,
        &queries,
        "multi_statement_failure_transfer",
        &serde_json::json!({}),
    );
    assert!(result.is_err());

    let accounts = query_run_sqlite(
        &mut conn,
        &queries,
        "select_accounts",
        &serde_json::json!({}),
    )
    .unwrap();
    assert_eq!(accounts.data.len(), 0);
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
    let mut conn = setup_db();

    let params = serde_json::json!({});
    let insert_result =
        query_run_sqlite(&mut conn, &queries, "insert_multiple_fixed", &params).unwrap();
    assert!(insert_result.data.is_empty());

    let result = query_run_sqlite(&mut conn, &queries, "select_multiple", &params).unwrap();
    assert_eq!(result.data.len(), 2);
}

#[test]
fn test_multi_statement_table_name_parameters() {
    let mut conn = Connection::open_in_memory().unwrap();

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

    // Test multi-statement query with table name parameters
    let json_definitions = serde_json::json!({
        "multi_statement_table_transfer": {
            "query": "INSERT INTO #[dest_table] SELECT * FROM #[source_table] WHERE id <= @limit; UPDATE #[dest_table] SET name = UPPER(name);",
            "args": {
                "limit": { "type": "integer" }
            }
        },
        "select_dest_table": {
            "query": "SELECT * FROM #[dest_table] ORDER BY id",
            "returns": ["id", "name"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Run multi-statement transfer
    let params =
        serde_json::json!({"source_table": "source_table", "dest_table": "dest_table", "limit": 2});
    let transfer_result = query_run_sqlite(
        &mut conn,
        &queries,
        "multi_statement_table_transfer",
        &params,
    )
    .unwrap();
    assert!(transfer_result.data.is_empty());

    // Verify the data was transferred and modified
    let params = serde_json::json!({"dest_table": "dest_table"});
    let result = query_run_sqlite(&mut conn, &queries, "select_dest_table", &params).unwrap();

    assert_eq!(result.data.len(), 2);
    assert_eq!(
        result.data[0],
        serde_json::json!({"id": 1, "name": "ALICE"})
    );
    assert_eq!(result.data[1], serde_json::json!({"id": 2, "name": "BOB"}));
}
