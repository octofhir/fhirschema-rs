//! Element validation for FHIRSchema validation
//!
//! This module handles validation of individual elements including cardinality,
//! type checking, shape validation, and choice type validation.

use crate::{
    error::{ValidationError, ValidationResult},
    ValidationConfig, ValidationIssue, ValidationStats, Severity,
};
use fhirschema_core::{Element, ElementType};
use serde_json::Value;

/// Element validator for individual element validation
pub struct ElementValidator {
    /// Configuration for validation behavior
    config: ValidationConfig,
}

impl ElementValidator {
    /// Create a new element validator
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }

    /// Validate an element against its definition
    pub fn validate_element(
        &self,
        value: &Option<Value>,
        element: &Element,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
        stats: &mut ValidationStats,
    ) -> ValidationResult<()> {
        // Validate cardinality
        self.validate_cardinality(value, element, path, issues)?;

        // If value is present, validate its content
        if let Some(val) = value {
            // Validate type
            self.validate_type(val, element, path, issues)?;

            // Validate shape (array vs scalar)
            self.validate_shape(val, element, path, issues)?;

            // Validate choice types if applicable
            self.validate_choice_types(val, element, path, issues)?;

            // Validate required/excluded elements
            self.validate_required_excluded(val, element, path, issues)?;
        }

        stats.elements_validated += 1;
        Ok(())
    }

    /// Validate element cardinality (min/max constraints)
    fn validate_cardinality(
        &self,
        value: &Option<Value>,
        element: &Element,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        let actual_count = match value {
            None => 0,
            Some(Value::Array(arr)) => arr.len(),
            Some(_) => 1,
        };

        // Check minimum cardinality
        if let Some(min) = element.min {
            if actual_count < min as usize {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "cardinality-min".to_string(),
                    message: format!(
                        "Element '{}' has {} occurrence(s), but minimum is {}",
                        path, actual_count, min
                    ),
                    location: path.to_string(),
                    context: Some(format!("min: {}, actual: {}", min, actual_count)),
                });
            }
        }

        // Check maximum cardinality
        if let Some(max_str) = &element.max {
            if max_str != "*" {
                if let Ok(max) = max_str.parse::<usize>() {
                    if actual_count > max {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            code: "cardinality-max".to_string(),
                            message: format!(
                                "Element '{}' has {} occurrence(s), but maximum is {}",
                                path, actual_count, max
                            ),
                            location: path.to_string(),
                            context: Some(format!("max: {}, actual: {}", max, actual_count)),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate element type
    fn validate_type(
        &self,
        value: &Value,
        element: &Element,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        if let Some(element_type) = element.get_element_type() {
            match element_type {
                ElementType::Simple(type_name) => {
                    self.validate_simple_type(value, &type_name, path, issues)?;
                }
                ElementType::Choice(choices) => {
                    let choice_names: Vec<String> = choices.keys().cloned().collect();
                    self.validate_choice_type(value, &choice_names, path, issues)?;
                }
                ElementType::Complex(_) => {
                    // Complex types should be objects
                    if !value.is_object() {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            code: "type-mismatch".to_string(),
                            message: format!("Element '{}' should be an object for complex type", path),
                            location: path.to_string(),
                            context: None,
                        });
                    }
                }
                ElementType::Reference(_) => {
                    // Reference types should be objects with reference field
                    if !value.is_object() || !value.get("reference").is_some() {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            code: "type-mismatch".to_string(),
                            message: format!("Element '{}' should be a reference object", path),
                            location: path.to_string(),
                            context: None,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate simple type
    fn validate_simple_type(
        &self,
        value: &Value,
        type_name: &str,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        // Basic type validation based on FHIR types
        let is_valid = match type_name {
            "string" | "markdown" | "code" | "id" | "uri" | "url" | "canonical" | "oid" | "uuid" => {
                value.is_string()
            }
            "boolean" => value.is_boolean(),
            "integer" | "positiveInt" | "unsignedInt" => {
                value.is_number() && value.as_f64().map_or(false, |n| n.fract() == 0.0)
            }
            "decimal" => value.is_number(),
            "date" | "dateTime" | "instant" | "time" => {
                // Date/time validation - basic check for string format
                value.is_string()
            }
            "base64Binary" => {
                // Base64 validation - basic check for string
                value.is_string()
            }
            // Complex types - should be objects
            "HumanName" | "Address" | "ContactPoint" | "Identifier" | "CodeableConcept"
            | "Coding" | "Quantity" | "Range" | "Period" | "Ratio" | "SampledData"
            | "Attachment" | "Signature" | "Reference" => value.is_object(),
            // Resource types - should be objects with resourceType
            "Patient" | "Observation" | "Practitioner" | "Organization" | "Location"
            | "Encounter" | "Procedure" | "Medication" | "MedicationRequest" => {
                value.is_object()
                    && value
                        .get("resourceType")
                        .and_then(|rt| rt.as_str())
                        .map_or(false, |rt| rt == type_name)
            }
            // Default: accept any value for unknown types
            _ => true,
        };

        if !is_valid {
            let actual_type = self.get_value_type_name(value);
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "type-mismatch".to_string(),
                message: format!(
                    "Element '{}' has type '{}', but expected '{}'",
                    path, actual_type, type_name
                ),
                location: path.to_string(),
                context: Some(format!("expected: {}, actual: {}", type_name, actual_type)),
            });
        }

        Ok(())
    }

    /// Validate choice type (one of multiple allowed types)
    fn validate_choice_type(
        &self,
        value: &Value,
        choices: &[String],
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        let mut valid_for_any_choice = false;

        // Check if value is valid for any of the choice types
        for choice in choices {
            // Create a temporary issues vector to check if validation passes
            let mut temp_issues = Vec::new();
            if self
                .validate_simple_type(value, choice, path, &mut temp_issues)
                .is_ok()
                && temp_issues.is_empty()
            {
                valid_for_any_choice = true;
                break;
            }
        }

        if !valid_for_any_choice {
            let actual_type = self.get_value_type_name(value);
            let choices_str = choices.join(" | ");
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "choice-type-mismatch".to_string(),
                message: format!(
                    "Element '{}' has type '{}', but expected one of: {}",
                    path, actual_type, choices_str
                ),
                location: path.to_string(),
                context: Some(format!("choices: {}, actual: {}", choices_str, actual_type)),
            });
        }

        Ok(())
    }

    /// Validate element shape (array vs scalar)
    fn validate_shape(
        &self,
        value: &Value,
        element: &Element,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        // Determine expected shape based on max cardinality
        let is_array_expected = element
            .max
            .as_ref()
            .map_or(false, |max| max == "*" || max.parse::<i32>().unwrap_or(1) > 1);

        let is_array_actual = value.is_array();

        // Check shape consistency
        if is_array_expected && !is_array_actual {
            // Expected array but got scalar - this might be acceptable in some cases
            // Add as warning rather than error
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                code: "shape-mismatch".to_string(),
                message: format!(
                    "Element '{}' is expected to be an array but found scalar value",
                    path
                ),
                location: path.to_string(),
                context: Some("Expected array, found scalar".to_string()),
            });
        } else if !is_array_expected && is_array_actual {
            // Expected scalar but got array
            if let Some(arr) = value.as_array() {
                if arr.len() > 1 {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        code: "shape-mismatch".to_string(),
                        message: format!(
                            "Element '{}' is expected to be scalar but found array with {} elements",
                            path,
                            arr.len()
                        ),
                        location: path.to_string(),
                        context: Some(format!("Expected scalar, found array[{}]", arr.len())),
                    });
                }
            }
        }

        Ok(())
    }

    /// Validate choice types (polymorphic elements)
    fn validate_choice_types(
        &self,
        _value: &Value,
        _element: &Element,
        _path: &str,
        _issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        // Choice type validation is handled in validate_type
        // This method is for additional choice-specific validations
        Ok(())
    }

    /// Validate required and excluded elements
    fn validate_required_excluded(
        &self,
        value: &Value,
        element: &Element,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        // Check if element is required but missing
        if element.min.unwrap_or(0) > 0 && self.is_empty_value(value) {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "required-element-missing".to_string(),
                message: format!("Required element '{}' is missing or empty", path),
                location: path.to_string(),
                context: Some(format!("min: {}", element.min.unwrap_or(0))),
            });
        }

        // Check if element is excluded (max = 0)
        if element.max.as_deref() == Some("0") && !self.is_empty_value(value) {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "excluded-element-present".to_string(),
                message: format!("Excluded element '{}' is present", path),
                location: path.to_string(),
                context: Some("max: 0".to_string()),
            });
        }

        Ok(())
    }

    /// Check if a value is considered empty
    fn is_empty_value(&self, value: &Value) -> bool {
        match value {
            Value::Null => true,
            Value::String(s) => s.is_empty(),
            Value::Array(arr) => arr.is_empty(),
            Value::Object(obj) => obj.is_empty(),
            _ => false,
        }
    }

    /// Get a human-readable type name for a JSON value
    fn get_value_type_name(&self, value: &Value) -> &'static str {
        match value {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(n) => {
                if n.is_f64() && n.as_f64().unwrap().fract() != 0.0 {
                    "decimal"
                } else {
                    "integer"
                }
            }
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    /// Validate array elements individually
    pub fn validate_array_elements(
        &self,
        array: &[Value],
        element: &Element,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
        stats: &mut ValidationStats,
    ) -> ValidationResult<()> {
        for (index, item) in array.iter().enumerate() {
            let item_path = format!("{}[{}]", path, index);
            let item_value = Some(item.clone());
            self.validate_element(&item_value, element, &item_path, issues, stats)?;
        }

        Ok(())
    }

    /// Get validation configuration
    pub fn config(&self) -> &ValidationConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_element(min: Option<u32>, max: Option<String>, element_type: Option<String>) -> Element {
        Element {
            min,
            max,
            element_type,
            ..Default::default()
        }
    }

    #[test]
    fn test_element_validator_creation() {
        let config = ValidationConfig::default();
        let validator = ElementValidator::new(config);
        assert!(validator.config().enable_constraints);
    }

    #[test]
    fn test_validate_cardinality_min() {
        let config = ValidationConfig::default();
        let validator = ElementValidator::new(config);
        let element = create_test_element(Some(1), Some("1".to_string()), None);
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Test missing required element
        validator
            .validate_element(&None, &element, "Patient.name", &mut issues, &mut stats)
            .unwrap();

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "cardinality-min");
    }

    #[test]
    fn test_validate_cardinality_max() {
        let config = ValidationConfig::default();
        let validator = ElementValidator::new(config);
        let element = create_test_element(Some(0), Some("1".to_string()), None);
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Test too many elements
        let value = Some(json!(["value1", "value2"]));
        validator
            .validate_element(&value, &element, "Patient.name", &mut issues, &mut stats)
            .unwrap();

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "cardinality-max");
    }

    #[test]
    fn test_validate_string_type() {
        let config = ValidationConfig::default();
        let validator = ElementValidator::new(config);
        let element = create_test_element(
            Some(1),
            Some("1".to_string()),
            Some("string".to_string()),
        );
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Test valid string
        let value = Some(json!("test string"));
        validator
            .validate_element(&value, &element, "Patient.name", &mut issues, &mut stats)
            .unwrap();

        assert_eq!(issues.len(), 0);

        // Test invalid type
        issues.clear();
        let value = Some(json!(123));
        validator
            .validate_element(&value, &element, "Patient.name", &mut issues, &mut stats)
            .unwrap();

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "type-mismatch");
    }

    #[test]
    fn test_validate_boolean_type() {
        let config = ValidationConfig::default();
        let validator = ElementValidator::new(config);
        let element = create_test_element(
            Some(1),
            Some("1".to_string()),
            Some("boolean".to_string()),
        );
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Test valid boolean
        let value = Some(json!(true));
        validator
            .validate_element(&value, &element, "Patient.active", &mut issues, &mut stats)
            .unwrap();

        assert_eq!(issues.len(), 0);

        // Test invalid type
        issues.clear();
        let value = Some(json!("true"));
        validator
            .validate_element(&value, &element, "Patient.active", &mut issues, &mut stats)
            .unwrap();

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "type-mismatch");
    }

    #[test]
    fn test_validate_choice_type() {
        let config = ValidationConfig::default();
        let validator = ElementValidator::new(config);

        // Create element with choices field for choice type
        let mut choices = std::collections::HashMap::new();
        choices.insert("string".to_string(), Element::new());
        choices.insert("integer".to_string(), Element::new());

        let element = Element {
            min: Some(1),
            max: Some("1".to_string()),
            choices: Some(choices),
            ..Default::default()
        };
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Test valid choice (string)
        let value = Some(json!("test"));
        validator
            .validate_element(&value, &element, "Patient.value", &mut issues, &mut stats)
            .unwrap();

        assert_eq!(issues.len(), 0);

        // Test valid choice (integer)
        issues.clear();
        let value = Some(json!(42));
        validator
            .validate_element(&value, &element, "Patient.value", &mut issues, &mut stats)
            .unwrap();

        assert_eq!(issues.len(), 0);

        // Test invalid choice
        issues.clear();
        let value = Some(json!(true));
        validator
            .validate_element(&value, &element, "Patient.value", &mut issues, &mut stats)
            .unwrap();

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "choice-type-mismatch");
    }

    #[test]
    fn test_validate_array_shape() {
        let config = ValidationConfig::default();
        let validator = ElementValidator::new(config);
        let element = create_test_element(Some(0), Some("*".to_string()), None);
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Test array for multi-cardinality element (should be OK)
        let value = Some(json!(["value1", "value2"]));
        validator
            .validate_element(&value, &element, "Patient.name", &mut issues, &mut stats)
            .unwrap();

        // Should not have shape errors for this case
        assert!(!issues.iter().any(|i| i.code == "shape-mismatch"));
    }

    #[test]
    fn test_is_empty_value() {
        let config = ValidationConfig::default();
        let validator = ElementValidator::new(config);

        assert!(validator.is_empty_value(&json!(null)));
        assert!(validator.is_empty_value(&json!("")));
        assert!(validator.is_empty_value(&json!([])));
        assert!(validator.is_empty_value(&json!({})));
        assert!(!validator.is_empty_value(&json!("test")));
        assert!(!validator.is_empty_value(&json!(0)));
        assert!(!validator.is_empty_value(&json!(false)));
    }

    #[test]
    fn test_get_value_type_name() {
        let config = ValidationConfig::default();
        let validator = ElementValidator::new(config);

        assert_eq!(validator.get_value_type_name(&json!(null)), "null");
        assert_eq!(validator.get_value_type_name(&json!(true)), "boolean");
        assert_eq!(validator.get_value_type_name(&json!(42)), "integer");
        assert_eq!(validator.get_value_type_name(&json!(3.14)), "decimal");
        assert_eq!(validator.get_value_type_name(&json!("test")), "string");
        assert_eq!(validator.get_value_type_name(&json!([])), "array");
        assert_eq!(validator.get_value_type_name(&json!({})), "object");
    }
}
