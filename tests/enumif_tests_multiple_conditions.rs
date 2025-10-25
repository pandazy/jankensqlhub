use jankensqlhub::{JankenError, QueryDefinitions, query_run_sqlite};
use rusqlite::Connection;

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

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE classified (priority TEXT, category TEXT, tags TEXT)",
        [],
    )
    .unwrap();

    // First insert some valid data
    let params = serde_json::json!({"priority": "high", "category": "work", "tags": "meeting"});
    let result = query_run_sqlite(&mut conn, &queries, "insert_classified", &params);
    assert!(result.is_ok(), "Valid insert should work");

    // Test that category conditions take precedence (processed first alphabetically) - query should work
    let params = serde_json::json!({"priority": "high", "category": "work", "tags": "meeting"});
    let result = query_run_sqlite(&mut conn, &queries, "select_classified", &params);
    assert!(
        result.is_ok(),
        "Category condition should be checked first alphabetically"
    );

    // Test invalid tags - should fail
    let params = serde_json::json!({"priority": "high", "category": "work", "tags": "immediate"}); // "immediate" from priority but "meeting" from category
    let err = query_run_sqlite(&mut conn, &queries, "insert_classified", &params).unwrap_err();
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
    let result = query_run_sqlite(&mut conn, &queries, "insert_classified", &params);
    assert!(result.is_ok(), "Personal category should allow family tags");

    // Test invalid tags for personal category
    let params = serde_json::json!({"priority": "low", "category": "personal", "tags": "optional"}); // "optional" from priority but not in personal category
    let err = query_run_sqlite(&mut conn, &queries, "insert_classified", &params).unwrap_err();
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
