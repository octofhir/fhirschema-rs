//! Debug test to inspect the Patient schema structure

use octofhir_fhir_model::provider::ModelProvider;
use octofhir_fhirschema::provider::EmbeddedModelProvider;

#[cfg(feature = "embedded-providers")]
#[tokio::test]
async fn debug_patient_schema_structure() {
    let provider = EmbeddedModelProvider::r4()
        .await
        .expect("Should create R4 provider");

    println!("🔍 Debugging Patient schema structure");

    // Get the Patient schema directly
    match provider.get_schema_by_type("Patient").await {
        Some(schema) => {
            println!("✅ Patient schema found");
            println!("   Schema ID: {:?}", schema.id);
            println!("   Schema title: {:?}", schema.title);
            println!("   Total properties: {}", schema.properties.len());

            // Look specifically at the 'name' property
            if let Some(name_property) = schema.properties.get("name") {
                println!("\n📝 Patient.name property details:");
                println!("   Type: {:?}", name_property.property_type);
                println!("   Items: {:?}", name_property.items);
                println!(
                    "   Array items schema: {:?}",
                    name_property.items.as_ref().map(|i| &i.items)
                );
                // Additional properties not available on FhirSchemaProperty
                println!(
                    "   Required: {}",
                    schema.required.contains(&"name".to_string())
                );
                println!(
                    "   Constraints: {} entries",
                    name_property.constraints.len()
                );

                // Check metadata for cardinality information
                println!("   Metadata:");
                for (key, value) in &name_property.metadata {
                    println!("     {key}: {value}");
                }

                // Check if there are cardinality constraints
                for constraint in &name_property.constraints {
                    println!(
                        "   Constraint {}: {} ({:?})",
                        constraint.key, constraint.human, constraint.severity
                    );
                    if let Some(expr) = &constraint.expression {
                        println!("     Expression: {expr}");
                    }
                }
            } else {
                println!("❌ Patient.name property not found!");
                println!("Available properties:");
                for (i, key) in schema.properties.keys().enumerate() {
                    if i < 20 {
                        // Limit output
                        println!("  - {key}");
                    } else if i == 20 {
                        println!("  ... and {} more", schema.properties.len() - 20);
                        break;
                    }
                }
            }

            // Also check identifier property (known to be 0..*)
            if let Some(identifier_property) = schema.properties.get("identifier") {
                println!("\n🆔 Patient.identifier property details:");
                println!("   Type: {:?}", identifier_property.property_type);
                println!("   Items: {:?}", identifier_property.items);
                println!(
                    "   Required: {}",
                    schema.required.contains(&"identifier".to_string())
                );

                // Check metadata for cardinality
                println!("   Metadata:");
                for (key, value) in &identifier_property.metadata {
                    println!("     {key}: {value}");
                }
            }

            // Check active property (known to be 0..1)
            if let Some(active_property) = schema.properties.get("active") {
                println!("\n✅ Patient.active property details:");
                println!("   Type: {:?}", active_property.property_type);
                println!("   Items: {:?}", active_property.items);
                println!(
                    "   Required: {}",
                    schema.required.contains(&"active".to_string())
                );

                // Check metadata for cardinality
                println!("   Metadata:");
                for (key, value) in &active_property.metadata {
                    println!("     {key}: {value}");
                }
            }
        }
        None => {
            println!("❌ Patient schema not found!");
        }
    }
}

#[cfg(feature = "embedded-providers")]
#[tokio::test]
async fn debug_navigation_result_structure() {
    let provider = EmbeddedModelProvider::r4()
        .await
        .expect("Should create R4 provider");

    println!("🔍 Debugging navigation result structure for Patient.name");

    match provider.navigate_typed_path("Patient", "name").await {
        Ok(result) => {
            println!("✅ Navigation successful");
            println!("   Result type: {:?}", result.result_type);
            println!("   Collection info: {:?}", result.collection_info);
            println!("   Navigation metadata: {:?}", result.navigation_metadata);
            println!(
                "   Validation results: {} entries",
                result.validation_results.len()
            );
            println!(
                "   Performance hints: {} entries",
                result.performance_hints.len()
            );
            println!("   Success: {}", result.is_success);
            println!("   Errors: {} entries", result.errors.len());

            // Check if collection info contains cardinality
            println!("\n📊 Collection information analysis:");
            println!("   Is collection: {:?}", result.collection_info);
        }
        Err(e) => {
            println!("❌ Navigation failed: {e}");
        }
    }

    // Also test Patient.identifier
    println!("\n🔍 Debugging navigation result structure for Patient.identifier");

    match provider.navigate_typed_path("Patient", "identifier").await {
        Ok(result) => {
            println!("✅ Navigation successful");
            println!("   Result type: {:?}", result.result_type);
            println!("   Collection info: {:?}", result.collection_info);
        }
        Err(e) => {
            println!("❌ Navigation failed: {e}");
        }
    }
}
