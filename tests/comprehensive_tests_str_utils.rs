#[test]
fn test_str_utils_functionality() {
    use jankensqlhub::str_utils;

    let sql_with_escape = "SELECT 'string\\'s' FROM table WHERE @param";
    let param_pos = sql_with_escape.find("@param").unwrap();
    assert!(!str_utils::is_in_quotes(sql_with_escape, param_pos));

    let complex_quotes = r#"SELECT "double" 'single' FROM table WHERE @param"#;
    let param_pos = complex_quotes.find("@param").unwrap();
    assert!(!str_utils::is_in_quotes(complex_quotes, param_pos));

    let multi_stmt = r#"INSERT INTO t VALUES ("val"); UPDATE t SET x='value'; SELECT 1"#;
    let statements = str_utils::split_sql_statements(multi_stmt);
    assert_eq!(statements.len(), 3);
    assert!(statements[0].starts_with("INSERT"));
    assert!(statements[1].starts_with("UPDATE"));
    assert!(statements[2].starts_with("SELECT"));

    let stmt = r#"SELECT col FROM t WHERE name='literal\'quote' AND @param='value'"#;
    let params = jankensqlhub::parameters::extract_parameters_in_statement(stmt);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0], "param");
}
