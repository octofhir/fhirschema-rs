mod common;

use common::*;
use octofhir_fhirschema::*;
use url::Url;

#[test]
fn test_element_creation() {
    let element = Element::new("Patient.id");
    assert_eq!(element.path, "Patient.id");
    assert!(element.element_type.is_none());
    assert!(element.constraints.is_empty());
}

#[test]
fn test_element_with_type() {
    let element_type = ElementType::new("string");
    let element = Element::new("Patient.id").with_type(element_type.clone());

    assert_eq!(element.element_type, Some(vec![element_type]));
}

#[test]
fn test_element_with_cardinality() {
    let element = Element::new("Patient.name").with_cardinality(1, "*");
    assert_eq!(element.min, Some(1));
    assert_eq!(element.max, Some("*".to_string()));
}

#[test]
fn test_element_validation_valid() {
    let element = create_test_element("Patient.id");
    assert!(element.validate().is_ok());
}

#[test]
fn test_element_validation_empty_path() {
    let element = Element::new("");
    assert!(element.validate().is_err());
}

#[test]
fn test_element_validation_invalid_cardinality() {
    let element = Element::new("Patient.id").with_cardinality(5, "3");
    assert!(element.validate().is_err());
}

#[test]
fn test_element_display() {
    let element = Element::new("Patient.id")
        .with_type(ElementType::new("string"))
        .with_cardinality(0, "1");

    let display = format!("{element}");
    assert!(display.contains("Patient.id"));
    assert!(display.contains("string"));
    assert!(display.contains("[0..1]"));
}

#[test]
fn test_element_type_creation() {
    let element_type = ElementType::new("string");
    assert_eq!(element_type.code, "string");
    assert!(element_type.profile.is_none());
}

#[test]
fn test_element_type_with_profile() {
    let profile_url = Url::parse("http://example.com/StructureDefinition/CustomString").unwrap();
    let element_type = ElementType::new("string").with_profile(profile_url.clone());

    assert_eq!(element_type.profile, Some(vec![profile_url]));
}

#[test]
fn test_binding_creation() {
    let binding = Binding::new("required");
    assert_eq!(binding.strength, "required");
    assert!(binding.value_set.is_none());
}

#[test]
fn test_binding_with_value_set() {
    let value_set_url = Url::parse("http://example.com/ValueSet/test").unwrap();
    let binding = Binding::new("required").with_value_set(value_set_url.clone());

    assert_eq!(binding.value_set, Some(value_set_url));
}
