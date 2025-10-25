use jankensqlhub::{JankenError, M_EXPECTED, QueryDefinitions, error_meta};

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
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(
            expected,
            "object mapping condition values to allowed arrays"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for malformed enumif, got: {err_str}");
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
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(expected, "array of allowed values");
    } else {
        panic!("Expected ParameterTypeMismatch for non-array enumif values, got: {err_str}");
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
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(
            expected,
            "object mapping conditional parameters to conditions"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for wrong enumif structure, got: {err_str}");
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
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(
            expected,
            "object mapping conditional parameters to conditions"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for null enumif, got: {err_str}");
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
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(
            expected,
            "object mapping conditional parameters to conditions"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for string enumif, got: {err_str}");
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
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(
            expected,
            "object mapping conditional parameters to conditions"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for number enumif, got: {err_str}");
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
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(
            expected,
            "object mapping conditional parameters to conditions"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for boolean enumif, got: {err_str}");
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
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(
            expected,
            "enumif allowed values to be primitives (string, number, or boolean)"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for blob values in enumif, got: {err_str}");
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
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(
            expected,
            "enumif allowed values to be primitives (string, number, or boolean)"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for object values in enumif, got: {err_str}");
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
    let err_str = format!("{err:?}");
    if let Ok(JankenError::ParameterTypeMismatch { data }) = err.downcast::<JankenError>() {
        let expected = error_meta(&data, M_EXPECTED).unwrap();
        assert_eq!(
            expected,
            "enumif allowed values to be primitives (string, number, or boolean)"
        );
    } else {
        panic!("Expected ParameterTypeMismatch for null values in enumif, got: {err_str}");
    }
}
