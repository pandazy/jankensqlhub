use jankensqlhub::{QueryDefinitions, query_run_sqlite};
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
    let mut conn = setup_db();
    conn.execute(
        "INSERT INTO source VALUES (1, 'John', NULL), (2, 'Jane', NULL)",
        [],
    )
    .unwrap();

    let params = serde_json::json!({});
    let result = query_run_sqlite(&mut conn, &queries, "select_all", &params).unwrap();
    assert_eq!(result.data.len(), 2);
    assert!(
        result
            .data
            .contains(&serde_json::json!({"id": 1, "name": "John", "score": null}))
    );
    assert!(
        result
            .data
            .contains(&serde_json::json!({"id": 2, "name": "Jane", "score": null}))
    );
}

#[test]
fn test_sqlite_insert_with_params() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let mut conn = setup_db();

    let params = serde_json::json!({"name": "NewGuy"});
    let insert_result = query_run_sqlite(&mut conn, &queries, "insert_single", &params).unwrap();
    assert!(insert_result.data.is_empty());

    let params = serde_json::json!({});
    let result = query_run_sqlite(&mut conn, &queries, "select_all", &params).unwrap();
    assert_eq!(result.data.len(), 1);
    assert_eq!(
        result.data[0],
        serde_json::json!({"id": 1, "name": "NewGuy", "score": null})
    );
}

#[test]
fn test_sqlite_update_with_params() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let mut conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'John', NULL)", [])
        .unwrap();

    // Update
    // Note: my_action doesn't use table name parameters, so it doesn't need "source"
    let params = serde_json::json!({"new_id": 10, "new_name": "NewJohn", "old_id": 1});
    query_run_sqlite(&mut conn, &queries, "my_action", &params).unwrap();

    // Verify by select specific with new id - returns structured data now
    let params = serde_json::json!({"id": 10, "name": "NewJohn", "source": "source"});
    let result = query_run_sqlite(&mut conn, &queries, "my_list", &params).unwrap();
    assert_eq!(
        result.data,
        vec![serde_json::json!({"id": 10, "name": "NewJohn"})]
    );
}

#[test]
fn test_sqlite_blob_column_type() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE test_table (id INTEGER, data BLOB)", [])
        .unwrap();
    conn.execute(
        "INSERT INTO test_table VALUES (1, X'010203'), (2, NULL)",
        [],
    )
    .unwrap();

    let json_definitions = serde_json::json!({
        "select_blob": {
            "query": "SELECT id, data FROM test_table ORDER BY id",
            "returns": ["id", "data"],
            "args": {}
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({});
    let result = query_run_sqlite(&mut conn, &queries, "select_blob", &params).unwrap();

    assert_eq!(result.data.len(), 2);
    assert_eq!(result.data[0].get("id"), Some(&serde_json::json!(1)));
    assert_eq!(
        result.data[0].get("data"),
        Some(&serde_json::json!([1, 2, 3]))
    );
    assert_eq!(result.data[1].get("id"), Some(&serde_json::json!(2)));
    assert_eq!(result.data[1].get("data"), Some(&serde_json::json!(null)));
}

#[test]
fn test_boolean_params() {
    let mut conn = setup_db();
    conn.execute(
        "INSERT INTO source VALUES (1, 'active', 1), (2, 'inactive', 0)",
        [],
    )
    .unwrap();

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
    let insert_result = query_run_sqlite(&mut conn, &queries, "insert_with_bool", &params).unwrap();
    assert!(insert_result.data.is_empty());

    let params = serde_json::json!({"id": 4, "name": "user4", "active": false});
    query_run_sqlite(&mut conn, &queries, "insert_with_bool", &params).unwrap();

    let params = serde_json::json!({"active": true});
    let result = query_run_sqlite(&mut conn, &queries, "select_by_bool", &params).unwrap();

    assert_eq!(result.data.len(), 2);
    assert!(
        result
            .data
            .contains(&serde_json::json!({"id": 1, "name": "active", "score": 1.0}))
    );
    assert!(
        result
            .data
            .contains(&serde_json::json!({"id": 3, "name": "user3", "score": 1.0}))
    );
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
    let mut conn = setup_db();
    conn.execute("INSERT INTO source VALUES (42, 'Test', NULL)", [])
        .unwrap();

    let params = serde_json::json!({"id": 42});
    let result = query_run_sqlite(&mut conn, &queries, "test_select", &params).unwrap();
    assert!(!result.data.is_empty());
    assert_eq!(
        result.data[0],
        serde_json::json!({"id": 42, "name": "Test", "score": null})
    );

    let params = serde_json::json!({"id": 99, "name": "JsonLoaded"});
    let insert_result = query_run_sqlite(&mut conn, &queries, "test_insert", &params).unwrap();
    assert!(insert_result.data.is_empty());

    let params = serde_json::json!({"id": 99});
    let result = query_run_sqlite(&mut conn, &queries, "test_select", &params).unwrap();
    assert!(!result.data.is_empty());
    assert_eq!(
        result.data[0],
        serde_json::json!({"id": 99, "name": "JsonLoaded", "score": null})
    );
}

#[test]
fn test_sqlite_float_params() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let mut conn = setup_db();
    conn.execute(
        "INSERT INTO source VALUES (1, 'John', 5.5), (2, 'Jane', 8.2)",
        [],
    )
    .unwrap();

    // Insert with float
    let params = serde_json::json!({"id": 3, "name": "Bob", "score": 7.0});
    query_run_sqlite(&mut conn, &queries, "insert_with_float", &params).unwrap();

    // Select with float param (score > 6.0)
    // Should return Jane (8.2) and Bob (7.0), but not John (5.5)
    let params = serde_json::json!({"min_score": 6.0});
    let result = query_run_sqlite(&mut conn, &queries, "select_with_float", &params).unwrap();

    // Should return both Bob (id=3) and Jane (id=2) as structured objects
    assert_eq!(result.data.len(), 2);
    // Check that we got the expected structured data for Bob and Jane
    assert!(
        result
            .data
            .contains(&serde_json::json!({"id": 2, "name": "Jane"}))
    ); // Jane with score 8.2
    assert!(
        result
            .data
            .contains(&serde_json::json!({"id": 3, "name": "Bob"}))
    ); // Bob with score 7.0
}

#[test]
fn test_debug_sql_statements() {
    let queries = QueryDefinitions::from_file("test_json/def.json").unwrap();
    let mut conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'TestUser', NULL)", [])
        .unwrap();

    // Test a simple select query to see the SQL statement
    let params = serde_json::json!({});
    let result = query_run_sqlite(&mut conn, &queries, "select_all", &params).unwrap();

    // Check that the SQL statement is as expected (lowercase select)
    assert!(
        result
            .sql_statements
            .iter()
            .any(|s| s.contains("select * from source"))
    );
    assert!(result.sql_statements[0] == "select * from source");
}

#[test]
fn test_blob_parameter_basics() {
    // MVP test showing blob parameters work with text-to-bytes conversion
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE files (id INTEGER, filename TEXT, content BLOB)",
        [],
    )
    .unwrap();

    let json_definitions = serde_json::json!({
        "save_file": {
            "query": "INSERT INTO files (id, filename, content) VALUES (@id, @filename, @content)",
            "args": {
                "id": { "type": "integer" },
                "filename": { "type": "string" },
                "content": { "type": "blob", "range": [1, 1000] }
            }
        },
        "get_file": {
            "query": "SELECT filename, content FROM files WHERE id=@id",
            "returns": ["filename", "content"],
            "args": {
                "id": { "type": "integer" }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Convert text to bytes and test blob storage/retrieval
    let file_content = "Hello World! This is a test file.";
    let blob_bytes: Vec<u8> = file_content.as_bytes().to_vec();
    let blob_json: Vec<serde_json::Value> =
        blob_bytes.iter().map(|&b| serde_json::json!(b)).collect();

    // Save the file with blob content
    let params = serde_json::json!({"id": 1, "filename": "hello.txt", "content": blob_json});
    let result = query_run_sqlite(&mut conn, &queries, "save_file", &params).unwrap();
    assert!(result.data.is_empty());
    assert!(result.sql_statements[0].contains("INSERT INTO files"));

    // Retrieve the file and verify blob content
    let params = serde_json::json!({"id": 1});
    let result = query_run_sqlite(&mut conn, &queries, "get_file", &params).unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["filename"], "hello.txt");
    assert_eq!(result.data[0]["content"], serde_json::json!(blob_json));
}

#[test]
fn test_table_name_column_syntax() {
    // Test demonstrating that # syntax works for column names, not just table names
    let mut conn = setup_db();
    conn.execute(
        "INSERT INTO source VALUES (1, 'John', 8.5), (2, 'Jane', 9.2)",
        [],
    )
    .unwrap();

    let json_definitions = serde_json::json!({
        "select_column": {
            "query": "SELECT #[column_name] FROM source ORDER BY #[column_name]",
            "returns": ["name"],
            "args": {
                "column_name": { "enum": ["id", "name", "score"] }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Test selecting from the "name" column
    let params = serde_json::json!({"column_name": "name"});
    let result = query_run_sqlite(&mut conn, &queries, "select_column", &params).unwrap();

    // Should return values ordered alphabetically: Jane, John
    assert_eq!(result.data.len(), 2);
    assert_eq!(result.data[0]["name"], "Jane");
    assert_eq!(result.data[1]["name"], "John");
}
