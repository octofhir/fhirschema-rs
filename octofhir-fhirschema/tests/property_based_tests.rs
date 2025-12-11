//! Property-Based Testing for FHIR Schema validation.
//!
//! Uses proptest to generate random valid/invalid resources and verify:
//! - Deterministic validation (same input → same output)
//! - Structural correctness of validation errors
//! - Graceful handling of edge cases
//! - Roundtrip consistency

use octofhir_fhirschema::embedded::{FhirVersion, get_schemas};
use octofhir_fhirschema::validation::FhirSchemaValidator;
use proptest::prelude::*;
use serde_json::{Value, json};

/// Create validator with embedded R4 schemas
fn create_r4_validator() -> FhirSchemaValidator {
    let schemas = get_schemas(FhirVersion::R4);
    FhirSchemaValidator::new(schemas.clone(), None)
}

// =============================================================================
// Property-Based Test Strategies
// =============================================================================

/// Strategy to generate valid FHIR resource types
fn valid_resource_type() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("Patient"),
        Just("Observation"),
        Just("Condition"),
        Just("Encounter"),
        Just("Practitioner"),
        Just("Organization"),
        Just("Location"),
        Just("Procedure"),
        Just("MedicationRequest"),
    ]
}

/// Strategy to generate valid identifier systems
fn valid_system() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("http://example.org/mrn".to_string()),
        Just("http://hospital.example.org".to_string()),
        Just("http://terminology.hl7.org/CodeSystem/v3-ActCode".to_string()),
        Just("http://loinc.org".to_string()),
        Just("http://snomed.info/sct".to_string()),
    ]
}

/// Strategy to generate valid identifier values
fn valid_identifier_value() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{1,20}".prop_map(|s| s)
}

/// Strategy to generate valid human names
fn valid_human_name() -> impl Strategy<Value = Value> {
    (
        prop::option::of("[A-Z][a-z]{2,15}"),
        prop::collection::vec("[A-Z][a-z]{2,10}", 0..3),
    )
        .prop_map(|(family, given)| {
            let mut name = json!({});
            if let Some(f) = family {
                name["family"] = json!(f);
            }
            if !given.is_empty() {
                name["given"] = json!(given);
            }
            name
        })
}

/// Strategy to generate valid Patient resources
fn valid_patient_strategy() -> impl Strategy<Value = Value> {
    (
        prop::option::of(valid_identifier_value()),
        prop::collection::vec(valid_human_name(), 0..3),
        prop::option::of(prop_oneof![
            Just("male"),
            Just("female"),
            Just("other"),
            Just("unknown"),
        ]),
        prop::option::of("[0-9]{4}-[0-1][0-9]-[0-3][0-9]"),
        prop::bool::ANY,
    )
        .prop_map(|(id, names, gender, birth_date, active)| {
            let mut patient = json!({
                "resourceType": "Patient"
            });

            if let Some(id_val) = id {
                patient["id"] = json!(id_val);
            }

            if !names.is_empty() {
                patient["name"] = json!(names);
            }

            if let Some(g) = gender {
                patient["gender"] = json!(g);
            }

            if let Some(bd) = birth_date {
                patient["birthDate"] = json!(bd);
            }

            patient["active"] = json!(active);

            patient
        })
}

/// Strategy to generate valid Observation resources
fn valid_observation_strategy() -> impl Strategy<Value = Value> {
    (
        prop::option::of(valid_identifier_value()),
        prop_oneof![
            Just("registered"),
            Just("preliminary"),
            Just("final"),
            Just("amended"),
        ],
        valid_system(),
        "[0-9A-Z]{3,10}".prop_map(|s| s),
    )
        .prop_map(|(id, status, system, code)| {
            let mut obs = json!({
                "resourceType": "Observation",
                "status": status,
                "code": {
                    "coding": [{
                        "system": system,
                        "code": code
                    }]
                }
            });

            if let Some(id_val) = id {
                obs["id"] = json!(id_val);
            }

            obs
        })
}

/// Strategy to generate minimal resources (just resourceType)
fn minimal_resource_strategy() -> impl Strategy<Value = Value> {
    valid_resource_type().prop_map(|rt| {
        json!({
            "resourceType": rt
        })
    })
}

/// Strategy to generate invalid resources (wrong types, unknown fields)
fn invalid_resource_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        // Unknown resource type
        Just(json!({
            "resourceType": "UnknownResource123",
            "id": "test"
        })),
        // Invalid field type (active should be boolean)
        Just(json!({
            "resourceType": "Patient",
            "active": "yes"
        })),
        // Unknown element
        Just(json!({
            "resourceType": "Patient",
            "unknownElement": "value"
        })),
        // Array where single value expected
        Just(json!({
            "resourceType": "Patient",
            "gender": ["male", "female"]
        })),
        // Object where array expected
        Just(json!({
            "resourceType": "Patient",
            "name": {"family": "Test"}
        })),
    ]
}

// =============================================================================
// Property-Based Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Test that validation is deterministic - same input always gives same output
    #[test]
    fn prop_validation_is_deterministic(patient in valid_patient_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let validator = create_r4_validator();

            // Validate twice
            let result1 = validator.validate(&patient, vec!["Patient".to_string()]).await;
            let result2 = validator.validate(&patient, vec!["Patient".to_string()]).await;

            // Results should be identical
            prop_assert_eq!(result1.valid, result2.valid);
            prop_assert_eq!(result1.errors.len(), result2.errors.len());

            for (e1, e2) in result1.errors.iter().zip(result2.errors.iter()) {
                prop_assert_eq!(&e1.error_type, &e2.error_type);
                prop_assert_eq!(&e1.path, &e2.path);
            }

            Ok(())
        })?;
    }

    /// Test that valid patients don't crash the validator
    #[test]
    fn prop_valid_patients_dont_crash(patient in valid_patient_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let validator = create_r4_validator();
            let result = validator.validate(&patient, vec!["Patient".to_string()]).await;

            // Should not panic, and should return a ValidationResult
            // We don't assert validity since generated dates might be invalid
            prop_assert!(result.errors.len() < 100, "Too many errors: {}", result.errors.len());

            Ok(())
        })?;
    }

    /// Test that valid observations have required fields checked
    #[test]
    fn prop_valid_observations_structured(obs in valid_observation_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let validator = create_r4_validator();
            let result = validator.validate(&obs, vec!["Observation".to_string()]).await;

            // Observation should be mostly valid (status and code are required and provided)
            // Only errors should be structural warnings, not crashes
            prop_assert!(result.errors.len() < 50, "Too many errors: {}", result.errors.len());

            Ok(())
        })?;
    }

    /// Test that minimal resources are recognized
    #[test]
    fn prop_minimal_resources_recognized(resource in minimal_resource_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let validator = create_r4_validator();
            let resource_type = resource.get("resourceType").and_then(|rt| rt.as_str()).unwrap();
            let result = validator.validate(&resource, vec![resource_type.to_string()]).await;

            // Resource type should be recognized
            // Errors should only be about missing required fields (FS1011), not unknown schema
            for error in &result.errors {
                prop_assert!(
                    error.error_type == "FS1011" || error.error_type == "FS1001",
                    "Unexpected error type for minimal resource: {} - {:?}",
                    error.error_type,
                    error.message
                );
            }

            Ok(())
        })?;
    }

    /// Test that invalid resources produce errors
    #[test]
    fn prop_invalid_resources_have_errors(resource in invalid_resource_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let validator = create_r4_validator();
            let resource_type = resource.get("resourceType").and_then(|rt| rt.as_str()).unwrap_or("Unknown");
            let result = validator.validate(&resource, vec![resource_type.to_string()]).await;

            // Invalid resources should either:
            // 1. Be invalid (have errors)
            // 2. Or be an unknown resource type
            prop_assert!(
                !result.valid || resource_type == "UnknownResource123",
                "Invalid resource should have errors: {:?}",
                resource
            );

            Ok(())
        })?;
    }
}

// =============================================================================
// Additional Property Tests (Non-proptest)
// =============================================================================

/// Test validation error structure
#[tokio::test]
async fn test_error_structure_consistency() {
    let validator = create_r4_validator();

    // Create resource with known error
    let invalid_patient = json!({
        "resourceType": "Patient",
        "unknownField": "value"
    });

    let result = validator
        .validate(&invalid_patient, vec!["Patient".to_string()])
        .await;

    // Check error structure
    for error in &result.errors {
        // All errors should have an error_type
        assert!(
            !error.error_type.is_empty(),
            "Error type should not be empty"
        );

        // Path should be non-empty for element errors
        if error.error_type == "FS1001" {
            assert!(
                !error.path.is_empty(),
                "Unknown element error should have a path"
            );
        }
    }
}

/// Test that empty object validates (as minimal resource)
#[tokio::test]
async fn test_empty_patient_validation() {
    let validator = create_r4_validator();

    let empty_patient = json!({
        "resourceType": "Patient"
    });

    let result = validator
        .validate(&empty_patient, vec!["Patient".to_string()])
        .await;

    // Empty patient is technically valid (no required fields in base Patient)
    println!(
        "Empty patient validation: valid={}, errors={}",
        result.valid,
        result.errors.len()
    );
}

/// Test validation with null values
#[tokio::test]
async fn test_null_value_handling() {
    let validator = create_r4_validator();

    let patient_with_null = json!({
        "resourceType": "Patient",
        "id": null,
        "name": null
    });

    let result = validator
        .validate(&patient_with_null, vec!["Patient".to_string()])
        .await;

    // Should handle nulls gracefully (not panic)
    println!(
        "Null values: valid={}, errors={}",
        result.valid,
        result.errors.len()
    );
}

/// Test validation with deeply nested structures
#[tokio::test]
async fn test_deeply_nested_validation() {
    let validator = create_r4_validator();

    let nested_patient = json!({
        "resourceType": "Patient",
        "name": [{
            "family": "Test",
            "given": ["First", "Middle"],
            "prefix": ["Dr"],
            "suffix": ["Jr"],
            "period": {
                "start": "2020-01-01"
            }
        }],
        "address": [{
            "line": ["123 Main St", "Apt 4"],
            "city": "Boston",
            "state": "MA",
            "postalCode": "02101"
        }],
        "contact": [{
            "name": {
                "family": "Contact"
            },
            "telecom": [{
                "system": "phone",
                "value": "555-1234"
            }]
        }]
    });

    let result = validator
        .validate(&nested_patient, vec!["Patient".to_string()])
        .await;

    println!(
        "Nested patient: valid={}, errors={}",
        result.valid,
        result.errors.len()
    );

    // Should not crash on deep nesting
    assert!(
        result.errors.len() < 100,
        "Too many errors for nested structure"
    );
}

/// Test validation with large arrays
#[tokio::test]
async fn test_large_array_validation() {
    let validator = create_r4_validator();

    // Create patient with many names
    let names: Vec<Value> = (0..100)
        .map(|i| {
            json!({
                "family": format!("Family{}", i),
                "given": [format!("Given{}", i)]
            })
        })
        .collect();

    let large_patient = json!({
        "resourceType": "Patient",
        "name": names
    });

    let result = validator
        .validate(&large_patient, vec!["Patient".to_string()])
        .await;

    println!(
        "Large array patient: valid={}, errors={}",
        result.valid,
        result.errors.len()
    );

    // Should handle large arrays
    assert!(
        result.valid || result.errors.len() < 500,
        "Too many errors for large array"
    );
}
