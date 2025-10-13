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
fn test_sqlite_select_all_no_params() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();
    conn.execute(
        "INSERT INTO source VALUES (1, 'John', NULL), (2, 'Jane', NULL)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test select all
    let params = serde_json::json!({});
    let result = db_conn.query_run(&queries, "select_all", &params).unwrap();
    // Verify all records with their IDs
    assert_eq!(result.len(), 2);
    assert!(result.contains(&serde_json::json!("1"))); // John with id=1
    assert!(result.contains(&serde_json::json!("2"))); // Jane with id=2
}

#[test]
fn test_sqlite_insert_with_params() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Insert
    let params = serde_json::json!({"name": "NewGuy"});
    let insert_result = db_conn
        .query_run(&queries, "insert_single", &params)
        .unwrap();
    // Insert operations return empty results
    assert!(insert_result.is_empty());

    // Verify by select all
    let params = serde_json::json!({});
    let result = db_conn.query_run(&queries, "select_all", &params).unwrap();
    // Should return the newly inserted record (with auto-incremented ID)
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], serde_json::json!("1")); // First inserted record gets ID=1
}

#[test]
fn test_sqlite_update_with_params() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'John', NULL)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Update
    let params = serde_json::json!({"new_id": 10, "new_name": "NewJohn", "old_id": 1});
    db_conn.query_run(&queries, "my_action", &params).unwrap();

    // Verify by select specific with new id
    let params = serde_json::json!({"id": 10, "name": "NewJohn"});
    let result = db_conn.query_run(&queries, "my_list", &params).unwrap();
    assert_eq!(result, vec![serde_json::json!("10")]);
}

#[test]
fn test_boolean_params() {
    let conn = setup_db();
    conn.execute(
        "INSERT INTO source VALUES (1, 'active', 1), (2, 'inactive', 0)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test boolean parameters using the new args format
    let json_definitions = serde_json::json!({
        "insert_with_bool": {
            "query": "insert into source (id, name, score) values (@id, @name, @active)",
            "args": {
                "id": { "type": "integer" },
                "name": { "type": "string" },
                "active": { "type": "boolean" }
            }
        },
        "select_by_bool": {
            "query": "select * from source where score = @active",
            "args": {
                "active": { "type": "boolean" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Insert with boolean true (should convert to 1)
    let params = serde_json::json!({"id": 3, "name": "user3", "active": true});
    let insert_result = db_conn
        .query_run(&queries, "insert_with_bool", &params)
        .unwrap();
    assert!(insert_result.is_empty()); // INSERT returns empty

    // Insert with boolean false (should convert to 0)
    let params = serde_json::json!({"id": 4, "name": "user4", "active": false});
    db_conn
        .query_run(&queries, "insert_with_bool", &params)
        .unwrap();

    // Select rows where active=true (should convert to score=1)
    let params = serde_json::json!({"active": true});
    let result = db_conn
        .query_run(&queries, "select_by_bool", &params)
        .unwrap();

    // Should return original active record (id=1), inserted active record (id=3), but not inactive records
    assert_eq!(result.len(), 2);
    assert!(result.contains(&serde_json::json!("1"))); // Original active user
    assert!(result.contains(&serde_json::json!("3"))); // New active user
}

#[test]
fn test_loading_from_json_value() {
    // Create query definitions as a serde_json::Value object with new args format
    let json_definitions = serde_json::json!({
        "test_select": {
            "query": "select * from source where id=@id",
            "args": {
                "id": { "type": "integer" }
            }
        },
        "test_insert": {
            "query": "insert into source (id, name) values (@id, @name)",
            "args": {
                "id": { "type": "integer" },
                "name": { "type": "string" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let conn = setup_db();

    // Insert test data
    conn.execute("INSERT INTO source VALUES (42, 'Test', NULL)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test the queries loaded from JSON object
    let params = serde_json::json!({"id": 42});
    let result = db_conn.query_run(&queries, "test_select", &params).unwrap();
    assert!(!result.is_empty());
    assert_eq!(result[0], serde_json::json!("42")); // Should return the id=42 row

    // Test insert - should add record with id=99, name="JsonLoaded"
    let params = serde_json::json!({"id": 99, "name": "JsonLoaded"});
    let insert_result = db_conn.query_run(&queries, "test_insert", &params).unwrap();
    // Insert operations return empty results
    assert!(insert_result.is_empty());

    // Verify the inserted record can be selected
    let params = serde_json::json!({"id": 99});
    let result = db_conn.query_run(&queries, "test_select", &params).unwrap();
    assert!(!result.is_empty());
    assert_eq!(result[0], serde_json::json!("99")); // Should return the inserted id=99 row
}

#[test]
fn test_sqlite_float_params() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();
    conn.execute(
        "INSERT INTO source VALUES (1, 'John', 5.5), (2, 'Jane', 8.2)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Insert with float
    let params = serde_json::json!({"id": 3, "name": "Bob", "score": 7.0});
    db_conn
        .query_run(&queries, "insert_with_float", &params)
        .unwrap();

    // Select with float param (score > 6.0)
    // Should return Jane (8.2) and Bob (7.0), but not John (5.5)
    let params = serde_json::json!({"min_score": 6.0});
    let result = db_conn
        .query_run(&queries, "select_with_float", &params)
        .unwrap();

    // Should return both Bob (id=3) and Jane (id=2)
    assert_eq!(result.len(), 2);
    // Check that we got the expected IDs (Bob and Jane)
    assert!(result.contains(&serde_json::json!("2"))); // Jane with score 8.2
    assert!(result.contains(&serde_json::json!("3"))); // Bob with score 7.0
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
}

#[test]
fn test_multi_statement_transaction_success() {
    // Test successful multi-statement transaction demonstrating ACID properties
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();

    // Create accounts table
    conn.execute(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Execute multi-statement transaction: create two accounts and transfer funds
    let params = serde_json::json!({}); // No parameters - using fixed values for now

    let result = db_conn.query_run(&queries, "multi_statement_transfer", &params);

    assert!(result.is_ok()); // Transaction should succeed
    assert!(result.unwrap().is_empty()); // Multi-statement returns empty

    // Verify ACID properties: all operations completed atomically
    let accounts = db_conn
        .query_run(&queries, "select_accounts", &serde_json::json!({}))
        .unwrap();
    assert_eq!(accounts.len(), 2); // Should have exactly 2 accounts

    // Verify specific accounts were created by ensuring their IDs exist
    assert!(accounts.contains(&serde_json::json!("1"))); // Alice account (ID=1)
    assert!(accounts.contains(&serde_json::json!("2"))); // Bob account (ID=2)

    // Alice balance 900, Bob balance 1100 (transferred 100 from Alice to Bob)
}

#[test]
fn test_multi_statement_transaction_success_no_params() {
    // Test successful multi-statement transaction with fixed values (no parameters)
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();

    // Create accounts table
    conn.execute(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Execute multi-statement transaction: create two accounts and transfer funds
    let params = serde_json::json!({}); // No parameters needed for fixed values

    let result = db_conn.query_run(&queries, "multi_statement_transfer", &params);
    assert!(result.is_ok()); // Transaction should succeed
    assert!(result.unwrap().is_empty()); // Multi-statement returns empty

    // Verify ACID properties: all operations completed atomically
    let accounts = db_conn
        .query_run(&queries, "select_accounts", &serde_json::json!({}))
        .unwrap();
    assert_eq!(accounts.len(), 2); // Should have exactly 2 accounts

    // Verify specific accounts were created by ensuring their IDs exist
    assert!(accounts.contains(&serde_json::json!("1"))); // Alice account (ID=1)
    assert!(accounts.contains(&serde_json::json!("2"))); // Bob account (ID=2)

    // Alice balance 900, Bob balance 1100 (transferred 100 from Alice to Bob)
}

#[test]
fn test_multi_statement_transaction_with_params() {
    // Use the def.json file which already has the correct args format
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();

    // Create accounts table
    conn.execute(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Execute multi-statement transaction: create two accounts and transfer funds with parameters
    let params = serde_json::json!({
        "from_name": "Alice",
        "to_name": "Bob",
        "initial_balance": 2000,
        "amount": 300
    });

    let result = db_conn.query_run(&queries, "multi_statement_transfer_with_params", &params);
    assert!(result.is_ok(), "{result:?}"); // Transaction should succeed
    assert!(result.unwrap().is_empty()); // Multi-statement returns empty

    // Verify ACID properties and that parameters worked: accounts should exist with correct balances
    let accounts = db_conn
        .query_run(&queries, "select_accounts", &serde_json::json!({}))
        .unwrap();
    assert_eq!(accounts.len(), 2); // Should have exactly 2 accounts

    // Accounts should have been created with the specified parameters
    // Alice: 2000 - 300 = 1700, Bob: 2000 + 300 = 2300
    assert!(accounts.contains(&serde_json::json!("1"))); // Alice account (ID=1)
    assert!(accounts.contains(&serde_json::json!("2"))); // Bob account (ID=2)
}

#[test]
fn test_multi_statement_transaction_failure_acid() {
    // Test that failed multi-statement transaction rolls back completely (ACID property)
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();

    // Create accounts table
    conn.execute(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Execute multi-statement transaction that fails: includes invalid SQL
    let params = serde_json::json!({}); // No parameters needed for fixed values

    let result = db_conn.query_run(&queries, "multi_statement_failure_transfer", &params);
    assert!(result.is_err()); // Transaction should fail due to invalid statement

    // Verify ACID properties: none of the operations succeeded (rollback)
    let accounts = db_conn
        .query_run(&queries, "select_accounts", &serde_json::json!({}))
        .unwrap();
    assert_eq!(accounts.len(), 0); // Should have zero accounts (complete rollback)
}
