use jankensqlhub::{JankenError, QueryDefinitions, query_run_sqlite};
use rusqlite::Connection;

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

    let mut conn = Connection::open_in_memory().unwrap();
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

    // Test conditional parameter as any string (no enum restriction)

    // Test with different string values for key_name - should work as long as conditions are defined
    let params = serde_json::json!({"key_name": "database_host", "value_type": "string", "allowed_values": "host.example.com"});
    let result = query_run_sqlite(&mut conn, &queries, "primitive_string_condition", &params);
    assert!(
        result.is_ok(),
        "String conditional parameter should work when condition matches"
    );

    let params = serde_json::json!({"key_name": "database_port", "value_type": "integer", "allowed_values": "5432"});
    let result = query_run_sqlite(&mut conn, &queries, "primitive_string_condition", &params);
    assert!(result.is_ok(), "Integer value for port should work");

    let params =
        serde_json::json!({"key_name": "timeout", "value_type": "float", "allowed_values": "30.5"});
    let result = query_run_sqlite(&mut conn, &queries, "primitive_string_condition", &params);
    assert!(result.is_ok(), "Float value for timeout should work");

    // Test validation failure when value doesn't match condition
    let params = serde_json::json!({"key_name": "database_host", "value_type": "integer", "allowed_values": "host.example.com"}); // "integer" not allowed for "database_host"
    let err =
        query_run_sqlite(&mut conn, &queries, "primitive_string_condition", &params).unwrap_err();
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
    let result = query_run_sqlite(&mut conn, &queries, "primitive_number_condition", &params);
    assert!(result.is_ok(), "Level 0 should allow info severity");

    let params = serde_json::json!({"level": 2, "severity": "error", "message": "Database connection failed"});
    let result = query_run_sqlite(&mut conn, &queries, "primitive_number_condition", &params);
    assert!(result.is_ok(), "Level 2 should allow error severity");

    let params =
        serde_json::json!({"level": 10, "severity": "critical", "message": "System meltdown"});
    let result = query_run_sqlite(&mut conn, &queries, "primitive_number_condition", &params);
    assert!(result.is_ok(), "Level 10 should allow critical severity");

    // Test validation failure for undefined level
    let params = serde_json::json!({"level": 5, "severity": "warning", "message": "Unknown level"}); // Level 5 not defined in conditions
    let err =
        query_run_sqlite(&mut conn, &queries, "primitive_number_condition", &params).unwrap_err();
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
    let result = query_run_sqlite(&mut conn, &queries, "primitive_boolean_condition", &params);
    assert!(result.is_ok(), "Admin user should allow admin permissions");

    let params =
        serde_json::json!({"user_id": "user456", "is_admin": false, "permissions": "write"});
    let result = query_run_sqlite(&mut conn, &queries, "primitive_boolean_condition", &params);
    assert!(
        result.is_ok(),
        "Regular user should allow write permissions"
    );

    // Test validation failure when boolean condition not met
    let params =
        serde_json::json!({"user_id": "user789", "is_admin": false, "permissions": "admin"}); // Regular user trying to get admin permissions
    let err =
        query_run_sqlite(&mut conn, &queries, "primitive_boolean_condition", &params).unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { expected, got } => {
            assert!(expected.contains("read") && expected.contains("write"));
            assert_eq!(got, "\"admin\"");
        }
        _ => panic!("Expected ParameterTypeMismatch for invalid boolean conditional, got: {err:?}"),
    }
}
