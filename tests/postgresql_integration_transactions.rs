//! Transaction-related PostgreSQL integration tests for JankenSQLHub
//!
//! Tests multi-statement transactions and rollback behavior.

use jankensqlhub::query_run_postgresql;
use tokio_postgres::NoTls;

// Helper function to get PostgreSQL connection string from environment
fn get_postgres_connection_string() -> Option<String> {
    std::env::var("POSTGRES_CONNECTION_STRING").ok()
}

// Helper function to establish PostgreSQL connection for tests
async fn setup_postgres_connection() -> Option<tokio_postgres::Client> {
    let connection_string = get_postgres_connection_string()?;
    let (client, connection) = tokio_postgres::connect(&connection_string, NoTls)
        .await
        .ok()?;

    // Run the connection in the background
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {e}");
        }
    });

    Some(client)
}

async fn setup_postgres_test_schema(
    client: &mut tokio_postgres::Client,
    test_name: &str,
) -> (String, String) {
    // Clean up any existing test tables (add unique suffix to avoid conflicts between tests)
    let source_table = format!("source_{}", test_name.replace("test_", ""));
    let accounts_table = format!("accounts_{}", test_name.replace("test_", ""));

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {source_table}"), &[])
        .await;
    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {accounts_table}"), &[])
        .await;

    // Create test tables with unique names
    client
        .execute(
            &format!(
                r#"
            CREATE TABLE {source_table} (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                score DOUBLE PRECISION,
                active BOOLEAN DEFAULT TRUE
            )
            "#
            ),
            &[],
        )
        .await
        .expect("Failed to create source table");

    client
        .execute(
            &format!(
                r#"
            CREATE TABLE {accounts_table} (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                balance INTEGER NOT NULL
            )
            "#
            ),
            &[],
        )
        .await
        .expect("Failed to create accounts table");

    (source_table, accounts_table)
}

#[tokio::test]
async fn test_postgres_single_statement_mutation() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let (_, accounts_table) =
        setup_postgres_test_schema(&mut client, "test_postgres_single_statement_mutation").await;

    // Test single INSERT statement (no returns, no semicolons)
    let json_definitions = serde_json::json!({
        "insert_single": {
            "query": format!("INSERT INTO {} (name, balance) VALUES (@name, @balance)", accounts_table),
            "args": {
                "name": {"type": "string"},
                "balance": {"type": "integer"}
            }
        },
        "update_single": {
            "query": format!("UPDATE {} SET balance = @new_balance WHERE name = @name", accounts_table),
            "args": {
                "name": {"type": "string"},
                "new_balance": {"type": "integer"}
            }
        },
        "delete_single": {
            "query": format!("DELETE FROM {} WHERE name = @name", accounts_table),
            "args": {
                "name": {"type": "string"}
            }
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    // Test INSERT (single statement mutation, no returns)
    let params = serde_json::json!({
        "name": "TestAccount",
        "balance": 500
    });

    let result = query_run_postgresql(&mut client, &queries, "insert_single", &params)
        .await
        .unwrap();
    assert!(result.data.is_empty()); // No returns means empty data
    assert_eq!(result.sql_statements.len(), 1); // One SQL statement

    // Test UPDATE (single statement mutation, no returns)
    let params = serde_json::json!({
        "name": "TestAccount",
        "new_balance": 750
    });

    let result = query_run_postgresql(&mut client, &queries, "update_single", &params)
        .await
        .unwrap();
    assert!(result.data.is_empty()); // No returns means empty data
    assert_eq!(result.sql_statements.len(), 1); // One SQL statement

    // Test DELETE (single statement mutation, no returns)
    let params = serde_json::json!({
        "name": "TestAccount"
    });

    let result = query_run_postgresql(&mut client, &queries, "delete_single", &params)
        .await
        .unwrap();
    assert!(result.data.is_empty()); // No returns means empty data
    assert_eq!(result.sql_statements.len(), 1); // One SQL statement

    // Verify the account was deleted
    let row_count = client
        .query_one(
            &format!("SELECT COUNT(*) as count FROM {accounts_table}"),
            &[],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(row_count, 0);
}

#[tokio::test]
async fn test_postgres_multi_statement_transaction() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let (_, accounts_table) =
        setup_postgres_test_schema(&mut client, "test_postgres_multi_statement_transaction").await;

    let json_definitions = serde_json::json!({
        "multi_statement_transfer": {
            "query": format!("INSERT INTO {} (name, balance) VALUES (@from_name, @initial_balance); INSERT INTO {} (name, balance) VALUES (@to_name, @initial_balance); UPDATE {} SET balance = balance - @amount WHERE name = @from_name; UPDATE {} SET balance = balance + @amount WHERE name = @to_name;", accounts_table, accounts_table, accounts_table, accounts_table),
            "args": {
                "from_name": {"type": "string"},
                "to_name": {"type": "string"},
                "initial_balance": {"type": "integer"},
                "amount": {"type": "integer"}
            }
        },
        "select_accounts": {
            "query": format!("SELECT name, balance FROM {} ORDER BY name", accounts_table),
            "returns": ["name", "balance"],
            "args": {}
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({
        "from_name": "Alice",
        "to_name": "Bob",
        "initial_balance": 1000,
        "amount": 100
    });

    let result = query_run_postgresql(&mut client, &queries, "multi_statement_transfer", &params)
        .await
        .unwrap();
    assert!(result.data.is_empty());
    assert_eq!(result.sql_statements.len(), 4); // 4 statements in the transaction

    // Verify accounts were updated correctly
    let params = serde_json::json!({});
    let result = query_run_postgresql(&mut client, &queries, "select_accounts", &params)
        .await
        .unwrap();

    assert_eq!(result.data.len(), 2);
    let alice = result
        .data
        .iter()
        .find(|row| row["name"] == "Alice")
        .unwrap();
    let bob = result.data.iter().find(|row| row["name"] == "Bob").unwrap();

    assert_eq!(alice["balance"], serde_json::json!(900));
    assert_eq!(bob["balance"], serde_json::json!(1100));
}

#[tokio::test]
async fn test_postgres_transaction_rollback_on_failure() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let (_, accounts_table) =
        setup_postgres_test_schema(&mut client, "test_postgres_transaction_rollback_on_failure")
            .await;

    let failing_sql = format!(
        "INSERT INTO {accounts_table} (name, balance) VALUES (@from_name, @initial_balance); INSERT INTO {accounts_table} (name, balance) VALUES (@to_name, @initial_balance); UPDATE {accounts_table} SET balance = balance - @amount WHERE name = @from_name; UPDATE {accounts_table} SET balance = balance / 0 WHERE name = @to_name;"
    );

    let json_definitions = serde_json::json!({
        "failing_transfer": {
            "query": failing_sql,
            "args": {
                "from_name": {"type": "string"},
                "to_name": {"type": "string"},
                "initial_balance": {"type": "integer"},
                "amount": {"type": "integer"}
            }
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({
        "from_name": "Alice",
        "to_name": "Bob",
        "initial_balance": 1000,
        "amount": 100
    });

    // The transaction should fail and rollback completely
    let result = query_run_postgresql(&mut client, &queries, "failing_transfer", &params).await;
    assert!(result.is_err());

    // Verify no data was committed due to rollback
    let row_count = client
        .query_one(
            &format!("SELECT COUNT(*) as count FROM {accounts_table}"),
            &[],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(row_count, 0);
}

#[tokio::test]
async fn test_postgres_multiple_list_parameters_in_multi_statement() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let (source_table, accounts_table) = setup_postgres_test_schema(
        &mut client,
        "test_postgres_multiple_list_parameters_in_multi_statement",
    )
    .await;

    // Insert test data for list parameter filtering
    client
        .execute(
            &format!("INSERT INTO {source_table} (name, score, active) VALUES ($1, $2, $3)"),
            &[&"Alice", &95.0, &true],
        )
        .await
        .expect("Failed to insert Alice");

    client
        .execute(
            &format!("INSERT INTO {source_table} (name, score, active) VALUES ($1, $2, $3)"),
            &[&"Bob", &87.5, &false],
        )
        .await
        .expect("Failed to insert Bob");

    client
        .execute(
            &format!("INSERT INTO {source_table} (name, score, active) VALUES ($1, $2, $3)"),
            &[&"Charlie", &92.0, &true],
        )
        .await
        .expect("Failed to insert Charlie");

    // Don't pre-insert accounts data - let the transaction create them

    // Test query with multiple list parameters in multi-statement transaction (mutations only, no returns)
    let json_definitions = serde_json::json!({
        "complex_multi_list_transaction": {
            "query": format!("INSERT INTO {} (name, balance) SELECT name, @initial_balance FROM {} WHERE score IN :[scores] AND active IN :[activities]; UPDATE {} SET balance = balance + @bonus WHERE name IN :[names];", accounts_table, source_table, accounts_table),
            "args": {
                "initial_balance": {"type": "integer"},
                "bonus": {"type": "integer"},
                "scores": {"type": "list", "itemtype": "float"},
                "activities": {"type": "list", "itemtype": "boolean"},
                "names": {"type": "list", "itemtype": "string"}
            }
        },
        "select_all_accounts": {
            "query": format!("SELECT name, balance FROM {} ORDER BY name", accounts_table),
            "returns": ["name", "balance"],
            "args": {}
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    // Execute complex transaction with multiple list parameters
    let params = serde_json::json!({
        "initial_balance": 1500,
        "bonus": 100,
        "scores": [95.0, 92.0],     // Alice (95) and Charlie (92) - Bob (87.5) excluded
        "activities": [true],       // Only active users - Bob (false) excluded
        "names": ["Alice", "Charlie"] // Names to get bonus - both Alice and Charlie
    });

    let result = query_run_postgresql(
        &mut client,
        &queries,
        "complex_multi_list_transaction",
        &params,
    )
    .await
    .unwrap();

    // Verify the transaction executed successfully (no returns for mutations)
    assert!(result.data.is_empty());
    // Verify the SQL statements executed (2 statements: INSERT, UPDATE)
    assert_eq!(result.sql_statements.len(), 2);

    // Now query the results to verify the transaction worked correctly
    let params = serde_json::json!({});
    let result = query_run_postgresql(&mut client, &queries, "select_all_accounts", &params)
        .await
        .unwrap();

    assert_eq!(result.data.len(), 2); // Only Alice and Charlie should be created (Bob is excluded by filters)

    // Verify each account's final balance
    let accounts: Vec<(String, i32)> = result
        .data
        .iter()
        .map(|row| {
            let name = row.get("name").unwrap().as_str().unwrap().to_string();
            let balance = row.get("balance").unwrap().as_i64().unwrap() as i32;
            (name, balance)
        })
        .collect();

    // Alice: initial_balance 1500 (from INSERT) + bonus 100 = 1600
    // Charlie: initial_balance 1500 (from INSERT) + bonus 100 = 1600
    // Bob: not created (filtered out by score and active criteria)
    let alice_balance = accounts
        .iter()
        .find(|(name, _)| name == "Alice")
        .map(|(_, balance)| *balance)
        .unwrap_or(0);
    let bob_balance = accounts
        .iter()
        .find(|(name, _)| name == "Bob")
        .map(|(_, balance)| *balance)
        .unwrap_or(0);
    let charlie_balance = accounts
        .iter()
        .find(|(name, _)| name == "Charlie")
        .map(|(_, balance)| *balance)
        .unwrap_or(0);

    assert_eq!(alice_balance, 1600, "Alice should have 1500 + 100 = 1600");
    assert_eq!(bob_balance, 0, "Bob should not exist (filtered out)");
    assert_eq!(
        charlie_balance, 1600,
        "Charlie should have 1500 + 100 = 1600"
    );
}

#[tokio::test]
async fn test_postgres_multiple_list_parameters_across_multiple_statements() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let (source_table, accounts_table) = setup_postgres_test_schema(
        &mut client,
        "test_postgres_multiple_list_parameters_across_multiple_statements",
    )
    .await;

    // Insert test data for complex multi-statement scenario
    client
        .execute(
            &format!("INSERT INTO {source_table} (name, score, active) VALUES ($1, $2, $3)"),
            &[&"Alice", &95.0, &true],
        )
        .await
        .expect("Failed to insert Alice");

    client
        .execute(
            &format!("INSERT INTO {source_table} (name, score, active) VALUES ($1, $2, $3)"),
            &[&"Bob", &87.5, &false],
        )
        .await
        .expect("Failed to insert Bob");

    client
        .execute(
            &format!("INSERT INTO {source_table} (name, score, active) VALUES ($1, $2, $3)"),
            &[&"Charlie", &92.0, &true],
        )
        .await
        .expect("Failed to insert Charlie");

    client
        .execute(
            &format!("INSERT INTO {source_table} (name, score, active) VALUES ($1, $2, $3)"),
            &[&"David", &89.0, &true],
        )
        .await
        .expect("Failed to insert David");

    // Test multiple statements each using different list parameters
    let json_definitions = serde_json::json!({
        "multi_statement_different_lists": {
            "query": format!("INSERT INTO {} (name, balance) SELECT name, @initial_balance FROM {} WHERE score IN :[scores]; UPDATE {} SET balance = balance + @bonus WHERE name IN :[names]; DELETE FROM {} WHERE active IN :[inactive_statuses];", accounts_table, source_table, accounts_table, source_table),
            "args": {
                "initial_balance": {"type": "integer"},
                "bonus": {"type": "integer"},
                "scores": {"type": "list", "itemtype": "float"},
                "names": {"type": "list", "itemtype": "string"},
                "inactive_statuses": {"type": "list", "itemtype": "boolean"}
            }
        },
        "select_final_accounts": {
            "query": format!("SELECT name, balance FROM {} ORDER BY name", accounts_table),
            "returns": ["name", "balance"],
            "args": {}
        },
        "select_remaining_source": {
            "query": format!("SELECT name FROM {} ORDER BY name", source_table),
            "returns": ["name"],
            "args": {}
        }
    });

    let queries = jankensqlhub::QueryDefinitions::from_json(json_definitions).unwrap();

    // Execute transaction with different list parameters across multiple statements:
    // 1. INSERT uses :[scores] list (floats)
    // 2. UPDATE uses :[names] list (strings)
    // 3. DELETE uses :[inactive_statuses] list (booleans)
    let params = serde_json::json!({
        "initial_balance": 2000,
        "bonus": 50,
        "scores": [95.0, 92.0, 89.0],    // Alice (95), Charlie (92), David (89) - Bob (87.5) excluded
        "names": ["Alice", "David"],       // Only Alice and David get bonus - Charlie excluded
        "inactive_statuses": [false]       // Delete inactive records (Bob) - active records kept
    });

    let result = query_run_postgresql(
        &mut client,
        &queries,
        "multi_statement_different_lists",
        &params,
    )
    .await
    .unwrap();

    // Verify transaction executed - should have 3 statements (INSERT, UPDATE, DELETE)
    assert!(result.data.is_empty()); // No returns for mutations
    assert_eq!(result.sql_statements.len(), 3);

    // Check final account balances
    let params = serde_json::json!({});
    let result = query_run_postgresql(&mut client, &queries, "select_final_accounts", &params)
        .await
        .unwrap();

    // Should have accounts for Alice, Charlie, and David (Bob excluded from INSERT)
    assert_eq!(result.data.len(), 3);

    let accounts: Vec<(String, i32)> = result
        .data
        .iter()
        .map(|row| {
            let name = row.get("name").unwrap().as_str().unwrap().to_string();
            let balance = row.get("balance").unwrap().as_i64().unwrap() as i32;
            (name, balance)
        })
        .collect();

    // Alice: initial_balance 2000 + bonus 50 = 2050 (included in scores AND names)
    // Charlie: initial_balance 2000 = 2000 (included in scores but not in names)
    // David: initial_balance 2000 + bonus 50 = 2050 (included in scores AND names)
    let alice_balance = accounts
        .iter()
        .find(|(name, _)| name == "Alice")
        .map(|(_, balance)| *balance)
        .unwrap();
    let charlie_balance = accounts
        .iter()
        .find(|(name, _)| name == "Charlie")
        .map(|(_, balance)| *balance)
        .unwrap();
    let david_balance = accounts
        .iter()
        .find(|(name, _)| name == "David")
        .map(|(_, balance)| *balance)
        .unwrap();

    assert_eq!(alice_balance, 2050, "Alice got initial balance + bonus");
    assert_eq!(charlie_balance, 2000, "Charlie got only initial balance");
    assert_eq!(david_balance, 2050, "David got initial balance + bonus");

    // Check which records remain in source table after DELETE
    let params = serde_json::json!({});
    let result = query_run_postgresql(&mut client, &queries, "select_remaining_source", &params)
        .await
        .unwrap();

    // Should only have Alice, Charlie, David (Bob deleted because active=false)
    assert_eq!(result.data.len(), 3);

    let remaining_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();

    assert!(remaining_names.contains(&"Alice".to_string()));
    assert!(remaining_names.contains(&"Charlie".to_string()));
    assert!(remaining_names.contains(&"David".to_string()));
    assert!(!remaining_names.contains(&"Bob".to_string()));
}
