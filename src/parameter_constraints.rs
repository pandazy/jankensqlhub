use crate::{
    ParameterType,
    result::{JankenError, Result},
};
use regex::Regex;
use std::collections::HashMap;
use std::str::FromStr;

/// Parameter constraints for validation
#[derive(Debug, Clone, Default)]
pub struct ParameterConstraints {
    pub range: Option<Vec<f64>>, // For numeric types: [min, max]
    pub pattern: Option<String>, // For string types: regex pattern
    pub enum_values: Option<Vec<serde_json::Value>>, // For any type: allowed values
    pub item_type: Option<crate::ParameterType>, // For list types: the type of each item
    pub enumif: Option<HashMap<String, HashMap<String, Vec<serde_json::Value>>>>, // Conditional enums: {"other_param": {"value": [allowed_values]}}
}

impl ParameterConstraints {
    /// Convert a JSON value to a condition key string for enumif constraints
    /// Only primitive values (String, Number, Bool) are allowed as condition keys
    pub(crate) fn value_to_condition_key(
        value: &serde_json::Value,
        param_name: &str,
    ) -> Result<String> {
        match value {
            serde_json::Value::String(s) => Ok(s.clone()),
            serde_json::Value::Number(n) => Ok(n.to_string()),
            serde_json::Value::Bool(b) => Ok(b.to_string()),
            _ => Err(JankenError::new_parameter_type_mismatch(
                "conditional parameter to be primitive (string, number, or boolean)",
                format!("{value} (type {value}) for parameter {param_name}"),
            )),
        }
    }

    /// Validate basic type (without any constraints)
    fn validate_basic_type(
        value: &serde_json::Value,
        param_type: &crate::ParameterType,
    ) -> Result<()> {
        match param_type {
            crate::ParameterType::String => {
                if !value.is_string() {
                    return Err(Self::constraint_mismatch_error(param_type, value));
                }
            }
            crate::ParameterType::Integer => {
                if !value.is_number() || value.as_number().unwrap().as_i64().is_none() {
                    return Err(Self::constraint_mismatch_error(param_type, value));
                }
            }
            crate::ParameterType::Float => {
                if !value.is_number() || value.as_number().unwrap().as_f64().is_none() {
                    return Err(Self::constraint_mismatch_error(param_type, value));
                }
            }
            crate::ParameterType::Boolean => {
                if !value.is_boolean() {
                    return Err(Self::constraint_mismatch_error(param_type, value));
                }
            }
            crate::ParameterType::TableName => {
                if !value.is_string() {
                    return Err(Self::constraint_mismatch_error(param_type, value));
                }
            }
            crate::ParameterType::Blob => {
                if !value.is_array() {
                    return Err(Self::constraint_mismatch_error(param_type, value));
                }
                // Validate that all elements are byte values (0-255)
                if let Some(arr) = value.as_array() {
                    for (i, item) in arr.iter().enumerate() {
                        if let Some(num) = item.as_u64() {
                            if num > 255 {
                                return Err(JankenError::new_parameter_type_mismatch(
                                    format!("byte values (0-255) at index {i}"),
                                    format!("{num}"),
                                ));
                            }
                        } else {
                            return Err(JankenError::new_parameter_type_mismatch(
                                format!("byte values (0-255) at index {i}"),
                                item.to_string(),
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn constraint_mismatch_error(
        param_type: &crate::ParameterType,
        value: &serde_json::Value,
    ) -> JankenError {
        JankenError::new_parameter_type_mismatch(param_type.to_string(), value.to_string())
    }

    /// Validate parameter constraints (range, pattern, enum, enumif) assuming basic type validation is already done
    fn validate_constraints(
        &self,
        value: &serde_json::Value,
        param_type: &crate::ParameterType,
        param_name: &str,
        all_params: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        // First validate basic type
        Self::validate_basic_type(value, param_type)?;
        // Then validate the constraint rules
        self.validate_constraint_rules(value, param_type, param_name, all_params)
    }

    /// Validate constraint rules (range, pattern, enum, enumif) only, assuming basic type is already validated
    fn validate_constraint_rules(
        &self,
        value: &serde_json::Value,
        param_type: &crate::ParameterType,
        param_name: &str,
        all_params: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        // Check that range is only specified for numeric types and blob
        if self.range.is_some()
            && !matches!(
                param_type,
                crate::ParameterType::Integer
                    | crate::ParameterType::Float
                    | crate::ParameterType::Blob
            )
        {
            return Err(JankenError::new_parameter_type_mismatch(
                "numeric type or blob",
                param_type.to_string(),
            ));
        }

        // Check range for numeric types and blob size
        if let Some(range) = &self.range {
            match param_type {
                crate::ParameterType::Integer | crate::ParameterType::Float => {
                    // Validated upfront that param_type is Integer or Float, so value is number
                    let num_val = value.as_f64().unwrap();

                    if let (Some(&min), Some(&max)) = (range.first(), range.get(1)) {
                        if num_val < min || num_val > max {
                            return Err(JankenError::new_parameter_type_mismatch(
                                format!("value between {min} and {max}"),
                                num_val.to_string(),
                            ));
                        }
                    }
                }
                crate::ParameterType::Blob => {
                    // For blob, range represents min/max size in bytes
                    let blob_size = value.as_array().unwrap().len() as f64;

                    if let (Some(&min), Some(&max)) = (range.first(), range.get(1)) {
                        if blob_size < min || blob_size > max {
                            return Err(JankenError::new_parameter_type_mismatch(
                                format!("blob size between {min} and {max} bytes"),
                                format!("{blob_size} bytes"),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        // Check pattern for string types
        if let Some(pattern) = &self.pattern {
            if let Some(string_val) = value.as_str() {
                let regex = Regex::new(pattern).map_err(|_| {
                    JankenError::new_parameter_type_mismatch("valid regex pattern", pattern.clone())
                })?;
                if !regex.is_match(string_val) {
                    return Err(JankenError::new_parameter_type_mismatch(
                        format!("string matching pattern '{pattern}'"),
                        string_val,
                    ));
                }
            } else {
                return Err(JankenError::new_parameter_type_mismatch(
                    ParameterType::String.to_string(),
                    value.to_string(),
                ));
            }
        }

        // Check enum values
        if let Some(enum_values) = &self.enum_values {
            if !enum_values.contains(value) {
                let enum_str = enum_values
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(JankenError::new_parameter_type_mismatch(
                    format!("one of [{enum_str}]"),
                    value.to_string(),
                ));
            }
        }

        // Check conditional enum constraints - this is security-critical, so we must find valid conditions
        if let Some(enumif) = &self.enumif {
            // Sort the conditional parameters alphabetically for deterministic behavior
            let mut sorted_conditional_params: Vec<&String> = enumif.keys().collect();
            sorted_conditional_params.sort();

            let mut found_matching_condition = false;
            let mut allowed_values: Option<&Vec<serde_json::Value>> = None;

            for conditional_param in sorted_conditional_params {
                if let Some(conditions) = enumif.get(conditional_param) {
                    if let Some(cond_val) = all_params.get(conditional_param) {
                        // Get the conditional value as a string key (without JSON quotes)
                        let cond_val_str =
                            Self::value_to_condition_key(cond_val, conditional_param)?;

                        if let Some(allowed) = conditions.get(&cond_val_str) {
                            found_matching_condition = true;
                            // Use the first matching condition (as processed in alphabetical order)
                            if allowed_values.is_none() {
                                allowed_values = Some(allowed);
                            }
                        }
                    }
                }
            }

            // Security: If we have enumif constraint but no matching condition was found,
            // this means the conditional parameter value doesn't correspond to any defined condition,
            // which implies invalid state and should be rejected to prevent injection
            if !found_matching_condition {
                return Err(JankenError::new_parameter_type_mismatch(
                    "conditional parameter value that matches a defined condition",
                    format!("value not covered by any enumif condition for parameter {param_name}"),
                ));
            }

            // Validate against the allowed values from the matching condition
            if let Some(allowed) = allowed_values {
                if !allowed.contains(value) {
                    let allowed_str = allowed
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(JankenError::new_parameter_type_mismatch(
                        format!("one of [{allowed_str}] based on conditional parameters"),
                        value.to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate a parameter value against these constraints
    pub fn validate(
        &self,
        value: &serde_json::Value,
        param_type: &crate::ParameterType,
        param_name: &str,
        all_params: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        if param_type == &crate::ParameterType::List {
            if !value.is_array() {
                return Err(Self::constraint_mismatch_error(param_type, value));
            }

            // Validate each item in the list if item_type is specified
            // Note: item_type validation is already done during constraint parsing at definition time
            if let Some(item_type) = &self.item_type {
                let array = value.as_array().unwrap(); // already verified at the beginning;
                for (index, item) in array.iter().enumerate() {
                    // Validate basic type and constraints for each item
                    if Self::validate_basic_type(item, item_type).is_err() {
                        return Err(JankenError::new_parameter_type_mismatch(
                            format!("{item_type} at index {index}"),
                            item.to_string(),
                        ));
                    }
                    self.validate_constraint_rules(item, item_type, param_name, all_params)?;
                }
            }
            // For Lists, constraints (range, pattern, enum) apply to items if item_type is set,
            // but not to the list itself, so we don't call validate_constraints after this match
            return Ok(());
        }

        self.validate_constraints(value, param_type, param_name, all_params)?;

        if param_type == &crate::ParameterType::TableName {
            // value cannot be a non-string here since basic type validation has been done
            let table_name_str = value.as_str().unwrap();
            if table_name_str.is_empty()
                || !table_name_str
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_')
            {
                return Err(JankenError::new_parameter_type_mismatch(
                    "valid table name (alphanumeric and underscores only)",
                    table_name_str,
                ));
            }
        }

        Ok(())
    }
}

/// Parse constraints from JSON into ParameterConstraints
pub fn parse_constraints(
    constraints: &mut ParameterConstraints,
    arg_def: &serde_json::Value,
) -> Result<()> {
    if let Some(range_val) = arg_def.get("range") {
        let expected_content = "array with exactly 2 numbers for range constraint";
        let range_array = match range_val.as_array() {
            Some(arr) if arr.len() == 2 => arr,
            Some(arr) => {
                return Err(JankenError::new_parameter_type_mismatch(
                    expected_content,
                    format!("array with {} elements", arr.len()),
                ));
            }
            None => {
                return Err(JankenError::new_parameter_type_mismatch(
                    expected_content,
                    format!("{range_val} (not an array)"),
                ));
            }
        };

        let range: Vec<f64> = range_array
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.as_f64().ok_or_else(|| {
                    JankenError::new_parameter_type_mismatch("number", format!("{v} at index {i}"))
                })
            })
            .collect::<Result<Vec<_>>>()?;

        constraints.range = Some(range);
    }

    if let Some(pattern_val) = arg_def.get("pattern") {
        if let Some(pattern_str) = pattern_val.as_str() {
            constraints.pattern = Some(pattern_str.to_string());
        }
    }

    if let Some(enum_val) = arg_def.get("enum") {
        if let Some(enum_array) = enum_val.as_array() {
            constraints.enum_values = Some(enum_array.clone());
        }
    }

    if let Some(enumif_val) = arg_def.get("enumif") {
        if let Some(enumif_obj) = enumif_val.as_object() {
            let mut enumif_map = HashMap::new();

            for (conditional_param, conditions) in enumif_obj {
                if let Some(conditions_obj) = conditions.as_object() {
                    let mut conditions_map = HashMap::new();

                    for (value_key, allowed_values) in conditions_obj {
                        if let Some(allowed_array) = allowed_values.as_array() {
                            // Validate that enumif allowed values are only primitives (numbers, booleans, strings) - no blobs/arrays
                            for (i, allowed_value) in allowed_array.iter().enumerate() {
                                match allowed_value {
                                    serde_json::Value::String(_)
                                    | serde_json::Value::Number(_)
                                    | serde_json::Value::Bool(_) => {
                                        // These are allowed
                                    }
                                    _ => {
                                        return Err(JankenError::new_parameter_type_mismatch(
                                            "enumif allowed values to be primitives (string, number, or boolean)",
                                            format!(
                                                "{allowed_value} (type {allowed_value}) at index {i} for condition {value_key}"
                                            ),
                                        ));
                                    }
                                }
                            }
                            conditions_map.insert(value_key.to_string(), allowed_array.clone());
                        } else {
                            return Err(JankenError::new_parameter_type_mismatch(
                                "array of allowed values",
                                format!("{allowed_values} for condition {value_key}"),
                            ));
                        }
                    }

                    enumif_map.insert(conditional_param.to_string(), conditions_map);
                } else {
                    return Err(JankenError::new_parameter_type_mismatch(
                        "object mapping condition values to allowed arrays",
                        format!("{conditions} for parameter {conditional_param}"),
                    ));
                }
            }

            constraints.enumif = Some(enumif_map);
        } else {
            return Err(JankenError::new_parameter_type_mismatch(
                "object mapping conditional parameters to conditions",
                format!("{enumif_val}"),
            ));
        }
    }

    // Validate that enum and enumif are mutually exclusive
    if constraints.enum_values.is_some() && constraints.enumif.is_some() {
        return Err(JankenError::new_parameter_type_mismatch(
            "either 'enum' or 'enumif', not both",
            "'enum' and 'enumif' both specified",
        ));
    }

    if let Some(itemtype_val) = arg_def.get("itemtype") {
        if let Some(itemtype_str) = itemtype_val.as_str() {
            let item_type = ParameterType::from_str(itemtype_str)?;
            // Validate item type - TableName and List are not allowed as item types
            match item_type {
                ParameterType::TableName | ParameterType::List => {
                    return Err(JankenError::new_parameter_type_mismatch(
                        "item_type for list items cannot be TableName or List",
                        item_type.to_string(),
                    ));
                }
                _ => {}
            }
            constraints.item_type = Some(item_type);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{M_EXPECTED, M_GOT, result::JankenError};

    #[test]
    fn test_value_to_condition_key_non_primitive_error() {
        // Test the impossible edge case: non-primitive values as condition keys
        // This should never happen in normal operation due to upstream validation,
        // but we test it to ensure the defensive error handling works correctly

        let non_primitive_val =
            serde_json::Value::Array(vec![serde_json::json!(1), serde_json::json!(2)]);
        let result = ParameterConstraints::value_to_condition_key(&non_primitive_val, "param");
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            JankenError::ParameterTypeMismatch { data } => {
                let metadata: serde_json::Value =
                    serde_json::from_str(&data.metadata.unwrap_or("{}".to_string())).unwrap();
                let expected = metadata.get(M_EXPECTED).unwrap().as_str().unwrap();
                let got = metadata.get(M_GOT).unwrap().as_str().unwrap();
                assert_eq!(
                    expected,
                    "conditional parameter to be primitive (string, number, or boolean)"
                );
                assert!(got.contains("[1,2]"));
                assert!(got.contains("param"));
            }
            _ => panic!("Expected ParameterTypeMismatch, got: {err:?}"),
        }
    }
}
