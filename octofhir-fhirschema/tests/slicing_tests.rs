//! Slicing validation tests for FHIR Schema validator
//!
//! Tests for Phase 5: Slicing Validation
//! - deep_partial_match pattern matching
//! - classify_slice slice classification
//! - validate_slicing integration
//! - Slicing rules (open, closed, openAtEnd)
//! - Slice cardinality validation

use octofhir_fhirschema::types::{
    FhirSchema, FhirSchemaDiscriminator, FhirSchemaElement, FhirSchemaSliceMatch, FhirSchemaSlicing,
};
use octofhir_fhirschema::validation::{FhirSchemaValidator, SliceClassification};
use serde_json::json;
use std::collections::HashMap;

// =============================================================================
// Helper Functions
// =============================================================================

/// Helper to create a minimal FhirSchema
fn create_schema(
    url: &str,
    name: &str,
    type_name: &str,
    kind: &str,
    base: Option<&str>,
    elements: Option<HashMap<String, FhirSchemaElement>>,
) -> FhirSchema {
    FhirSchema {
        url: url.to_string(),
        version: Some("1.0.0".to_string()),
        name: name.to_string(),
        type_name: type_name.to_string(),
        kind: kind.to_string(),
        derivation: if base.is_some() {
            Some("constraint".to_string())
        } else {
            None
        },
        base: base.map(|s| s.to_string()),
        abstract_type: None,
        class: if base.is_some() {
            "profile".to_string()
        } else {
            "resource".to_string()
        },
        description: None,
        package_name: None,
        package_version: None,
        package_id: None,
        package_meta: None,
        elements,
        required: None,
        excluded: None,
        extensions: None,
        constraint: None,
        primitive_type: None,
        choices: None,
    }
}

/// Helper to create a minimal FhirSchemaElement
fn create_element(
    type_name: Option<&str>,
    min: Option<i32>,
    max: Option<i32>,
) -> FhirSchemaElement {
    let is_array = match max {
        None => Some(true), // unbounded = array
        Some(m) if m > 1 => Some(true),
        Some(1) => None, // single value, not array
        _ => None,
    };
    FhirSchemaElement {
        type_name: type_name.map(|s| s.to_string()),
        min,
        max,
        array: is_array,
        ..Default::default()
    }
}

/// Create a validator with no schemas (for unit tests)
fn create_empty_validator() -> FhirSchemaValidator {
    FhirSchemaValidator::new(HashMap::new(), None)
}

// =============================================================================
// Unit Tests for deep_partial_match
// =============================================================================

#[test]
fn test_deep_partial_match_simple_object() {
    let item = json!({"system": "http://mrn", "value": "12345", "use": "official"});
    let pattern = json!({"system": "http://mrn"});

    assert!(FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_missing_field() {
    let item = json!({"value": "12345"});
    let pattern = json!({"system": "http://mrn"});

    assert!(!FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_nested_object() {
    let item = json!({
        "code": {
            "coding": [
                {"system": "http://loinc.org", "code": "12345", "display": "Test"}
            ]
        }
    });
    let pattern = json!({
        "code": {
            "coding": [
                {"system": "http://loinc.org", "code": "12345"}
            ]
        }
    });

    assert!(FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_array_contains() {
    // Item array has multiple elements
    let item = json!([
        {"system": "a", "code": "1"},
        {"system": "b", "code": "2"},
        {"system": "c", "code": "3"}
    ]);
    // Pattern requires at least element with system "b"
    let pattern = json!([
        {"system": "b", "code": "2"}
    ]);

    assert!(FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_array_multiple_required() {
    // Item array
    let item = json!([
        {"system": "a", "code": "1"},
        {"system": "b", "code": "2"}
    ]);
    // Pattern requires both elements
    let pattern = json!([
        {"system": "a"},
        {"system": "b"}
    ]);

    assert!(FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_array_missing_element() {
    let item = json!([
        {"system": "a", "code": "1"}
    ]);
    // Pattern requires element with system "b" which doesn't exist
    let pattern = json!([
        {"system": "b"}
    ]);

    assert!(!FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_empty_pattern() {
    let item = json!({"anything": "here", "more": "fields"});
    let pattern = json!({});

    assert!(FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_null_pattern() {
    let item = json!({"anything": "here"});
    let pattern = json!(null);

    assert!(FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_scalar_string() {
    let item = json!("http://example.com");
    let pattern = json!("http://example.com");

    assert!(FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_scalar_string_mismatch() {
    let item = json!("http://example.com");
    let pattern = json!("http://other.com");

    assert!(!FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_scalar_number() {
    let item = json!(42);
    let pattern = json!(42);

    assert!(FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_scalar_boolean() {
    let item = json!(true);
    let pattern = json!(true);

    assert!(FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_type_mismatch() {
    let item = json!({"field": "value"});
    let pattern = json!("string");

    assert!(!FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

#[test]
fn test_deep_partial_match_codeable_concept() {
    // Real-world FHIR pattern: CodeableConcept matching
    let item = json!({
        "coding": [
            {
                "system": "http://loinc.org",
                "code": "8480-6",
                "display": "Systolic blood pressure"
            }
        ],
        "text": "Systolic BP"
    });
    let pattern = json!({
        "coding": [
            {
                "system": "http://loinc.org",
                "code": "8480-6"
            }
        ]
    });

    assert!(FhirSchemaValidator::deep_partial_match(&item, &pattern));
}

// =============================================================================
// Unit Tests for classify_slice
// =============================================================================

#[test]
fn test_classify_slice_single_match() {
    let validator = create_empty_validator();

    let item = json!({"system": "http://hospital.org/mrn", "value": "123"});

    let mut slices = HashMap::new();
    slices.insert(
        "MRN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hospital.org/mrn"})),
            schema: None,
            min: Some(1),
            max: Some(1),
        },
    );
    slices.insert(
        "SSN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hl7.org/fhir/sid/us-ssn"})),
            schema: None,
            min: None,
            max: Some(1),
        },
    );

    let result = validator.classify_slice(&item, &slices);

    assert!(matches!(result, SliceClassification::Matched(name) if name == "MRN"));
}

#[test]
fn test_classify_slice_unmatched() {
    let validator = create_empty_validator();

    let item = json!({"system": "http://unknown", "value": "123"});

    let mut slices = HashMap::new();
    slices.insert(
        "MRN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hospital.org/mrn"})),
            schema: None,
            min: None,
            max: None,
        },
    );

    let result = validator.classify_slice(&item, &slices);

    assert!(matches!(result, SliceClassification::Unmatched));
}

#[test]
fn test_classify_slice_ambiguous() {
    let validator = create_empty_validator();

    // Item matches both slices (overlapping patterns)
    let item = json!({"system": "http://common", "use": "official"});

    let mut slices = HashMap::new();
    slices.insert(
        "Slice1".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://common"})),
            schema: None,
            min: None,
            max: None,
        },
    );
    slices.insert(
        "Slice2".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"use": "official"})),
            schema: None,
            min: None,
            max: None,
        },
    );

    let result = validator.classify_slice(&item, &slices);

    match result {
        SliceClassification::Ambiguous(names) => {
            assert_eq!(names.len(), 2);
            assert!(names.contains(&"Slice1".to_string()));
            assert!(names.contains(&"Slice2".to_string()));
        }
        _ => panic!("Expected Ambiguous classification"),
    }
}

#[test]
fn test_classify_slice_empty_match_catches_all() {
    let validator = create_empty_validator();

    let item = json!({"anything": "here"});

    let mut slices = HashMap::new();
    slices.insert(
        "@default".to_string(),
        FhirSchemaSliceMatch {
            match_value: None, // Empty = catch-all
            schema: None,
            min: None,
            max: None,
        },
    );

    let result = validator.classify_slice(&item, &slices);

    assert!(matches!(result, SliceClassification::Matched(name) if name == "@default"));
}

#[test]
fn test_classify_slice_empty_object_catches_all() {
    let validator = create_empty_validator();

    let item = json!({"anything": "here"});

    let mut slices = HashMap::new();
    slices.insert(
        "@default".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({})), // Empty object = catch-all
            schema: None,
            min: None,
            max: None,
        },
    );

    let result = validator.classify_slice(&item, &slices);

    assert!(matches!(result, SliceClassification::Matched(name) if name == "@default"));
}

// =============================================================================
// Integration Tests for validate_slicing
// =============================================================================

#[tokio::test]
async fn test_closed_slicing_rejects_unmatched() {
    // Create schema with closed slicing on identifier
    let mut identifier_elem = create_element(Some("Identifier"), Some(0), None);
    identifier_elem.array = Some(true);

    let mut slices = HashMap::new();
    slices.insert(
        "MRN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hospital.org/mrn"})),
            schema: None,
            min: None,
            max: None,
        },
    );

    identifier_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: Some(vec![FhirSchemaDiscriminator {
            type_name: "value".to_string(),
            path: "system".to_string(),
        }]),
        rules: Some("closed".to_string()),
        ordered: Some(false),
        slices: Some(slices),
    });

    let mut elements = HashMap::new();
    elements.insert("identifier".to_string(), identifier_elem);

    let schema = create_schema(
        "http://test/Patient",
        "Patient",
        "Patient",
        "resource",
        None,
        Some(elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("Patient".to_string(), schema);

    let validator = FhirSchemaValidator::new(schemas, None);

    let patient = json!({
        "resourceType": "Patient",
        "identifier": [
            {"system": "http://hospital.org/mrn", "value": "123"},
            {"system": "http://unknown", "value": "456"}  // Should fail - unmatched in closed slicing
        ]
    });

    let result = validator
        .validate(&patient, vec!["Patient".to_string()])
        .await;

    assert!(
        !result.valid,
        "Validation should fail for unmatched item in closed slicing"
    );
    assert!(
        result.errors.iter().any(|e| e.error_type == "FS1007"),
        "Should have SlicingUnmatched error (FS1007)"
    );
}

#[tokio::test]
async fn test_open_slicing_allows_unmatched() {
    // Create schema with open slicing on identifier
    let mut identifier_elem = create_element(Some("Identifier"), Some(0), None);
    identifier_elem.array = Some(true);

    let mut slices = HashMap::new();
    slices.insert(
        "MRN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hospital.org/mrn"})),
            schema: None,
            min: None,
            max: None,
        },
    );

    identifier_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: Some(vec![FhirSchemaDiscriminator {
            type_name: "value".to_string(),
            path: "system".to_string(),
        }]),
        rules: Some("open".to_string()),
        ordered: Some(false),
        slices: Some(slices),
    });

    let mut elements = HashMap::new();
    elements.insert("identifier".to_string(), identifier_elem);

    let schema = create_schema(
        "http://test/Patient",
        "Patient",
        "Patient",
        "resource",
        None,
        Some(elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("Patient".to_string(), schema);

    let validator = FhirSchemaValidator::new(schemas, None);

    let patient = json!({
        "resourceType": "Patient",
        "identifier": [
            {"system": "http://hospital.org/mrn", "value": "123"},
            {"system": "http://unknown", "value": "456"}  // Should be OK - open slicing
        ]
    });

    let result = validator
        .validate(&patient, vec!["Patient".to_string()])
        .await;

    // Should not have slicing errors (may have other errors due to missing Identifier schema)
    let slicing_errors: Vec<_> = result
        .errors
        .iter()
        .filter(|e| e.error_type == "FS1007")
        .collect();

    assert!(
        slicing_errors.is_empty(),
        "Should not have SlicingUnmatched errors in open slicing"
    );
}

#[tokio::test]
async fn test_slice_cardinality_minimum_violation() {
    // Create schema with MRN slice requiring min=2
    let mut identifier_elem = create_element(Some("Identifier"), Some(0), None);
    identifier_elem.array = Some(true);

    let mut slices = HashMap::new();
    slices.insert(
        "MRN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hospital.org/mrn"})),
            schema: None,
            min: Some(2), // Require at least 2 MRN identifiers
            max: None,
        },
    );

    identifier_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: Some(vec![FhirSchemaDiscriminator {
            type_name: "value".to_string(),
            path: "system".to_string(),
        }]),
        rules: Some("open".to_string()),
        ordered: Some(false),
        slices: Some(slices),
    });

    let mut elements = HashMap::new();
    elements.insert("identifier".to_string(), identifier_elem);

    let schema = create_schema(
        "http://test/Patient",
        "Patient",
        "Patient",
        "resource",
        None,
        Some(elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("Patient".to_string(), schema);

    let validator = FhirSchemaValidator::new(schemas, None);

    let patient = json!({
        "resourceType": "Patient",
        "identifier": [
            {"system": "http://hospital.org/mrn", "value": "123"}
            // Only 1 MRN, but min=2 required
        ]
    });

    let result = validator
        .validate(&patient, vec!["Patient".to_string()])
        .await;

    assert!(
        !result.valid,
        "Validation should fail for slice cardinality violation"
    );
    assert!(
        result.errors.iter().any(|e| e.error_type == "FS1009"),
        "Should have SliceCardinality error (FS1009)"
    );
}

#[tokio::test]
async fn test_slice_cardinality_maximum_violation() {
    // Create schema with MRN slice allowing max=1
    let mut identifier_elem = create_element(Some("Identifier"), Some(0), None);
    identifier_elem.array = Some(true);

    let mut slices = HashMap::new();
    slices.insert(
        "MRN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hospital.org/mrn"})),
            schema: None,
            min: None,
            max: Some(1), // Allow at most 1 MRN identifier
        },
    );

    identifier_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: Some(vec![FhirSchemaDiscriminator {
            type_name: "value".to_string(),
            path: "system".to_string(),
        }]),
        rules: Some("open".to_string()),
        ordered: Some(false),
        slices: Some(slices),
    });

    let mut elements = HashMap::new();
    elements.insert("identifier".to_string(), identifier_elem);

    let schema = create_schema(
        "http://test/Patient",
        "Patient",
        "Patient",
        "resource",
        None,
        Some(elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("Patient".to_string(), schema);

    let validator = FhirSchemaValidator::new(schemas, None);

    let patient = json!({
        "resourceType": "Patient",
        "identifier": [
            {"system": "http://hospital.org/mrn", "value": "123"},
            {"system": "http://hospital.org/mrn", "value": "456"}
            // 2 MRNs, but max=1 allowed
        ]
    });

    let result = validator
        .validate(&patient, vec!["Patient".to_string()])
        .await;

    assert!(
        !result.valid,
        "Validation should fail for slice max cardinality violation"
    );
    assert!(
        result.errors.iter().any(|e| e.error_type == "FS1009"),
        "Should have SliceCardinality error (FS1009)"
    );
}

#[tokio::test]
async fn test_slice_cardinality_passes_when_satisfied() {
    // Create schema with MRN slice requiring min=1, max=2
    let mut identifier_elem = create_element(Some("Identifier"), Some(0), None);
    identifier_elem.array = Some(true);

    let mut slices = HashMap::new();
    slices.insert(
        "MRN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hospital.org/mrn"})),
            schema: None,
            min: Some(1),
            max: Some(2),
        },
    );

    identifier_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: Some(vec![FhirSchemaDiscriminator {
            type_name: "value".to_string(),
            path: "system".to_string(),
        }]),
        rules: Some("open".to_string()),
        ordered: Some(false),
        slices: Some(slices),
    });

    let mut elements = HashMap::new();
    elements.insert("identifier".to_string(), identifier_elem);

    let schema = create_schema(
        "http://test/Patient",
        "Patient",
        "Patient",
        "resource",
        None,
        Some(elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("Patient".to_string(), schema);

    let validator = FhirSchemaValidator::new(schemas, None);

    let patient = json!({
        "resourceType": "Patient",
        "identifier": [
            {"system": "http://hospital.org/mrn", "value": "123"},
            {"system": "http://hospital.org/mrn", "value": "456"}
            // 2 MRNs, within min=1, max=2 bounds
        ]
    });

    let result = validator
        .validate(&patient, vec!["Patient".to_string()])
        .await;

    // Should not have slice cardinality errors
    let cardinality_errors: Vec<_> = result
        .errors
        .iter()
        .filter(|e| e.error_type == "FS1009")
        .collect();

    assert!(
        cardinality_errors.is_empty(),
        "Should not have SliceCardinality errors when cardinality is satisfied"
    );
}

#[tokio::test]
async fn test_multiple_slices_with_different_cardinalities() {
    // Create schema with multiple slices
    let mut identifier_elem = create_element(Some("Identifier"), Some(0), None);
    identifier_elem.array = Some(true);

    let mut slices = HashMap::new();
    slices.insert(
        "MRN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hospital.org/mrn"})),
            schema: None,
            min: Some(1), // Require at least 1
            max: Some(1), // Allow at most 1
        },
    );
    slices.insert(
        "SSN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hl7.org/fhir/sid/us-ssn"})),
            schema: None,
            min: Some(0), // Optional
            max: Some(1), // Allow at most 1
        },
    );

    identifier_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: Some(vec![FhirSchemaDiscriminator {
            type_name: "value".to_string(),
            path: "system".to_string(),
        }]),
        rules: Some("open".to_string()),
        ordered: Some(false),
        slices: Some(slices),
    });

    let mut elements = HashMap::new();
    elements.insert("identifier".to_string(), identifier_elem);

    let schema = create_schema(
        "http://test/Patient",
        "Patient",
        "Patient",
        "resource",
        None,
        Some(elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("Patient".to_string(), schema);

    let validator = FhirSchemaValidator::new(schemas, None);

    // Valid: 1 MRN, 0 SSN (both within bounds)
    let patient = json!({
        "resourceType": "Patient",
        "identifier": [
            {"system": "http://hospital.org/mrn", "value": "123"}
        ]
    });

    let result = validator
        .validate(&patient, vec!["Patient".to_string()])
        .await;

    let cardinality_errors: Vec<_> = result
        .errors
        .iter()
        .filter(|e| e.error_type == "FS1009")
        .collect();

    assert!(
        cardinality_errors.is_empty(),
        "Should not have cardinality errors for valid slice counts"
    );
}

#[tokio::test]
async fn test_extension_slicing_by_url() {
    // Test slicing extensions by URL (common FHIR pattern)
    let mut extension_elem = create_element(Some("Extension"), Some(0), None);
    extension_elem.array = Some(true);

    let mut slices = HashMap::new();
    slices.insert(
        "race".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(
                json!({"url": "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race"}),
            ),
            schema: None,
            min: Some(0),
            max: Some(1),
        },
    );
    slices.insert(
        "ethnicity".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(
                json!({"url": "http://hl7.org/fhir/us/core/StructureDefinition/us-core-ethnicity"}),
            ),
            schema: None,
            min: Some(0),
            max: Some(1),
        },
    );

    extension_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: Some(vec![FhirSchemaDiscriminator {
            type_name: "value".to_string(),
            path: "url".to_string(),
        }]),
        rules: Some("open".to_string()),
        ordered: Some(false),
        slices: Some(slices),
    });

    let mut elements = HashMap::new();
    elements.insert("extension".to_string(), extension_elem);

    let schema = create_schema(
        "http://test/Patient",
        "Patient",
        "Patient",
        "resource",
        None,
        Some(elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("Patient".to_string(), schema);

    let validator = FhirSchemaValidator::new(schemas, None);

    let patient = json!({
        "resourceType": "Patient",
        "extension": [
            {
                "url": "http://hl7.org/fhir/us/core/StructureDefinition/us-core-race",
                "valueCodeableConcept": {"text": "White"}
            },
            {
                "url": "http://hl7.org/fhir/us/core/StructureDefinition/us-core-ethnicity",
                "valueCodeableConcept": {"text": "Not Hispanic"}
            }
        ]
    });

    let result = validator
        .validate(&patient, vec!["Patient".to_string()])
        .await;

    // Should not have slicing errors
    let slicing_errors: Vec<_> = result
        .errors
        .iter()
        .filter(|e| e.error_type.starts_with("FS100"))
        .filter(|e| {
            e.error_type == "FS1007" || e.error_type == "FS1008" || e.error_type == "FS1009"
        })
        .collect();

    assert!(
        slicing_errors.is_empty(),
        "Should not have slicing errors for valid extension slicing"
    );
}

#[tokio::test]
async fn test_observation_component_slicing() {
    // Test slicing Observation.component by code (common pattern for vitals panels)
    let mut component_elem = create_element(Some("BackboneElement"), Some(0), None);
    component_elem.array = Some(true);

    let mut slices = HashMap::new();
    slices.insert(
        "systolic".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({
                "code": {
                    "coding": [{"system": "http://loinc.org", "code": "8480-6"}]
                }
            })),
            schema: None,
            min: Some(1),
            max: Some(1),
        },
    );
    slices.insert(
        "diastolic".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({
                "code": {
                    "coding": [{"system": "http://loinc.org", "code": "8462-4"}]
                }
            })),
            schema: None,
            min: Some(1),
            max: Some(1),
        },
    );

    component_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: Some(vec![FhirSchemaDiscriminator {
            type_name: "pattern".to_string(),
            path: "code".to_string(),
        }]),
        rules: Some("open".to_string()),
        ordered: Some(false),
        slices: Some(slices),
    });

    let mut elements = HashMap::new();
    elements.insert("component".to_string(), component_elem);

    let schema = create_schema(
        "http://test/BloodPressure",
        "BloodPressure",
        "Observation",
        "resource",
        None,
        Some(elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("BloodPressure".to_string(), schema);

    let validator = FhirSchemaValidator::new(schemas, None);

    let observation = json!({
        "resourceType": "Observation",
        "component": [
            {
                "code": {
                    "coding": [{"system": "http://loinc.org", "code": "8480-6", "display": "Systolic"}]
                },
                "valueQuantity": {"value": 120, "unit": "mmHg"}
            },
            {
                "code": {
                    "coding": [{"system": "http://loinc.org", "code": "8462-4", "display": "Diastolic"}]
                },
                "valueQuantity": {"value": 80, "unit": "mmHg"}
            }
        ]
    });

    let result = validator
        .validate(&observation, vec!["BloodPressure".to_string()])
        .await;

    // Should not have slicing cardinality errors (both required slices present)
    let cardinality_errors: Vec<_> = result
        .errors
        .iter()
        .filter(|e| e.error_type == "FS1009")
        .collect();

    assert!(
        cardinality_errors.is_empty(),
        "Should not have cardinality errors when all required component slices are present"
    );
}

#[tokio::test]
async fn test_observation_component_missing_required_slice() {
    // Test missing required slice in Observation.component
    let mut component_elem = create_element(Some("BackboneElement"), Some(0), None);
    component_elem.array = Some(true);

    let mut slices = HashMap::new();
    slices.insert(
        "systolic".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({
                "code": {
                    "coding": [{"system": "http://loinc.org", "code": "8480-6"}]
                }
            })),
            schema: None,
            min: Some(1), // Required
            max: Some(1),
        },
    );
    slices.insert(
        "diastolic".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({
                "code": {
                    "coding": [{"system": "http://loinc.org", "code": "8462-4"}]
                }
            })),
            schema: None,
            min: Some(1), // Required
            max: Some(1),
        },
    );

    component_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: Some(vec![FhirSchemaDiscriminator {
            type_name: "pattern".to_string(),
            path: "code".to_string(),
        }]),
        rules: Some("open".to_string()),
        ordered: Some(false),
        slices: Some(slices),
    });

    let mut elements = HashMap::new();
    elements.insert("component".to_string(), component_elem);

    let schema = create_schema(
        "http://test/BloodPressure",
        "BloodPressure",
        "Observation",
        "resource",
        None,
        Some(elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("BloodPressure".to_string(), schema);

    let validator = FhirSchemaValidator::new(schemas, None);

    // Missing diastolic component
    let observation = json!({
        "resourceType": "Observation",
        "component": [
            {
                "code": {
                    "coding": [{"system": "http://loinc.org", "code": "8480-6", "display": "Systolic"}]
                },
                "valueQuantity": {"value": 120, "unit": "mmHg"}
            }
        ]
    });

    let result = validator
        .validate(&observation, vec!["BloodPressure".to_string()])
        .await;

    // Should have cardinality error for missing diastolic slice
    assert!(
        result.errors.iter().any(|e| e.error_type == "FS1009"),
        "Should have SliceCardinality error for missing required diastolic slice"
    );
}
