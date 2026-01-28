use jankensqlhub::{
    JankenError, M_EXPECTED, M_GOT, QueryDefinitions, error_meta, query_run_sqlite,
};
use rusqlite::Connection;

#[test]
fn test_enumif_fuzzy_start_match() {
    // Test fuzzy matching with "start:" pattern

    let json_definitions = serde_json::json!({
        "user_search": {
            "query": "SELECT * FROM users WHERE role=@role AND permission=@permission",
            "returns": ["id", "role", "permission"],
            "args": {
                "role": {},
                "permission": {
                    "enumif": {
                        "role": {
                            "start:admin": ["read_all", "write_all", "delete_all"],
                            "start:user": ["read_own", "write_own"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, role TEXT, permission TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO users VALUES (1, 'admin_super', 'read_all')",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO users VALUES (2, 'user_basic', 'read_own')", [])
        .unwrap();

    // Test that "admin_super" matches "start:admin"
    let params = serde_json::json!({"role": "admin_super", "permission": "read_all"});
    let result = query_run_sqlite(&mut conn, &queries, "user_search", &params);
    assert!(result.is_ok(), "admin_super should match start:admin");

    // Test that "admin_level2" also matches "start:admin"
    let params = serde_json::json!({"role": "admin_level2", "permission": "write_all"});
    let result = query_run_sqlite(&mut conn, &queries, "user_search", &params);
    assert!(result.is_ok(), "admin_level2 should match start:admin");

    // Test that "user_basic" matches "start:user"
    let params = serde_json::json!({"role": "user_basic", "permission": "read_own"});
    let result = query_run_sqlite(&mut conn, &queries, "user_search", &params);
    assert!(result.is_ok(), "user_basic should match start:user");

    // Test that "user_premium" also matches "start:user"
    let params = serde_json::json!({"role": "user_premium", "permission": "write_own"});
    let result = query_run_sqlite(&mut conn, &queries, "user_search", &params);
    assert!(result.is_ok(), "user_premium should match start:user");

    // Test that permission from wrong role is rejected
    let params = serde_json::json!({"role": "admin_super", "permission": "read_own"});
    let err = query_run_sqlite(&mut conn, &queries, "user_search", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("read_all"));
        assert!(expected.contains("write_all"));
        assert!(expected.contains("delete_all"));
        assert_eq!(got, "\"read_own\"");
    } else {
        panic!("Expected ParameterTypeMismatch for wrong permission, got: {err_str}");
    }

    // Test that role not matching any pattern is rejected
    let params = serde_json::json!({"role": "guest", "permission": "read_all"});
    let err = query_run_sqlite(&mut conn, &queries, "user_search", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert!(expected.contains("conditional parameter value that matches a defined condition"));
    } else {
        panic!("Expected ParameterTypeMismatch for unmatched role, got: {err_str}");
    }
}

#[test]
fn test_enumif_fuzzy_end_match() {
    // Test fuzzy matching with "end:" pattern

    let json_definitions = serde_json::json!({
        "file_operations": {
            "query": "SELECT * FROM files WHERE filename=@filename AND action=@action",
            "returns": ["id", "filename", "action"],
            "args": {
                "filename": {},
                "action": {
                    "enumif": {
                        "filename": {
                            "end:txt": ["read_text", "edit_text", "append_text"],
                            "end:jpg": ["view_image", "resize_image"],
                            "end:pdf": ["read_doc", "print_doc"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE files (id INTEGER PRIMARY KEY, filename TEXT, action TEXT)",
        [],
    )
    .unwrap();

    // Test that "document.txt" matches "end:txt"
    let params = serde_json::json!({"filename": "document.txt", "action": "read_text"});
    let result = query_run_sqlite(&mut conn, &queries, "file_operations", &params);
    assert!(result.is_ok(), "document.txt should match end:txt");

    // Test that "photo.jpg" matches "end:jpg"
    let params = serde_json::json!({"filename": "photo.jpg", "action": "view_image"});
    let result = query_run_sqlite(&mut conn, &queries, "file_operations", &params);
    assert!(result.is_ok(), "photo.jpg should match end:jpg");

    // Test that "report.pdf" matches "end:pdf"
    let params = serde_json::json!({"filename": "report.pdf", "action": "read_doc"});
    let result = query_run_sqlite(&mut conn, &queries, "file_operations", &params);
    assert!(result.is_ok(), "report.pdf should match end:pdf");

    // Test that action from wrong file type is rejected
    let params = serde_json::json!({"filename": "photo.jpg", "action": "read_text"});
    let err = query_run_sqlite(&mut conn, &queries, "file_operations", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("view_image"));
        assert!(expected.contains("resize_image"));
        assert_eq!(got, "\"read_text\"");
    } else {
        panic!("Expected ParameterTypeMismatch for wrong action, got: {err_str}");
    }
}

#[test]
fn test_enumif_fuzzy_contain_match() {
    // Test fuzzy matching with "contain:" pattern

    let json_definitions = serde_json::json!({
        "log_filter": {
            "query": "SELECT * FROM logs WHERE message=@message AND level=@level",
            "returns": ["id", "message", "level"],
            "args": {
                "message": {},
                "level": {
                    "enumif": {
                        "message": {
                            "contain:error": ["critical", "high", "medium"],
                            "contain:warn": ["medium", "low"],
                            "contain:info": ["low", "debug"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE logs (id INTEGER PRIMARY KEY, message TEXT, level TEXT)",
        [],
    )
    .unwrap();

    // Test that "Database error occurred" matches "contain:error"
    let params = serde_json::json!({"message": "Database error occurred", "level": "critical"});
    let result = query_run_sqlite(&mut conn, &queries, "log_filter", &params);
    assert!(
        result.is_ok(),
        "Message with error should match contain:error"
    );

    // Test that "System warning detected" matches "contain:warn"
    let params = serde_json::json!({"message": "System warning detected", "level": "medium"});
    let result = query_run_sqlite(&mut conn, &queries, "log_filter", &params);
    assert!(
        result.is_ok(),
        "Message with warn should match contain:warn"
    );

    // Test that "User information updated" matches "contain:info"
    let params = serde_json::json!({"message": "User information updated", "level": "low"});
    let result = query_run_sqlite(&mut conn, &queries, "log_filter", &params);
    assert!(
        result.is_ok(),
        "Message with info should match contain:info"
    );

    // Test that level from wrong message type is rejected
    let params = serde_json::json!({"message": "Database error occurred", "level": "debug"});
    let err = query_run_sqlite(&mut conn, &queries, "log_filter", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("critical"));
        assert!(expected.contains("high"));
        assert!(expected.contains("medium"));
        assert_eq!(got, "\"debug\"");
    } else {
        panic!("Expected ParameterTypeMismatch for wrong level, got: {err_str}");
    }
}

#[test]
fn test_enumif_fuzzy_mixed_exact_and_fuzzy() {
    // Test mixing exact matches with fuzzy patterns

    let json_definitions = serde_json::json!({
        "resource_access": {
            "query": "SELECT * FROM resources WHERE resource_type=@resource_type AND action=@action",
            "returns": ["id", "resource_type", "action"],
            "args": {
                "resource_type": {},
                "action": {
                    "enumif": {
                        "resource_type": {
                            "database": ["query", "update", "delete"],
                            "start:api": ["call", "monitor"],
                            "end:service": ["start", "stop", "restart"],
                            "contain:cache": ["clear", "refresh"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE resources (id INTEGER PRIMARY KEY, resource_type TEXT, action TEXT)",
        [],
    )
    .unwrap();

    // Test exact match
    let params = serde_json::json!({"resource_type": "database", "action": "query"});
    let result = query_run_sqlite(&mut conn, &queries, "resource_access", &params);
    assert!(result.is_ok(), "Exact match should work");

    // Test start: pattern
    let params = serde_json::json!({"resource_type": "api_gateway", "action": "call"});
    let result = query_run_sqlite(&mut conn, &queries, "resource_access", &params);
    assert!(result.is_ok(), "Start pattern should work");

    // Test end: pattern
    let params = serde_json::json!({"resource_type": "web_service", "action": "start"});
    let result = query_run_sqlite(&mut conn, &queries, "resource_access", &params);
    assert!(result.is_ok(), "End pattern should work");

    // Test contain: pattern
    let params = serde_json::json!({"resource_type": "redis_cache", "action": "clear"});
    let result = query_run_sqlite(&mut conn, &queries, "resource_access", &params);
    assert!(result.is_ok(), "Contain pattern should work");

    // Test that unmatched resource type is rejected
    let params = serde_json::json!({"resource_type": "storage", "action": "query"});
    let err = query_run_sqlite(&mut conn, &queries, "resource_access", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert!(expected.contains("conditional parameter value that matches a defined condition"));
    } else {
        panic!("Expected ParameterTypeMismatch for unmatched resource_type, got: {err_str}");
    }
}

#[test]
fn test_enumif_fuzzy_alphabetical_precedence() {
    // Test that when multiple fuzzy conditions match, alphabetical order determines precedence

    let json_definitions = serde_json::json!({
        "multi_match": {
            "query": "SELECT * FROM data WHERE key=@key AND value=@value",
            "returns": ["id", "key", "value"],
            "args": {
                "key": {},
                "value": {
                    "enumif": {
                        "key": {
                            "contain:test": ["option1", "option2"],
                            "start:test": ["option3", "option4"],
                            "end:test": ["option5", "option6"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE data (id INTEGER PRIMARY KEY, key TEXT, value TEXT)",
        [],
    )
    .unwrap();

    // "test123" matches both "start:test" and "contain:test"
    // Alphabetically "contain:test" comes before "start:test", so it should be used
    let params = serde_json::json!({"key": "test123", "value": "option1"});
    let result = query_run_sqlite(&mut conn, &queries, "multi_match", &params);
    assert!(
        result.is_ok(),
        "Should use contain:test (alphabetically first)"
    );

    // Test that option from "start:test" is rejected when "contain:test" takes precedence
    let params = serde_json::json!({"key": "test123", "value": "option3"});
    let err = query_run_sqlite(&mut conn, &queries, "multi_match", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        // Should show options from "contain:test" since it comes first alphabetically
        assert!(expected.contains("option1"));
        assert!(expected.contains("option2"));
        assert_eq!(got, "\"option3\"");
    } else {
        panic!("Expected ParameterTypeMismatch for alphabetical precedence, got: {err_str}");
    }
}

#[test]
fn test_enumif_fuzzy_invalid_pattern_name() {
    // Test that invalid pattern names are rejected at definition time

    let json_definitions = serde_json::json!({
        "invalid_pattern": {
            "query": "SELECT * FROM data WHERE key=@key AND value=@value",
            "args": {
                "key": {},
                "value": {
                    "enumif": {
                        "key": {
                            "start:invalid-pattern": ["option1", "option2"]
                        }
                    }
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(
        result.is_err(),
        "Invalid pattern name with dash should be rejected"
    );

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("alphanumeric with underscores"));
        assert!(got.contains("invalid-pattern"));
    } else {
        panic!("Expected ParameterTypeMismatch for invalid pattern name, got: {err_str}");
    }
}

#[test]
fn test_enumif_fuzzy_empty_pattern() {
    // Test that empty pattern after colon is rejected

    let json_definitions = serde_json::json!({
        "empty_pattern": {
            "query": "SELECT * FROM data WHERE key=@key AND value=@value",
            "args": {
                "key": {},
                "value": {
                    "enumif": {
                        "key": {
                            "start:": ["option1", "option2"]
                        }
                    }
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err(), "Empty pattern should be rejected");

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert!(expected.contains("alphanumeric with underscores"));
    } else {
        panic!("Expected ParameterTypeMismatch for empty pattern, got: {err_str}");
    }
}

#[test]
fn test_enumif_fuzzy_invalid_match_type() {
    // Test that invalid match types are rejected at definition time

    let json_definitions = serde_json::json!({
        "invalid_match": {
            "query": "SELECT * FROM data WHERE key=@key AND value=@value",
            "args": {
                "key": {},
                "value": {
                    "enumif": {
                        "key": {
                            "invalid:pattern": ["option1", "option2"]
                        }
                    }
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions);
    assert!(result.is_err(), "Invalid match type should be rejected");

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("'start', 'end', or 'contain'"));
        assert!(got.contains("invalid"));
    } else {
        panic!("Expected ParameterTypeMismatch for invalid match type, got: {err_str}");
    }
}
