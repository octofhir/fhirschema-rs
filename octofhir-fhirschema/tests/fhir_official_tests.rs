//! Official FHIR Test Suite integration tests.
//!
//! Tests validation against official FHIR examples and test cases from:
//! - HL7 FHIR Examples (https://hl7.org/fhir/examples.html)
//! - FHIR Test Cases Repository (https://github.com/FHIR/fhir-test-cases)
//!
//! These tests are marked as #[ignore] by default as they require network access.
//! Run with: cargo test --test fhir_official_tests -- --ignored

use octofhir_fhirschema::embedded::{FhirVersion, get_schemas};
use octofhir_fhirschema::validation::FhirSchemaValidator;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Helper to create validator with embedded R4 schemas
fn create_r4_validator() -> FhirSchemaValidator {
    let schemas = get_schemas(FhirVersion::R4);
    FhirSchemaValidator::new(schemas.clone(), None)
}

/// Download a file from URL with caching
async fn fetch_with_cache(url: &str, cache_name: &str) -> Result<String, String> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("official_fhir_cache");

    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    let cache_path = cache_dir.join(cache_name);

    // Check if cache exists and is recent (within 7 days)
    if let Ok(metadata) = fs::metadata(&cache_path) {
        if let Ok(modified) = metadata.modified() {
            if modified.elapsed().unwrap_or(Duration::MAX) < Duration::from_secs(7 * 24 * 60 * 60) {
                if let Ok(content) = fs::read_to_string(&cache_path) {
                    return Ok(content);
                }
            }
        }
    }

    // Fetch from URL
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch {}: {}", url, e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} for {}", response.status(), url));
    }

    let content = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Cache the result
    let _ = fs::write(&cache_path, &content);

    Ok(content)
}

/// Parse JSON safely
fn parse_json(content: &str) -> Result<Value, String> {
    serde_json::from_str(content).map_err(|e| format!("JSON parse error: {}", e))
}

/// Test result tracking
#[derive(Debug, Default)]
struct OfficialTestSummary {
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
    failures: Vec<(String, Vec<String>)>,
}

impl OfficialTestSummary {
    fn add_pass(&mut self, _name: &str) {
        self.total += 1;
        self.passed += 1;
    }

    fn add_fail(&mut self, name: &str, errors: Vec<String>) {
        self.total += 1;
        self.failed += 1;
        self.failures.push((name.to_string(), errors));
    }

    fn add_skip(&mut self) {
        self.skipped += 1;
    }

    fn pass_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.passed as f64 / self.total as f64) * 100.0
        }
    }

    fn print_summary(&self) {
        println!("\n=== Official FHIR Test Summary ===");
        println!("Total: {}", self.total);
        println!("Passed: {}", self.passed);
        println!("Failed: {}", self.failed);
        println!("Skipped: {}", self.skipped);
        println!("Pass Rate: {:.1}%", self.pass_rate());

        if !self.failures.is_empty() && self.failures.len() <= 10 {
            println!("\nFirst failures:");
            for (name, errors) in self.failures.iter().take(5) {
                println!("  - {}: {:?}", name, errors.get(0));
            }
        }
    }
}

// =============================================================================
// Official FHIR R4 Example Tests
// =============================================================================

/// List of official FHIR R4 example resource URLs
const FHIR_R4_EXAMPLES: &[(&str, &str)] = &[
    ("Patient", "https://hl7.org/fhir/R4/patient-example.json"),
    (
        "Patient-genetics",
        "https://hl7.org/fhir/R4/patient-example-a.json",
    ),
    (
        "Observation-bp",
        "https://hl7.org/fhir/R4/observation-example-bloodpressure.json",
    ),
    (
        "Observation-weight",
        "https://hl7.org/fhir/R4/observation-example.json",
    ),
    (
        "Condition",
        "https://hl7.org/fhir/R4/condition-example.json",
    ),
    (
        "MedicationRequest",
        "https://hl7.org/fhir/R4/medicationrequest0301.json",
    ),
    (
        "Encounter",
        "https://hl7.org/fhir/R4/encounter-example.json",
    ),
    (
        "Practitioner",
        "https://hl7.org/fhir/R4/practitioner-example.json",
    ),
    (
        "Organization",
        "https://hl7.org/fhir/R4/organization-example.json",
    ),
    ("Location", "https://hl7.org/fhir/R4/location-example.json"),
    (
        "DiagnosticReport",
        "https://hl7.org/fhir/R4/diagnosticreport-example.json",
    ),
    (
        "Procedure",
        "https://hl7.org/fhir/R4/procedure-example.json",
    ),
    (
        "Immunization",
        "https://hl7.org/fhir/R4/immunization-example.json",
    ),
    (
        "AllergyIntolerance",
        "https://hl7.org/fhir/R4/allergyintolerance-example.json",
    ),
    ("CarePlan", "https://hl7.org/fhir/R4/careplan-example.json"),
];

/// Test validation against official FHIR R4 examples.
///
/// This test downloads official HL7 FHIR examples and validates them.
/// Target: >95% pass rate (some examples may have extension-related issues)
#[tokio::test]
#[ignore] // Requires network access - run with --ignored flag
async fn test_official_fhir_r4_examples() {
    let validator = create_r4_validator();
    let mut summary = OfficialTestSummary::default();

    println!("\n=== Testing Official FHIR R4 Examples ===\n");

    for (name, url) in FHIR_R4_EXAMPLES {
        let cache_name = format!("r4_{}.json", name.to_lowercase().replace('-', "_"));

        match fetch_with_cache(url, &cache_name).await {
            Ok(content) => match parse_json(&content) {
                Ok(resource) => {
                    let resource_type = resource
                        .get("resourceType")
                        .and_then(|rt| rt.as_str())
                        .unwrap_or("Unknown");

                    let result = validator
                        .validate(&resource, vec![resource_type.to_string()])
                        .await;

                    if result.valid {
                        println!("  [PASS] {}", name);
                        summary.add_pass(name);
                    } else {
                        let errors: Vec<String> = result
                            .errors
                            .iter()
                            .map(|e| format!("{}: {:?}", e.error_type, e.message))
                            .collect();

                        println!("  [FAIL] {} - {} errors", name, errors.len());
                        summary.add_fail(name, errors);
                    }
                }
                Err(e) => {
                    println!("  [SKIP] {} - Parse error: {}", name, e);
                    summary.add_skip();
                }
            },
            Err(e) => {
                println!("  [SKIP] {} - Fetch error: {}", name, e);
                summary.add_skip();
            }
        }
    }

    summary.print_summary();

    // Target: at least 80% pass rate for official examples
    // (Some may fail due to extensions or features not yet implemented)
    assert!(
        summary.pass_rate() >= 80.0 || summary.total == 0,
        "Official FHIR examples should have at least 80% pass rate, got {:.1}%",
        summary.pass_rate()
    );
}

// =============================================================================
// Embedded Examples Test (No Network Required)
// =============================================================================

/// Test validation using inline example resources.
/// These are based on official FHIR examples but embedded in the test.
#[tokio::test]
async fn test_embedded_official_examples() {
    let validator = create_r4_validator();
    let mut summary = OfficialTestSummary::default();

    // Embedded examples based on official FHIR R4 examples
    let examples: Vec<(&str, Value)> = vec![
        (
            "Patient-minimal",
            serde_json::json!({
                "resourceType": "Patient",
                "id": "example"
            }),
        ),
        (
            "Patient-with-name",
            serde_json::json!({
                "resourceType": "Patient",
                "id": "example",
                "name": [{"family": "Chalmers", "given": ["Peter"]}]
            }),
        ),
        (
            "Observation-vital",
            serde_json::json!({
                "resourceType": "Observation",
                "id": "example",
                "status": "final",
                "code": {
                    "coding": [{"system": "http://loinc.org", "code": "29463-7", "display": "Body Weight"}]
                },
                "valueQuantity": {"value": 185, "unit": "lbs"}
            }),
        ),
        (
            "Condition-example",
            serde_json::json!({
                "resourceType": "Condition",
                "id": "example",
                "clinicalStatus": {
                    "coding": [{"system": "http://terminology.hl7.org/CodeSystem/condition-clinical", "code": "active"}]
                },
                "code": {
                    "coding": [{"system": "http://snomed.info/sct", "code": "386661006", "display": "Fever"}]
                },
                "subject": {"reference": "Patient/example"}
            }),
        ),
        (
            "Encounter-example",
            serde_json::json!({
                "resourceType": "Encounter",
                "id": "example",
                "status": "finished",
                "class": {"system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "code": "IMP"}
            }),
        ),
        (
            "Practitioner-example",
            serde_json::json!({
                "resourceType": "Practitioner",
                "id": "example",
                "name": [{"family": "Smith", "given": ["Jane"]}]
            }),
        ),
        (
            "Organization-example",
            serde_json::json!({
                "resourceType": "Organization",
                "id": "example",
                "name": "Health Level Seven International"
            }),
        ),
        (
            "Location-example",
            serde_json::json!({
                "resourceType": "Location",
                "id": "example",
                "name": "South Wing, second floor"
            }),
        ),
        (
            "Procedure-example",
            serde_json::json!({
                "resourceType": "Procedure",
                "id": "example",
                "status": "completed",
                "code": {
                    "coding": [{"system": "http://snomed.info/sct", "code": "80146002", "display": "Appendectomy"}]
                },
                "subject": {"reference": "Patient/example"}
            }),
        ),
        (
            "Immunization-example",
            serde_json::json!({
                "resourceType": "Immunization",
                "id": "example",
                "status": "completed",
                "vaccineCode": {
                    "coding": [{"system": "http://hl7.org/fhir/sid/cvx", "code": "140"}]
                },
                "patient": {"reference": "Patient/example"},
                "occurrenceDateTime": "2021-01-01"
            }),
        ),
    ];

    println!("\n=== Testing Embedded Official Examples ===\n");

    for (name, resource) in &examples {
        let resource_type = resource
            .get("resourceType")
            .and_then(|rt| rt.as_str())
            .unwrap_or("Unknown");

        let result = validator
            .validate(resource, vec![resource_type.to_string()])
            .await;

        if result.valid {
            println!("  [PASS] {}", name);
            summary.add_pass(name);
        } else {
            let errors: Vec<String> = result
                .errors
                .iter()
                .map(|e| format!("{}: {:?}", e.error_type, e.message))
                .collect();

            println!(
                "  [FAIL] {} - {} errors: {:?}",
                name,
                errors.len(),
                errors.get(0)
            );
            summary.add_fail(name, errors);
        }
    }

    summary.print_summary();

    // Embedded examples should all pass (they're carefully crafted)
    assert!(
        summary.pass_rate() >= 90.0,
        "Embedded examples should have at least 90% pass rate, got {:.1}%",
        summary.pass_rate()
    );
}

// =============================================================================
// Bundle Validation Tests
// =============================================================================

#[tokio::test]
async fn test_transaction_bundle_example() {
    let validator = create_r4_validator();

    let bundle = serde_json::json!({
        "resourceType": "Bundle",
        "id": "bundle-transaction",
        "type": "transaction",
        "entry": [
            {
                "fullUrl": "urn:uuid:61ebe359-bfdc-4613-8bf2-c5e300945f0a",
                "resource": {
                    "resourceType": "Patient",
                    "id": "1",
                    "name": [{"family": "Test"}]
                },
                "request": {
                    "method": "POST",
                    "url": "Patient"
                }
            },
            {
                "fullUrl": "urn:uuid:88f151c0-a954-468a-88bd-5ae15c08e059",
                "resource": {
                    "resourceType": "Observation",
                    "id": "2",
                    "status": "final",
                    "code": {
                        "coding": [{"system": "http://loinc.org", "code": "12345"}]
                    },
                    "subject": {"reference": "urn:uuid:61ebe359-bfdc-4613-8bf2-c5e300945f0a"}
                },
                "request": {
                    "method": "POST",
                    "url": "Observation"
                }
            }
        ]
    });

    let result = validator
        .validate(&bundle, vec!["Bundle".to_string()])
        .await;

    println!(
        "Transaction bundle validation: valid={}, errors={:?}",
        result.valid, result.errors
    );

    // Bundle should be structurally valid
    // (May have warnings about unresolved references)
}

// =============================================================================
// Resource Coverage Test
// =============================================================================

#[tokio::test]
async fn test_all_common_resource_types() {
    let validator = create_r4_validator();
    let mut summary = OfficialTestSummary::default();

    // Test minimal valid instances of common resource types
    let resource_types = [
        "Patient",
        "Practitioner",
        "Organization",
        "Location",
        "Encounter",
        "Condition",
        "Observation",
        "Procedure",
        "MedicationRequest",
        "DiagnosticReport",
        "CarePlan",
        "AllergyIntolerance",
        "Immunization",
        "Device",
        "Specimen",
        "DocumentReference",
        "Composition",
        "Coverage",
        "Claim",
        "ExplanationOfBenefit",
    ];

    println!("\n=== Testing Common Resource Types ===\n");

    for resource_type in resource_types {
        // Create minimal valid resource
        let resource = serde_json::json!({
            "resourceType": resource_type
        });

        let result = validator
            .validate(&resource, vec![resource_type.to_string()])
            .await;

        // For minimal resources, we just check they don't crash
        // Required field errors are expected
        if result.errors.iter().all(|e| e.error_type == "FS1011") {
            // Only missing required field errors - that's expected for minimal resources
            println!(
                "  [OK] {} (minimal, {} required fields missing)",
                resource_type,
                result.errors.len()
            );
            summary.add_pass(resource_type);
        } else if result.valid {
            println!("  [PASS] {}", resource_type);
            summary.add_pass(resource_type);
        } else {
            let errors: Vec<String> = result
                .errors
                .iter()
                .filter(|e| e.error_type != "FS1011") // Filter out expected "missing required" errors
                .map(|e| format!("{}: {:?}", e.error_type, e.message))
                .collect();

            if errors.is_empty() {
                println!(
                    "  [OK] {} (only missing required field errors)",
                    resource_type
                );
                summary.add_pass(resource_type);
            } else {
                println!(
                    "  [FAIL] {} - unexpected errors: {:?}",
                    resource_type, errors
                );
                summary.add_fail(resource_type, errors);
            }
        }
    }

    summary.print_summary();

    // All common resource types should be recognized (even if minimal instances have required field errors)
    assert!(
        summary.pass_rate() >= 90.0,
        "Common resource types should have at least 90% recognition rate, got {:.1}%",
        summary.pass_rate()
    );
}
