//! Fixture-based validation tests for FHIR Schema validator.
//!
//! Tests validation against JSON fixtures organized by category:
//! - r4/base/valid - Valid R4 resources
//! - r4/base/invalid - Invalid R4 resources
//! - r4/profiles - Profile-specific resources
//! - r4/constraints - Constraint validation tests
//! - r4/slicing - Slicing validation tests

mod common;

use common::{FixtureTestResult, FixtureTestSummary, load_all_fixtures};
use octofhir_fhirschema::embedded::{FhirVersion, get_schemas};
use octofhir_fhirschema::validation::FhirValidator;
use serde_json::Value;

/// Helper to create validator with embedded R4 schemas
fn create_r4_validator() -> FhirValidator {
    let schemas = get_schemas(FhirVersion::R4);
    FhirValidator::from_schemas(schemas.clone(), None)
}

/// Get resource type from JSON
fn get_resource_type(resource: &Value) -> &str {
    resource
        .get("resourceType")
        .and_then(|rt| rt.as_str())
        .unwrap_or("Unknown")
}

// =============================================================================
// R4 Base Resource Validation Tests
// =============================================================================

#[tokio::test]
async fn test_valid_r4_base_resources() {
    let validator = create_r4_validator();
    let fixtures = load_all_fixtures("r4/base/valid");

    if fixtures.is_empty() {
        println!("No fixtures found in r4/base/valid - skipping test");
        return;
    }

    let mut summary = FixtureTestSummary::new();

    for (name, resource) in fixtures {
        let resource_type = get_resource_type(&resource);
        let result = validator
            .validate(&resource, vec![resource_type.to_string()])
            .await;

        let errors: Vec<String> = result
            .errors
            .iter()
            .map(|e| format!("{}: {:?}", e.error_type, e.message))
            .collect();

        summary.add(FixtureTestResult {
            fixture_name: name.clone(),
            expected_valid: true,
            actual_valid: result.valid,
            errors,
        });
    }

    summary.print_summary();
    assert!(
        summary.pass_rate() >= 80.0,
        "Valid fixtures should have at least 80% pass rate, got {:.1}%",
        summary.pass_rate()
    );
}

#[tokio::test]
async fn test_invalid_r4_base_resources() {
    let validator = create_r4_validator();
    let fixtures = load_all_fixtures("r4/base/invalid");

    if fixtures.is_empty() {
        println!("No fixtures found in r4/base/invalid - skipping test");
        return;
    }

    let mut summary = FixtureTestSummary::new();

    for (name, resource) in fixtures {
        let resource_type = get_resource_type(&resource);
        let result = validator
            .validate(&resource, vec![resource_type.to_string()])
            .await;

        // For invalid fixtures, we expect validation to fail (valid = false)
        let errors: Vec<String> = result
            .errors
            .iter()
            .map(|e| format!("{}: {:?}", e.error_type, e.message))
            .collect();

        summary.add(FixtureTestResult {
            fixture_name: name.clone(),
            expected_valid: false,
            actual_valid: result.valid,
            errors,
        });
    }

    summary.print_summary();
    assert!(
        summary.pass_rate() >= 50.0,
        "Invalid fixtures should be detected as invalid at least 50% of the time, got {:.1}%",
        summary.pass_rate()
    );
}

// =============================================================================
// Individual Resource Type Tests
// =============================================================================

#[tokio::test]
async fn test_patient_simple_fixture() {
    let validator = create_r4_validator();

    if let Some(patient) = common::try_load_fixture("r4/base/valid/patient_simple.json") {
        let result = validator
            .validate(&patient, vec!["Patient".to_string()])
            .await;

        assert!(
            result.valid,
            "Simple patient should be valid: {:?}",
            result.errors
        );
    } else {
        println!("Fixture not found - skipping");
    }
}

#[tokio::test]
async fn test_patient_minimal_fixture() {
    let validator = create_r4_validator();

    if let Some(patient) = common::try_load_fixture("r4/base/valid/patient_minimal.json") {
        let result = validator
            .validate(&patient, vec!["Patient".to_string()])
            .await;

        assert!(
            result.valid,
            "Minimal patient should be valid: {:?}",
            result.errors
        );
    } else {
        println!("Fixture not found - skipping");
    }
}

#[tokio::test]
async fn test_observation_simple_fixture() {
    let validator = create_r4_validator();

    if let Some(observation) = common::try_load_fixture("r4/base/valid/observation_simple.json") {
        let result = validator
            .validate(&observation, vec!["Observation".to_string()])
            .await;

        assert!(
            result.valid,
            "Simple observation should be valid: {:?}",
            result.errors
        );
    } else {
        println!("Fixture not found - skipping");
    }
}

#[tokio::test]
async fn test_encounter_simple_fixture() {
    let validator = create_r4_validator();

    if let Some(encounter) = common::try_load_fixture("r4/base/valid/encounter_simple.json") {
        let result = validator
            .validate(&encounter, vec!["Encounter".to_string()])
            .await;

        assert!(
            result.valid,
            "Simple encounter should be valid: {:?}",
            result.errors
        );
    } else {
        println!("Fixture not found - skipping");
    }
}

// =============================================================================
// Invalid Resource Detection Tests
// =============================================================================

#[tokio::test]
async fn test_patient_unknown_element_detected() {
    let validator = create_r4_validator();

    if let Some(patient) = common::try_load_fixture("r4/base/invalid/patient_unknown_element.json")
    {
        let result = validator
            .validate(&patient, vec!["Patient".to_string()])
            .await;

        assert!(
            !result.valid,
            "Patient with unknown element should be invalid"
        );

        // Check that we got an unknown element error
        let has_unknown_element_error = result.errors.iter().any(|e| {
            e.error_type == "FS1001" || e.message.as_ref().map_or(false, |m| m.contains("unknown"))
        });

        assert!(
            has_unknown_element_error,
            "Should have unknown element error, got: {:?}",
            result.errors
        );
    } else {
        println!("Fixture not found - skipping");
    }
}

#[tokio::test]
async fn test_patient_wrong_type_detected() {
    let validator = create_r4_validator();

    if let Some(patient) = common::try_load_fixture("r4/base/invalid/patient_wrong_type.json") {
        let result = validator
            .validate(&patient, vec!["Patient".to_string()])
            .await;

        // Should detect type errors
        assert!(
            !result.valid || !result.errors.is_empty(),
            "Patient with wrong types should have errors"
        );
    } else {
        println!("Fixture not found - skipping");
    }
}

#[tokio::test]
async fn test_observation_missing_required_detected() {
    let validator = create_r4_validator();

    if let Some(observation) =
        common::try_load_fixture("r4/base/invalid/observation_missing_required.json")
    {
        let result = validator
            .validate(&observation, vec!["Observation".to_string()])
            .await;

        assert!(
            !result.valid,
            "Observation missing required fields should be invalid"
        );

        // Should have missing required field errors (FS1011 = Missing Required Element)
        let has_required_error = result.errors.iter().any(|e| {
            e.error_type == "FS1011"
                || e.error_type == "FS1003"
                || e.message.as_ref().map_or(false, |m| {
                    m.contains("required") || m.contains("missing") || m.contains("Missing")
                })
        });

        assert!(
            has_required_error,
            "Should have required field error, got: {:?}",
            result.errors
        );
    } else {
        println!("Fixture not found - skipping");
    }
}

#[tokio::test]
async fn test_patient_array_expected_detected() {
    let validator = create_r4_validator();

    if let Some(patient) = common::try_load_fixture("r4/base/invalid/patient_array_expected.json") {
        let result = validator
            .validate(&patient, vec!["Patient".to_string()])
            .await;

        // Should detect that name should be an array
        assert!(
            !result.valid || !result.errors.is_empty(),
            "Patient with object instead of array should have errors"
        );
    } else {
        println!("Fixture not found - skipping");
    }
}

// =============================================================================
// Bundle/Slicing Tests
// =============================================================================

#[tokio::test]
async fn test_bundle_with_entries_fixture() {
    let validator = create_r4_validator();

    if let Some(bundle) = common::try_load_fixture("r4/slicing/bundle_with_entries.json") {
        let result = validator
            .validate(&bundle, vec!["Bundle".to_string()])
            .await;

        // Bundle validation may have some warnings but should be structurally valid
        println!(
            "Bundle validation result: valid={}, errors={:?}",
            result.valid, result.errors
        );
    } else {
        println!("Fixture not found - skipping");
    }
}

// =============================================================================
// Fixture Statistics Test
// =============================================================================

#[tokio::test]
async fn test_fixture_statistics() {
    println!("\n=== Fixture Statistics ===");

    let categories = [
        "r4/base/valid",
        "r4/base/invalid",
        "r4/profiles",
        "r4/constraints",
        "r4/slicing",
        "r4b/base/valid",
        "r4b/base/invalid",
    ];

    let mut total = 0;
    for category in categories {
        let fixtures = load_all_fixtures(category);
        println!("{}: {} fixtures", category, fixtures.len());
        total += fixtures.len();
    }

    println!("Total fixtures: {}", total);
}
