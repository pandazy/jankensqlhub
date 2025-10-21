use crate::{
    ParameterType,
    result::{JankenError, Result},
};
use regex::Regex;
use std::str::FromStr;

/// Parameter constraints for validation
#[derive(Debug, Clone, Default)]
pub struct ParameterConstraints {
    pub range: Option<Vec<f64>>, // For numeric types: [min, max]
    pub pattern: Option<String>, // For string types: regex pattern
    pub enum_values: Option<Vec<serde_json::Value>>, // For any type: allowed values
    pub item_type: Option<crate::ParameterType>, // For list types: the type of each item
}

impl ParameterConstraints {
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
                                return Err(JankenError::ParameterTypeMismatch {
                                    expected: format!("byte values (0-255) at index {i}"),
                                    got: format!("{num}"),
                                });
                            }
                        } else {
                            return Err(JankenError::ParameterTypeMismatch {
                                expected: format!("byte values (0-255) at index {i}"),
                                got: item.to_string(),
                            });
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
        JankenError::ParameterTypeMismatch {
            expected: param_type.to_string(),
            got: value.to_string(),
        }
    }

    /// Validate parameter constraints (range, pattern, enum) assuming basic type validation is already done
    fn validate_constraints(
        &self,
        value: &serde_json::Value,
        param_type: &crate::ParameterType,
    ) -> Result<()> {
        // First validate basic type
        Self::validate_basic_type(value, param_type)?;
        // Then validate the constraint rules
        self.validate_constraint_rules(value, param_type)
    }

    /// Validate constraint rules (range, pattern, enum) only, assuming basic type is already validated
    fn validate_constraint_rules(
        &self,
        value: &serde_json::Value,
        param_type: &crate::ParameterType,
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
            return Err(JankenError::ParameterTypeMismatch {
                expected: "numeric type or blob".to_string(),
                got: param_type.to_string(),
            });
        }

        // Check range for numeric types and blob size
        if let Some(range) = &self.range {
            match param_type {
                crate::ParameterType::Integer | crate::ParameterType::Float => {
                    // Validated upfront that param_type is Integer or Float, so value is number
                    let num_val = value.as_f64().unwrap();

                    if let (Some(&min), Some(&max)) = (range.first(), range.get(1)) {
                        if num_val < min || num_val > max {
                            return Err(JankenError::ParameterTypeMismatch {
                                expected: format!("value between {min} and {max}"),
                                got: num_val.to_string(),
                            });
                        }
                    }
                }
                crate::ParameterType::Blob => {
                    // For blob, range represents min/max size in bytes
                    let blob_size = value.as_array().unwrap().len() as f64;

                    if let (Some(&min), Some(&max)) = (range.first(), range.get(1)) {
                        if blob_size < min || blob_size > max {
                            return Err(JankenError::ParameterTypeMismatch {
                                expected: format!("blob size between {min} and {max} bytes"),
                                got: format!("{blob_size} bytes"),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        // Check pattern for string types
        if let Some(pattern) = &self.pattern {
            if let Some(string_val) = value.as_str() {
                let regex =
                    Regex::new(pattern).map_err(|_| JankenError::ParameterTypeMismatch {
                        expected: "valid regex pattern".to_string(),
                        got: pattern.clone(),
                    })?;
                if !regex.is_match(string_val) {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: format!("string matching pattern '{pattern}'"),
                        got: string_val.to_string(),
                    });
                }
            } else {
                return Err(JankenError::ParameterTypeMismatch {
                    expected: ParameterType::String.to_string(),
                    got: value.to_string(),
                });
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
                return Err(JankenError::ParameterTypeMismatch {
                    expected: format!("one of [{enum_str}]"),
                    got: value.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Validate a parameter value against these constraints
    pub fn validate(
        &self,
        value: &serde_json::Value,
        param_type: &crate::ParameterType,
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
                        return Err(JankenError::ParameterTypeMismatch {
                            expected: format!("{item_type} at index {index}"),
                            got: item.to_string(),
                        });
                    }
                    self.validate_constraint_rules(item, item_type)?;
                }
            }
            // For Lists, constraints (range, pattern, enum) apply to items if item_type is set,
            // but not to the list itself, so we don't call validate_constraints after this match
            return Ok(());
        }

        self.validate_constraints(value, param_type)?;

        if param_type == &crate::ParameterType::TableName {
            // value cannot be a non-string here since basic type validation has been done
            let table_name_str = value.as_str().unwrap();
            if table_name_str.is_empty()
                || !table_name_str
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_')
            {
                return Err(JankenError::ParameterTypeMismatch {
                    expected: "valid table name (alphanumeric and underscores only)".to_string(),
                    got: table_name_str.to_string(),
                });
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
                return Err(JankenError::ParameterTypeMismatch {
                    expected: expected_content.to_string(),
                    got: format!("array with {} elements", arr.len()),
                });
            }
            None => {
                return Err(JankenError::ParameterTypeMismatch {
                    expected: expected_content.to_string(),
                    got: format!("{range_val} (not an array)"),
                });
            }
        };

        let range: Vec<f64> = range_array
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.as_f64()
                    .ok_or_else(|| JankenError::ParameterTypeMismatch {
                        expected: "number".to_string(),
                        got: format!("{v} at index {i}"),
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

    if let Some(itemtype_val) = arg_def.get("itemtype") {
        if let Some(itemtype_str) = itemtype_val.as_str() {
            let item_type = ParameterType::from_str(itemtype_str)?;
            // Validate item type - TableName and List are not allowed as item types
            match item_type {
                ParameterType::TableName | ParameterType::List => {
                    return Err(JankenError::ParameterTypeMismatch {
                        expected: "item_type for list items cannot be TableName or List"
                            .to_string(),
                        got: item_type.to_string(),
                    });
                }
                _ => {}
            }
            constraints.item_type = Some(item_type);
        }
    }

    Ok(())
}
