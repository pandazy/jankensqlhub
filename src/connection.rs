use crate::{
    QueryDefinitions,
    result::Result,
    runner::{QueryRunner, query_run_sqlite},
};
use rusqlite::Connection;

/// Database connection enum that holds different database backends
pub enum DatabaseConnection {
    /// SQLite connection
    SQLite(Connection),
}

impl QueryRunner for DatabaseConnection {
    fn query_run(
        &mut self,
        queries: &QueryDefinitions,
        query_name: &str,
        params: &serde_json::Value,
    ) -> Result<Vec<serde_json::Value>> {
        match self {
            DatabaseConnection::SQLite(conn) => query_run_sqlite(conn, queries, query_name, params),
        }
    }
}
