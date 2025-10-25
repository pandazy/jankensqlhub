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
fn test_parameter_parsing_edge_cases() {
    use jankensqlhub::parameters::parse_parameters_with_quotes;

    // Test with various valid parameter names that match the regex \w+
    let test_cases = vec![
        "SELECT * FROM table WHERE value=@a",        // Single character
        "SELECT * FROM table WHERE value=@param123", // Alphanumeric
        "SELECT * FROM table WHERE value=@PARAM",    // Uppercase
        "SELECT * FROM table WHERE value=@param_name", // Underscore
        "SELECT * FROM table WHERE value=@p0a1r2a3m4", // Numbers mixed
        "@valid @another",                           // Multiple parameters
    ];

    for sql in &test_cases {
        // These should all parse without triggering "missing parameter name" errors
        let result = parse_parameters_with_quotes(sql);
        assert!(result.is_ok(), "Failed to parse parameters from SQL: {sql}",);
    }

    // Test edge case with just @ - this should NOT match the regex since @ is not followed by \w+
    let result = parse_parameters_with_quotes("@");
    assert!(result.is_ok());
    let parameters = result.unwrap();
    assert_eq!(parameters.len(), 0); // Should not parse as a parameter

    // Test exclusion: parameters inside quotes should not be parsed
    let sql = "SELECT * FROM table WHERE value='@not_param' AND other=@real_param";
    let parameters = parse_parameters_with_quotes(sql).unwrap();
    assert_eq!(parameters.len(), 1); // Only @real_param should be found
    assert_eq!(parameters[0].name, "real_param");
}

#[test]
fn test_multi_table_name_parameters() {
    let mut conn = Connection::open_in_memory().unwrap();

    // Create three tables with test data
    conn.execute(
        "CREATE TABLE table1 (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE table2 (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE table3 (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();

    conn.execute("INSERT INTO table1 VALUES (1, 'Alice')", [])
        .unwrap();
    conn.execute("INSERT INTO table2 VALUES (2, 'Bob')", [])
        .unwrap();
    conn.execute("INSERT INTO table3 VALUES (3, 'Charlie')", [])
        .unwrap();

    // Test query with three table name parameters
    let json_definitions = serde_json::json!({
        "multi_table_test": {
            "query": "SELECT DISTINCT t1.id AS id1, t1.name AS name1, t2.id AS id2, t2.name AS name2, t3.id AS id3, t3.name AS name3 FROM #[table1] t1, #[table2] t2, #[table3] t3 WHERE t1.id = 1 AND t2.id = 2 AND t3.id = 3",
            "returns": ["id1", "name1", "id2", "name2", "id3", "name3"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let params = serde_json::json!({"table1": "table1", "table2": "table2", "table3": "table3"});
    let result = query_run_sqlite(&mut conn, &queries, "multi_table_test", &params).unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(
        result.data[0],
        serde_json::json!({"id1": 1, "name1": "Alice", "id2": 2, "name2": "Bob", "id3": 3, "name3": "Charlie"})
    );
}

#[test]
fn test_concatenated_table_name_parameters() {
    use jankensqlhub::parameters::parse_parameters_with_quotes;

    // Test that the new #[ ] syntax correctly handles concatenated table names
    // The old # syntax couldn't handle this because underscores are valid in table names

    let sql = "SELECT * FROM rel_#[resource_a]_#[resource_b] WHERE id = @id";
    let parameters = parse_parameters_with_quotes(sql).unwrap();

    // Should parse two table name parameters and one regular parameter
    assert_eq!(parameters.len(), 3);

    // Check parameter names
    let mut param_names: Vec<String> = parameters.iter().map(|p| p.name.clone()).collect();
    param_names.sort();

    assert_eq!(param_names, vec!["id", "resource_a", "resource_b"]);

    // Check parameter types
    assert!(
        parameters
            .iter()
            .any(|p| p.name == "id" && p.param_type.to_string() == "string")
    );
    assert!(
        parameters
            .iter()
            .any(|p| p.name == "resource_a" && p.param_type.to_string() == "table_name")
    );
    assert!(
        parameters
            .iter()
            .any(|p| p.name == "resource_b" && p.param_type.to_string() == "table_name")
    );

    // With the new #[ ] syntax, we can clearly separate table name parameters
    // even when they appear in concatenated strings like relationship table names
}

#[test]
fn test_list_parameter_functionality() {
    let queries = QueryDefinitions::from_file("test_json/crud.json").unwrap();

    // Create test table with several rows
    let mut conn = setup_db();
    conn.execute("INSERT INTO source VALUES (1, 'Alice', 95.0, 1)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (5, 'Bob', 87.5, 0)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (10, 'Charlie', 92.0, 1)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (15, 'David', 88.5, 0)", [])
        .unwrap();
    conn.execute("INSERT INTO source VALUES (20, 'Eve', 91.0, 1)", [])
        .unwrap();

    let params = serde_json::json!({"table": "source", "targets": [1, 5, 10]});

    let result = query_run_sqlite(&mut conn, &queries, "read", &params).unwrap();

    // Should return 3 rows matching the ids [1, 5, 10]
    assert_eq!(result.data.len(), 3);

    let names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"Alice".to_string()));
    assert!(names.contains(&"Bob".to_string()));
    assert!(names.contains(&"Charlie".to_string()));

    // Test with different array - should only return IDs 5 and 15
    let params = serde_json::json!({"table": "source", "targets": [5, 15]});
    let result = query_run_sqlite(&mut conn, &queries, "read", &params).unwrap();
    assert_eq!(result.data.len(), 2);

    let names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"Bob".to_string()));
    assert!(names.contains(&"David".to_string()));

    // Test empty list - should fail
    let params = serde_json::json!({"table": "source", "targets": []});
    let result = query_run_sqlite(&mut conn, &queries, "read", &params);
    assert!(result.is_err());

    // Test multiple list parameters
    let params = serde_json::json!({"table": "source", "ids": [1, 5], "scores": [95.0, 87.5]});
    let result = query_run_sqlite(&mut conn, &queries, "multi_list", &params).unwrap();

    // Should return records where id IN [1, 5] AND score IN [95.0, 87.5]
    // This matches Alice (id=1, score=95.0) and Bob (id=5, score=87.5)
    assert_eq!(result.data.len(), 2);

    let names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"Alice".to_string()));
    assert!(names.contains(&"Bob".to_string()));

    // Test string list parameters
    let params = serde_json::json!({"table": "source", "names": ["Alice", "Charlie", "Eve"]});
    let result = query_run_sqlite(&mut conn, &queries, "string_list", &params).unwrap();

    // Should return 3 rows matching the names ["Alice", "Charlie", "Eve"]
    assert_eq!(result.data.len(), 3);

    let returned_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(returned_names.contains(&"Alice".to_string()));
    assert!(returned_names.contains(&"Charlie".to_string()));
    assert!(returned_names.contains(&"Eve".to_string()));

    // Test with different string array - should only return Alice and Eve
    let params = serde_json::json!({"table": "source", "names": ["Alice", "Eve"]});
    let result = query_run_sqlite(&mut conn, &queries, "string_list", &params).unwrap();
    assert_eq!(result.data.len(), 2);

    let returned_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(returned_names.contains(&"Alice".to_string()));
    assert!(returned_names.contains(&"Eve".to_string()));

    // Test boolean list parameters
    let params = serde_json::json!({"table": "source", "statuses": [true, false]});
    let result = query_run_sqlite(&mut conn, &queries, "boolean_list", &params).unwrap();

    // Should return all 5 rows since active contains both true and false values
    assert_eq!(result.data.len(), 5);

    // Test with only true values
    let params = serde_json::json!({"table": "source", "statuses": [true]});
    let result = query_run_sqlite(&mut conn, &queries, "boolean_list", &params).unwrap();

    // Should return 3 rows with active=true (Alice=1, Charlie=1, Eve=1)
    assert_eq!(result.data.len(), 3);

    let returned_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(returned_names.contains(&"Alice".to_string()));
    assert!(returned_names.contains(&"Charlie".to_string()));
    assert!(returned_names.contains(&"Eve".to_string()));

    // Test with only false values
    let params = serde_json::json!({"table": "source", "statuses": [false]});
    let result = query_run_sqlite(&mut conn, &queries, "boolean_list", &params).unwrap();

    // Should return 2 rows with active=false (Bob=0, David=0)
    assert_eq!(result.data.len(), 2);

    let returned_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(returned_names.contains(&"Bob".to_string()));
    assert!(returned_names.contains(&"David".to_string()));
}
