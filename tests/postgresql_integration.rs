//! PostgreSQL integration tests for JankenSQLHub
//!
//! These tests verify that PostgreSQL functionality works correctly
//! with prepared statements and parameterized queries.
//!
//! Tests are only run when POSTGRES_CONNECTION_STRING environment variable is set.

use jankensqlhub::{QueryDefinitions, query_run_postgresql};
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
async fn test_postgres_basic_query() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let (source_table, _) =
        setup_postgres_test_schema(&mut client, "test_postgres_basic_query").await;

    // Create a simple query definition
    let json_definitions = serde_json::json!({
        "insert_basic": {
            "query": format!("INSERT INTO {} (name, score, active) VALUES (@name, @score, @active)", source_table),
            "args": {
                "name": {"type": "string"},
                "score": {"type": "float"},
                "active": {"type": "boolean"}
            }
        },
        "select_all": {
            "query": format!("SELECT id, name, score, active FROM {} ORDER BY id", source_table),
            "returns": ["id", "name", "score", "active"],
            "args": {}
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Test insert
    let params = serde_json::json!({"name": "Alice", "score": 95.5, "active": true});
    let result = query_run_postgresql(&mut client, &queries, "insert_basic", &params)
        .await
        .unwrap();
    assert!(result.data.is_empty());
    assert_eq!(result.sql_statements.len(), 1);

    // Test select
    let params = serde_json::json!({});
    let result = query_run_postgresql(&mut client, &queries, "select_all", &params)
        .await
        .unwrap();
    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], serde_json::json!("Alice"));
    assert_eq!(result.data[0]["score"], serde_json::json!(95.5));
    assert_eq!(result.data[0]["active"], serde_json::json!(true));
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

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

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

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

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
async fn test_postgres_list_parameters() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let (source_table, _) =
        setup_postgres_test_schema(&mut client, "test_postgres_list_parameters").await;

    // Insert test data
    client
        .execute(
            &format!("INSERT INTO {source_table} (name, score, active) VALUES ($1, $2, $3), ($4, $5, $6), ($7, $8, $9)"),
            &[&"Alice", &95.0, &true, &"Bob", &87.5, &false, &"Charlie", &92.0, &true],
        )
        .await
        .unwrap();

    let json_definitions = serde_json::json!({
        "select_by_names": {
            "query": format!("SELECT id, name, score FROM {} WHERE name IN :[names] ORDER BY id", source_table),
            "returns": ["id", "name", "score"],
            "args": {
                "names": { "itemtype": "string" }
            }
        },
        "select_by_ids": {
            "query": format!("SELECT name FROM {} WHERE id IN :[ids] ORDER BY id", source_table),
            "returns": ["name"],
            "args": {
                "ids": { "itemtype": "integer" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Test string list
    let params = serde_json::json!({ "names": ["Alice", "Charlie"] });
    let result = query_run_postgresql(&mut client, &queries, "select_by_names", &params)
        .await
        .unwrap();
    assert_eq!(result.data.len(), 2);
    assert_eq!(result.data[0]["name"], serde_json::json!("Alice"));
    assert_eq!(result.data[1]["name"], serde_json::json!("Charlie"));

    // Test integer list
    let params = serde_json::json!({ "ids": [2, 3] });
    let result = query_run_postgresql(&mut client, &queries, "select_by_ids", &params)
        .await
        .unwrap();
    assert_eq!(result.data.len(), 2);
    assert_eq!(result.data[0]["name"], serde_json::json!("Bob"));
    assert_eq!(result.data[1]["name"], serde_json::json!("Charlie"));
}

#[tokio::test]
async fn test_postgres_table_name_parameters() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    // Create test tables with similar schemas
    let _ = client.execute("DROP TABLE IF EXISTS table1", &[]).await;
    let _ = client.execute("DROP TABLE IF EXISTS table2", &[]).await;

    client
        .execute("CREATE TABLE table1 (id INTEGER, name TEXT)", &[])
        .await
        .unwrap();
    client
        .execute("CREATE TABLE table2 (id INTEGER, name TEXT)", &[])
        .await
        .unwrap();

    client
        .execute("INSERT INTO table1 VALUES (1, 'Alice'), (2, 'Bob')", &[])
        .await
        .unwrap();
    client
        .execute("INSERT INTO table2 VALUES (1, 'Charlie')", &[])
        .await
        .unwrap();

    let json_definitions = serde_json::json!({
        "select_from_table": {
            "query": "SELECT id, name FROM #[table_name] WHERE id = @id",
            "returns": ["id", "name"],
            "args": {
                "table_name": { "type": "table_name" },
                "id": { "type": "integer" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Test table1
    let params = serde_json::json!({"table_name": "table1", "id": 1});
    let result = query_run_postgresql(&mut client, &queries, "select_from_table", &params)
        .await
        .unwrap();
    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], serde_json::json!("Alice"));

    // Test table2
    let params = serde_json::json!({"table_name": "table2", "id": 1});
    let result = query_run_postgresql(&mut client, &queries, "select_from_table", &params)
        .await
        .unwrap();
    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], serde_json::json!("Charlie"));
}

#[tokio::test]
async fn test_postgres_sql_injection_protection() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let (source_table, _) =
        setup_postgres_test_schema(&mut client, "test_postgres_sql_injection_protection").await;

    // Insert safe data first
    client
        .execute(
            &format!("INSERT INTO {source_table} (name, score, active) VALUES ($1, $2, $3)"),
            &[&"TestUser", &95.0, &true],
        )
        .await
        .unwrap();

    let json_definitions = serde_json::json!({
        "select_by_name": {
            "query": format!("SELECT id FROM {} WHERE name = @name", source_table),
            "returns": ["id"],
            "args": {
                "name": {"type": "string"}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Test SQL injection attempt (should not work with prepared statements)
    let injection_attempt = "TestUser'; DROP TABLE source; --";
    let params = serde_json::json!({"name": injection_attempt});
    let result = query_run_postgresql(&mut client, &queries, "select_by_name", &params)
        .await
        .unwrap();

    // Should find no matches (injection prevented)
    assert_eq!(result.data.len(), 0);

    // Try with safe name
    let params = serde_json::json!({"name": "TestUser"});
    let result = query_run_postgresql(&mut client, &queries, "select_by_name", &params)
        .await
        .unwrap();
    assert_eq!(result.data.len(), 1);
}

#[tokio::test]
async fn test_postgres_empty_list_error() {
    let Some(mut client) = setup_postgres_connection().await else {
        println!("Skipping PostgreSQL tests - POSTGRES_CONNECTION_STRING not set");
        return;
    };

    let json_definitions = serde_json::json!({
        "select_empty_list": {
            "query": "SELECT 1 as dummy WHERE 1 IN :[ids]",
            "returns": ["dummy"],
            "args": {
                "ids": { "itemtype": "integer" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Empty list should result in error (no table needed for this parameter validation)
    let params = serde_json::json!({"ids": []});
    let result = query_run_postgresql(&mut client, &queries, "select_empty_list", &params).await;
    assert!(result.is_err());
}
