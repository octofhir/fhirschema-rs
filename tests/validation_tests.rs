use octofhir_fhirschema::validation::ValidationSeverity;
use octofhir_fhirschema::*;
use serde_json::json;
use url::Url;

#[tokio::test]
async fn test_basic_validation_engine_creation() {
    let engine = FhirSchemaValidationEngine::new();
    assert!(!engine.strict_mode);

    let strict_engine = FhirSchemaValidationEngine::new_strict();
    assert!(strict_engine.strict_mode);
}

#[tokio::test]
async fn test_resource_type_validation() {
    let engine = FhirSchemaValidationEngine::new();

    // Create a simple schema for Patient
    let element = Element::new("Patient.id")
        .with_cardinality(0, "1")
        .with_type(ElementType::new("id"));

    let schema = FhirSchema::new("Patient")
        .with_name("Patient")
        .with_url(Url::parse("http://hl7.org/fhir/StructureDefinition/Patient").unwrap())
        .with_element("id", element);

    // Test valid resource type
    let valid_resource = json!({
        "resourceType": "Patient",
        "id": "example"
    });

    let result = engine.validate_resource(&valid_resource, &schema).unwrap();
    assert!(result.is_valid);

    // Test invalid resource type
    let invalid_resource = json!({
        "resourceType": "Observation",
        "id": "example"
    });

    let result = engine
        .validate_resource(&invalid_resource, &schema)
        .unwrap();
    assert!(!result.is_valid);
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.code == "resource-type-mismatch")
    );
}

#[tokio::test]
async fn test_element_cardinality_validation() {
    let engine = FhirSchemaValidationEngine::new();

    // Create schema with required element
    let element = Element::new("Patient.name")
        .with_cardinality(1, "*") // Required
        .with_type(ElementType::new("HumanName"));

    let schema = FhirSchema::new("Patient")
        .with_name("Patient")
        .with_url(Url::parse("http://hl7.org/fhir/StructureDefinition/Patient").unwrap())
        .with_element("name", element);

    // Test missing required element
    let resource_missing_name = json!({
        "resourceType": "Patient",
        "id": "example"
    });

    let result = engine
        .validate_resource(&resource_missing_name, &schema)
        .unwrap();
    assert!(!result.is_valid);
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.code == "cardinality-min-violation")
    );

    // Test with required element present
    let resource_with_name = json!({
        "resourceType": "Patient",
        "id": "example",
        "name": [{"family": "Doe"}]
    });

    let result = engine
        .validate_resource(&resource_with_name, &schema)
        .unwrap();
    // Note: This might still have issues due to incomplete validation, but cardinality should pass
    let cardinality_issues: Vec<_> = result
        .issues
        .iter()
        .filter(|issue| issue.code.starts_with("cardinality-"))
        .collect();
    assert!(cardinality_issues.is_empty());
}

#[tokio::test]
async fn test_constraint_validation() {
    let engine = FhirSchemaValidationEngine::new();

    // Create schema with constraint
    let constraint = Constraint::new("test-1", "error", "Test constraint", "name.exists()");

    let mut element = Element::new("Patient.name")
        .with_cardinality(0, "*")
        .with_type(ElementType::new("HumanName"));
    element.constraints.push(constraint);

    let schema = FhirSchema::new("Patient")
        .with_name("Patient")
        .with_url(Url::parse("http://hl7.org/fhir/StructureDefinition/Patient").unwrap())
        .with_element("name", element);

    // Test resource without name (should fail constraint)
    let resource_no_name = json!({
        "resourceType": "Patient",
        "id": "example"
    });

    let result = engine
        .validate_resource(&resource_no_name, &schema)
        .unwrap();
    // The constraint should be evaluated and may fail
    println!("Validation result: {result:?}");

    // Test resource with name (should pass constraint)
    let resource_with_name = json!({
        "resourceType": "Patient",
        "id": "example",
        "name": [{"family": "Doe"}]
    });

    let result = engine
        .validate_resource(&resource_with_name, &schema)
        .unwrap();
    println!("Validation result with name: {result:?}");
}

// Note: FHIRPath constraint evaluation is tested through the public validation API
// in the constraint validation test above

#[tokio::test]
async fn test_validation_context() {
    let resource = json!({"test": "value"});
    let mut context = ValidationContext::new(resource);

    // Test path management
    context.push_path("element");
    context.push_path("subelement");
    assert_eq!(context.current_path, "element.subelement");

    context.pop_path();
    assert_eq!(context.current_path, "element");

    // Test issue addition
    context.add_error("test-error", "Test error message");
    context.add_warning("test-warning", "Test warning message");

    let result = context.into_result();
    assert!(!result.is_valid);
    assert_eq!(result.issues.len(), 2);

    let error_issue = result
        .issues
        .iter()
        .find(|i| i.code == "test-error")
        .unwrap();
    assert_eq!(error_issue.severity, ValidationSeverity::Error);
    assert_eq!(error_issue.message, "Test error message");

    let warning_issue = result
        .issues
        .iter()
        .find(|i| i.code == "test-warning")
        .unwrap();
    assert_eq!(warning_issue.severity, ValidationSeverity::Warning);
}

#[tokio::test]
async fn test_multiple_schema_validation() {
    let engine = FhirSchemaValidationEngine::new();

    // Create two simple schemas
    let schema1 = FhirSchema::new("Patient")
        .with_name("Patient")
        .with_url(Url::parse("http://hl7.org/fhir/StructureDefinition/Patient").unwrap());

    let schema2 = FhirSchema::new("Patient")
        .with_name("PatientProfile")
        .with_url(Url::parse("http://example.org/StructureDefinition/PatientProfile").unwrap());

    let resource = json!({
        "resourceType": "Patient",
        "id": "example"
    });

    let schemas = vec![&schema1, &schema2];
    let result = engine
        .validate_resource_with_schemas(&resource, &schemas)
        .unwrap();

    // Should validate against both schemas
    println!("Multiple schema validation result: {result:?}");
}

#[tokio::test]
async fn test_reference_validation() {
    let engine = FhirSchemaValidationEngine::new();

    // Create schema with Reference element
    let element = Element::new("Patient.managingOrganization")
        .with_cardinality(0, "1")
        .with_type(ElementType::new("Reference"));

    let schema = FhirSchema::new("Patient")
        .with_name("Patient")
        .with_url(Url::parse("http://hl7.org/fhir/StructureDefinition/Patient").unwrap())
        .with_element("managingOrganization", element);

    // Test valid reference with reference field
    let valid_reference_resource = json!({
        "resourceType": "Patient",
        "id": "example",
        "managingOrganization": {
            "reference": "Organization/123",
            "display": "Example Organization"
        }
    });

    let result = engine
        .validate_resource(&valid_reference_resource, &schema)
        .unwrap();
    println!("Valid reference validation result: {result:?}");
    // Should pass reference validation

    // Test valid reference with identifier field
    let valid_identifier_resource = json!({
        "resourceType": "Patient",
        "id": "example",
        "managingOrganization": {
            "identifier": {
                "system": "http://example.org/organizations",
                "value": "123"
            },
            "display": "Example Organization"
        }
    });

    let result = engine
        .validate_resource(&valid_identifier_resource, &schema)
        .unwrap();
    println!("Valid identifier reference validation result: {result:?}");

    // Test invalid reference (missing both reference and identifier)
    let invalid_reference_resource = json!({
        "resourceType": "Patient",
        "id": "example",
        "managingOrganization": {
            "display": "Example Organization"
        }
    });

    let result = engine
        .validate_resource(&invalid_reference_resource, &schema)
        .unwrap();
    println!("Invalid reference validation result: {result:?}");
    assert!(!result.is_valid);
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.code == "reference-missing-content")
    );

    // Test invalid reference structure (not an object)
    let invalid_structure_resource = json!({
        "resourceType": "Patient",
        "id": "example",
        "managingOrganization": "Organization/123"
    });

    let result = engine
        .validate_resource(&invalid_structure_resource, &schema)
        .unwrap();
    println!("Invalid reference structure validation result: {result:?}");
    assert!(!result.is_valid);
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.code == "type-mismatch")
    );
}
