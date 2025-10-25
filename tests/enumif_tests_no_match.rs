use jankensqlhub::{
    JankenError, M_EXPECTED, M_GOT, QueryDefinitions, error_meta, query_run_sqlite,
};
use rusqlite::Connection;

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

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE media (id INTEGER PRIMARY KEY, type TEXT, source TEXT, data TEXT)",
        [],
    )
    .unwrap();

    // Test with valid enum value but no matching enumif condition - should fail
    let params = serde_json::json!({"media_type": "movie", "source": "director"}); // "movie" not in enumif conditions
    let err = query_run_sqlite(&mut conn, &queries, "conditional_enum_query", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(
            expected,
            "conditional parameter value that matches a defined condition"
        );
        assert_eq!(
            got,
            "value not covered by any enumif condition for parameter source"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for no matching enumif condition, got: {err_str}");
    }

    // Test with another value not covered by enumif
    let params = serde_json::json!({"media_type": "book", "source": "author"}); // "book" not in enumif conditions
    let err = query_run_sqlite(&mut conn, &queries, "conditional_enum_query", &params).unwrap_err();
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        let got = error_meta(&data, M_GOT).unwrap();
        assert_eq!(
            expected,
            "conditional parameter value that matches a defined condition"
        );
        assert_eq!(
            got,
            "value not covered by any enumif condition for parameter source"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for no matching enumif condition, got: {err_str}");
    }

    // Verify that values covered by enumif conditions work correctly
    let params = serde_json::json!({"media_type": "song", "source": "artist"});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_enum_query", &params);
    assert!(result.is_ok(), "Valid enumif condition should work");

    let params = serde_json::json!({"media_type": "show", "source": "channel"});
    let result = query_run_sqlite(&mut conn, &queries, "conditional_enum_query", &params);
    assert!(result.is_ok(), "Valid enumif condition should work");
}
