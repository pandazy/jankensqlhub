use jankensqlhub::{
    M_EXPECTED, M_GOT, QueryDefinitions, error_meta, get_error_data, query_run_sqlite,
};
use rusqlite::Connection;
use serde_json::json;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER, name TEXT, email TEXT, age INTEGER)",
        [],
    )
    .unwrap();
    conn.execute("CREATE TABLE posts (id INTEGER, title TEXT, body TEXT)", [])
        .unwrap();
    conn.execute(
        "CREATE TABLE comments (id INTEGER, post_id INTEGER, text TEXT)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO users VALUES (1, 'Alice', 'alice@test.com', 25), (2, 'Bob', 'bob@test.com', 30)", []).unwrap();
    conn.execute(
        "INSERT INTO posts VALUES (1, 'First Post', 'Hello World')",
        [],
    )
    .unwrap();
    conn
}

#[test]
fn test_comma_list_select_fields() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "select_fields": {
            "query": "SELECT ~[fields] FROM users WHERE id = 1",
            "returns": ["name", "email"],
            "args": {
                "fields": {"enum": ["name", "email", "age"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({"fields": ["name", "email"]});
    let result = query_run_sqlite(&mut conn, &queries, "select_fields", &params).unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], "Alice");
    assert_eq!(result.data[0]["email"], "alice@test.com");
}

#[test]
fn test_comma_list_table_names() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "union_tables": {
            "query": "SELECT id FROM users UNION SELECT id FROM ~[tables]",
            "returns": ["id"],
            "args": {
                "tables": {"enum": ["posts", "comments"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({"tables": ["posts"]});
    let result = query_run_sqlite(&mut conn, &queries, "union_tables", &params).unwrap();

    // Should get id from users and posts (UNION removes duplicates if any)
    assert!(result.data.len() >= 2); // At least 2 users + 1 post
}

#[test]
fn test_comma_list_single_element() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "select_single_field": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name"],
            "args": {
                "fields": {"enum": ["name", "email"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({"fields": ["name"]});
    let result = query_run_sqlite(&mut conn, &queries, "select_single_field", &params).unwrap();

    assert_eq!(result.data.len(), 2);
    assert_eq!(result.data[0]["name"], "Alice");
    assert_eq!(result.data[1]["name"], "Bob");
}

#[test]
fn test_comma_list_multiple_occurrences() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "multi_comma_list": {
            "query": "SELECT ~[fields1] FROM users UNION SELECT ~[fields2] FROM posts",
            "returns": ["id"],
            "args": {
                "fields1": {"enum": ["id", "name"]},
                "fields2": {"enum": ["id", "title"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({
        "fields1": ["id"],
        "fields2": ["id"]
    });
    let result = query_run_sqlite(&mut conn, &queries, "multi_comma_list", &params).unwrap();

    assert!(result.data.len() >= 2);
}

#[test]
fn test_comma_list_empty_array_error() {
    let json_definitions = json!({
        "empty_list": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();
    let params = json!({"fields": []});

    let result = query_run_sqlite(&mut conn, &queries, "empty_list", &params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("non-empty"),
            "Expected non-empty array error, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_non_string_element_error() {
    let json_definitions = json!({
        "invalid_element": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name"],
            "args": {
                "fields": {"enum": ["name", "email"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();
    let params = json!({"fields": ["name", 123, "email"]});

    let result = query_run_sqlite(&mut conn, &queries, "invalid_element", &params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("string"),
            "Expected string type error, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_non_array_error() {
    let json_definitions = json!({
        "not_array": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name"],
            "args": {
                "fields": {"enum": ["name"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();
    let params = json!({"fields": "not_an_array"});

    let result = query_run_sqlite(&mut conn, &queries, "not_array", &params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("comma_list"),
            "Expected comma_list type error, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_mixed_with_regular_params() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "mixed_params": {
            "query": "SELECT ~[fields] FROM users WHERE id = @id",
            "returns": ["name", "email"],
            "args": {
                "fields": {"enum": ["name", "email", "age"]},
                "id": {"type": "integer"}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({
        "fields": ["name", "email"],
        "id": 1
    });
    let result = query_run_sqlite(&mut conn, &queries, "mixed_params", &params).unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], "Alice");
}

#[test]
fn test_comma_list_mixed_with_table_name_param() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "table_and_comma": {
            "query": "SELECT ~[fields] FROM #[table_name] WHERE id = 1",
            "returns": ["name"],
            "args": {
                "fields": {"enum": ["name", "email"]},
                "table_name": {"enum": ["users", "posts"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({
        "fields": ["name"],
        "table_name": "users"
    });
    let result = query_run_sqlite(&mut conn, &queries, "table_and_comma", &params).unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], "Alice");
}

#[test]
fn test_comma_list_mixed_with_list_param() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "comma_and_list": {
            "query": "SELECT ~[fields] FROM users WHERE id IN :[ids]",
            "returns": ["name"],
            "args": {
                "fields": {"enum": ["name", "email"]},
                "ids": {"itemtype": "integer"}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({
        "fields": ["name"],
        "ids": [1, 2]
    });
    let result = query_run_sqlite(&mut conn, &queries, "comma_and_list", &params).unwrap();

    assert_eq!(result.data.len(), 2);
    assert_eq!(result.data[0]["name"], "Alice");
    assert_eq!(result.data[1]["name"], "Bob");
}

#[test]
fn test_comma_list_with_enum_constraint() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "enum_constraint": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name", "email"],
            "args": {
                "fields": {
                    "enum": ["name", "email", "age"]
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Valid values from enum
    let params = json!({"fields": ["name", "email"]});
    let result = query_run_sqlite(&mut conn, &queries, "enum_constraint", &params);
    assert!(result.is_ok());

    // Invalid value not in enum
    let params = json!({"fields": ["name", "invalid_field"]});
    let result = query_run_sqlite(&mut conn, &queries, "enum_constraint", &params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("name") && expected.contains("email"),
            "Expected enum values in error, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_with_pattern_constraint() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "pattern_constraint": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name", "email"],
            "args": {
                "fields": {
                    "pattern": "^[a-z_]+$"
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Valid field names matching pattern
    let params = json!({"fields": ["name", "email"]});
    let result = query_run_sqlite(&mut conn, &queries, "pattern_constraint", &params);
    assert!(result.is_ok());

    // Invalid field name with uppercase
    let params = json!({"fields": ["Name", "email"]});
    let result = query_run_sqlite(&mut conn, &queries, "pattern_constraint", &params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("pattern"),
            "Expected pattern validation error, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_missing_parameter() {
    let json_definitions = json!({
        "missing_param": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name"],
            "args": {
                "fields": {"enum": ["name", "email"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();
    let params = json!({});

    let result = query_run_sqlite(&mut conn, &queries, "missing_param", &params);
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let param_name = error_meta(data, "parameter_name").unwrap();
        assert_eq!(
            param_name, "fields",
            "Expected missing parameter 'fields', got: {}",
            param_name
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_in_quotes_ignored() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "quoted_syntax": {
            "query": "SELECT name FROM users WHERE name = '~[fields] should not be replaced'",
            "returns": ["name"],
            "args": {}
        }
    });

    // Should succeed because ~[fields] in quotes is ignored
    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({});
    let result = query_run_sqlite(&mut conn, &queries, "quoted_syntax", &params);
    // Should succeed because no parameters are actually required
    assert!(result.is_ok());
}

#[test]
fn test_comma_list_table_name_with_underscores() {
    let mut conn = setup_db();

    conn.execute("CREATE TABLE user_profiles (id INTEGER)", [])
        .unwrap();

    let json_definitions = json!({
        "union_tables": {
            "query": "SELECT id FROM users UNION SELECT id FROM ~[tables]",
            "returns": ["id"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Table name with underscores should be valid
    let params = json!({"tables": ["user_profiles"]});
    let result = query_run_sqlite(&mut conn, &queries, "union_tables", &params);
    assert!(
        result.is_ok(),
        "Table names with underscores should be accepted"
    );
}

#[test]
fn test_comma_list_invalid_table_name_with_special_chars() {
    let json_definitions = json!({
        "union_tables": {
            "query": "SELECT id FROM users UNION SELECT id FROM ~[tables]",
            "returns": ["id"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    // Table name with special characters should be rejected
    let params = json!({"tables": ["posts", "user-profiles"]});
    let result = query_run_sqlite(&mut conn, &queries, "union_tables", &params);

    assert!(
        result.is_err(),
        "Table names with dashes should be rejected"
    );
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("alphanumeric"),
            "Expected alphanumeric validation, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_invalid_table_name_with_sql_injection() {
    let json_definitions = json!({
        "union_tables": {
            "query": "SELECT id FROM users UNION SELECT id FROM ~[tables]",
            "returns": ["id"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    // SQL injection attempt should be rejected
    let params = json!({"tables": ["posts; DROP TABLE users--"]});
    let result = query_run_sqlite(&mut conn, &queries, "union_tables", &params);

    assert!(result.is_err(), "SQL injection attempt should be rejected");
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("alphanumeric"),
            "Expected alphanumeric validation, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_invalid_table_name_with_spaces() {
    let json_definitions = json!({
        "union_tables": {
            "query": "SELECT id FROM users UNION SELECT id FROM ~[tables]",
            "returns": ["id"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    // Table name with spaces should be rejected
    let params = json!({"tables": ["user profiles"]});
    let result = query_run_sqlite(&mut conn, &queries, "union_tables", &params);

    assert!(
        result.is_err(),
        "Table names with spaces should be rejected"
    );
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("alphanumeric"),
            "Expected alphanumeric validation, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_invalid_empty_table_name() {
    let json_definitions = json!({
        "union_tables": {
            "query": "SELECT id FROM users UNION SELECT id FROM ~[tables]",
            "returns": ["id"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    // Empty table name should be rejected
    let params = json!({"tables": [""]});
    let result = query_run_sqlite(&mut conn, &queries, "union_tables", &params);

    assert!(result.is_err(), "Empty table names should be rejected");
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("alphanumeric"),
            "Expected alphanumeric validation, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_multiple_tables_one_invalid() {
    let json_definitions = json!({
        "union_tables": {
            "query": "SELECT id FROM users UNION SELECT id FROM ~[tables]",
            "returns": ["id"]
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    // One valid, one invalid table name
    let params = json!({"tables": ["posts", "invalid@table", "comments"]});
    let result = query_run_sqlite(&mut conn, &queries, "union_tables", &params);

    assert!(
        result.is_err(),
        "Should reject if any table name is invalid"
    );
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("alphanumeric") && expected.contains("at index 1"),
            "Expected alphanumeric validation at index 1, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_enum_and_table_name_validation() {
    let mut conn = setup_db();

    let json_definitions = json!({
        "union_tables": {
            "query": "SELECT id FROM users UNION SELECT id FROM ~[tables]",
            "returns": ["id"],
            "args": {
                "tables": {
                    "enum": ["posts", "comments"]
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Valid table names in enum
    let params = json!({"tables": ["posts"]});
    let result = query_run_sqlite(&mut conn, &queries, "union_tables", &params);
    assert!(
        result.is_ok(),
        "Valid table name in enum should be accepted"
    );

    // Invalid table name not in enum
    let params = json!({"tables": ["users"]});
    let result = query_run_sqlite(&mut conn, &queries, "union_tables", &params);
    assert!(result.is_err(), "Table name not in enum should be rejected");
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("posts") && expected.contains("comments"),
            "Expected enum values in error, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_conflict_with_regular_param() {
    let json_definitions = json!({
        "conflicting_names": {
            "query": "SELECT name FROM users WHERE status = @fields AND id IN (SELECT id FROM ~[fields])",
            "returns": ["name"]
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(
        result.is_err(),
        "Should detect parameter name conflict between @fields and ~[fields]"
    );
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let param_name = error_meta(data, "conflicting_name").unwrap();
        assert_eq!(
            param_name, "fields",
            "Expected conflict on parameter 'fields', got: {}",
            param_name
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_invalid_regex_pattern() {
    // Test that an invalid regex pattern causes appropriate error
    let mut conn = setup_db();

    let json_definitions = json!({
        "invalid_regex": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name"],
            "args": {
                "fields": {
                    "pattern": "[invalid(regex"
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let params = json!({"fields": ["name"]});
    let result = query_run_sqlite(&mut conn, &queries, "invalid_regex", &params);

    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        let got = error_meta(data, M_GOT).unwrap();

        // Verify error from Regex::new() failure
        assert!(
            expected.contains("valid regex pattern"),
            "Expected 'valid regex pattern' in error"
        );
        assert!(
            got.contains("[invalid(regex"),
            "Expected invalid pattern in error"
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_with_enumif_constraint() {
    // Test comma_list with conditional enum (enumif) constraint
    let mut conn = setup_db();

    let json_definitions = json!({
        "conditional_fields": {
            "query": "SELECT ~[fields] FROM users WHERE id = @id",
            "returns": ["name", "email", "age"],
            "args": {
                "fields": {
                    "enumif": {
                        "role": {
                            "admin": ["name", "email", "age"],
                            "user": ["name"]
                        }
                    }
                },
                "role": {"enum": ["admin", "user"]},
                "id": {"type": "integer"}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // Admin can select all fields
    let params = json!({"fields": ["name", "email"], "role": "admin", "id": 1});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_fields", &params).unwrap();
    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], "Alice");
    assert_eq!(result.data[0]["email"], "alice@test.com");

    // User can only select name
    let params = json!({"fields": ["name"], "role": "user", "id": 1});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_fields", &params).unwrap();
    assert_eq!(result.data.len(), 1);
    assert_eq!(result.data[0]["name"], "Alice");
}

#[test]
fn test_comma_list_with_enumif_constraint_invalid_field() {
    // Test that comma_list with enumif rejects unauthorized fields
    let json_definitions = json!({
        "conditional_fields": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name", "email"],
            "args": {
                "fields": {
                    "enumif": {
                        "role": {
                            "admin": ["name", "email", "age"],
                            "user": ["name"]
                        }
                    }
                },
                "role": {"enum": ["admin", "user"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    // User trying to select email should fail
    let params = json!({"fields": ["name", "email"], "role": "user"});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_fields", &params);

    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("\"name\"") && expected.contains("conditional parameters"),
            "Expected conditional parameters error with allowed values, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_with_enumif_fuzzy_matching() {
    // Test comma_list with enumif fuzzy matching patterns
    let mut conn = setup_db();

    let json_definitions = json!({
        "conditional_fields": {
            "query": "SELECT ~[fields] FROM users WHERE id = @id",
            "returns": ["name", "email", "age"],
            "args": {
                "fields": {
                    "enumif": {
                        "role": {
                            "start:admin": ["name", "email", "age"],
                            "start:user": ["name"]
                        }
                    }
                },
                "role": {"pattern": "^(admin|user)_.*$"},
                "id": {"type": "integer"}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    // admin_super matches start:admin, can select all fields
    let params = json!({"fields": ["name", "email"], "role": "admin_super", "id": 1});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_fields", &params).unwrap();
    assert_eq!(result.data.len(), 1);

    // user_basic matches start:user, can only select name
    let params = json!({"fields": ["name"], "role": "user_basic", "id": 1});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_fields", &params).unwrap();
    assert_eq!(result.data.len(), 1);
}

#[test]
fn test_comma_list_with_enumif_no_matching_condition() {
    // Test that comma_list with enumif rejects when no condition matches
    let json_definitions = json!({
        "conditional_fields": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name"],
            "args": {
                "fields": {
                    "enumif": {
                        "role": {
                            "admin": ["name", "email"],
                            "user": ["name"]
                        }
                    }
                },
                "role": {"type": "string"}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    // guest role is not defined in enumif conditions
    let params = json!({"fields": ["name"], "role": "guest"});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_fields", &params);

    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        assert!(
            expected.contains("conditional parameter value that matches a defined condition"),
            "Expected no matching condition error, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}

#[test]
fn test_comma_list_with_enumif_multiple_items_validation() {
    // Test that enumif validation applies to each item in the comma_list
    let json_definitions = json!({
        "conditional_fields": {
            "query": "SELECT ~[fields] FROM users",
            "returns": ["name", "email"],
            "args": {
                "fields": {
                    "enumif": {
                        "role": {
                            "admin": ["name", "email", "age"],
                            "user": ["name"]
                        }
                    }
                },
                "role": {"enum": ["admin", "user"]}
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();
    let mut conn = setup_db();

    // User trying to select multiple fields (second one is invalid)
    let params = json!({"fields": ["name", "email"], "role": "user"});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_fields", &params);

    assert!(result.is_err());
    let err = result.unwrap_err();
    if let Some(janken_err) = err.downcast_ref::<jankensqlhub::JankenError>() {
        let data = get_error_data(janken_err);
        let expected = error_meta(data, M_EXPECTED).unwrap();
        // Should include index information
        assert!(
            expected.contains("at index 1"),
            "Expected error to mention index 1, got: {}",
            expected
        );
    } else {
        panic!("Expected JankenError, got: {:?}", err);
    }
}
