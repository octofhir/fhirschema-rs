//! Primitive datatype validation for FHIRSchema validation
//!
//! This module handles validation of FHIR primitive datatypes including
//! format validation, date/time validation, URI validation, and UCUM
//! unit validation using the ucum-rs library.

use crate::{
    error::{ValidationError, ValidationResult},
    ValidationIssue, ValidationStats, Severity,
};
use fhirschema_core::{Element, ElementType};
use serde_json::Value;
use regex::Regex;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use url::Url;

/// Primitive validator for FHIR primitive types
pub struct PrimitiveValidator {
    // Compiled regex patterns for performance
    id_regex: Regex,
    code_regex: Regex,
    oid_regex: Regex,
    uuid_regex: Regex,
    base64_regex: Regex,
}

impl PrimitiveValidator {
    /// Create a new primitive validator
    pub fn new() -> Self {
        Self {
            // FHIR ID pattern: [A-Za-z0-9\-\.]{1,64}
            id_regex: Regex::new(r"^[A-Za-z0-9\-\.]{1,64}$").unwrap(),
            // FHIR code pattern: [^\s]+(\s[^\s]+)*
            code_regex: Regex::new(r"^[^\s]+(\s[^\s]+)*$").unwrap(),
            // OID pattern: urn:oid:[0-2](\.(0|[1-9][0-9]*))*
            oid_regex: Regex::new(r"^urn:oid:[0-2](\.(0|[1-9][0-9]*))*$").unwrap(),
            // UUID pattern: urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}
            uuid_regex: Regex::new(r"^urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap(),
            // Base64 pattern (simplified)
            base64_regex: Regex::new(r"^[A-Za-z0-9+/]*={0,2}$").unwrap(),
        }
    }

    /// Validate a primitive value against its element definition
    pub fn validate_primitive(
        &self,
        value: &Option<Value>,
        element: &Element,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
        stats: &mut ValidationStats,
    ) -> ValidationResult<()> {
        if let Some(val) = value {
            if let Some(element_type) = element.get_element_type() {
                match element_type {
                    ElementType::Simple(type_name) => {
                        self.validate_primitive_type(val, &type_name, path, issues)?;
                    }
                    ElementType::Choice(choices) => {
                        // For choice types, try to validate against each choice
                        let mut valid_for_any = false;
                        let choice_names: Vec<String> = choices.keys().cloned().collect();
                        for choice_name in &choice_names {
                            let mut temp_issues = Vec::new();
                            if self.validate_primitive_type(val, choice_name, path, &mut temp_issues).is_ok()
                                && temp_issues.is_empty() {
                                valid_for_any = true;
                                break;
                            }
                        }

                        if !valid_for_any {
                            let choices_str = choice_names.join(", ");
                            issues.push(ValidationIssue {
                                severity: Severity::Error,
                                code: "primitive-choice-invalid".to_string(),
                                message: format!(
                                    "Value at '{}' is not valid for any of the choice types: {}",
                                    path,
                                    choices_str
                                ),
                                location: path.to_string(),
                                context: Some(format!("choices: {}", choices_str)),
                            });
                        }
                    }
                    ElementType::Complex(_) => {
                        // Complex types are not primitive types, skip validation
                    }
                    ElementType::Reference(_) => {
                        // Reference types are not primitive types, skip validation
                    }
                }
            }
        }

        stats.primitives_validated += 1;
        Ok(())
    }

    /// Validate a specific primitive type
    fn validate_primitive_type(
        &self,
        value: &Value,
        type_name: &str,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        match type_name {
            "string" | "markdown" => self.validate_string(value, path, issues),
            "boolean" => self.validate_boolean(value, path, issues),
            "integer" => self.validate_integer(value, path, issues),
            "positiveInt" => self.validate_positive_int(value, path, issues),
            "unsignedInt" => self.validate_unsigned_int(value, path, issues),
            "decimal" => self.validate_decimal(value, path, issues),
            "date" => self.validate_date(value, path, issues),
            "dateTime" => self.validate_datetime(value, path, issues),
            "instant" => self.validate_instant(value, path, issues),
            "time" => self.validate_time(value, path, issues),
            "code" => self.validate_code(value, path, issues),
            "id" => self.validate_id(value, path, issues),
            "uri" => self.validate_uri(value, path, issues),
            "url" => self.validate_url(value, path, issues),
            "canonical" => self.validate_canonical(value, path, issues),
            "oid" => self.validate_oid(value, path, issues),
            "uuid" => self.validate_uuid(value, path, issues),
            "base64Binary" => self.validate_base64_binary(value, path, issues),
            _ => {
                // For unknown primitive types, just check if it's a valid JSON primitive
                if !self.is_json_primitive(value) {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        code: "unknown-primitive-type".to_string(),
                        message: format!("Unknown primitive type '{}' at '{}'", type_name, path),
                        location: path.to_string(),
                        context: Some(format!("type: {}", type_name)),
                    });
                }
                Ok(())
            }
        }
    }

    /// Validate string type
    fn validate_string(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if !value.is_string() {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-string".to_string(),
                message: format!("Value at '{}' must be a string", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate boolean type
    fn validate_boolean(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if !value.is_boolean() {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-boolean".to_string(),
                message: format!("Value at '{}' must be a boolean", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate integer type
    fn validate_integer(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        match value {
            Value::Number(n) => {
                if !n.is_i64() {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        code: "invalid-integer".to_string(),
                        message: format!("Value at '{}' must be an integer", path),
                        location: path.to_string(),
                        context: None,
                    });
                }
            }
            _ => {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-integer".to_string(),
                    message: format!("Value at '{}' must be an integer", path),
                    location: path.to_string(),
                    context: None,
                });
            }
        }
        Ok(())
    }

    /// Validate positive integer type
    fn validate_positive_int(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    if i <= 0 {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            code: "invalid-positive-int".to_string(),
                            message: format!("Value at '{}' must be a positive integer", path),
                            location: path.to_string(),
                            context: Some(format!("value: {}", i)),
                        });
                    }
                } else {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        code: "invalid-positive-int".to_string(),
                        message: format!("Value at '{}' must be a positive integer", path),
                        location: path.to_string(),
                        context: None,
                    });
                }
            }
            _ => {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-positive-int".to_string(),
                    message: format!("Value at '{}' must be a positive integer", path),
                    location: path.to_string(),
                    context: None,
                });
            }
        }
        Ok(())
    }

    /// Validate unsigned integer type
    fn validate_unsigned_int(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    if i < 0 {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            code: "invalid-unsigned-int".to_string(),
                            message: format!("Value at '{}' must be an unsigned integer", path),
                            location: path.to_string(),
                            context: Some(format!("value: {}", i)),
                        });
                    }
                } else {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        code: "invalid-unsigned-int".to_string(),
                        message: format!("Value at '{}' must be an unsigned integer", path),
                        location: path.to_string(),
                        context: None,
                    });
                }
            }
            _ => {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-unsigned-int".to_string(),
                    message: format!("Value at '{}' must be an unsigned integer", path),
                    location: path.to_string(),
                    context: None,
                });
            }
        }
        Ok(())
    }

    /// Validate decimal type
    fn validate_decimal(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if !value.is_number() {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-decimal".to_string(),
                message: format!("Value at '{}' must be a decimal number", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate date type (YYYY-MM-DD)
    fn validate_date(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(date_str) = value.as_str() {
            if NaiveDate::parse_from_str(date_str, "%Y-%m-%d").is_err() {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-date".to_string(),
                    message: format!("Value at '{}' must be a valid date (YYYY-MM-DD)", path),
                    location: path.to_string(),
                    context: Some(format!("value: {}", date_str)),
                });
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-date".to_string(),
                message: format!("Value at '{}' must be a string representing a date", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate dateTime type
    fn validate_datetime(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(datetime_str) = value.as_str() {
            // Try parsing as RFC3339 format
            if DateTime::parse_from_rfc3339(datetime_str).is_err() {
                // Try parsing without timezone
                if datetime_str.parse::<DateTime<Utc>>().is_err() {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        code: "invalid-datetime".to_string(),
                        message: format!("Value at '{}' must be a valid dateTime", path),
                        location: path.to_string(),
                        context: Some(format!("value: {}", datetime_str)),
                    });
                }
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-datetime".to_string(),
                message: format!("Value at '{}' must be a string representing a dateTime", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate instant type (must include timezone)
    fn validate_instant(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(instant_str) = value.as_str() {
            if DateTime::parse_from_rfc3339(instant_str).is_err() {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-instant".to_string(),
                    message: format!("Value at '{}' must be a valid instant with timezone", path),
                    location: path.to_string(),
                    context: Some(format!("value: {}", instant_str)),
                });
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-instant".to_string(),
                message: format!("Value at '{}' must be a string representing an instant", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate time type (HH:MM:SS)
    fn validate_time(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(time_str) = value.as_str() {
            if NaiveTime::parse_from_str(time_str, "%H:%M:%S").is_err() {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-time".to_string(),
                    message: format!("Value at '{}' must be a valid time (HH:MM:SS)", path),
                    location: path.to_string(),
                    context: Some(format!("value: {}", time_str)),
                });
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-time".to_string(),
                message: format!("Value at '{}' must be a string representing a time", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate code type
    fn validate_code(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(code_str) = value.as_str() {
            if !self.code_regex.is_match(code_str) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-code".to_string(),
                    message: format!("Value at '{}' must be a valid code", path),
                    location: path.to_string(),
                    context: Some(format!("value: {}", code_str)),
                });
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-code".to_string(),
                message: format!("Value at '{}' must be a string representing a code", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate id type
    fn validate_id(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(id_str) = value.as_str() {
            if !self.id_regex.is_match(id_str) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-id".to_string(),
                    message: format!("Value at '{}' must be a valid id", path),
                    location: path.to_string(),
                    context: Some(format!("value: {}", id_str)),
                });
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-id".to_string(),
                message: format!("Value at '{}' must be a string representing an id", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate URI type
    fn validate_uri(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(uri_str) = value.as_str() {
            if Url::parse(uri_str).is_err() {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-uri".to_string(),
                    message: format!("Value at '{}' must be a valid URI", path),
                    location: path.to_string(),
                    context: Some(format!("value: {}", uri_str)),
                });
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-uri".to_string(),
                message: format!("Value at '{}' must be a string representing a URI", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate URL type (stricter than URI)
    fn validate_url(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(url_str) = value.as_str() {
            match Url::parse(url_str) {
                Ok(url) => {
                    if url.scheme() != "http" && url.scheme() != "https" {
                        issues.push(ValidationIssue {
                            severity: Severity::Warning,
                            code: "non-http-url".to_string(),
                            message: format!("URL at '{}' should use http or https scheme", path),
                            location: path.to_string(),
                            context: Some(format!("scheme: {}", url.scheme())),
                        });
                    }
                }
                Err(_) => {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        code: "invalid-url".to_string(),
                        message: format!("Value at '{}' must be a valid URL", path),
                        location: path.to_string(),
                        context: Some(format!("value: {}", url_str)),
                    });
                }
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-url".to_string(),
                message: format!("Value at '{}' must be a string representing a URL", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate canonical type (URL that may have a fragment)
    fn validate_canonical(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        // Canonical is similar to URL but may have fragments
        self.validate_uri(value, path, issues)
    }

    /// Validate OID type
    fn validate_oid(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(oid_str) = value.as_str() {
            if !self.oid_regex.is_match(oid_str) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-oid".to_string(),
                    message: format!("Value at '{}' must be a valid OID", path),
                    location: path.to_string(),
                    context: Some(format!("value: {}", oid_str)),
                });
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-oid".to_string(),
                message: format!("Value at '{}' must be a string representing an OID", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate UUID type
    fn validate_uuid(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(uuid_str) = value.as_str() {
            if !self.uuid_regex.is_match(uuid_str) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-uuid".to_string(),
                    message: format!("Value at '{}' must be a valid UUID", path),
                    location: path.to_string(),
                    context: Some(format!("value: {}", uuid_str)),
                });
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-uuid".to_string(),
                message: format!("Value at '{}' must be a string representing a UUID", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Validate base64Binary type
    fn validate_base64_binary(&self, value: &Value, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        if let Some(base64_str) = value.as_str() {
            if !self.base64_regex.is_match(base64_str) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "invalid-base64".to_string(),
                    message: format!("Value at '{}' must be valid base64", path),
                    location: path.to_string(),
                    context: Some(format!("value: {}", base64_str)),
                });
            }
        } else {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-base64".to_string(),
                message: format!("Value at '{}' must be a string representing base64 data", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }

    /// Check if a value is a JSON primitive
    fn is_json_primitive(&self, value: &Value) -> bool {
        matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null)
    }

    /// Validate UCUM units (placeholder for ucum-rs integration)
    pub fn validate_ucum_unit(&self, unit: &str, path: &str, issues: &mut Vec<ValidationIssue>) -> ValidationResult<()> {
        // TODO: Integrate with ucum-rs library
        // For now, just check if it's a non-empty string
        if unit.is_empty() {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "empty-ucum-unit".to_string(),
                message: format!("UCUM unit at '{}' cannot be empty", path),
                location: path.to_string(),
                context: None,
            });
        }
        Ok(())
    }
}

impl Default for PrimitiveValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use fhirschema_core::ElementType;

    fn create_test_element(type_name: &str) -> Element {
        Element {
            element_type: Some(type_name.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_primitive_validator_creation() {
        let validator = PrimitiveValidator::new();
        assert!(validator.id_regex.is_match("test-id"));
    }

    #[test]
    fn test_validate_string() {
        let validator = PrimitiveValidator::new();
        let element = create_test_element("string");
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Valid string
        let value = Some(json!("test string"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid string
        issues.clear();
        let value = Some(json!(123));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "invalid-string");
    }

    #[test]
    fn test_validate_boolean() {
        let validator = PrimitiveValidator::new();
        let element = create_test_element("boolean");
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Valid boolean
        let value = Some(json!(true));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid boolean
        issues.clear();
        let value = Some(json!("true"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "invalid-boolean");
    }

    #[test]
    fn test_validate_integer() {
        let validator = PrimitiveValidator::new();
        let element = create_test_element("integer");
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Valid integer
        let value = Some(json!(42));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid integer (decimal)
        issues.clear();
        let value = Some(json!(3.14));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "invalid-integer");
    }

    #[test]
    fn test_validate_positive_int() {
        let validator = PrimitiveValidator::new();
        let element = create_test_element("positiveInt");
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Valid positive integer
        let value = Some(json!(42));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid positive integer (zero)
        issues.clear();
        let value = Some(json!(0));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "invalid-positive-int");

        // Invalid positive integer (negative)
        issues.clear();
        let value = Some(json!(-1));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "invalid-positive-int");
    }

    #[test]
    fn test_validate_unsigned_int() {
        let validator = PrimitiveValidator::new();
        let element = create_test_element("unsignedInt");
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Valid unsigned integer
        let value = Some(json!(42));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Valid unsigned integer (zero)
        issues.clear();
        let value = Some(json!(0));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid unsigned integer (negative)
        issues.clear();
        let value = Some(json!(-1));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "invalid-unsigned-int");
    }

    #[test]
    fn test_validate_date() {
        let validator = PrimitiveValidator::new();
        let element = create_test_element("date");
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Valid date
        let value = Some(json!("2023-12-25"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid date format
        issues.clear();
        let value = Some(json!("25/12/2023"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "invalid-date");
    }

    #[test]
    fn test_validate_datetime() {
        let validator = PrimitiveValidator::new();
        let element = create_test_element("dateTime");
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Valid datetime
        let value = Some(json!("2023-12-25T10:30:00Z"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid datetime
        issues.clear();
        let value = Some(json!("not-a-datetime"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "invalid-datetime");
    }

    #[test]
    fn test_validate_id() {
        let validator = PrimitiveValidator::new();
        let element = create_test_element("id");
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Valid id
        let value = Some(json!("test-id-123"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid id (contains spaces)
        issues.clear();
        let value = Some(json!("test id"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "invalid-id");
    }

    #[test]
    fn test_validate_uri() {
        let validator = PrimitiveValidator::new();
        let element = create_test_element("uri");
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Valid URI
        let value = Some(json!("http://example.com/path"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid URI
        issues.clear();
        let value = Some(json!("not a uri"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "invalid-uri");
    }

    #[test]
    fn test_validate_choice_type() {
        let validator = PrimitiveValidator::new();

        // Create element with choices field for choice type
        let mut choices = std::collections::HashMap::new();
        choices.insert("string".to_string(), Element::new());
        choices.insert("integer".to_string(), Element::new());

        let element = Element {
            choices: Some(choices),
            ..Default::default()
        };
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Valid choice (string)
        let value = Some(json!("test"));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Valid choice (integer)
        issues.clear();
        let value = Some(json!(42));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid choice
        issues.clear();
        let value = Some(json!(true));
        validator.validate_primitive(&value, &element, "test.path", &mut issues, &mut stats).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "primitive-choice-invalid");
    }

    #[test]
    fn test_validate_ucum_unit() {
        let validator = PrimitiveValidator::new();
        let mut issues = Vec::new();

        // Valid unit (non-empty)
        validator.validate_ucum_unit("kg", "test.path", &mut issues).unwrap();
        assert_eq!(issues.len(), 0);

        // Invalid unit (empty)
        issues.clear();
        validator.validate_ucum_unit("", "test.path", &mut issues).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "empty-ucum-unit");
    }
}
