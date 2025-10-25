use jankensqlhub::{
    ERR_CODE_IO, ERR_CODE_JSON, ERR_CODE_PARAMETER_NAME_CONFLICT, ERR_CODE_PARAMETER_NOT_PROVIDED,
    ERR_CODE_PARAMETER_TYPE_MISMATCH, ERR_CODE_POSTGRES, ERR_CODE_QUERY_NOT_FOUND, ERR_CODE_REGEX,
    ERR_CODE_SQLITE, JankenError, get_error_data, get_error_info,
};

#[test]
fn test_parameter_parsing_with_valid_parameters() {
    // Test normal parameter parsing works and indirectly tests the regex capture
    // We test valid parameter parsing to ensure no errors occur in the normal case
    use jankensqlhub::parameters::parse_parameters_with_quotes;

    // Test parsing parameters from a normal SQL query
    let sql = "SELECT * FROM users WHERE id=@user_id AND name=@user_name AND age=@user_age";
    let parameters = parse_parameters_with_quotes(sql).unwrap();

    // Verify we captured all parameters correctly
    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[0].name, "user_id");
    assert_eq!(parameters[1].name, "user_name");
    assert_eq!(parameters[2].name, "user_age");

    // Verify all parameters default to string type and have no constraints
    for param in &parameters {
        assert_eq!(param.param_type.to_string(), "string");
        assert!(param.constraints.range.is_none());
        assert!(param.constraints.pattern.is_none());
        assert!(param.constraints.enum_values.is_none());
    }
}

#[test]
fn test_all_error_codes_are_present() {
    // Additional verification that all expected error codes have mappings
    let expected_codes = [
        ERR_CODE_IO,
        ERR_CODE_JSON,
        ERR_CODE_SQLITE,
        ERR_CODE_POSTGRES,
        ERR_CODE_QUERY_NOT_FOUND,
        ERR_CODE_PARAMETER_NOT_PROVIDED,
        ERR_CODE_PARAMETER_TYPE_MISMATCH,
        ERR_CODE_PARAMETER_NAME_CONFLICT,
        ERR_CODE_REGEX,
    ];

    for &code in &expected_codes {
        assert!(
            get_error_info(code).is_some(),
            "Error code {code} should have info"
        );
    }
}

#[test]
fn test_no_args_provided_for_parameter_in_sql() {
    // Test that parameters in SQL work with no args - they get default string type
    use jankensqlhub::QueryDef;

    let sql = "SELECT * FROM source WHERE id=@param";
    let result = QueryDef::from_sql(sql, None);

    assert!(result.is_ok());
    let query_def = result.unwrap();

    // Verify the parameter was created with default string type and no constraints
    assert_eq!(query_def.parameters.len(), 1);
    let param = &query_def.parameters[0];
    assert_eq!(param.name, "param");
    assert_eq!(param.param_type.to_string(), "string");
    assert!(param.constraints.range.is_none());
    assert!(param.constraints.pattern.is_none());
    assert!(param.constraints.enum_values.is_none());
}

#[test]
fn test_get_error_data() {
    // Test get_error_data helper function extracts ErrorData from all error variants

    // Test ParameterTypeMismatch variant
    let err = JankenError::new_parameter_type_mismatch("integer", "string");
    let data = get_error_data(&err);
    assert_eq!(data.code, 2020); // ERR_CODE_PARAMETER_TYPE_MISMATCH
    assert!(data.metadata.is_some());

    // Test QueryNotFound variant
    let err = JankenError::new_query_not_found("test_query");
    let data = get_error_data(&err);
    assert_eq!(data.code, 2000); // ERR_CODE_QUERY_NOT_FOUND
    assert!(data.metadata.is_some());

    // Test ParameterNotProvided variant
    let err = JankenError::new_parameter_not_provided("missing_param");
    let data = get_error_data(&err);
    assert_eq!(data.code, 2010); // ERR_CODE_PARAMETER_NOT_PROVIDED
    assert!(data.metadata.is_some());

    // Test ParameterNameConflict variant
    let err = JankenError::new_parameter_name_conflict("conflicting_param");
    let data = get_error_data(&err);
    assert_eq!(data.code, 2030); // ERR_CODE_PARAMETER_NAME_CONFLICT
    assert!(data.metadata.is_some());
}

#[test]
fn test_get_error_info() {
    // Test get_error_info helper function looks up error information by code

    // Test by creating actual error instances for all error types and verifying their codes work with get_error_info

    // Io error
    let io_err = JankenError::new_io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
    let io_code = get_error_data(&io_err).code;
    let io_info = get_error_info(io_code).unwrap();
    assert_eq!(io_info.code, ERR_CODE_IO);
    assert_eq!(io_info.name, "IO_ERROR");
    assert_eq!(io_info.category, "System");
    assert_eq!(io_info.description, "Input/output operation failed");

    // Json error
    let json_err = JankenError::new_json(
        serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err(),
    );
    let json_code = get_error_data(&json_err).code;
    let json_info = get_error_info(json_code).unwrap();
    assert_eq!(json_info.code, ERR_CODE_JSON);
    assert_eq!(json_info.name, "JSON_ERROR");
    assert_eq!(json_info.category, "Serialization");
    assert_eq!(
        json_info.description,
        "JSON parsing or serialization failed"
    );

    // Sqlite error (using a mock error)
    let sqlite_err = JankenError::new_sqlite(rusqlite::Error::QueryReturnedNoRows);
    let sqlite_code = get_error_data(&sqlite_err).code;
    let sqlite_info = get_error_info(sqlite_code).unwrap();
    assert_eq!(sqlite_info.code, ERR_CODE_SQLITE);
    assert_eq!(sqlite_info.name, "SQLITE_ERROR");
    assert_eq!(sqlite_info.category, "Database");
    assert_eq!(sqlite_info.description, "SQLite database operation failed");

    // Postgres error (test constant directly since tokio_postgres::Error is hard to construct for testing)
    let postgres_info = get_error_info(ERR_CODE_POSTGRES).unwrap();
    assert_eq!(postgres_info.code, ERR_CODE_POSTGRES);
    assert_eq!(postgres_info.name, "POSTGRES_ERROR");
    assert_eq!(postgres_info.category, "Database");
    assert_eq!(
        postgres_info.description,
        "PostgreSQL database operation failed"
    );

    // QueryNotFound error
    let query_not_found_err = JankenError::new_query_not_found("test_query");
    let query_not_found_code = get_error_data(&query_not_found_err).code;
    let query_not_found_info = get_error_info(query_not_found_code).unwrap();
    assert_eq!(query_not_found_info.code, ERR_CODE_QUERY_NOT_FOUND);
    assert_eq!(query_not_found_info.name, "QUERY_NOT_FOUND");
    assert_eq!(query_not_found_info.category, "Query");
    assert_eq!(
        query_not_found_info.description,
        "Requested query definition was not found"
    );

    // ParameterNotProvided error
    let param_not_provided_err = JankenError::new_parameter_not_provided("missing_param");
    let param_not_provided_code = get_error_data(&param_not_provided_err).code;
    let param_not_provided_info = get_error_info(param_not_provided_code).unwrap();
    assert_eq!(
        param_not_provided_info.code,
        ERR_CODE_PARAMETER_NOT_PROVIDED
    );
    assert_eq!(param_not_provided_info.name, "PARAMETER_NOT_PROVIDED");
    assert_eq!(param_not_provided_info.category, "Parameter");
    assert_eq!(
        param_not_provided_info.description,
        "Required parameter was not provided"
    );

    // ParameterTypeMismatch error
    let param_type_mismatch_err = JankenError::new_parameter_type_mismatch("integer", "string");
    let param_type_mismatch_code = get_error_data(&param_type_mismatch_err).code;
    let param_type_mismatch_info = get_error_info(param_type_mismatch_code).unwrap();
    assert_eq!(
        param_type_mismatch_info.code,
        ERR_CODE_PARAMETER_TYPE_MISMATCH
    );
    assert_eq!(param_type_mismatch_info.name, "PARAMETER_TYPE_MISMATCH");
    assert_eq!(param_type_mismatch_info.category, "Parameter");
    assert_eq!(
        param_type_mismatch_info.description,
        "Parameter value does not match expected type"
    );

    // ParameterNameConflict error
    let param_name_conflict_err = JankenError::new_parameter_name_conflict("conflicting_param");
    let param_name_conflict_code = get_error_data(&param_name_conflict_err).code;
    let param_name_conflict_info = get_error_info(param_name_conflict_code).unwrap();
    assert_eq!(
        param_name_conflict_info.code,
        ERR_CODE_PARAMETER_NAME_CONFLICT
    );
    assert_eq!(param_name_conflict_info.name, "PARAMETER_NAME_CONFLICT");
    assert_eq!(param_name_conflict_info.category, "Parameter");
    assert_eq!(
        param_name_conflict_info.description,
        "Parameter name conflicts with table name"
    );

    // Regex error
    let regex_err = JankenError::new_regex(regex::Error::Syntax("invalid regex".to_string()));
    let regex_code = get_error_data(&regex_err).code;
    let regex_info = get_error_info(regex_code).unwrap();
    assert_eq!(regex_info.code, ERR_CODE_REGEX);
    assert_eq!(regex_info.name, "REGEX_ERROR");
    assert_eq!(regex_info.category, "Pattern");
    assert_eq!(
        regex_info.description,
        "Regular expression compilation or matching failed"
    );

    // Test invalid code
    let invalid_info = get_error_info(9999);
    assert!(invalid_info.is_none());

    // Verify all error constants match their mappings (postgres already verified above)
    assert_eq!(io_info.code, ERR_CODE_IO);
    assert_eq!(json_info.code, ERR_CODE_JSON);
    assert_eq!(sqlite_info.code, ERR_CODE_SQLITE);
    assert_eq!(query_not_found_info.code, ERR_CODE_QUERY_NOT_FOUND);
    assert_eq!(
        param_not_provided_info.code,
        ERR_CODE_PARAMETER_NOT_PROVIDED
    );
    assert_eq!(
        param_type_mismatch_info.code,
        ERR_CODE_PARAMETER_TYPE_MISMATCH
    );
    assert_eq!(
        param_name_conflict_info.code,
        ERR_CODE_PARAMETER_NAME_CONFLICT
    );
    assert_eq!(regex_info.code, ERR_CODE_REGEX);
}
