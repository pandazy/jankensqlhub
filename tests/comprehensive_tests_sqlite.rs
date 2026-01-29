use jankensqlhub::QueryDefinitions;
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
fn test_sqlite_row_error_handling() {
    let mut conn = setup_db();
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

    let json_definitions = serde_json::json!({
        "select_wide_with_types": {
            "query": "SELECT a, b, c, d, e FROM test_wide",
            "returns": ["a", "b", "c", "d", "e", "missing_field"],
            "args": {}
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({});
    let result =
        jankensqlhub::query_run_sqlite(&mut conn, &queries, "select_wide_with_types", &params)
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
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE test_float (id INTEGER, value REAL)", [])
        .unwrap();
    // Insert Infinity (1.0/0.0) and invalid text that SQLite can't convert to float
    conn.execute(
        "INSERT INTO test_float VALUES (1, 1.0 / 0.0), (2, 'invalid')",
        [],
    )
    .unwrap();

    let json_definitions = serde_json::json!({
        "select_float": {
            "query": "SELECT value FROM test_float ORDER BY id",
            "returns": ["value"],
            "args": {}
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({});
    let result =
        jankensqlhub::query_run_sqlite(&mut conn, &queries, "select_float", &params).unwrap();

    assert_eq!(result.data.len(), 2);
    // Check exact response values
    assert_eq!(result.data[0], serde_json::json!({"value": null})); // Infinity -> null
    assert_eq!(result.data[1], serde_json::json!({"value": "invalid"}));
}

#[test]
fn test_sqlite_missing_field_returns_null() {
    // This test specifically covers line 183 in runner_sqlite.rs
    // where a field in returns doesn't exist in the actual column names
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE test_data (id INTEGER, name TEXT)", [])
        .unwrap();
    conn.execute("INSERT INTO test_data VALUES (1, 'test')", [])
        .unwrap();

    let json_definitions = serde_json::json!({
        "select_with_missing_field": {
            "query": "SELECT id, name FROM test_data",
            "returns": ["id", "name", "nonexistent_column"],
            "args": {}
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = serde_json::json!({});
    let result =
        jankensqlhub::query_run_sqlite(&mut conn, &queries, "select_with_missing_field", &params)
            .unwrap();

    assert_eq!(result.data.len(), 1);
    let row = &result.data[0];

    // Verify existing fields are returned correctly
    assert_eq!(row.get("id"), Some(&serde_json::json!(1)));
    assert_eq!(row.get("name"), Some(&serde_json::json!("test")));

    // Verify non-existent field returns null (this exercises line 183)
    assert_eq!(
        row.get("nonexistent_column"),
        Some(&serde_json::json!(null))
    );
}
