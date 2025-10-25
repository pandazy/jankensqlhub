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
