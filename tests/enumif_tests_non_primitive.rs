use jankensqlhub::{
    JankenError, M_EXPECTED, M_GOT, QueryDefinitions, error_meta, query_run_sqlite,
};
use rusqlite::Connection;

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

    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE media (id INTEGER PRIMARY KEY, type TEXT, source TEXT, data TEXT)",
        [],
    )
    .unwrap();

    // Test with array as conditional parameter - should fail with basic type validation for the constrained parameter
    let params = serde_json::json!({"media_type": [1, 2, 3], "source": "artist"});
    let err = query_run_sqlite(
        &mut conn,
        &queries,
        "enumif_with_array_conditional",
        &params,
    )
    .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            // Since media_type has an enum constraint, it expects string type
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert_eq!(expected, "string");
            assert_eq!(got, "[1,2,3]");
        }
        _ => panic!(
            "Expected ParameterTypeMismatch for non-string conditional parameter, got: {err:?}"
        ),
    }

    // Test with object as conditional parameter - should fail with basic type validation
    let params = serde_json::json!({"media_type": {"nested": "object"}, "source": "channel"});
    let err = query_run_sqlite(
        &mut conn,
        &queries,
        "enumif_with_array_conditional",
        &params,
    )
    .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            // Since media_type has an enum constraint, it expects string type
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert_eq!(expected, "string");
            assert_eq!(got, "{\"nested\":\"object\"}");
        }
        _ => {
            panic!("Expected ParameterTypeMismatch for object conditional parameter, got: {err:?}")
        }
    }

    // Test with null as conditional parameter - should fail with basic type validation
    let params = serde_json::json!({"media_type": null, "source": "artist"});
    let err = query_run_sqlite(
        &mut conn,
        &queries,
        "enumif_with_array_conditional",
        &params,
    )
    .unwrap_err();
    match err {
        JankenError::ParameterTypeMismatch { data } => {
            // Since media_type has an enum constraint, it expects string type
            let expected = error_meta(&data, M_EXPECTED).unwrap();
            let got = error_meta(&data, M_GOT).unwrap();
            assert_eq!(expected, "string");
            assert_eq!(got, "null");
        }
        _ => panic!("Expected ParameterTypeMismatch for null conditional parameter, got: {err:?}"),
    }

    // Test with valid primitive conditional parameter - should work
    let params = serde_json::json!({"media_type": "song", "source": "artist"});
    let result = query_run_sqlite(
        &mut conn,
        &queries,
        "enumif_with_array_conditional",
        &params,
    );
    assert!(
        result.is_ok(),
        "Valid primitive conditional parameter should work"
    );
}
