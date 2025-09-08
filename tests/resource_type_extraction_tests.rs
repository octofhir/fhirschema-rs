use octofhir_fhir_model::provider::ModelProvider;
use octofhir_fhirschema::provider::FhirSchemaModelProvider;
use serde_json::json;

#[tokio::test]
async fn test_resource_type_extraction_and_o1_check() {
    // Create a FHIR R4 model provider
    let provider = FhirSchemaModelProvider::r4().await.unwrap();

    // Initially, no resource types should be available (empty schema storage)
    let initial_types = provider.get_supported_resource_types().await.unwrap();
    let initial_count = initial_types.len();
    println!("Initial resource types count: {initial_count}");

    // Test O(1) existence check - should be false for non-existent type
    assert!(!provider.resource_type_exists("Patient"));

    // Create a mock Patient StructureDefinition
    let patient_structdef = json!({
        "resourceType": "StructureDefinition",
        "url": "http://hl7.org/fhir/StructureDefinition/Patient",
        "name": "Patient",
        "title": "Patient",
        "status": "active",
        "kind": "resource",
        "type": "Patient",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/DomainResource",
        "differential": {
            "element": [
                {
                    "id": "Patient",
                    "path": "Patient",
                    "definition": "Demographics and other administrative information about an individual"
                }
            ]
        }
    });

    // Convert and store the StructureDefinition
    let conversion_result = provider
        .schema_manager()
        .convert_structure_definition(patient_structdef)
        .await
        .unwrap();

    assert!(conversion_result.is_success());

    // Store the converted schema
    let schema = conversion_result.schema.unwrap();
    provider
        .schema_manager()
        .store_schema("http://hl7.org/fhir/StructureDefinition/Patient", schema)
        .await
        .unwrap();

    // Refresh resource types cache to pick up the new schema
    provider.refresh_resource_types().await.unwrap();

    // Now Patient should exist - O(1) check
    assert!(provider.resource_type_exists("Patient"));

    // Test that non-existent type still returns false
    assert!(!provider.resource_type_exists("NonExistentResource"));

    // Get all available resource types - should include Patient now
    let updated_types = provider.get_supported_resource_types().await.unwrap();
    assert!(updated_types.contains(&"Patient".to_string()));

    // Test the ModelProvider trait methods
    let supported_types = provider.get_supported_resource_types().await.unwrap();
    assert!(supported_types.contains(&"Patient".to_string()));

    // Test cache stats include resource types count
    let stats = provider.get_cache_stats().await.unwrap();
    assert!(stats["cached_resource_types"].as_u64().unwrap() >= 1);

    println!("✅ All tests passed! Resource type extraction and O(1) checking works correctly.");
}

#[tokio::test]
async fn test_resource_type_url_extraction() {
    use octofhir_fhirschema::provider::fhir_model_provider::FhirSchemaModelProvider;

    // Test URL extraction logic
    let test_cases = vec![
        (
            "http://hl7.org/fhir/StructureDefinition/Patient",
            Some("Patient".to_string()),
        ),
        (
            "http://hl7.org/fhir/StructureDefinition/Observation",
            Some("Observation".to_string()),
        ),
        (
            "http://hl7.org/fhir/StructureDefinition/Practitioner",
            Some("Practitioner".to_string()),
        ),
        (
            "http://example.com/StructureDefinition/CustomResource",
            Some("CustomResource".to_string()),
        ),
        ("http://hl7.org/fhir/StructureDefinition/", None), // Empty after last slash
        ("http://hl7.org/fhir/StructureDefinition/string", None), // Lowercase - not a resource
        ("not-a-structdef-url", None),
    ];

    for (url, expected) in test_cases {
        let result = FhirSchemaModelProvider::extract_resource_type_from_url(url);
        assert_eq!(result, expected, "Failed for URL: {url}");
    }

    println!("✅ URL extraction tests passed!");
}

#[tokio::test]
async fn test_multiple_resource_types() {
    let provider = FhirSchemaModelProvider::r4().await.unwrap();

    // Create multiple mock StructureDefinitions
    let resource_types = vec!["Patient", "Observation", "Practitioner", "Organization"];

    for resource_type in &resource_types {
        let structdef = json!({
            "resourceType": "StructureDefinition",
            "url": format!("http://hl7.org/fhir/StructureDefinition/{}", resource_type),
            "name": resource_type,
            "title": resource_type,
            "status": "active",
            "kind": "resource",
            "type": resource_type,
            "baseDefinition": "http://hl7.org/fhir/StructureDefinition/DomainResource"
        });

        let conversion_result = provider
            .schema_manager()
            .convert_structure_definition(structdef)
            .await
            .unwrap();

        let schema = conversion_result.schema.unwrap();
        provider
            .schema_manager()
            .store_schema(
                &format!("http://hl7.org/fhir/StructureDefinition/{resource_type}"),
                schema,
            )
            .await
            .unwrap();
    }

    // Refresh cache
    provider.refresh_resource_types().await.unwrap();

    // Test all resource types exist
    for resource_type in &resource_types {
        assert!(
            provider.resource_type_exists(resource_type),
            "Resource type {resource_type} should exist"
        );
    }

    // Test non-existent type
    assert!(!provider.resource_type_exists("NonExistentType"));

    // Check that all types are in the list
    let available_types = provider.get_supported_resource_types().await.unwrap();
    for resource_type in &resource_types {
        assert!(available_types.contains(&resource_type.to_string()));
    }

    println!(
        "✅ Multiple resource types test passed! Found {} resource types",
        available_types.len()
    );
}
