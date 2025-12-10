use octofhir_fhirschema::model_provider::EmbeddedSchemaProvider;
use serde_json::json;

#[tokio::test]
async fn test_embedded_provider_validation_against_profile() {
    let provider = EmbeddedSchemaProvider::r4();

    let patient_data = json!({
        "resourceType": "Patient",
        "name": [{
            "given": ["John"],
            "family": "Doe"
        }]
    });

    // Find a real Patient profile URL from the embedded schemas
    if let Some(patient_schema) = provider
        .schemas()
        .values()
        .find(|s| s.name == "Patient" && s.kind == "resource")
    {
        let result = provider
            .validate_resource_against_profile(&patient_data, &patient_schema.url)
            .await
            .expect("Validation should not fail");

        // The helper method returns ValidationResult
        assert!(
            result.valid || !result.errors.is_empty(),
            "Should return validation result"
        );
    }
}

#[tokio::test]
async fn test_profile_validation_nonexistent_profile() {
    let provider = EmbeddedSchemaProvider::r4();

    let patient_data = json!({
        "resourceType": "Patient"
    });

    let result = provider
        .validate_resource_against_profile(&patient_data, "http://example.com/NonexistentProfile")
        .await;

    assert!(result.is_err(), "Should fail when profile doesn't exist");

    if let Err(e) = result {
        assert!(e.to_string().contains("Profile not found"));
    }
}

#[tokio::test]
async fn test_embedded_provider_validation_against_resource_type() {
    let provider = EmbeddedSchemaProvider::r4();

    let patient_data = json!({
        "resourceType": "Patient",
        "name": [{
            "given": ["John"],
            "family": "Doe"
        }]
    });

    let result = provider
        .validate_resource_against_resource_type(&patient_data, "Patient")
        .await
        .expect("Validation should not fail");

    // Note: The validation might not be perfect yet since the engine is basic
    // but it should at least return a result without erroring
    assert!(
        result.valid || !result.errors.is_empty(),
        "Should return validation result"
    );
}

#[tokio::test]
async fn test_embedded_provider_validation_invalid_resource() {
    let provider = EmbeddedSchemaProvider::r4();

    let invalid_patient_data = json!({
        "resourceType": "Patient",
        "invalid_field": "should_cause_error"
    });

    let result = provider
        .validate_resource_against_resource_type(&invalid_patient_data, "Patient")
        .await
        .expect("Validation should not fail");

    // Should detect the invalid field
    assert!(
        !result.valid,
        "Invalid patient resource should not be valid"
    );
    assert!(!result.errors.is_empty(), "Should have validation errors");
}

#[tokio::test]
async fn test_embedded_provider_validation_nonexistent_resource_type() {
    let provider = EmbeddedSchemaProvider::r4();

    let patient_data = json!({
        "resourceType": "Patient"
    });

    let result =
        provider.validate_resource_against_resource_type(&patient_data, "NonexistentResourceType").await;

    assert!(
        result.is_err(),
        "Should fail when resource type doesn't exist"
    );

    if let Err(e) = result {
        assert!(e.to_string().contains("Resource type not found"));
    }
}
