use crate::QueryDef;
use crate::parameters::COMMA_LIST_REGEX;
use crate::result::JankenError;
use anyhow;
use serde_json;
use std::collections::{HashMap, HashSet};
use std::fs;

/// Collection of parsed SQL query definitions loaded from JSON configuration
#[derive(Debug)]
pub struct QueryDefinitions {
    /// Named query definitions keyed by their identifying name
    pub definitions: HashMap<String, QueryDef>,
}

impl QueryDefinitions {
    /// Load query definitions from a JSON file
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let json: serde_json::Value = serde_json::from_str(&content)?;
        Self::from_json(json)
    }

    /// Load query definitions from a serde_json::Value object
    pub fn from_json(json: serde_json::Value) -> anyhow::Result<Self> {
        let json_map = match json.as_object() {
            Some(map) => map,
            None => {
                let expected = "object";
                let got = json.to_string();
                let err = JankenError::new_parameter_type_mismatch(expected, got);
                return Err(err.into());
            }
        };

        let mut definitions = HashMap::new();
        for (name, value) in json_map {
            let map = value.as_object().ok_or_else(|| {
                let expected = "object";
                let got = format!("{name}: {value}");
                JankenError::new_parameter_type_mismatch(expected, got)
            })?;

            let sql = map.get("query").and_then(|q| q.as_str()).ok_or_else(|| {
                let expected = "required 'query' field with string value";
                let got = format!("{name}: missing 'query' field");
                JankenError::new_parameter_type_mismatch(expected, got)
            })?;

            let args = map.get("args").and_then(|a| a.as_object());
            let mut query_def = QueryDef::from_sql(sql, args)?;

            // Parse returns field
            if let Some(returns_val) = map.get("returns") {
                if let Some(returns_array) = returns_val.as_array() {
                    // Static array of field names
                    let returns: Vec<String> = returns_array
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();
                    // Deduplicate using a set but maintain order
                    let mut seen = HashSet::new();
                    let unique_returns: Vec<String> = returns
                        .into_iter()
                        .filter(|item| seen.insert(item.clone()))
                        .collect();
                    query_def.returns = crate::query::ReturnsSpec::Static(unique_returns);
                } else if let Some(returns_str) = returns_val.as_str() {
                    // Dynamic reference to a comma_list parameter: ~[param_name]
                    // Use the existing COMMA_LIST_REGEX to validate and extract parameter name
                    if let Some(cap) = COMMA_LIST_REGEX.captures(returns_str) {
                        // Ensure the entire string matches (no extra characters)
                        if cap
                            .get(0)
                            .expect("regex capture group 0 always exists when captures matches")
                            .as_str()
                            == returns_str
                        {
                            let param_name = cap
                                .get(1)
                                .expect("COMMA_LIST_REGEX always has capture group 1")
                                .as_str();

                            // Verify this parameter exists and is a CommaList type
                            let param_exists = query_def.parameters.iter().any(|p| {
                                p.name == param_name
                                    && p.param_type == crate::ParameterType::CommaList
                            });

                            if param_exists {
                                query_def.returns =
                                    crate::query::ReturnsSpec::Dynamic(param_name.to_string());
                            } else {
                                let expected = format!(
                                    "returns reference ~[{}] to point to an existing comma_list parameter",
                                    param_name
                                );
                                let got = format!(
                                    "parameter '{}' not found or not a comma_list type",
                                    param_name
                                );
                                return Err(JankenError::new_parameter_type_mismatch(
                                    &expected, &got,
                                )
                                .into());
                            }
                        } else {
                            let expected = "array of strings or ~[param_name] format";
                            let got =
                                format!("string not in ~[param_name] format: {}", returns_str);
                            return Err(
                                JankenError::new_parameter_type_mismatch(expected, &got).into()
                            );
                        }
                    } else {
                        let expected = "array of strings or ~[param_name] format";
                        let got = format!("string not in ~[param_name] format: {}", returns_str);
                        return Err(JankenError::new_parameter_type_mismatch(expected, &got).into());
                    }
                } else {
                    let expected = "array of strings or ~[param_name] format";
                    let got = returns_val.to_string();
                    return Err(JankenError::new_parameter_type_mismatch(expected, got).into());
                }
            } else {
                // No returns specified - empty static array
                query_def.returns = crate::query::ReturnsSpec::Static(Vec::new());
            }

            definitions.insert(name.clone(), query_def);
        }
        Ok(QueryDefinitions { definitions })
    }
}
