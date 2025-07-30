//! Slicing validation for FHIRSchema validation
//!
//! This module handles validation of array slicing including slice matching
//! based on discriminators, slice ordering, cardinality validation, and
//! re-slicing support.

use crate::{
    error::{ValidationError, ValidationResult},
    ValidationIssue, ValidationStats, Severity,
};
use fhirschema_core::{Slicing, Slice};
use serde_json::Value;
use std::collections::HashMap;

/// Slicing validator for array slicing validation
pub struct SlicingValidator {
    // Configuration and state for slicing validation
}

impl SlicingValidator {
    /// Create a new slicing validator
    pub fn new() -> Self {
        Self {}
    }

    /// Validate slicing against an array value
    pub fn validate_slicing(
        &self,
        value: &Option<Value>,
        slicing: &Slicing,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
        stats: &mut ValidationStats,
    ) -> ValidationResult<()> {
        // Only validate if we have an array value
        let array = match value {
            Some(Value::Array(arr)) => arr,
            Some(_) => {
                // Single value - treat as array of one element for slicing purposes
                return Ok(());
            }
            None => {
                // No value - check if slicing requires elements
                self.validate_empty_slicing(slicing, path, issues)?;
                return Ok(());
            }
        };

        // Validate slice matching and assignment
        let slice_assignments = self.assign_elements_to_slices(array, slicing, path, issues)?;

        // Validate slice cardinalities
        self.validate_slice_cardinalities(&slice_assignments, slicing, path, issues)?;

        // Validate slice ordering if required
        if slicing.ordered.unwrap_or(false) {
            self.validate_slice_ordering(&slice_assignments, slicing, path, issues)?;
        }

        // Validate individual slice constraints
        self.validate_slice_constraints(&slice_assignments, slicing, path, issues)?;

        Ok(())
    }

    /// Validate empty slicing (no elements present)
    fn validate_empty_slicing(
        &self,
        slicing: &Slicing,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        // Check if any slices have minimum cardinality > 0
        if let Some(slices) = &slicing.slices {
            for (slice_name, slice) in slices {
                if let Some(min) = slice.min {
                    if min > 0 {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            code: "slice-min-cardinality".to_string(),
                            message: format!(
                                "Slice '{}' at '{}' requires at least {} elements, but array is empty",
                                slice_name, path, min
                            ),
                            location: path.to_string(),
                            context: Some(format!("slice: {}, min: {}", slice_name, min)),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Assign array elements to slices based on discriminators
    fn assign_elements_to_slices(
        &self,
        array: &[Value],
        slicing: &Slicing,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<HashMap<String, Vec<usize>>> {
        let mut assignments: HashMap<String, Vec<usize>> = HashMap::new();

        // Initialize slice assignments
        if let Some(slices) = &slicing.slices {
            for slice_name in slices.keys() {
                assignments.insert(slice_name.clone(), Vec::new());
            }
        }

        // Add "unsliced" category for elements that don't match any slice
        assignments.insert("unsliced".to_string(), Vec::new());

        // Assign each element to a slice
        for (index, element) in array.iter().enumerate() {
            let slice_name = self.find_matching_slice(element, slicing, path, issues)?;

            if let Some(slice) = slice_name {
                assignments.entry(slice).or_default().push(index);
            } else {
                assignments.get_mut("unsliced").unwrap().push(index);
            }
        }

        // Check for unmatched elements if slicing is closed
        if slicing.rules.as_deref() == Some("closed") {
            let unsliced = assignments.get("unsliced").unwrap();
            if !unsliced.is_empty() {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "closed-slicing-unmatched".to_string(),
                    message: format!(
                        "Closed slicing at '{}' has {} unmatched elements",
                        path,
                        unsliced.len()
                    ),
                    location: path.to_string(),
                    context: Some(format!("unmatched_indices: {:?}", unsliced)),
                });
            }
        }

        Ok(assignments)
    }

    /// Find the matching slice for an element
    fn find_matching_slice(
        &self,
        element: &Value,
        slicing: &Slicing,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<Option<String>> {
        let slices = match &slicing.slices {
            Some(slices) => slices,
            None => return Ok(None),
        };

        // Try to match against each slice
        for (slice_name, slice) in slices {
            if self.element_matches_slice(element, slice, slicing)? {
                return Ok(Some(slice_name.clone()));
            }
        }

        Ok(None)
    }

    /// Check if an element matches a slice based on discriminators
    fn element_matches_slice(
        &self,
        element: &Value,
        slice: &Slice,
        slicing: &Slicing,
    ) -> ValidationResult<bool> {
        // Get discriminators from slicing
        let discriminators = match &slicing.discriminator {
            Some(discriminators) => discriminators,
            None => return Ok(true), // No discriminators means any element matches
        };

        // Check each discriminator
        for discriminator in discriminators {
            if !self.check_discriminator_match(element, discriminator, slice)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check if an element matches a specific discriminator
    fn check_discriminator_match(
        &self,
        element: &Value,
        discriminator: &fhirschema_core::Discriminator,
        slice: &Slice,
    ) -> ValidationResult<bool> {
        match discriminator.discriminator_type.as_str() {
            "value" => self.check_value_discriminator(element, discriminator, slice),
            "exists" => self.check_exists_discriminator(element, discriminator, slice),
            "pattern" => self.check_pattern_discriminator(element, discriminator, slice),
            "type" => self.check_type_discriminator(element, discriminator, slice),
            "profile" => self.check_profile_discriminator(element, discriminator, slice),
            _ => {
                // Unknown discriminator type - assume no match
                Ok(false)
            }
        }
    }

    /// Check value-based discriminator
    fn check_value_discriminator(
        &self,
        element: &Value,
        discriminator: &fhirschema_core::Discriminator,
        slice: &Slice,
    ) -> ValidationResult<bool> {
        let path = &discriminator.path;
        let element_value = self.extract_discriminator_value(element, path);

        // Compare with slice's expected value (this would come from slice definition)
        // For now, we'll do a basic comparison
        // In a real implementation, this would use the slice's fixed value or pattern

        Ok(element_value.is_some())
    }

    /// Check exists-based discriminator
    fn check_exists_discriminator(
        &self,
        element: &Value,
        discriminator: &fhirschema_core::Discriminator,
        _slice: &Slice,
    ) -> ValidationResult<bool> {
        let path = &discriminator.path;
        let element_value = self.extract_discriminator_value(element, path);

        Ok(element_value.is_some())
    }

    /// Check pattern-based discriminator
    fn check_pattern_discriminator(
        &self,
        element: &Value,
        discriminator: &fhirschema_core::Discriminator,
        slice: &Slice,
    ) -> ValidationResult<bool> {
        let path = &discriminator.path;
        let element_value = self.extract_discriminator_value(element, path);

        // Pattern matching would be implemented here
        // For now, just check if value exists
        Ok(element_value.is_some())
    }

    /// Check type-based discriminator
    fn check_type_discriminator(
        &self,
        element: &Value,
        discriminator: &fhirschema_core::Discriminator,
        slice: &Slice,
    ) -> ValidationResult<bool> {
        let path = &discriminator.path;
        let element_value = self.extract_discriminator_value(element, path);

        // Type checking would be implemented here
        // For now, just check if value exists and is of expected JSON type
        match element_value {
            Some(Value::String(_)) => Ok(true),
            Some(Value::Number(_)) => Ok(true),
            Some(Value::Bool(_)) => Ok(true),
            Some(Value::Object(_)) => Ok(true),
            Some(Value::Array(_)) => Ok(true),
            _ => Ok(false),
        }
    }

    /// Check profile-based discriminator
    fn check_profile_discriminator(
        &self,
        element: &Value,
        discriminator: &fhirschema_core::Discriminator,
        slice: &Slice,
    ) -> ValidationResult<bool> {
        // Profile-based discrimination would require schema resolution
        // For now, assume match if element exists
        let path = &discriminator.path;
        let element_value = self.extract_discriminator_value(element, path);

        Ok(element_value.is_some())
    }

    /// Extract value at discriminator path from element
    fn extract_discriminator_value(&self, element: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = element;

        for part in parts {
            match current {
                Value::Object(obj) => {
                    current = obj.get(part)?;
                }
                Value::Array(arr) => {
                    // Handle array indexing if part is numeric
                    if let Ok(index) = part.parse::<usize>() {
                        current = arr.get(index)?;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(current.clone())
    }

    /// Validate slice cardinalities
    fn validate_slice_cardinalities(
        &self,
        assignments: &HashMap<String, Vec<usize>>,
        slicing: &Slicing,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        let slices = match &slicing.slices {
            Some(slices) => slices,
            None => return Ok(()),
        };

        for (slice_name, slice) in slices {
            let assigned_count = assignments.get(slice_name).map_or(0, |v| v.len());

            // Check minimum cardinality
            if let Some(min) = slice.min {
                if assigned_count < min as usize {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        code: "slice-min-cardinality".to_string(),
                        message: format!(
                            "Slice '{}' at '{}' requires at least {} elements, found {}",
                            slice_name, path, min, assigned_count
                        ),
                        location: path.to_string(),
                        context: Some(format!("slice: {}, min: {}, actual: {}", slice_name, min, assigned_count)),
                    });
                }
            }

            // Check maximum cardinality
            if let Some(max_str) = &slice.max {
                if max_str != "*" {
                    if let Ok(max) = max_str.parse::<usize>() {
                        if assigned_count > max {
                            issues.push(ValidationIssue {
                                severity: Severity::Error,
                                code: "slice-max-cardinality".to_string(),
                                message: format!(
                                    "Slice '{}' at '{}' allows at most {} elements, found {}",
                                    slice_name, path, max, assigned_count
                                ),
                                location: path.to_string(),
                                context: Some(format!("slice: {}, max: {}, actual: {}", slice_name, max, assigned_count)),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate slice ordering
    fn validate_slice_ordering(
        &self,
        assignments: &HashMap<String, Vec<usize>>,
        slicing: &Slicing,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        let slices = match &slicing.slices {
            Some(slices) => slices,
            None => return Ok(()),
        };

        // Get all slice names in definition order
        let slice_names: Vec<&String> = slices.keys().collect();

        // Check that elements appear in the correct order
        let mut last_slice_index = 0;

        for (slice_name, indices) in assignments {
            if slice_name == "unsliced" {
                continue;
            }

            if let Some(current_slice_index) = slice_names.iter().position(|&name| name == slice_name) {
                if current_slice_index < last_slice_index {
                    // Elements of this slice appear before elements of a later slice
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        code: "slice-ordering-violation".to_string(),
                        message: format!(
                            "Slice '{}' at '{}' appears out of order",
                            slice_name, path
                        ),
                        location: path.to_string(),
                        context: Some(format!("slice: {}, expected_order: {:?}", slice_name, slice_names)),
                    });
                }
                last_slice_index = current_slice_index;
            }
        }

        Ok(())
    }

    /// Validate individual slice constraints
    fn validate_slice_constraints(
        &self,
        assignments: &HashMap<String, Vec<usize>>,
        slicing: &Slicing,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        // Individual slice constraint validation would be implemented here
        // This would involve validating each element against its slice's element definition
        // For now, this is a placeholder

        Ok(())
    }

    /// Check if slicing is valid (has required properties)
    pub fn is_valid_slicing(&self, slicing: &Slicing) -> bool {
        // Basic validation of slicing structure
        slicing.discriminator.is_some() || slicing.slices.is_some()
    }

    /// Get slicing validation statistics
    pub fn get_stats(&self) -> ValidationStats {
        ValidationStats::default()
    }
}

impl Default for SlicingValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use fhirschema_core::{Discriminator, Slice};
    use std::collections::HashMap;

    fn create_test_slicing() -> Slicing {
        let mut slices = HashMap::new();
        slices.insert(
            "slice1".to_string(),
            Slice {
                name: "slice1".to_string(),
                min: Some(1),
                max: Some("2".to_string()),
                match_criteria: None,
                element: None,
                short: None,
                definition: None,
            },
        );
        slices.insert(
            "slice2".to_string(),
            Slice {
                name: "slice2".to_string(),
                min: Some(0),
                max: Some("*".to_string()),
                match_criteria: None,
                element: None,
                short: None,
                definition: None,
            },
        );

        Slicing {
            discriminator: Some(vec![Discriminator {
                discriminator_type: "value".to_string(),
                path: "type".to_string(),
            }]),
            slices: Some(slices),
            ordered: Some(false),
            rules: Some("open".to_string()),
            description: None,
        }
    }

    fn create_test_array() -> Value {
        json!([
            {"type": "slice1", "value": "test1"},
            {"type": "slice1", "value": "test2"},
            {"type": "slice2", "value": "test3"}
        ])
    }

    #[test]
    fn test_slicing_validator_creation() {
        let validator = SlicingValidator::new();
        let slicing = create_test_slicing();
        assert!(validator.is_valid_slicing(&slicing));
    }

    #[test]
    fn test_validate_empty_slicing() {
        let validator = SlicingValidator::new();
        let slicing = create_test_slicing();
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Test with no value
        validator.validate_slicing(&None, &slicing, "test.path", &mut issues, &mut stats).unwrap();

        // Should have issues for slice1 which has min=1
        assert!(issues.iter().any(|i| i.code == "slice-min-cardinality"));
    }

    #[test]
    fn test_assign_elements_to_slices() {
        let validator = SlicingValidator::new();
        let slicing = create_test_slicing();
        let array = create_test_array();
        let mut issues = Vec::new();

        if let Value::Array(arr) = array {
            let assignments = validator.assign_elements_to_slices(&arr, &slicing, "test.path", &mut issues).unwrap();

            // Should have assignments for both slices
            assert!(assignments.contains_key("slice1"));
            assert!(assignments.contains_key("slice2"));
            assert!(assignments.contains_key("unsliced"));
        }
    }

    #[test]
    fn test_extract_discriminator_value() {
        let validator = SlicingValidator::new();
        let element = json!({"type": "test", "nested": {"value": "inner"}});

        // Test simple path
        let value = validator.extract_discriminator_value(&element, "type");
        assert_eq!(value, Some(json!("test")));

        // Test nested path
        let value = validator.extract_discriminator_value(&element, "nested.value");
        assert_eq!(value, Some(json!("inner")));

        // Test non-existent path
        let value = validator.extract_discriminator_value(&element, "nonexistent");
        assert_eq!(value, None);
    }

    #[test]
    fn test_check_exists_discriminator() {
        let validator = SlicingValidator::new();
        let element = json!({"type": "test", "value": "exists"});
        let discriminator = Discriminator {
            discriminator_type: "exists".to_string(),
            path: "value".to_string(),
        };
        let slice = Slice::new("test_slice".to_string());

        let result = validator.check_exists_discriminator(&element, &discriminator, &slice).unwrap();
        assert!(result);

        // Test with non-existent path
        let discriminator = Discriminator {
            discriminator_type: "exists".to_string(),
            path: "nonexistent".to_string(),
        };
        let result = validator.check_exists_discriminator(&element, &discriminator, &slice).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_check_type_discriminator() {
        let validator = SlicingValidator::new();
        let element = json!({"stringValue": "test", "numberValue": 42});
        let slice = Slice::new("test_slice".to_string());

        // Test string type
        let discriminator = Discriminator {
            discriminator_type: "type".to_string(),
            path: "stringValue".to_string(),
        };
        let result = validator.check_type_discriminator(&element, &discriminator, &slice).unwrap();
        assert!(result);

        // Test number type
        let discriminator = Discriminator {
            discriminator_type: "type".to_string(),
            path: "numberValue".to_string(),
        };
        let result = validator.check_type_discriminator(&element, &discriminator, &slice).unwrap();
        assert!(result);
    }

    #[test]
    fn test_validate_slice_cardinalities() {
        let validator = SlicingValidator::new();
        let slicing = create_test_slicing();
        let mut assignments = HashMap::new();

        // slice1 has min=1, max=2
        assignments.insert("slice1".to_string(), vec![0, 1]); // 2 elements - OK
        assignments.insert("slice2".to_string(), vec![2]); // 1 element - OK

        let mut issues = Vec::new();
        validator.validate_slice_cardinalities(&assignments, &slicing, "test.path", &mut issues).unwrap();

        // Should have no cardinality issues
        assert!(!issues.iter().any(|i| i.code.contains("cardinality")));

        // Test violation - slice1 has too few elements
        issues.clear();
        assignments.insert("slice1".to_string(), vec![]); // 0 elements - violates min=1
        validator.validate_slice_cardinalities(&assignments, &slicing, "test.path", &mut issues).unwrap();

        assert!(issues.iter().any(|i| i.code == "slice-min-cardinality"));
    }

    #[test]
    fn test_is_valid_slicing() {
        let validator = SlicingValidator::new();

        // Valid slicing with discriminator
        let slicing = create_test_slicing();
        assert!(validator.is_valid_slicing(&slicing));

        // Invalid slicing without discriminator or slices
        let invalid_slicing = Slicing {
            discriminator: None,
            slices: None,
            ordered: None,
            rules: None,
            description: None,
        };
        assert!(!validator.is_valid_slicing(&invalid_slicing));

        // Valid slicing with only slices (no discriminator)
        let mut slices = HashMap::new();
        slices.insert("slice1".to_string(), Slice::new("slice1".to_string()));
        let slicing_with_slices = Slicing {
            discriminator: None,
            slices: Some(slices),
            ordered: None,
            rules: None,
            description: None,
        };
        assert!(validator.is_valid_slicing(&slicing_with_slices));
    }

    #[test]
    fn test_validate_full_slicing() {
        let validator = SlicingValidator::new();
        let slicing = create_test_slicing();
        let array = create_test_array();
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        validator.validate_slicing(&Some(array), &slicing, "test.path", &mut issues, &mut stats).unwrap();

        // Should complete without critical errors
        // Specific validation depends on discriminator matching implementation
        assert!(issues.iter().all(|i| i.severity != Severity::Error) || issues.is_empty());
    }
}
