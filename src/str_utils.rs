/// Utility functions for string processing, particularly SQL statement handling
use regex::Regex;

// Regex compiled once as a lazy static for performance
static PARAMETER_REGEX: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"@(\w+)").unwrap());

/// Check if a position in SQL is inside quotes (handles both single and double quotes)
pub fn is_in_quotes(sql: &str, pos: usize) -> bool {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    let chars: Vec<char> = sql.chars().take(pos + 1).collect();

    for &ch in chars.iter() {
        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '\'' => {
                if !in_double_quote {
                    in_single_quote = !in_single_quote;
                }
            }
            '"' => {
                if !in_single_quote {
                    in_double_quote = !in_double_quote;
                }
            }
            _ => {}
        }
    }

    in_single_quote || in_double_quote
}

/// Split multi-statement SQL into individual statements (respects quote boundaries)
pub fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let chars: Vec<char> = sql.chars().collect();
    let mut current_statement = String::new();
    let mut in_string = false;
    let mut string_char = '"';
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // Handle string literals
        if ch == '"' && !in_string {
            in_string = true;
            string_char = '"';
        } else if ch == '\'' && !in_string {
            in_string = true;
            string_char = '\'';
        } else if ch == string_char && in_string {
            in_string = false;
        } else if ch == ';' && !in_string {
            // Found statement terminator - add the statement
            let trimmed = current_statement.trim();
            if !trimmed.is_empty() {
                statements.push(trimmed.to_string());
            }
            current_statement.clear();
            i += 1;
            continue;
        }

        current_statement.push(ch);
        i += 1;
    }

    // Handle final statement
    let trimmed = current_statement.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }

    statements
}

/// Extract parameter names from a single statement
pub fn extract_parameters_in_statement(statement: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in PARAMETER_REGEX.captures_iter(statement) {
        if let Some(named_match) = cap.get(0) {
            if !is_in_quotes(statement, named_match.start()) {
                let name = cap.get(1).unwrap().as_str().to_string();
                if seen.insert(name.clone()) {
                    params.push(name);
                }
            }
        }
    }

    params
}
