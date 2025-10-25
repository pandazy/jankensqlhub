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
