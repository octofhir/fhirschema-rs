//! Integration tests for FHIRPath constraint validation
//!
//! These tests verify that the FhirSchemaValidator correctly integrates
//! with FhirPathEvaluator implementations to validate FHIRPath constraints.

use octofhir_fhirschema::types::{FhirSchema, FhirSchemaConstraint, FhirSchemaElement};
use octofhir_fhirschema::validation::FhirSchemaValidator;
use octofhir_fhirschema_tests::mock_evaluator::{
    AlwaysInvalidEvaluator, AlwaysValidEvaluator, ConfigurableEvaluator,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Helper to create basic Patient schema elements
fn create_basic_patient_elements() -> HashMap<String, FhirSchemaElement> {
    let mut elements = HashMap::new();

    // Add id element
    elements.insert(
        "id".to_string(),
        FhirSchemaElement {
            type_name: Some("string".to_string()),
            ..Default::default()
        },
    );

    // Add resourceType element
    elements.insert(
        "resourceType".to_string(),
        FhirSchemaElement {
            type_name: Some("string".to_string()),
            ..Default::default()
        },
    );

    // Add name element (array of HumanName)
    elements.insert(
        "name".to_string(),
        FhirSchemaElement {
            array: Some(true),
            type_name: Some("HumanName".to_string()),
            ..Default::default()
        },
    );

    // Add gender element
    elements.insert(
        "gender".to_string(),
        FhirSchemaElement {
            type_name: Some("code".to_string()),
            ..Default::default()
        },
    );

    elements
}

#[tokio::test]
async fn test_validation_without_evaluator_skips_constraints() {
    // Create a simple schema with a constraint
    let mut schemas = HashMap::new();
    let mut constraints = HashMap::new();
    constraints.insert(
        "test-1".to_string(),
        FhirSchemaConstraint {
            severity: "error".to_string(),
            human: "Test constraint".to_string(),
            expression: "name.exists()".to_string(),
        },
    );

    let patient_schema = FhirSchema {
        url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
        name: "Patient".to_string(),
        type_name: "Patient".to_string(),
        kind: "resource".to_string(),
        class: "resource".to_string(),
        constraint: Some(constraints),
        elements: Some(create_basic_patient_elements()),
        version: None,
        derivation: None,
        base: None,
        abstract_type: None,
        description: None,
        package_name: None,
        package_version: None,
        package_id: None,
        package_meta: None,
        required: None,
        excluded: None,
        extensions: None,
        primitive_type: None,
        choices: None,
    };

    schemas.insert("Patient".to_string(), patient_schema);

    // Create validator WITHOUT evaluator
    let validator = FhirSchemaValidator::new(schemas, None);

    // Resource that would fail the constraint (no name)
    // Use only resourceType to avoid structural validation issues
    let resource = json!({
        "resourceType": "Patient"
    });

    // Should pass because constraints are skipped without evaluator
    let result = validator
        .validate(&resource, vec!["Patient".to_string()])
        .await;

    // Structural validation passes, constraints skipped
    if !result.valid {
        println!("Validation errors: {:?}", result.errors);
    }
    assert!(result.valid, "Should pass without evaluator (constraints skipped)");
}

#[tokio::test]
async fn test_validation_with_always_valid_evaluator() {
    let mut schemas = HashMap::new();
    let mut constraints = HashMap::new();
    constraints.insert(
        "test-1".to_string(),
        FhirSchemaConstraint {
            severity: "error".to_string(),
            human: "Name must exist".to_string(),
            expression: "name.exists()".to_string(),
        },
    );

    let patient_schema = FhirSchema {
        url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
        name: "Patient".to_string(),
        type_name: "Patient".to_string(),
        kind: "resource".to_string(),
        class: "resource".to_string(),
        constraint: Some(constraints),
        elements: Some(create_basic_patient_elements()),
        version: None,
        derivation: None,
        base: None,
        abstract_type: None,
        description: None,
        package_name: None,
        package_version: None,
        package_id: None,
        package_meta: None,
        required: None,
        excluded: None,
        extensions: None,
        primitive_type: None,
        choices: None,
    };

    schemas.insert("Patient".to_string(), patient_schema);

    // Create validator WITH mock evaluator that always returns valid
    let evaluator = Arc::new(AlwaysValidEvaluator);
    let validator = FhirSchemaValidator::new(schemas, Some(evaluator));

    let resource = json!({
        "resourceType": "Patient"
    });

    let result = validator
        .validate(&resource, vec!["Patient".to_string()])
        .await;

    if !result.valid {
        println!("Validation errors: {:?}", result.errors);
    }
    assert!(result.valid, "Should pass with AlwaysValidEvaluator");
    assert!(result.errors.is_empty());
}

#[tokio::test]
async fn test_validation_with_always_invalid_evaluator() {
    let mut schemas = HashMap::new();
    let mut constraints = HashMap::new();
    constraints.insert(
        "pat-1".to_string(),
        FhirSchemaConstraint {
            severity: "error".to_string(),
            human: "Name must exist".to_string(),
            expression: "name.exists()".to_string(),
        },
    );

    let patient_schema = FhirSchema {
        url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
        name: "Patient".to_string(),
        type_name: "Patient".to_string(),
        kind: "resource".to_string(),
        class: "resource".to_string(),
        constraint: Some(constraints),
        elements: Some(create_basic_patient_elements()),
        version: None,
        derivation: None,
        base: None,
        abstract_type: None,
        description: None,
        package_name: None,
        package_version: None,
        package_id: None,
        package_meta: None,
        required: None,
        excluded: None,
        extensions: None,
        primitive_type: None,
        choices: None,
    };

    schemas.insert("Patient".to_string(), patient_schema);

    // Create validator with mock evaluator that always fails
    let evaluator = Arc::new(AlwaysInvalidEvaluator::new("Name is required"));
    let validator = FhirSchemaValidator::new(schemas, Some(evaluator));

    let resource = json!({
        "resourceType": "Patient"
    });

    let result = validator
        .validate(&resource, vec!["Patient".to_string()])
        .await;

    assert!(!result.valid, "Should fail with AlwaysInvalidEvaluator");
    assert!(!result.errors.is_empty());

    // Check that error message contains constraint info
    let has_constraint_error = result.errors.iter().any(|e| {
        e.message
            .as_ref()
            .map(|m| m.contains("Constraint") && m.contains("pat-1"))
            .unwrap_or(false)
    });
    assert!(has_constraint_error, "Should have constraint violation error");
}

#[tokio::test]
async fn test_validation_with_configurable_evaluator() {
    let mut schemas = HashMap::new();
    let mut constraints = HashMap::new();

    // Add multiple constraints
    constraints.insert(
        "pat-1".to_string(),
        FhirSchemaConstraint {
            severity: "error".to_string(),
            human: "Name must exist".to_string(),
            expression: "name.exists()".to_string(),
        },
    );
    constraints.insert(
        "pat-2".to_string(),
        FhirSchemaConstraint {
            severity: "warning".to_string(),
            human: "Gender should exist".to_string(),
            expression: "gender.exists()".to_string(),
        },
    );

    let patient_schema = FhirSchema {
        url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
        name: "Patient".to_string(),
        type_name: "Patient".to_string(),
        kind: "resource".to_string(),
        class: "resource".to_string(),
        constraint: Some(constraints),
        elements: Some(create_basic_patient_elements()),
        version: None,
        derivation: None,
        base: None,
        abstract_type: None,
        description: None,
        package_name: None,
        package_version: None,
        package_id: None,
        package_meta: None,
        required: None,
        excluded: None,
        extensions: None,
        primitive_type: None,
        choices: None,
    };

    schemas.insert("Patient".to_string(), patient_schema);

    // Create configurable evaluator - pat-1 passes, pat-2 fails
    let mut evaluator = ConfigurableEvaluator::new();
    evaluator.set_constraint_result("pat-1", true);
    evaluator.set_constraint_result("pat-2", false);

    let validator = FhirSchemaValidator::new(schemas, Some(Arc::new(evaluator)));

    let resource = json!({
        "resourceType": "Patient",
        "id": "test",
        "name": [{"family": "Doe"}]
    });

    let result = validator
        .validate(&resource, vec!["Patient".to_string()])
        .await;

    assert!(!result.valid, "Should fail because pat-2 constraint fails");
    assert!(!result.errors.is_empty());

    // Verify error is about pat-2
    let has_pat2_error = result.errors.iter().any(|e| {
        e.message
            .as_ref()
            .map(|m| m.contains("pat-2"))
            .unwrap_or(false)
    });
    assert!(has_pat2_error, "Should have error for pat-2 constraint");
}

#[tokio::test]
async fn test_validation_with_warning_severity() {
    let mut schemas = HashMap::new();
    let mut constraints = HashMap::new();

    constraints.insert(
        "pat-w1".to_string(),
        FhirSchemaConstraint {
            severity: "warning".to_string(),
            human: "Gender should be specified".to_string(),
            expression: "gender.exists()".to_string(),
        },
    );

    let patient_schema = FhirSchema {
        url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
        name: "Patient".to_string(),
        type_name: "Patient".to_string(),
        kind: "resource".to_string(),
        class: "resource".to_string(),
        constraint: Some(constraints),
        elements: Some(create_basic_patient_elements()),
        version: None,
        derivation: None,
        base: None,
        abstract_type: None,
        description: None,
        package_name: None,
        package_version: None,
        package_id: None,
        package_meta: None,
        required: None,
        excluded: None,
        extensions: None,
        primitive_type: None,
        choices: None,
    };

    schemas.insert("Patient".to_string(), patient_schema);

    let evaluator = Arc::new(AlwaysInvalidEvaluator::new("Gender missing"));
    let validator = FhirSchemaValidator::new(schemas, Some(evaluator));

    let resource = json!({
        "resourceType": "Patient"
    });

    let result = validator
        .validate(&resource, vec!["Patient".to_string()])
        .await;

    // Warnings still cause validation to fail for now
    // (Phase 1.5 will add proper warning support)
    assert!(!result.valid);
    assert!(!result.errors.is_empty());
}
