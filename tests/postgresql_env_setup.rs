#[cfg(test)]
mod tests {
    use tokio_postgres::NoTls;

    /// Simple test to verify PostgreSQL environment setup
    /// This test demonstrates basic connectivity and can be run locally or in CI
    #[tokio::test]
    async fn test_postgresql_basic_connectivity() {
        // Require POSTGRES_CONNECTION_STRING environment variable
        let connection_string = std::env::var("POSTGRES_CONNECTION_STRING")
            .expect("PostgreSQL test requires POSTGRES_CONNECTION_STRING environment variable. See op.md for setup instructions.");

        // Attempt to connect to PostgreSQL
        let (client, connection) = match tokio_postgres::connect(&connection_string, NoTls).await {
            Ok(result) => result,
            Err(e) => {
                panic!("Failed to connect to PostgreSQL: {e}");
            }
        };

        // Spawn connection handling task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {e}");
            }
        });

        // Test basic query execution
        match client.execute("SELECT 1", &[]).await {
            Ok(_) => println!("PostgreSQL basic connectivity test passed"),
            Err(e) => panic!("Failed to execute basic query: {e}"),
        }
    }

    /// Test that demonstrates setting up and tearing down test database
    #[tokio::test]
    async fn test_postgresql_database_setup() {
        let connection_string = match std::env::var("POSTGRES_CONNECTION_STRING") {
            Ok(conn_str) => conn_str,
            Err(_) => {
                println!("Skipping PostgreSQL database setup test");
                return;
            }
        };

        let (client, connection) = tokio_postgres::connect(&connection_string, NoTls)
            .await
            .expect("Failed to connect to PostgreSQL");

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {e}");
            }
        });

        // Create a test table
        client
            .execute(
                "CREATE TEMP TABLE test_table (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
                &[],
            )
            .await
            .expect("Failed to create test table");

        // Insert test data
        client
            .execute(
                "INSERT INTO test_table (name) VALUES ($1), ($2)",
                &[&"Alice", &"Bob"],
            )
            .await
            .expect("Failed to insert test data");

        // Query the data back
        let rows = client
            .query("SELECT id, name FROM test_table ORDER BY id", &[])
            .await
            .expect("Failed to query test data");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<_, i32>(0), 1);
        assert_eq!(rows[0].get::<_, &str>(1), "Alice");
        assert_eq!(rows[1].get::<_, i32>(0), 2);
        assert_eq!(rows[1].get::<_, &str>(1), "Bob");

        println!("PostgreSQL database setup and teardown test passed");
    }
}
