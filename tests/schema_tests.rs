mod common;

use common::*;
use octofhir_fhirschema::*;
use url::Url;

#[test]
fn test_schema_creation() {
    let schema = FhirSchema::new("Patient");
    assert_eq!(schema.schema_type, "Patient");
    assert!(schema.elements.is_empty());
    assert!(schema.constraints.is_empty());
    assert!(schema.slicing.is_empty());
}

#[test]
fn test_schema_with_url() {
    let url = Url::parse("http://example.com/Patient").unwrap();
    let schema = FhirSchema::new("Patient").with_url(url.clone());
    assert_eq!(schema.url, Some(url));
}

#[test]
fn test_schema_with_element() {
    let element = create_test_element("Patient.id");
    let schema = FhirSchema::new("Patient").with_element("Patient.id", element.clone());

    assert_eq!(schema.elements.len(), 1);
    assert_eq!(schema.elements.get("Patient.id"), Some(&element));
}

#[test]
fn test_schema_validation_valid() {
    let schema = create_test_schema();
    assert!(schema.validate_structure().is_ok());
}

#[test]
fn test_schema_validation_empty_type() {
    let schema = FhirSchema::new("");
    assert!(schema.validate_structure().is_err());
}

#[test]
fn test_schema_display() {
    let schema = FhirSchema::new("Patient")
        .with_name("TestPatient")
        .with_url(Url::parse("http://example.com/Patient").unwrap());

    let display = format!("{schema}");
    assert!(display.contains("Patient"));
    assert!(display.contains("TestPatient"));
    assert!(display.contains("http://example.com/Patient"));
}

#[test]
fn test_schema_serde() {
    let schema = create_test_schema();

    let json = serde_json::to_string(&schema).unwrap();
    let deserialized: FhirSchema = serde_json::from_str(&json).unwrap();

    assert_eq!(schema, deserialized);
}
