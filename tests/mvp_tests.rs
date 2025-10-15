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

    let params = serde_json::json!({});
    let result = db_conn.query_run(&queries, "select_all", &params).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.contains(&serde_json::json!({"id": 1, "name": "John", "score": null})));
    assert!(result.contains(&serde_json::json!({"id": 2, "name": "Jane", "score": null})));
}

#[test]
fn test_sqlite_insert_with_params() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let conn = setup_db();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    let params = serde_json::json!({"name": "NewGuy"});
    let insert_result = db_conn
        .query_run(&queries, "insert_single", &params)
        .unwrap();
    assert!(insert_result.is_empty());

    let params = serde_json::json!({});
    let result = db_conn.query_run(&queries, "select_all", &params).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        serde_json::json!({"id": 1, "name": "NewGuy", "score": null})
    );
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

    // Verify by select specific with new id - returns structured data now
    let params = serde_json::json!({"id": 10, "name": "NewJohn"});
    let result = db_conn.query_run(&queries, "my_list", &params).unwrap();
    assert_eq!(
        result,
        vec![serde_json::json!({"id": 10, "name": "NewJohn"})]
    );
}

#[test]
fn test_sqlite_blob_column_type() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE test_table (id INTEGER, data BLOB)", [])
        .unwrap();
    conn.execute(
        "INSERT INTO test_table VALUES (1, X'010203'), (2, NULL)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    let json_definitions = serde_json::json!({
        "select_blob": {
            "query": "SELECT id, data FROM test_table ORDER BY id",
            "returns": ["id", "data"],
            "args": {}
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({});
    let result = db_conn.query_run(&queries, "select_blob", &params).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].get("id"), Some(&serde_json::json!(1)));
    assert_eq!(result[0].get("data"), Some(&serde_json::json!([1, 2, 3])));
    assert_eq!(result[1].get("id"), Some(&serde_json::json!(2)));
    assert_eq!(result[1].get("data"), Some(&serde_json::json!(null)));
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
            "returns": ["id", "name", "score"],
            "args": {
                "active": { "type": "boolean" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({"id": 3, "name": "user3", "active": true});
    let insert_result = db_conn
        .query_run(&queries, "insert_with_bool", &params)
        .unwrap();
    assert!(insert_result.is_empty());

    let params = serde_json::json!({"id": 4, "name": "user4", "active": false});
    db_conn
        .query_run(&queries, "insert_with_bool", &params)
        .unwrap();

    let params = serde_json::json!({"active": true});
    let result = db_conn
        .query_run(&queries, "select_by_bool", &params)
        .unwrap();

    assert_eq!(result.len(), 2);
    assert!(result.contains(&serde_json::json!({"id": 1, "name": "active", "score": 1.0})));
    assert!(result.contains(&serde_json::json!({"id": 3, "name": "user3", "score": 1.0})));
}

#[test]
fn test_loading_from_json_value() {
    let json_definitions = serde_json::json!({
        "test_select": {
            "query": "select * from source where id=@id",
            "returns": ["id", "name", "score"],
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
    conn.execute("INSERT INTO source VALUES (42, 'Test', NULL)", [])
        .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    let params = serde_json::json!({"id": 42});
    let result = db_conn.query_run(&queries, "test_select", &params).unwrap();
    assert!(!result.is_empty());
    assert_eq!(
        result[0],
        serde_json::json!({"id": 42, "name": "Test", "score": null})
    );

    let params = serde_json::json!({"id": 99, "name": "JsonLoaded"});
    let insert_result = db_conn.query_run(&queries, "test_insert", &params).unwrap();
    assert!(insert_result.is_empty());

    let params = serde_json::json!({"id": 99});
    let result = db_conn.query_run(&queries, "test_select", &params).unwrap();
    assert!(!result.is_empty());
    assert_eq!(
        result[0],
        serde_json::json!({"id": 99, "name": "JsonLoaded", "score": null})
    );
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

    // Should return both Bob (id=3) and Jane (id=2) as structured objects
    assert_eq!(result.len(), 2);
    // Check that we got the expected structured data for Bob and Jane
    assert!(result.contains(&serde_json::json!({"id": 2, "name": "Jane"}))); // Jane with score 8.2
    assert!(result.contains(&serde_json::json!({"id": 3, "name": "Bob"}))); // Bob with score 7.0
}
