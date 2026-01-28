use jankensqlhub::{
    JankenError, M_EXPECTED, M_GOT, QueryDefinitions, error_meta, query_run_sqlite,
};
use rusqlite::Connection;

#[test]
fn test_enum_and_enumif_mutual_exclusion() {
    // Test that a parameter cannot have both enum and enumif defined - they are mutually exclusive

    // Test with both enum and enumif - should fail at definition time
    let json_definitions_both = serde_json::json!({
        "invalid_both": {
            "query": "SELECT * FROM source WHERE type=@param",
            "args": {
                "param": {
                    "enum": ["value1", "value2"],  // Regular enum constraint
                    "enumif": {                    // Conditional enum constraint - not allowed together
                        "other_param": {
                            "cond_val": ["allowed1", "allowed2"]
                        }
                    }
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_both);
    assert!(
        result.is_err(),
        "Should reject parameter with both enum and enumif"
    );

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(expected, "either 'enum' or 'enumif', not both");
        assert_eq!(got, "'enum' and 'enumif' both specified");
    } else {
        panic!("Expected ParameterTypeMismatch for enum and enumif both present, got: {err_str}");
    }

    // Test that only enum works fine (no enumif)
    let json_definitions_enum_only = serde_json::json!({
        "valid_enum": {
            "query": "SELECT * FROM source WHERE type=@param",
            "args": {
                "param": {
                    "enum": ["value1", "value2"]  // Only enum - should be valid
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_enum_only);
    assert!(result.is_ok(), "Parameter with only enum should be valid");

    // Test that only enumif works fine (no enum)
    let json_definitions_enumif_only = serde_json::json!({
        "valid_enumif": {
            "query": "SELECT * FROM source WHERE type=@param",
            "args": {
                "param": {
                    "enumif": {  // Only enumif - should be valid
                        "other_param": {
                            "cond_val": ["allowed1", "allowed2"]
                        }
                    }
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_enumif_only);
    assert!(result.is_ok(), "Parameter with only enumif should be valid");
}

#[test]
fn test_enumif_constraint_validation() {
    // Test the new enumif (conditional enum) constraint

    let json_definitions = serde_json::json!({
        "conditional_enum_query": {
            "query": "SELECT * FROM media WHERE type=@media_type AND source=@source",
            "returns": ["id", "type", "source", "data"],
            "args": {
                "media_type": {
                    "enum": ["song", "show"]
                },
                "source": {
                    "enumif": {
                        "media_type": {
                            "song": ["artist", "album", "title"],
                            "show": ["channel", "category", "episodes"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE media (id INTEGER PRIMARY KEY, type TEXT, source TEXT, data TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO media VALUES (1, 'song', 'artist', 'Alice in Chains')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO media VALUES (2, 'show', 'channel', 'Netflix')",
        [],
    )
    .unwrap();

    // Test valid conditional enum values should work
    let params = serde_json::json!({"media_type": "song", "source": "artist"});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_enum_query", &params);
    assert!(result.is_ok(), "Valid conditional enum should work");

    let params = serde_json::json!({"media_type": "show", "source": "channel"});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_enum_query", &params);
    assert!(result.is_ok(), "Valid conditional enum should work");

    // Test invalid conditional enum values should fail
    let params = serde_json::json!({"media_type": "song", "source": "channel"}); // "channel" is not allowed for "song"
    let err = query_run_sqlite(&mut conn, &queries, "conditional_enum_query", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("artist"));
        assert!(expected.contains("album"));
        assert!(expected.contains("title"));
        assert_eq!(got, "\"channel\"");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid conditional enum, got: {err_str}");
    }

    let params = serde_json::json!({"media_type": "show", "source": "album"}); // "album" is not allowed for "show"
    let err = query_run_sqlite(&mut conn, &queries, "conditional_enum_query", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("channel"));
        assert!(expected.contains("category"));
        assert!(expected.contains("episodes"));
        assert_eq!(got, "\"album\"");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid conditional enum, got: {err_str}");
    }

    // Test with unknown media_type that violates the enum constraint first - should fail
    let params = serde_json::json!({"media_type": "unknown", "source": "any_value"}); // "unknown" is not in enum ["song", "show"]
    let err = query_run_sqlite(&mut conn, &queries, "conditional_enum_query", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("song") && expected.contains("show"));
        assert_eq!(got, "\"unknown\"");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid enum value, got: {err_str}");
    }

    // Test with missing conditional parameter - should fail
    let params = serde_json::json!({"source": "artist"}); // missing media_type
    let err = query_run_sqlite(&mut conn, &queries, "conditional_enum_query", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterNotProvided { data }) = err.downcast::<JankenError>() {
        let name = error_meta(&data, "parameter_name").unwrap();
        assert_eq!(name, "media_type");
    } else {
        panic!("Expected ParameterNotProvided for missing conditional param, got: {err_str}");
    }
}

#[test]
fn test_enumif_exact_match_allows_any_string() {
    // Test that exact match keys allow any string values (no alphanumeric restriction)

    let json_definitions = serde_json::json!({
        "flexible_exact": {
            "query": "SELECT * FROM data WHERE key=@key AND value=@value",
            "returns": ["id", "key", "value"],
            "args": {
                "key": {},
                "value": {
                    "enumif": {
                        "key": {
                            "exact-key": ["option1", "option2"],
                            "key_with_spaces": ["option3", "option4"],
                            "key.with.dots": ["option5", "option6"]
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

    // Test exact match with dash
    let params = serde_json::json!({"key": "exact-key", "value": "option1"});
    let result = query_run_sqlite(&mut conn, &queries, "flexible_exact", &params);
    assert!(result.is_ok(), "Exact match with dash should work");

    // Test exact match with spaces
    let params = serde_json::json!({"key": "key_with_spaces", "value": "option3"});
    let result = query_run_sqlite(&mut conn, &queries, "flexible_exact", &params);
    assert!(result.is_ok(), "Exact match with spaces should work");

    // Test exact match with dots
    let params = serde_json::json!({"key": "key.with.dots", "value": "option5"});
    let result = query_run_sqlite(&mut conn, &queries, "flexible_exact", &params);
    assert!(result.is_ok(), "Exact match with dots should work");
}

#[test]
fn test_enumif_enum_values_allow_any_string() {
    // Test that enum values (the allowed values in arrays) can be any string,
    // unlike fuzzy match patterns which must be alphanumeric with underscores

    let json_definitions = serde_json::json!({
        "special_values": {
            "query": "SELECT * FROM data WHERE category=@category AND value=@value",
            "returns": ["id", "category", "value"],
            "args": {
                "category": {},
                "value": {
                    "enumif": {
                        "category": {
                            "products": ["SKU-123", "ITEM.456", "product with spaces", "special@char"],
                            "users": ["user-name", "email@domain.com", "first.last", "user 123"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE data (id INTEGER PRIMARY KEY, category TEXT, value TEXT)",
        [],
    )
    .unwrap();

    // Test enum value with dash
    let params = serde_json::json!({"category": "products", "value": "SKU-123"});
    let result = query_run_sqlite(&mut conn, &queries, "special_values", &params);
    assert!(result.is_ok(), "Enum value with dash should work");

    // Test enum value with dot
    let params = serde_json::json!({"category": "products", "value": "ITEM.456"});
    let result = query_run_sqlite(&mut conn, &queries, "special_values", &params);
    assert!(result.is_ok(), "Enum value with dot should work");

    // Test enum value with spaces
    let params = serde_json::json!({"category": "products", "value": "product with spaces"});
    let result = query_run_sqlite(&mut conn, &queries, "special_values", &params);
    assert!(result.is_ok(), "Enum value with spaces should work");

    // Test enum value with special characters
    let params = serde_json::json!({"category": "products", "value": "special@char"});
    let result = query_run_sqlite(&mut conn, &queries, "special_values", &params);
    assert!(
        result.is_ok(),
        "Enum value with special characters should work"
    );

    // Test another category with various special characters
    let params = serde_json::json!({"category": "users", "value": "email@domain.com"});
    let result = query_run_sqlite(&mut conn, &queries, "special_values", &params);
    assert!(result.is_ok(), "Enum value with email format should work");

    let params = serde_json::json!({"category": "users", "value": "first.last"});
    let result = query_run_sqlite(&mut conn, &queries, "special_values", &params);
    assert!(result.is_ok(), "Enum value with dots should work");

    let params = serde_json::json!({"category": "users", "value": "user 123"});
    let result = query_run_sqlite(&mut conn, &queries, "special_values", &params);
    assert!(
        result.is_ok(),
        "Enum value with space and numbers should work"
    );

    // Test that invalid value is rejected
    let params = serde_json::json!({"category": "products", "value": "invalid-value"});
    let err = query_run_sqlite(&mut conn, &queries, "special_values", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert!(expected.contains("SKU-123"));
        assert!(expected.contains("special@char"));
        assert_eq!(got, "\"invalid-value\"");
    } else {
        panic!("Expected ParameterTypeMismatch for invalid value, got: {err_str}");
    }
}
