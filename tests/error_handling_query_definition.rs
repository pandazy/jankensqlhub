use jankensqlhub::{
    JankenError, M_EXPECTED, M_GOT, M_QUERY_NAME, QueryDefinitions, error_meta, query_run_sqlite,
};
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
fn test_query_not_found() {
    // Test QueryNotFound error for non-existent query names
    let mut conn = setup_db();

    // Load valid queries from inline JSON
    let queries_json = serde_json::json!({
        "my_list": {
            "query": "select * from source where id=@id and name=@name",
            "args": {
                "id": { "type": "integer" },
                "name": { "type": "string" }
            }
        },
        "select_all": {
            "query": "select * from source"
        }
    });
    let queries = QueryDefinitions::from_json(queries_json).unwrap();

    // Try to run a query that doesn't exist
    let params = serde_json::json!({});
    let err = query_run_sqlite(&mut conn, &queries, "non_existent_query", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::QueryNotFound { data }) = err.downcast::<JankenError>() {
        let name = error_meta(&data, M_QUERY_NAME).unwrap();
        assert_eq!(name, "non_existent_query");
    } else {
        panic!("Expected QueryNotFound error, got: {err_str}");
    }
}

#[test]
fn test_transaction_keywords_error_from_sql() {
    // Test that transaction keywords in SQL cause QueryDef::from_sql to return an error
    let bad_sql = "SELECT * FROM table; COMMIT;";
    let result = jankensqlhub::query::QueryDef::from_sql(bad_sql, None);
    assert!(result.is_err());

    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { .. } => {
            // Error is the expected type, good
        }
        _ => panic!("Unexpected error type: {err:?}"),
    }
}

#[test]
fn test_invalid_json_input_from_json() {
    // Test QueryDefinitions::from_json with non-object input (covers line 115-117)
    let bad_json = serde_json::json!(["array_instead_of_object"]);
    let result = jankensqlhub::query::QueryDefinitions::from_json(bad_json);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { .. }) = err.downcast::<JankenError>() {
        // Error is the expected type, good
    } else {
        panic!("Unexpected error type: {err_str}");
    }
}

#[test]
fn test_invalid_query_definition_structure_from_json() {
    // Test QueryDefinitions::from_json with invalid query definition structure (covers line 118-120)
    let bad_definition = serde_json::json!({
        "bad_query_def": "string_instead_of_object"
    });
    let result = jankensqlhub::query::QueryDefinitions::from_json(bad_definition);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { .. }) = err.downcast::<JankenError>() {
        // Error is the expected type, good
    } else {
        panic!("Unexpected error type: {err_str}");
    }
}

#[test]
fn test_from_json_invalid_returns_field() {
    // Test that QueryDefinitions::from_json fails when returns field is not an array
    let json_definitions = serde_json::json!({
        "bad_query": {
            "query": "SELECT * FROM test",
            "returns": "not an array"
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "array of strings");
        assert!(got.contains("not an array"));
    } else {
        panic!("Expected ParameterTypeMismatch for invalid returns field, got: {err_str}");
    }
}

#[test]
fn test_from_json_non_object_input() {
    // Test that QueryDefinitions::from_json fails with expected error when input is not an object
    use jankensqlhub::QueryDefinitions;

    // Test with string value
    let json_string = serde_json::json!("not an object");
    let result = QueryDefinitions::from_json(json_string);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "object");
        assert_eq!(got, "\"not an object\"");
    } else {
        panic!("Expected ParameterTypeMismatch, got: {err_str}");
    }

    // Test with array value
    let json_array = serde_json::json!(["not", "an", "object"]);
    let result = QueryDefinitions::from_json(json_array);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(expected, "object");
    } else {
        panic!("Expected ParameterTypeMismatch, got: {err_str}");
    }

    // Test with number value
    let json_number = serde_json::json!(42);
    let result = QueryDefinitions::from_json(json_number);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(expected, "object");
    } else {
        panic!("Expected ParameterTypeMismatch, got: {err_str}");
    }
}

#[test]
fn test_from_json_missing_query_field() {
    // Test that QueryDefinitions::from_json fails when 'query' field is missing
    use jankensqlhub::QueryDefinitions;

    let json_definitions = serde_json::json!({
        "missing_query": {
            "args": {
                "id": {"type": "integer"}
            },
            "returns": ["id", "name"]
        },
        "missing_query2": {
            // completely empty object
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "required 'query' field with string value");
        assert_eq!(got, "missing_query: missing 'query' field");
    } else {
        panic!("Expected ParameterTypeMismatch for missing query field, got: {err_str}");
    }
}

#[test]
fn test_from_json_query_definition_not_object() {
    // Test that QueryDefinitions::from_json fails when query definition is not an object
    use jankensqlhub::QueryDefinitions;

    let json_definitions = serde_json::json!({
        "invalid_query_def": "not an object"
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "object");
        assert_eq!(got, "invalid_query_def: \"not an object\"");
    } else {
        panic!("Expected ParameterTypeMismatch for non-object query definition, got: {err_str}");
    }
}

/// Test parameter name conflict errors
#[test]
fn test_parameter_name_conflict_error() {
    use jankensqlhub::parameters::parse_parameters_with_quotes;

    // Test conflict cases
    let conflict_cases = vec![
        ("SELECT * FROM #[conflict] WHERE id=@conflict", "conflict"), // table vs param
        ("SELECT * FROM #[conflict] WHERE id=:[conflict]", "conflict"), // table vs list
        (
            "SELECT * FROM table WHERE id=:[conflict] AND name=@conflict",
            "conflict",
        ), // list vs param
    ];

    for (sql, expected_conflict_name) in conflict_cases {
        let result = parse_parameters_with_quotes(sql);
        assert!(result.is_err(), "Expected conflict error for SQL: {sql}");

        let err = result.unwrap_err();
        match err {
            JankenError::ParameterNameConflict { data } => {
                let name = error_meta(&data, "conflicting_name").unwrap();
                assert_eq!(
                    name, expected_conflict_name,
                    "Conflict name mismatch for SQL: {sql}"
                );
            }
            _ => panic!("Expected ParameterNameConflict for SQL: {sql}, got: {err:?}"),
        }
    }
}

#[test]
fn test_invalid_itemtype_definition_error() {
    // Test that invalid itemtypes are caught at definition time (parse_constraints)
    // TableName and List should not be allowed as item types

    // Test TableName as item type - should fail at definition time
    let json_definitions_invalid_table = serde_json::json!({
        "list_table_item": {
            "query": "SELECT * FROM source WHERE id IN :[tables]",
            "args": {
                "tables": { "type": "list", "itemtype": "table_name" }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_invalid_table);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(
            expected,
            "item_type for list items cannot be TableName or List"
        );
        assert_eq!(got, "table_name");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid itemtype TableName, got: {err_str}");
    }

    // Test List as item type - should fail at definition time
    let json_definitions_invalid_list = serde_json::json!({
        "list_list_item": {
            "query": "SELECT * FROM source WHERE id IN :[nested_lists]",
            "args": {
                "nested_lists": { "type": "list", "itemtype": "list" }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_invalid_list);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(
            expected,
            "item_type for list items cannot be TableName or List"
        );
        assert_eq!(got, "list");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid itemtype List, got: {err_str}");
    }

    // Test invalid/malformed itemtype string - should fail at definition time
    let json_definitions_invalid_type = serde_json::json!({
        "list_invalid_item": {
            "query": "SELECT * FROM source WHERE id IN :[items]",
            "args": {
                "items": { "type": "list", "itemtype": "invalid_type_not_supported" }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_invalid_type);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("integer, string, float, boolean, table_name, list or blob"));
        assert_eq!(got, "invalid_type_not_supported");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid type string, got: {err_str}");
    }
}

#[test]
fn test_parameter_validation_range_definition_errors() {
    // Test that invalid range constraint definitions are caught at definition time (parse_constraints)
    // Range must be an array with exactly 2 numbers

    // Test range not an array - should fail at definition time
    let json_definitions_not_array = serde_json::json!({
        "query_not_array": {
            "query": "SELECT * FROM source WHERE id=@id",
            "args": {
                "id": { "type": "integer", "range": "not_an_array" }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_not_array);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(
            expected,
            "array with exactly 2 numbers for range constraint"
        );
        assert!(got.contains("not_an_array"));
    } else {
        panic!("Expected ParameterTypeMismatch for range not being an array, got: {err_str}");
    }

    // Test range array with wrong length (empty) - should fail at definition time
    let json_definitions_empty_array = serde_json::json!({
        "query_empty_array": {
            "query": "SELECT * FROM source WHERE id=@id",
            "args": {
                "id": { "type": "integer", "range": [] }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_empty_array);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(
            expected,
            "array with exactly 2 numbers for range constraint"
        );
        assert_eq!(got, "array with 0 elements");
    } else {
        panic!("Expected ParameterTypeMismatch for empty range array, got: {err_str}");
    }

    // Test range array with wrong length (3 elements) - should fail at definition time
    let json_definitions_three_elements = serde_json::json!({
        "query_three_elements": {
            "query": "SELECT * FROM source WHERE id=@id",
            "args": {
                "id": { "type": "integer", "range": [1, 2, 3] }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_three_elements);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(
            expected,
            "array with exactly 2 numbers for range constraint"
        );
        assert_eq!(got, "array with 3 elements");
    } else {
        panic!("Expected ParameterTypeMismatch for range array with 3 elements, got: {err_str}");
    }

    // Test range array with non-number at first position - should fail at definition time
    let json_definitions_non_number_first = serde_json::json!({
        "query_non_number_first": {
            "query": "SELECT * FROM source WHERE id=@id",
            "args": {
                "id": { "type": "integer", "range": ["not_a_number", 100] }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_non_number_first);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "number");
        assert_eq!(got, "\"not_a_number\" at index 0");
    } else {
        panic!("Expected ParameterTypeMismatch for non-number at first position, got: {err_str}");
    }

    // Test range array with non-number at second position - should fail at definition time
    let json_definitions_non_number_second = serde_json::json!({
        "query_non_number_second": {
            "query": "SELECT * FROM source WHERE id=@id",
            "args": {
                "id": { "type": "integer", "range": [1, "not_a_number"] }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_non_number_second);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "number");
        assert_eq!(got, "\"not_a_number\" at index 1");
    } else {
        panic!("Expected ParameterTypeMismatch for non-number at second position, got: {err_str}");
    }

    // Test that valid range definition works (should not fail)
    let json_definitions_valid = serde_json::json!({
        "query_valid_range": {
            "query": "SELECT * FROM source WHERE score=@score",
            "args": {
                "score": { "type": "float", "range": [0.0, 100.0] }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_valid);
    assert!(result.is_ok(), "Valid range definition should work");
}
