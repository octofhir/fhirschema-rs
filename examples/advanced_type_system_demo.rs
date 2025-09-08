// Demo of the advanced type system integration in OctoFHIR FHIRSchema

use serde_json::json;

use octofhir_fhirschema::core::ResolutionContext;
use octofhir_fhirschema::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 OctoFHIR FHIRSchema Advanced Type System Demo");

    // Initialize the FhirSchemaManager with default configuration
    let config = FhirSchemaConfig::default();
    let canonical_manager = octofhir_canonical_manager::CanonicalManager::new(
        octofhir_canonical_manager::FcmConfig::default(),
    )
    .await
    .map_err(|e| FhirSchemaError::conversion_failed("CanonicalManager", &e.to_string()))?;

    let manager = FhirSchemaManager::new(config, canonical_manager).await?;

    // Demo 1: Access Advanced Type System Components
    println!("\n📋 Demo 1: Advanced Type System Components");
    let _type_resolver = manager.type_resolver();
    let _path_navigator = manager.path_navigator();
    println!("  ✅ Type resolver and path navigator initialized");

    // Demo 2: Use context for type operations
    println!("\n🧠 Demo 2: Resolution Context");
    let patient_context = ResolutionContext::new("Patient").with_resource_type("Patient");

    println!("  Context created for: {:?}", patient_context.resource_type);
    println!("  Base path: {}", patient_context.base_path);

    // Demo 3: Complex StructureDefinition Conversion
    println!("\n🏗️  Demo 3: Complex StructureDefinition Conversion");
    let complex_structure_def = create_demo_structure_definition();
    let conversion_result = manager
        .convert_structure_definition(complex_structure_def)
        .await?;

    println!("  Conversion successful: {}", conversion_result.success);
    if let Some(schema) = conversion_result.schema {
        println!("  Generated schema title: {:?}", schema.title);
        println!("  Number of properties: {}", schema.properties.len());

        // Check for choice type properties
        let choice_properties: Vec<_> = schema
            .properties
            .keys()
            .filter(|key| key.contains("value"))
            .collect();
        println!("  Choice type properties: {choice_properties:?}");
    }

    // Demo 4: Cache Management
    println!("\n📊 Demo 4: Cache Management");
    manager.clear_cache().await?;
    println!("  ✅ Schema cache cleared successfully");

    println!("\n✅ Advanced Type System Demo completed successfully!");
    Ok(())
}

fn create_demo_structure_definition() -> serde_json::Value {
    json!({
        "resourceType": "StructureDefinition",
        "id": "demo-observation-profile",
        "url": "http://example.org/fhir/StructureDefinition/DemoObservationProfile",
        "name": "DemoObservationProfile",
        "title": "Demo Observation Profile",
        "status": "active",
        "kind": "resource",
        "abstract": false,
        "type": "Observation",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Observation",
        "derivation": "constraint",
        "differential": {
            "element": [
                {
                    "id": "Observation",
                    "path": "Observation",
                    "definition": "Demo observation profile with choice types",
                    "min": 0,
                    "max": "*"
                },
                {
                    "id": "Observation.value[x]",
                    "path": "Observation.value[x]",
                    "definition": "Observed value with multiple type options",
                    "min": 0,
                    "max": "1",
                    "type": [
                        {"code": "Quantity"},
                        {"code": "CodeableConcept"},
                        {"code": "string"},
                        {"code": "boolean"},
                        {"code": "integer"},
                        {"code": "Range"}
                    ]
                },
                {
                    "id": "Observation.effective[x]",
                    "path": "Observation.effective[x]",
                    "definition": "When the observation was made",
                    "min": 0,
                    "max": "1",
                    "type": [
                        {"code": "dateTime"},
                        {"code": "Period"},
                        {"code": "Timing"}
                    ]
                },
                {
                    "id": "Observation.component",
                    "path": "Observation.component",
                    "definition": "Component results",
                    "min": 0,
                    "max": "*",
                    "type": [
                        {"code": "BackboneElement"}
                    ]
                },
                {
                    "id": "Observation.component.value[x]",
                    "path": "Observation.component.value[x]",
                    "definition": "Component value",
                    "min": 0,
                    "max": "1",
                    "type": [
                        {"code": "Quantity"},
                        {"code": "string"},
                        {"code": "boolean"}
                    ]
                }
            ]
        }
    })
}
