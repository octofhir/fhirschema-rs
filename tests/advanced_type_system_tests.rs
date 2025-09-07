// Integration tests for the advanced type system with complex StructureDefinitions

use serde_json::json;
use std::sync::Arc;
use tokio;

use octofhir_fhirschema::core::{FhirSchemaConfig, FhirSchemaManager, ResolutionContext};
use octofhir_fhirschema::types::{
    ChoiceTypeResolver, PathNavigator, TypeHierarchyBuilder, TypeResolver,
};
use octofhir_fhirschema::{FhirSchemaError, Result};

/// Test the TypeResolver with complex choice types
#[tokio::test]
async fn test_type_resolver_with_choice_types() -> Result<()> {
    let canonical_manager = create_test_canonical_manager().await?;
    let type_resolver = Arc::new(TypeResolver::new(Arc::clone(&canonical_manager)).await?);

    let context = ResolutionContext::new("Patient").with_resource_type("Patient");

    // Test resolving choice types
    let resolved_types = type_resolver
        .resolve_choice_type("value", "String", &context)
        .await?;

    assert!(!resolved_types.is_empty());
    assert!(resolved_types.iter().any(|t| t.type_name == "string"));

    // Test resolving complex choice types
    let resolved_types = type_resolver
        .resolve_choice_type("onset", "", &context)
        .await?;

    assert!(!resolved_types.is_empty());
    // Should contain dateTime, Age, Period, Range, string
    assert!(resolved_types.len() >= 3);

    println!("✓ TypeResolver choice type resolution test passed");
    Ok(())
}

/// Test the ChoiceTypeResolver with context patterns
#[tokio::test]
async fn test_choice_type_resolver_context_patterns() -> Result<()> {
    let canonical_manager = create_test_canonical_manager().await?;
    let choice_resolver = ChoiceTypeResolver::new(Arc::clone(&canonical_manager)).await?;

    // Test Patient context
    let patient_context = ResolutionContext::new("Patient").with_resource_type("Patient");

    let resolved_types = choice_resolver
        .resolve_with_context("multipleBirth", "", &patient_context)
        .await?;

    assert!(!resolved_types.is_empty());
    // Should prefer boolean and integer for multipleBirth
    assert!(resolved_types.iter().any(|t| t.type_name == "boolean"));

    // Test Observation context
    let obs_context = ResolutionContext::new("Observation").with_resource_type("Observation");

    let resolved_types = choice_resolver
        .resolve_with_context("value", "", &obs_context)
        .await?;

    assert!(!resolved_types.is_empty());
    // Should prefer Quantity, CodeableConcept for Observation.value
    assert!(resolved_types.iter().any(|t| t.type_name == "Quantity"));

    println!("✓ ChoiceTypeResolver context patterns test passed");
    Ok(())
}

/// Test the TypeHierarchyBuilder with FHIR type relationships
#[tokio::test]
async fn test_type_hierarchy_builder() -> Result<()> {
    let canonical_manager = create_test_canonical_manager().await?;
    let hierarchy_builder = TypeHierarchyBuilder::new(Arc::clone(&canonical_manager)).await?;

    let context = ResolutionContext::new("test");

    // Test basic hierarchy
    let hierarchy = hierarchy_builder
        .build_hierarchy("Patient", &context)
        .await?;
    assert!(hierarchy.contains(&"Patient".to_string()));
    assert!(hierarchy.contains(&"DomainResource".to_string()));
    assert!(hierarchy.contains(&"Resource".to_string()));

    // Test subtype relationships
    let is_subtype = hierarchy_builder
        .is_subtype("Patient", "Resource", &context)
        .await?;
    assert!(is_subtype);

    let is_not_subtype = hierarchy_builder
        .is_subtype("Patient", "Observation", &context)
        .await?;
    assert!(!is_not_subtype);

    // Test common ancestor
    let common_ancestor = hierarchy_builder
        .get_common_ancestor("Patient", "Organization", &context)
        .await?;
    assert_eq!(common_ancestor, Some("DomainResource".to_string()));

    println!("✓ TypeHierarchyBuilder test passed");
    Ok(())
}

/// Test the PathNavigator with complex FHIR paths
#[tokio::test]
async fn test_path_navigator_complex_paths() -> Result<()> {
    let canonical_manager = create_test_canonical_manager().await?;
    let type_resolver = Arc::new(TypeResolver::new(Arc::clone(&canonical_manager)).await?);
    let path_navigator =
        PathNavigator::new(Arc::clone(&type_resolver), Arc::clone(&canonical_manager)).await?;

    let context = ResolutionContext::new("Patient").with_resource_type("Patient");

    // Test simple path navigation
    let result = path_navigator
        .navigate_path("Patient.name", &context)
        .await?;

    assert!(result.is_valid);
    assert_eq!(result.resolved_type, "HumanName");

    // Test choice element path
    let result = path_navigator
        .navigate_path("Patient.multipleBirthBoolean", &context)
        .await?;

    assert!(result.is_valid);
    assert_eq!(result.resolved_type, "boolean");

    // Test path parsing
    let fhir_path = path_navigator.parse_fhir_path("Patient.contact[0].relationship")?;

    assert_eq!(fhir_path.segments.len(), 3);
    assert_eq!(fhir_path.segments[1].name, "contact");
    assert_eq!(fhir_path.segments[1].array_index, Some(0));
    assert_eq!(fhir_path.segments[2].name, "relationship");

    println!("✓ PathNavigator complex paths test passed");
    Ok(())
}

/// Test type inference capabilities
#[tokio::test]
async fn test_type_inference() -> Result<()> {
    let canonical_manager = create_test_canonical_manager().await?;
    let type_resolver = Arc::new(TypeResolver::new(Arc::clone(&canonical_manager)).await?);
    let path_navigator =
        PathNavigator::new(Arc::clone(&type_resolver), Arc::clone(&canonical_manager)).await?;

    let context = ResolutionContext::new("Patient").with_resource_type("Patient");

    // Test pattern-based inference
    let inferred_type = path_navigator
        .infer_element_type("Patient", "birthDate", &context)
        .await?;
    assert_eq!(inferred_type, "date");

    let inferred_type = path_navigator
        .infer_element_type("Patient", "active", &context)
        .await?;
    assert_eq!(inferred_type, "boolean");

    let inferred_type = path_navigator
        .infer_element_type("Observation", "valueQuantity", &context)
        .await?;
    assert_eq!(inferred_type, "Quantity");

    println!("✓ Type inference test passed");
    Ok(())
}

/// Test integration between all type system components
#[tokio::test]
async fn test_integrated_type_system() -> Result<()> {
    let config = FhirSchemaConfig::default();
    let canonical_manager = create_test_canonical_manager().await?;
    let manager = FhirSchemaManager::new(config, canonical_manager).await?;

    // Create a complex StructureDefinition for testing
    let complex_structure_def = create_complex_structure_definition();

    // Test conversion
    let result = manager
        .convert_structure_definition(complex_structure_def)
        .await?;

    assert!(result.success);
    assert!(result.schema.is_some());

    let schema = result.schema.unwrap();
    assert_eq!(schema.title, "TestComplexResource");
    assert!(!schema.properties.is_empty());

    // Test that choice types were properly resolved
    assert!(schema.properties.contains_key("valueString"));
    assert!(schema.properties.contains_key("valueQuantity"));
    assert!(schema.properties.contains_key("valueCodeableConcept"));

    println!("✓ Integrated type system test passed");
    Ok(())
}

/// Test performance with large batches
#[tokio::test]
async fn test_type_system_performance() -> Result<()> {
    let canonical_manager = create_test_canonical_manager().await?;
    let type_resolver = Arc::new(TypeResolver::new(Arc::clone(&canonical_manager)).await?);

    let start = std::time::Instant::now();

    // Preload common types
    type_resolver.preload_common_types().await?;

    let preload_duration = start.elapsed();
    println!("Type preloading took: {:?}", preload_duration);

    // Test batch type resolution
    let type_requests = vec![
        ("Patient".to_string(), ResolutionContext::new("Patient")),
        (
            "Observation".to_string(),
            ResolutionContext::new("Observation"),
        ),
        ("Condition".to_string(), ResolutionContext::new("Condition")),
        ("Procedure".to_string(), ResolutionContext::new("Procedure")),
        (
            "MedicationRequest".to_string(),
            ResolutionContext::new("MedicationRequest"),
        ),
    ];

    let batch_start = std::time::Instant::now();
    let resolved_types = type_resolver.resolve_types_batch(type_requests).await?;
    let batch_duration = batch_start.elapsed();

    assert_eq!(resolved_types.len(), 5);
    println!("Batch type resolution took: {:?}", batch_duration);

    // Verify cache performance
    let cache_stats = type_resolver.get_cache_stats().await;
    println!(
        "Cache stats: {} entries out of {} capacity",
        cache_stats.0, cache_stats.1
    );

    println!("✓ Type system performance test passed");
    Ok(())
}

/// Test error handling and recovery
#[tokio::test]
async fn test_type_system_error_handling() -> Result<()> {
    let canonical_manager = create_test_canonical_manager().await?;
    let type_resolver = Arc::new(TypeResolver::new(Arc::clone(&canonical_manager)).await?);
    let path_navigator =
        PathNavigator::new(Arc::clone(&type_resolver), Arc::clone(&canonical_manager)).await?;

    let context = ResolutionContext::new("InvalidResource");

    // Test with invalid path
    let result = path_navigator
        .navigate_path("InvalidResource.nonExistentProperty", &context)
        .await?;

    assert!(!result.is_valid);
    assert!(!result.validation_messages.is_empty());

    // Test path syntax validation
    let is_valid = path_navigator
        .validate_path_syntax("Patient.name.given[0]")
        .await?;
    assert!(is_valid);

    let is_invalid = path_navigator.validate_path_syntax("Patient..name").await?;
    assert!(!is_invalid);

    println!("✓ Type system error handling test passed");
    Ok(())
}

// Helper functions

async fn create_test_canonical_manager() -> Result<Arc<octofhir_canonical_manager::CanonicalManager>>
{
    let config = octofhir_canonical_manager::FcmConfig::default();
    let manager = octofhir_canonical_manager::CanonicalManager::new(config)
        .await
        .map_err(|e| FhirSchemaError::conversion_failed("CanonicalManager", &e.to_string()))?;
    Ok(Arc::new(manager))
}

fn create_complex_structure_definition() -> serde_json::Value {
    json!({
        "resourceType": "StructureDefinition",
        "id": "test-complex-resource",
        "url": "http://example.org/fhir/StructureDefinition/TestComplexResource",
        "name": "TestComplexResource",
        "title": "Test Complex Resource",
        "status": "active",
        "kind": "resource",
        "abstract": false,
        "type": "TestComplexResource",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/DomainResource",
        "derivation": "specialization",
        "differential": {
            "element": [
                {
                    "id": "TestComplexResource",
                    "path": "TestComplexResource",
                    "definition": "A complex test resource with various element types",
                    "min": 0,
                    "max": "*"
                },
                {
                    "id": "TestComplexResource.identifier",
                    "path": "TestComplexResource.identifier",
                    "definition": "Identifier for this resource",
                    "min": 0,
                    "max": "*",
                    "type": [
                        {
                            "code": "Identifier"
                        }
                    ]
                },
                {
                    "id": "TestComplexResource.value[x]",
                    "path": "TestComplexResource.value[x]",
                    "definition": "Value with choice type",
                    "min": 0,
                    "max": "1",
                    "type": [
                        {"code": "string"},
                        {"code": "Quantity"},
                        {"code": "CodeableConcept"},
                        {"code": "boolean"},
                        {"code": "integer"},
                        {"code": "dateTime"}
                    ]
                },
                {
                    "id": "TestComplexResource.complex",
                    "path": "TestComplexResource.complex",
                    "definition": "Complex nested element",
                    "min": 0,
                    "max": "*",
                    "type": [
                        {
                            "code": "BackboneElement"
                        }
                    ]
                },
                {
                    "id": "TestComplexResource.complex.name",
                    "path": "TestComplexResource.complex.name",
                    "definition": "Name of the complex element",
                    "min": 1,
                    "max": "1",
                    "type": [
                        {
                            "code": "string"
                        }
                    ]
                },
                {
                    "id": "TestComplexResource.complex.reference",
                    "path": "TestComplexResource.complex.reference",
                    "definition": "Reference to another resource",
                    "min": 0,
                    "max": "1",
                    "type": [
                        {
                            "code": "Reference",
                            "targetProfile": [
                                "http://hl7.org/fhir/StructureDefinition/Patient",
                                "http://hl7.org/fhir/StructureDefinition/Practitioner"
                            ]
                        }
                    ]
                },
                {
                    "id": "TestComplexResource.sliced",
                    "path": "TestComplexResource.sliced",
                    "definition": "Sliced element",
                    "min": 0,
                    "max": "*",
                    "type": [
                        {
                            "code": "Extension"
                        }
                    ],
                    "slicing": {
                        "discriminator": [
                            {
                                "type": "value",
                                "path": "url"
                            }
                        ],
                        "description": "Sliced by extension URL",
                        "ordered": false,
                        "rules": "open"
                    }
                },
                {
                    "id": "TestComplexResource.constrainedString",
                    "path": "TestComplexResource.constrainedString",
                    "definition": "String with constraints",
                    "min": 0,
                    "max": "1",
                    "type": [
                        {
                            "code": "string"
                        }
                    ],
                    "constraint": [
                        {
                            "key": "test-1",
                            "severity": "error",
                            "human": "Must be at least 3 characters",
                            "expression": "length() >= 3"
                        }
                    ],
                    "binding": {
                        "strength": "required",
                        "valueSet": "http://example.org/fhir/ValueSet/test-codes"
                    }
                }
            ]
        }
    })
}
