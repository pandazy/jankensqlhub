use jankensqlhub::{DatabaseConnection, JankenError, QueryDefinitions, QueryRunner};
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
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(expected, "either 'enum' or 'enumif', not both");
            assert_eq!(got, "'enum' and 'enumif' both specified");
        }
        _ => {
            panic!("Expected ParameterTypeMismatch for enum and enumif both present, got: {err:?}")
        }
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

    let conn = Connection::open_in_memory().unwrap();
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

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test valid conditional enum values should work
    let params = serde_json::json!({"media_type": "song", "source": "artist"});
    let result = db_conn.query_run(&queries, "conditional_enum_query", &params);
    assert!(result.is_ok(), "Valid conditional enum should work");

    let params = serde_json::json!({"media_type": "show", "source": "channel"});
    let result = db_conn.query_run(&queries, "conditional_enum_query", &params);
    assert!(result.is_ok(), "Valid conditional enum should work");

    // Test invalid conditional enum values should fail
    let params = serde_json::json!({"media_type": "song", "source": "channel"}); // "channel" is not allowed for "song"
    let err = db_conn
        .query_run(&queries, "conditional_enum_query", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("artist"));
            assert!(expected.contains("album"));
            assert!(expected.contains("title"));
            assert_eq!(got, "\"channel\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid conditional enum, got: {err:?}"),
    }

    let params = serde_json::json!({"media_type": "show", "source": "album"}); // "album" is not allowed for "show"
    let err = db_conn
        .query_run(&queries, "conditional_enum_query", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("channel"));
            assert!(expected.contains("category"));
            assert!(expected.contains("episodes"));
            assert_eq!(got, "\"album\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid conditional enum, got: {err:?}"),
    }

    // Test with unknown media_type that violates the enum constraint first - should fail
    let params = serde_json::json!({"media_type": "unknown", "source": "any_value"}); // "unknown" is not in enum ["song", "show"]
    let err = db_conn
        .query_run(&queries, "conditional_enum_query", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("song") && expected.contains("show"));
            assert_eq!(got, "\"unknown\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid enum value, got: {err:?}"),
    }

    // Test with missing conditional parameter - should fail
    let params = serde_json::json!({"source": "artist"}); // missing media_type
    let err = db_conn
        .query_run(&queries, "conditional_enum_query", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterNotProvided(name) => {
            assert_eq!(name, "media_type");
        }
        _ => panic!("Expected ParameterNotProvided for missing conditional param, got: {err:?}"),
    }
}

#[test]
fn test_enumif_constraint_no_matching_condition() {
    // Test that parameter fails validation when conditional parameter value doesn't match any enumif condition
    // This triggers the error: "value not covered by any enumif condition for parameter {param_name}"

    let json_definitions = serde_json::json!({
        "conditional_enum_query": {
            "query": "SELECT * FROM media WHERE type=@media_type AND source=@source",
            "returns": ["id", "type", "source", "data"],
            "args": {
                "media_type": {
                    "enum": ["song", "show", "movie", "book"]
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

    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE media (id INTEGER PRIMARY KEY, type TEXT, source TEXT, data TEXT)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test with valid enum value but no matching enumif condition - should fail
    let params = serde_json::json!({"media_type": "movie", "source": "director"}); // "movie" not in enumif conditions
    let err = db_conn
        .query_run(&queries, "conditional_enum_query", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(
                expected,
                "conditional parameter value that matches a defined condition"
            );
            assert_eq!(
                got,
                "value not covered by any enumif condition for parameter source"
            );
        }
        _ => {
            panic!("Expected ParameterTypeMismatch for no matching enumif condition, got: {err:?}")
        }
    }

    // Test with another value not covered by enumif
    let params = serde_json::json!({"media_type": "book", "source": "author"}); // "book" not in enumif conditions
    let err = db_conn
        .query_run(&queries, "conditional_enum_query", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(
                expected,
                "conditional parameter value that matches a defined condition"
            );
            assert_eq!(
                got,
                "value not covered by any enumif condition for parameter source"
            );
        }
        _ => {
            panic!("Expected ParameterTypeMismatch for no matching enumif condition, got: {err:?}")
        }
    }

    // Verify that values covered by enumif conditions work correctly
    let params = serde_json::json!({"media_type": "song", "source": "artist"});
    let result = db_conn.query_run(&queries, "conditional_enum_query", &params);
    assert!(result.is_ok(), "Valid enumif condition should work");

    let params = serde_json::json!({"media_type": "show", "source": "channel"});
    let result = db_conn.query_run(&queries, "conditional_enum_query", &params);
    assert!(result.is_ok(), "Valid enumif condition should work");
}

#[test]
fn test_enumif_primitive_conditional_parameter_validation() {
    // Test enumif constraints where the conditional parameter can be any primitive value (string, number, boolean)
    // rather than being restricted to enum values

    let json_definitions = serde_json::json!({
        "primitive_string_condition": {
            "query": "INSERT INTO config (key_name, value_type, allowed_values) VALUES (@key_name, @value_type, @allowed_values)",
            "args": {
                "key_name": { "type": "string" },  // Conditional parameter - any string, no enum constraint
                "value_type": {
                    "enumif": {
                        "key_name": {
                            "database_host": ["string"],
                            "database_port": ["integer"],
                            "debug_mode": ["boolean"],
                            "timeout": ["integer", "float"]  // Multiple allowed types
                        }
                    }
                },
                "allowed_values": { "type": "string" }  // Just another parameter, not relevant for this test
            }
        },
        "primitive_number_condition": {
            "query": "INSERT INTO metrics (level, severity, message) VALUES (@level, @severity, @message)",
            "args": {
                "level": { "type": "integer" },  // Conditional parameter - any integer, no enum constraint
                "severity": {
                    "enumif": {
                        "level": {
                            "0": ["info"],
                            "1": ["warning"],
                            "2": ["error"],
                            "10": ["critical"]
                        }
                    }
                },
                "message": { "type": "string" }  // Just another parameter
            }
        },
        "primitive_boolean_condition": {
            "query": "INSERT INTO auth (user_id, is_admin, permissions) VALUES (@user_id, @is_admin, @permissions)",
            "args": {
                "user_id": { "type": "string" },
                "is_admin": { "type": "boolean" },  // Conditional parameter - any boolean, no enum constraint
                "permissions": {
                    "enumif": {
                        "is_admin": {
                            "true": ["read", "write", "admin"],
                            "false": ["read", "write"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE config (key_name TEXT, value_type TEXT, allowed_values TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE metrics (level INTEGER, severity TEXT, message TEXT)",
        [],
    )
    .unwrap();
    conn.execute(
        "CREATE TABLE auth (user_id TEXT, is_admin BOOLEAN, permissions TEXT)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test conditional parameter as any string (no enum restriction)

    // Test with different string values for key_name - should work as long as conditions are defined
    let params = serde_json::json!({"key_name": "database_host", "value_type": "string", "allowed_values": "host.example.com"});
    let result = db_conn.query_run(&queries, "primitive_string_condition", &params);
    assert!(
        result.is_ok(),
        "String conditional parameter should work when condition matches"
    );

    let params = serde_json::json!({"key_name": "database_port", "value_type": "integer", "allowed_values": "5432"});
    let result = db_conn.query_run(&queries, "primitive_string_condition", &params);
    assert!(result.is_ok(), "Integer value for port should work");

    let params =
        serde_json::json!({"key_name": "timeout", "value_type": "float", "allowed_values": "30.5"});
    let result = db_conn.query_run(&queries, "primitive_string_condition", &params);
    assert!(result.is_ok(), "Float value for timeout should work");

    // Test validation failure when value doesn't match condition
    let params = serde_json::json!({"key_name": "database_host", "value_type": "integer", "allowed_values": "host.example.com"}); // "integer" not allowed for "database_host"
    let err = db_conn
        .query_run(&queries, "primitive_string_condition", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("string"));
            assert_eq!(got, "\"integer\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid enumif value, got: {err:?}"),
    }

    // Test conditional parameter as any number (no enum restriction)

    // Test with different integer values for level
    let params = serde_json::json!({"level": 0, "severity": "info", "message": "System started"});
    let result = db_conn.query_run(&queries, "primitive_number_condition", &params);
    assert!(result.is_ok(), "Level 0 should allow info severity");

    let params = serde_json::json!({"level": 2, "severity": "error", "message": "Database connection failed"});
    let result = db_conn.query_run(&queries, "primitive_number_condition", &params);
    assert!(result.is_ok(), "Level 2 should allow error severity");

    let params =
        serde_json::json!({"level": 10, "severity": "critical", "message": "System meltdown"});
    let result = db_conn.query_run(&queries, "primitive_number_condition", &params);
    assert!(result.is_ok(), "Level 10 should allow critical severity");

    // Test validation failure for undefined level
    let params = serde_json::json!({"level": 5, "severity": "warning", "message": "Unknown level"}); // Level 5 not defined in conditions
    let err = db_conn
        .query_run(&queries, "primitive_number_condition", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert_eq!(
                expected,
                "conditional parameter value that matches a defined condition"
            );
            assert_eq!(
                got,
                "value not covered by any enumif condition for parameter severity"
            );
        }
        _ => {
            panic!("Expected ParameterTypeMismatch for no matching enumif condition, got: {err:?}")
        }
    }

    // Test conditional parameter as boolean

    // Test with boolean conditional parameter
    let params =
        serde_json::json!({"user_id": "user123", "is_admin": true, "permissions": "admin"});
    let result = db_conn.query_run(&queries, "primitive_boolean_condition", &params);
    assert!(result.is_ok(), "Admin user should allow admin permissions");

    let params =
        serde_json::json!({"user_id": "user456", "is_admin": false, "permissions": "write"});
    let result = db_conn.query_run(&queries, "primitive_boolean_condition", &params);
    assert!(
        result.is_ok(),
        "Regular user should allow write permissions"
    );

    // Test validation failure when boolean condition not met
    let params =
        serde_json::json!({"user_id": "user789", "is_admin": false, "permissions": "admin"}); // Regular user trying to get admin permissions
    let err = db_conn
        .query_run(&queries, "primitive_boolean_condition", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("read") && expected.contains("write"));
            assert_eq!(got, "\"admin\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid boolean conditional, got: {err:?}"),
    }
}

#[test]
fn test_enumif_constraint_malformed_definition_errors() {
    // Test that malformed enumif constraint definitions are caught at parsing time

    // Test enumif with invalid structure (single object instead of nested)
    let json_definitions_invalid = serde_json::json!({
        "bad_enumif": {
            "query": "SELECT * FROM test WHERE type=@type AND value=@value",
            "args": {
                "type": { "enum": ["A", "B"] },
                "value": {
                    "enumif": {
                        "single_level": ["not", "nested"]  // Invalid - should be nested objects
                    }
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_invalid);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(
                expected,
                "object mapping condition values to allowed arrays"
            );
        }
        _ => panic!("Expected ParameterTypeMismatch for malformed enumif, got: {err:?}"),
    }

    // Test enumif with non-array values in conditions
    let json_definitions_non_array = serde_json::json!({
        "bad_enumif2": {
            "query": "SELECT * FROM test WHERE type=@type",
            "args": {
                "type": {
                    "enumif": {
                        "condition_param": {
                            "cond_val": "not_an_array"  // Invalid - should be an array of values
                        }
                    }
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_non_array);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(expected, "array of allowed values");
        }
        _ => panic!("Expected ParameterTypeMismatch for non-array enumif values, got: {err:?}"),
    }

    // Test enumif with wrong top-level structure
    let json_definitions_wrong_top = serde_json::json!({
        "bad_enumif3": {
            "query": "SELECT * FROM test WHERE type=@type",
            "args": {
                "type": {
                    "enumif": ["not", "an", "object"]  // Invalid - should be an object
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_wrong_top);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(
                expected,
                "object mapping conditional parameters to conditions"
            );
        }
        _ => panic!("Expected ParameterTypeMismatch for wrong enumif structure, got: {err:?}"),
    }

    // Test enumif with null value
    let json_definitions_null = serde_json::json!({
        "bad_enumif_null": {
            "query": "SELECT * FROM test WHERE type=@type",
            "args": {
                "type": {
                    "enumif": null  // Invalid - should be an object
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_null);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(
                expected,
                "object mapping conditional parameters to conditions"
            );
        }
        _ => panic!("Expected ParameterTypeMismatch for null enumif, got: {err:?}"),
    }

    // Test enumif with string value
    let json_definitions_string = serde_json::json!({
        "bad_enumif_string": {
            "query": "SELECT * FROM test WHERE type=@type",
            "args": {
                "type": {
                    "enumif": "invalid_string"  // Invalid - should be an object
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_string);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(
                expected,
                "object mapping conditional parameters to conditions"
            );
        }
        _ => panic!("Expected ParameterTypeMismatch for string enumif, got: {err:?}"),
    }

    // Test enumif with number value
    let json_definitions_number = serde_json::json!({
        "bad_enumif_number": {
            "query": "SELECT * FROM test WHERE type=@type",
            "args": {
                "type": {
                    "enumif": 42  // Invalid - should be an object
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_number);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(
                expected,
                "object mapping conditional parameters to conditions"
            );
        }
        _ => panic!("Expected ParameterTypeMismatch for number enumif, got: {err:?}"),
    }

    // Test enumif with boolean value
    let json_definitions_boolean = serde_json::json!({
        "bad_enumif_boolean": {
            "query": "SELECT * FROM test WHERE type=@type",
            "args": {
                "type": {
                    "enumif": true  // Invalid - should be an object
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_boolean);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(
                expected,
                "object mapping conditional parameters to conditions"
            );
        }
        _ => panic!("Expected ParameterTypeMismatch for boolean enumif, got: {err:?}"),
    }

    // Test enumif with blob/array values - should be rejected
    let json_definitions_blob_values = serde_json::json!({
        "bad_enumif4": {
            "query": "SELECT * FROM test WHERE type=@type",
            "args": {
                "type": {
                    "enumif": {
                        "condition_param": {
                            "cond_val": [[1, 2, 3], "valid_string", true]  // Invalid - blob first element
                        }
                    }
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_blob_values);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(
                expected,
                "enumif allowed values to be primitives (string, number, or boolean)"
            );
        }
        _ => panic!("Expected ParameterTypeMismatch for blob values in enumif, got: {err:?}"),
    }

    // Test enumif with object values - should be rejected
    let json_definitions_object_values = serde_json::json!({
        "bad_enumif5": {
            "query": "SELECT * FROM test WHERE type=@type",
            "args": {
                "type": {
                    "enumif": {
                        "condition_param": {
                            "cond_val": [{"nested": "object"}, "valid_string", 42]  // Invalid - object first element
                        }
                    }
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_object_values);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(
                expected,
                "enumif allowed values to be primitives (string, number, or boolean)"
            );
        }
        _ => panic!("Expected ParameterTypeMismatch for object values in enumif, got: {err:?}"),
    }

    // Test enumif with null values - should be rejected
    let json_definitions_null_values = serde_json::json!({
        "bad_enumif6": {
            "query": "SELECT * FROM test WHERE type=@type",
            "args": {
                "type": {
                    "enumif": {
                        "condition_param": {
                            "cond_val": [null, "valid_string", true]  // Invalid - null first element
                        }
                    }
                }
            }
        }
    });

    let result = QueryDefinitions::from_json(json_definitions_null_values);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got: _ } => {
            assert_eq!(
                expected,
                "enumif allowed values to be primitives (string, number, or boolean)"
            );
        }
        _ => panic!("Expected ParameterTypeMismatch for null values in enumif, got: {err:?}"),
    }
}

#[test]
fn test_enumif_constraint_non_primitive_conditional_parameter_validation_error() {
    // Test that using non-primitive types (arrays, objects, null) as conditional parameters in enumif validation throws an error

    let json_definitions = serde_json::json!({
        "enumif_with_array_conditional": {
            "query": "SELECT * FROM media WHERE type=@media_type AND source=@source",
            "returns": ["id", "type", "source", "data"],
            "args": {
                "media_type": { "enum": ["song", "show"] },
                "source": {
                    "enumif": {
                        "media_type": {
                            "song": ["artist", "album"],
                            "show": ["channel", "category"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE media (id INTEGER PRIMARY KEY, type TEXT, source TEXT, data TEXT)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test with array as conditional parameter - should fail with basic type validation for the constrained parameter
    let params = serde_json::json!({"media_type": [1, 2, 3], "source": "artist"});
    let err = db_conn
        .query_run(&queries, "enumif_with_array_conditional", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            // Since media_type has an enum constraint, it expects string type
            assert_eq!(expected, "string");
            assert_eq!(got, "[1,2,3]");
        }
        _ => panic!(
            "Expected ParameterTypeMismatch for non-string conditional parameter, got: {err:?}"
        ),
    }

    // Test with object as conditional parameter - should fail with basic type validation
    let params = serde_json::json!({"media_type": {"nested": "object"}, "source": "channel"});
    let err = db_conn
        .query_run(&queries, "enumif_with_array_conditional", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            // Since media_type has an enum constraint, it expects string type
            assert_eq!(expected, "string");
            assert_eq!(got, "{\"nested\":\"object\"}");
        }
        _ => {
            panic!("Expected ParameterTypeMismatch for object conditional parameter, got: {err:?}")
        }
    }

    // Test with null as conditional parameter - should fail with basic type validation
    let params = serde_json::json!({"media_type": null, "source": "artist"});
    let err = db_conn
        .query_run(&queries, "enumif_with_array_conditional", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            // Since media_type has an enum constraint, it expects string type
            assert_eq!(expected, "string");
            assert_eq!(got, "null");
        }
        _ => panic!("Expected ParameterTypeMismatch for null conditional parameter, got: {err:?}"),
    }

    // Test with valid primitive conditional parameter - should work
    let params = serde_json::json!({"media_type": "song", "source": "artist"});
    let result = db_conn.query_run(&queries, "enumif_with_array_conditional", &params);
    assert!(
        result.is_ok(),
        "Valid primitive conditional parameter should work"
    );
}

#[test]
fn test_enumif_constraint_multiple_conditions_alphabetical() {
    // Test enumif with multiple conditional parameters - should be evaluated in alphabetical order

    let json_definitions = serde_json::json!({
        "insert_classified": {
            "query": "INSERT INTO classified (priority, category, tags) VALUES (@priority, @category, @tags)",
            "args": {
                "priority": { "enum": ["high", "medium", "low"] },
                "category": { "enum": ["work", "personal", "other"] },
                "tags": {
                    "enumif": {
                        // Multiple conditions - should be processed in alphabetical order: category then priority
                        // This means category takes precedence over priority in defining the allowed values
                        "category": {
                            "work": ["urgent", "meeting", "project"],
                            "personal": ["family", "health", "hobby"],
                            "other": ["misc", "unknown"]
                        },
                        "priority": {
                            "high": ["critical", "immediate"],
                            "medium": ["normal", "standard"],
                            "low": ["optional", "backlog"]
                        }
                    }
                }
            }
        },
        "select_classified": {
            "query": "SELECT priority, category, tags FROM classified WHERE priority=@priority AND category=@category AND tags=@tags",
            "returns": ["priority", "category", "tags"],
            "args": {
                "priority": { "enum": ["high", "medium", "low"] },
                "category": { "enum": ["work", "personal", "other"] },
                "tags": {
                    "enumif": {
                        // Same enumif conditions
                        "category": {
                            "work": ["urgent", "meeting", "project"],
                            "personal": ["family", "health", "hobby"],
                            "other": ["misc", "unknown"]
                        },
                        "priority": {
                            "high": ["critical", "immediate"],
                            "medium": ["normal", "standard"],
                            "low": ["optional", "backlog"]
                        }
                    }
                }
            }
        }
    });

    let queries = QueryDefinitions::from_json(json_definitions).unwrap();

    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE classified (priority TEXT, category TEXT, tags TEXT)",
        [],
    )
    .unwrap();

    let mut db_conn = DatabaseConnection::SQLite(conn);

    // First insert some valid data
    let params = serde_json::json!({"priority": "high", "category": "work", "tags": "meeting"});
    let result = db_conn.query_run(&queries, "insert_classified", &params);
    assert!(result.is_ok(), "Valid insert should work");

    // Test that category conditions take precedence (processed first alphabetically) - query should work
    let params = serde_json::json!({"priority": "high", "category": "work", "tags": "meeting"});
    let result = db_conn.query_run(&queries, "select_classified", &params);
    assert!(
        result.is_ok(),
        "Category condition should be checked first alphabetically"
    );

    // Test invalid tags - should fail
    let params = serde_json::json!({"priority": "high", "category": "work", "tags": "immediate"}); // "immediate" from priority but "meeting" from category
    let err = db_conn
        .query_run(&queries, "insert_classified", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            // Should show category conditions (processed first alphabetically)
            assert!(expected.contains("urgent"));
            assert!(expected.contains("meeting"));
            assert!(expected.contains("project"));
            assert_eq!(got, "\"immediate\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for wrong category precedence, got: {err:?}"),
    }

    // Test with different category - should allow different values
    let params = serde_json::json!({"priority": "low", "category": "personal", "tags": "family"});
    let result = db_conn.query_run(&queries, "insert_classified", &params);
    assert!(result.is_ok(), "Personal category should allow family tags");

    // Test invalid tags for personal category
    let params = serde_json::json!({"priority": "low", "category": "personal", "tags": "optional"}); // "optional" from priority but not in personal category
    let err = db_conn
        .query_run(&queries, "insert_classified", &params)
        .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            // Should show personal category conditions
            assert!(expected.contains("family"));
            assert!(expected.contains("health"));
            assert!(expected.contains("hobby"));
            assert_eq!(got, "\"optional\"");
        }
        _ => {
            panic!("Expected ParameterTypeMismatch for personal category validation, got: {err:?}")
        }
    }
}
