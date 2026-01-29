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
    pub enumif: Option<HashMap<String, HashMap<String, Vec<serde_json::Value>>>>, // Conditional enums: {"other_param": {"value": [allowed_values]}} where value can be "exact_match", "start:pattern", "end:pattern", or "contain:pattern"
}

impl ParameterConstraints {
    /// Check if a condition key matches a value, supporting fuzzy matching patterns
    ///
    /// Any condition key containing ':' will be treated as a fuzzy match pattern.
    ///
    /// Patterns can be:
    /// - "exact_value" - exact match (no colon)
    /// - "start:pattern" - value starts with pattern
    /// - "end:pattern" - value ends with pattern
    /// - "contain:pattern" - value contains pattern
    fn matches_condition_key(condition_key: &str, value_str: &str) -> bool {
        if let Some(colon_pos) = condition_key.find(':') {
            let (match_type, pattern) = condition_key.split_at(colon_pos);
            let pattern = &pattern[1..]; // Skip the colon

            match match_type {
                "start" => value_str.starts_with(pattern),
                "end" => value_str.ends_with(pattern),
                "contain" => value_str.contains(pattern),
                _ => unreachable!(
                    "Invalid match type - should be validated at parse time in parse_constraints"
                ),
            }
        } else {
            // Exact match
            condition_key == value_str
        }
    }

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

    /// Validate a single value against pattern constraint
    /// Returns Ok(()) if validation passes, Err if it fails
    /// `context` is used to provide context in error messages (e.g., " at index 0" for array items)
    fn validate_pattern(&self, value: &serde_json::Value, context: &str) -> Result<()> {
        let Some(pattern) = &self.pattern else {
            return Ok(());
        };

        // Defensive null check for resilience - null values are typically rejected
        // by validate_basic_type before this is called, but we handle it here as well
        if value.is_null() {
            return Ok(());
        }

        if let Some(string_val) = value.as_str() {
            let regex = Regex::new(pattern).map_err(|_| {
                JankenError::new_parameter_type_mismatch("valid regex pattern", pattern.clone())
            })?;
            if !regex.is_match(string_val) {
                return Err(JankenError::new_parameter_type_mismatch(
                    format!("string matching pattern '{pattern}'{context}"),
                    string_val,
                ));
            }
        } else {
            return Err(JankenError::new_parameter_type_mismatch(
                ParameterType::String.to_string(),
                value.to_string(),
            ));
        }

        Ok(())
    }

    /// Validate a single value against enum constraint
    /// Returns Ok(()) if validation passes, Err if it fails
    /// `context` is used to provide context in error messages (e.g., " at index 0" for array items)
    fn validate_enum(&self, value: &serde_json::Value, context: &str) -> Result<()> {
        let Some(enum_values) = &self.enum_values else {
            return Ok(());
        };

        if !enum_values.contains(value) {
            let enum_str = enum_values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(JankenError::new_parameter_type_mismatch(
                format!("one of [{enum_str}]{context}"),
                value.to_string(),
            ));
        }

        Ok(())
    }

    /// Validate a single value against enumif constraints
    /// Returns Ok(()) if validation passes, Err if it fails
    /// `context` is used to provide context in error messages (e.g., " at index 0" for array items)
    fn validate_enumif(
        &self,
        value: &serde_json::Value,
        param_name: &str,
        all_params: &serde_json::Map<String, serde_json::Value>,
        context: &str,
    ) -> Result<()> {
        let Some(enumif) = &self.enumif else {
            return Ok(());
        };

        // Sort the conditional parameters alphabetically for deterministic behavior
        let mut sorted_conditional_params: Vec<&String> = enumif.keys().collect();
        sorted_conditional_params.sort();

        let mut found_matching_condition = false;
        let mut allowed_values: Option<&Vec<serde_json::Value>> = None;

        for conditional_param in sorted_conditional_params {
            if let Some(conditions) = enumif.get(conditional_param) {
                if let Some(cond_val) = all_params.get(conditional_param) {
                    // Get the conditional value as a string key (without JSON quotes)
                    let cond_val_str = Self::value_to_condition_key(cond_val, conditional_param)?;

                    // Sort condition keys alphabetically for deterministic matching order
                    let mut sorted_condition_keys: Vec<&String> = conditions.keys().collect();
                    sorted_condition_keys.sort();

                    // CONFLICT RESOLUTION: When multiple fuzzy patterns could match the same value
                    // (e.g., "contain:admin" and "start:admin" both matching "admin_user"),
                    // we use the first match in alphabetically sorted order.
                    // This avoids unnecessary complication of trying to determine the "best" match
                    // and provides predictable, deterministic behavior.
                    for condition_key in sorted_condition_keys {
                        if Self::matches_condition_key(condition_key, &cond_val_str) {
                            found_matching_condition = true;
                            // Use the first matching condition (as processed in alphabetical order)
                            // If multiple conditional params match, the first one (alphabetically) wins
                            if allowed_values.is_none() {
                                allowed_values = conditions.get(condition_key);
                            }
                            break; // Stop at first match for this conditional param
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
                format!(
                    "value not covered by any enumif condition for parameter {param_name}{context}"
                ),
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
                    format!("one of [{allowed_str}] based on conditional parameters{context}"),
                    value.to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Validate basic type (without any constraints)
    fn validate_basic_type(
        value: &serde_json::Value,
        param_type: &crate::ParameterType,
    ) -> Result<()> {
        // Null values are no longer allowed since ParameterValue::Null was removed
        if value.is_null() {
            return Err(Self::constraint_mismatch_error(param_type, value));
        }

        match param_type {
            crate::ParameterType::String => {
                if !value.is_string() {
                    return Err(Self::constraint_mismatch_error(param_type, value));
                }
            }
            crate::ParameterType::Integer => {
                if !value.is_number()
                    || value
                        .as_number()
                        .expect("is_number() already verified this is a number")
                        .as_i64()
                        .is_none()
                {
                    return Err(Self::constraint_mismatch_error(param_type, value));
                }
            }
            crate::ParameterType::Float => {
                if !value.is_number()
                    || value
                        .as_number()
                        .expect("is_number() already verified this is a number")
                        .as_f64()
                        .is_none()
                {
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

    /// Validate that a string is a valid table name (alphanumeric and underscores only)
    fn validate_table_name_format(table_name: &str, context: &str) -> Result<()> {
        if table_name.is_empty() || !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(JankenError::new_parameter_type_mismatch(
                format!("valid table name (alphanumeric and underscores only){context}"),
                table_name,
            ));
        }
        Ok(())
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
        self.validate_constraint_rules(value, param_type, param_name, all_params, "")
    }

    /// Validate constraint rules (range, pattern, enum, enumif) only, assuming basic type is already validated
    /// `context` is used to provide context in error messages (e.g., " at index 0" for array items)
    fn validate_constraint_rules(
        &self,
        value: &serde_json::Value,
        param_type: &crate::ParameterType,
        param_name: &str,
        all_params: &serde_json::Map<String, serde_json::Value>,
        context: &str,
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

        // Check range for numeric types and blob size (skip if value is null)
        if let Some(range) = &self.range {
            if !value.is_null() {
                match param_type {
                    crate::ParameterType::Integer | crate::ParameterType::Float => {
                        // Validated upfront that param_type is Integer or Float, so value is number
                        let num_val = value
                            .as_f64()
                            .expect("value already validated as numeric type");

                        if let (Some(&min), Some(&max)) = (range.first(), range.get(1)) {
                            if num_val < min || num_val > max {
                                return Err(JankenError::new_parameter_type_mismatch(
                                    format!("value between {min} and {max}{context}"),
                                    num_val.to_string(),
                                ));
                            }
                        }
                    }
                    crate::ParameterType::Blob => {
                        // For blob, range represents min/max size in bytes
                        let blob_size = value
                            .as_array()
                            .expect("value already validated as Blob type")
                            .len() as f64;

                        if let (Some(&min), Some(&max)) = (range.first(), range.get(1)) {
                            if blob_size < min || blob_size > max {
                                return Err(JankenError::new_parameter_type_mismatch(
                                    format!("blob size between {min} and {max} bytes{context}"),
                                    format!("{blob_size} bytes"),
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Check pattern for string types
        self.validate_pattern(value, context)?;

        // Check enum values
        self.validate_enum(value, context)?;

        // Check conditional enum constraints
        self.validate_enumif(value, param_name, all_params, context)?;

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
                let array = value
                    .as_array()
                    .expect("is_array() already verified at the beginning of this block");
                for (index, item) in array.iter().enumerate() {
                    let context = format!(" at index {index}");
                    // Validate basic type and constraints for each item
                    if Self::validate_basic_type(item, item_type).is_err() {
                        return Err(JankenError::new_parameter_type_mismatch(
                            format!("{item_type}{context}"),
                            item.to_string(),
                        ));
                    }
                    self.validate_constraint_rules(
                        item, item_type, param_name, all_params, &context,
                    )?;
                }
            }
            // For Lists, constraints (range, pattern, enum) apply to items if item_type is set,
            // but not to the list itself, so we don't call validate_constraints after this match
            return Ok(());
        }

        if param_type == &crate::ParameterType::CommaList {
            if !value.is_array() {
                return Err(Self::constraint_mismatch_error(param_type, value));
            }

            // Validate each item in the comma list - must be strings
            let array = value
                .as_array()
                .expect("is_array() already verified at the beginning of this block");
            for (index, item) in array.iter().enumerate() {
                let context = format!(" at index {index}");
                // Each item must be a string
                if !item.is_string() {
                    return Err(JankenError::new_parameter_type_mismatch(
                        format!("string{context}"),
                        item.to_string(),
                    ));
                }

                // Apply all constraints (pattern, enum, enumif) using the unified method
                // Note: We use ParameterType::String since CommaList items are always strings
                self.validate_constraint_rules(
                    item,
                    &crate::ParameterType::String,
                    param_name,
                    all_params,
                    &context,
                )?;

                // Apply table name validation (alphanumeric and underscores only)
                let string_val = item
                    .as_str()
                    .expect("is_string() already verified for this item");
                Self::validate_table_name_format(string_val, &context)?;
            }
            return Ok(());
        }

        self.validate_constraints(value, param_type, param_name, all_params)?;

        if param_type == &crate::ParameterType::TableName {
            // value cannot be a non-string here since basic type validation has been done
            let table_name_str = value
                .as_str()
                .expect("basic type validation already verified this is a string");
            Self::validate_table_name_format(table_name_str, "")?;
        }

        Ok(())
    }
}

/// Parse constraints from JSON into ParameterConstraints
pub fn parse_constraints(
    constraints: &mut ParameterConstraints,
    arg_def: &serde_json::Value,
) -> Result<()> {
    // Determine the type string for error messages
    let arg_type = match arg_def {
        serde_json::Value::Array(_) => "array",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "null",
        serde_json::Value::Object(_) => "object",
    };

    // Validate that arg_def is an object
    // If a parameter is explicitly defined in args, it must be an object with constraint fields
    if !arg_def.is_object() {
        return Err(JankenError::new_parameter_type_mismatch(
            "parameter definition to be an object with constraint fields",
            format!("{arg_def} (type: {arg_type})"),
        ));
    }

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
                        // Validate the condition key format: either "name" or "match_type:name"
                        // where match_type is "start", "end", or "contain"
                        // and name is alphanumeric with underscores
                        if let Some(colon_pos) = value_key.find(':') {
                            let (match_type, pattern) = value_key.split_at(colon_pos);
                            let pattern = &pattern[1..]; // Skip the colon

                            // Validate match type
                            if !matches!(match_type, "start" | "end" | "contain") {
                                return Err(JankenError::new_parameter_type_mismatch(
                                    "enumif condition key match type to be 'start', 'end', or 'contain'",
                                    format!("{match_type} in condition key {value_key}"),
                                ));
                            }

                            // Validate pattern name (alphanumeric with underscores) for security
                            if pattern.is_empty()
                                || !pattern.chars().all(|c| c.is_alphanumeric() || c == '_')
                            {
                                return Err(JankenError::new_parameter_type_mismatch(
                                    "enumif fuzzy match pattern to be alphanumeric with underscores",
                                    format!("{pattern} in condition key {value_key}"),
                                ));
                            }
                        }
                        // Exact match - no validation required, any string value is allowed

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
    fn test_validate_pattern_with_null_value() {
        // Test that validate_pattern handles null values gracefully
        // This is a defensive check for resilience - null values are typically
        // rejected by validate_basic_type before reaching validate_pattern,
        // but we handle it here as well for robustness
        let constraints = ParameterConstraints {
            pattern: Some("^[a-z]+$".to_string()),
            ..Default::default()
        };

        let null_value = serde_json::Value::Null;
        let result = constraints.validate_pattern(&null_value, "");

        // Null values should pass pattern validation (early return with Ok)
        assert!(result.is_ok());
    }

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
