use octofhir_fhirschema::{ChoiceTypeExpander, ConverterConfig, Element, ResolvedChoiceType};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_choice_type_value_resolution() {
    let config = ConverterConfig::default();
    let expander = ChoiceTypeExpander::new(&config);

    // Test string value resolution
    let string_value = json!("test");
    let resolved = expander.resolve_choice_from_value(&string_value, "value[x]");
    assert!(resolved.is_some());
    let resolved = resolved.unwrap();
    assert_eq!(resolved.actual_type, "string");
    assert_eq!(resolved.expanded_path, "valueString");

    // Test integer value resolution
    let int_value = json!(42);
    let resolved = expander.resolve_choice_from_value(&int_value, "value[x]");
    assert!(resolved.is_some());
    let resolved = resolved.unwrap();
    assert_eq!(resolved.actual_type, "integer");
    assert_eq!(resolved.expanded_path, "valueInteger");

    // Test boolean value resolution
    let bool_value = json!(true);
    let resolved = expander.resolve_choice_from_value(&bool_value, "value[x]");
    assert!(resolved.is_some());
    let resolved = resolved.unwrap();
    assert_eq!(resolved.actual_type, "boolean");
    assert_eq!(resolved.expanded_path, "valueBoolean");

    // Test complex type - Coding
    let coding_value = json!({
        "system": "http://example.com",
        "code": "test"
    });
    let resolved = expander.resolve_choice_from_value(&coding_value, "value[x]");
    assert!(resolved.is_some());
    let resolved = resolved.unwrap();
    assert_eq!(resolved.actual_type, "Coding");
    assert_eq!(resolved.expanded_path, "valueCoding");
}

#[test]
fn test_choice_type_expansions() {
    let config = ConverterConfig::default();
    let expander = ChoiceTypeExpander::new(&config);

    let result = expander.get_all_choice_expansions("value[x]");
    assert!(result.is_ok());
    let expansions = result.unwrap();

    // Should contain common types
    assert!(expansions.contains_key("valueString"));
    assert!(expansions.contains_key("valueInteger"));
    assert!(expansions.contains_key("valueBoolean"));
    assert!(expansions.contains_key("valueCoding"));
    assert!(expansions.contains_key("valueCodeableConcept"));

    // Test error case - not a choice type
    let result = expander.get_all_choice_expansions("regularPath");
    assert!(result.is_err());
}

#[test]
fn test_choice_pattern_recognition() {
    let config = ConverterConfig::default();
    let expander = ChoiceTypeExpander::new(&config);

    let mut elements = HashMap::new();

    // Add some choice type expansions
    elements.insert("valueString".to_string(), Element::new("valueString"));
    elements.insert("valueInteger".to_string(), Element::new("valueInteger"));
    elements.insert("valueBoolean".to_string(), Element::new("valueBoolean"));
    elements.insert("otherPath".to_string(), Element::new("otherPath"));

    let patterns = expander.identify_choice_patterns(&elements);

    // Should identify the value[x] pattern
    assert_eq!(patterns.len(), 1);
    let pattern = &patterns[0];
    assert_eq!(pattern.base_path, "value[x]");
    assert!(pattern.expansions.contains(&"valueString".to_string()));
    assert!(pattern.expansions.contains(&"valueInteger".to_string()));
    assert!(pattern.expansions.contains(&"valueBoolean".to_string()));
    assert!(!pattern.expansions.contains(&"otherPath".to_string()));

    // Check detected types
    assert!(pattern.detected_types.contains(&"string".to_string()));
    assert!(pattern.detected_types.contains(&"integer".to_string()));
    assert!(pattern.detected_types.contains(&"boolean".to_string()));
}

#[test]
fn test_choice_validation() {
    let config = ConverterConfig::default();
    let expander = ChoiceTypeExpander::new(&config);

    let mut elements = HashMap::new();

    // Add choice type elements with consistent cardinality
    let mut element1 = Element::new("valueString");
    element1.min = Some(1);
    element1.max = Some("1".to_string());

    let mut element2 = Element::new("valueInteger");
    element2.min = Some(1);
    element2.max = Some("1".to_string());

    elements.insert("valueString".to_string(), element1);
    elements.insert("valueInteger".to_string(), element2);

    let result = expander.validate_choice_type_consistency("value", &elements);
    assert!(result.is_ok());
    let validation = result.unwrap();
    assert!(validation.is_valid);
    assert!(validation.errors.is_empty());
}

#[test]
fn test_choice_validation_inconsistency() {
    let config = ConverterConfig::default();
    let expander = ChoiceTypeExpander::new(&config);

    let mut elements = HashMap::new();

    // Add choice type elements with inconsistent cardinality
    let mut element1 = Element::new("valueString");
    element1.min = Some(1);
    element1.max = Some("1".to_string());

    let mut element2 = Element::new("valueInteger");
    element2.min = Some(0); // Different min
    element2.max = Some("1".to_string());

    elements.insert("valueString".to_string(), element1);
    elements.insert("valueInteger".to_string(), element2);

    let result = expander.validate_choice_type_consistency("value", &elements);
    assert!(result.is_ok());
    let validation = result.unwrap();
    // Should still be valid (just warnings for cardinality mismatch)
    assert!(!validation.inconsistencies.is_empty());
}

#[test]
fn test_primitive_type_detection() {
    assert!(octofhir_fhirschema::ChoiceTypeExpander::is_primitive_type(
        "string"
    ));
    assert!(octofhir_fhirschema::ChoiceTypeExpander::is_primitive_type(
        "integer"
    ));
    assert!(octofhir_fhirschema::ChoiceTypeExpander::is_primitive_type(
        "boolean"
    ));
    assert!(octofhir_fhirschema::ChoiceTypeExpander::is_primitive_type(
        "decimal"
    ));

    assert!(!octofhir_fhirschema::ChoiceTypeExpander::is_primitive_type(
        "Coding"
    ));
    assert!(!octofhir_fhirschema::ChoiceTypeExpander::is_primitive_type(
        "CodeableConcept"
    ));
    assert!(!octofhir_fhirschema::ChoiceTypeExpander::is_primitive_type(
        "Reference"
    ));
}

#[test]
fn test_resolved_choice_type_creation() {
    let resolved = ResolvedChoiceType::new("value[x]", "string");

    assert_eq!(resolved.base_path, "value[x]");
    assert_eq!(resolved.expanded_path, "valueString");
    assert_eq!(resolved.actual_type, "string");
    assert!(resolved.is_primitive);

    let resolved = ResolvedChoiceType::new("value[x]", "Coding");
    assert_eq!(resolved.expanded_path, "valueCoding");
    assert!(!resolved.is_primitive);
}
