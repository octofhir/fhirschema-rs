use octofhir_fhirschema::provider::EmbeddedModelProvider;
use std::collections::HashMap;

// Create a simplified FhirSchema structure that matches what should be serialized
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SimpleFhirSchema {
    id: Option<String>,
    title: Option<String>,
    properties: HashMap<String, serde_json::Value>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Inspecting precompiled FHIR schemas");
    println!("=======================================\n");

    // First, let's try to access raw binary data directly
    println!("📊 Checking precompiled schema files directly:");

    // Read the R4 schemas binary file
    match std::fs::read(
        "/Users/alexanderstreltsov/work/octofhir/fhirschema/precompiled_schemas/r4_schemas.bin",
    ) {
        Ok(data) => {
            println!("✅ R4 schema file size: {} bytes", data.len());

            if !data.is_empty() {
                println!(
                    "First 50 bytes: {:?}",
                    &data[..std::cmp::min(50, data.len())]
                );

                // Try JSON deserialization (what the generation script actually uses)
                match serde_json::from_slice::<Vec<SimpleFhirSchema>>(&data) {
                    Ok(schemas) => {
                        println!("✅ Successfully deserialized with JSON");
                        println!("Number of schemas: {}", schemas.len());

                        // List the first few schema IDs
                        println!("\nSchema IDs (first 10):");
                        for (i, schema) in schemas.iter().take(10).enumerate() {
                            if let Some(id) = &schema.id {
                                println!("{}. {}", i + 1, id);
                            } else if let Some(title) = &schema.title {
                                println!("{}. [title: {}]", i + 1, title);
                            } else {
                                println!("{}. [no id/title]", i + 1);
                            }
                        }

                        if schemas.len() > 10 {
                            println!("... and {} more schemas", schemas.len() - 10);
                        }

                        // Check for complex types we're looking for
                        println!("\n🎯 Checking for critical complex types:");
                        let complex_types = [
                            "http://hl7.org/fhir/StructureDefinition/HumanName",
                            "http://hl7.org/fhir/StructureDefinition/Period",
                            "http://hl7.org/fhir/StructureDefinition/Address",
                            "http://hl7.org/fhir/StructureDefinition/ContactPoint",
                            "http://hl7.org/fhir/StructureDefinition/Coding",
                            "http://hl7.org/fhir/StructureDefinition/CodeableConcept",
                            "http://hl7.org/fhir/StructureDefinition/Identifier",
                            "http://hl7.org/fhir/StructureDefinition/Reference",
                            "http://hl7.org/fhir/StructureDefinition/Quantity",
                            "http://hl7.org/fhir/StructureDefinition/Meta",
                        ];

                        let schema_ids: Vec<String> =
                            schemas.iter().filter_map(|s| s.id.clone()).collect();

                        for complex_type in &complex_types {
                            if schema_ids.contains(&complex_type.to_string()) {
                                println!("✅ Found: {complex_type}");
                            } else {
                                println!("❌ Missing: {complex_type}");
                            }
                        }

                        // Show all available schema IDs
                        println!("\n📋 All Schema IDs:");
                        let mut all_ids: Vec<_> = schema_ids.clone();
                        all_ids.sort();
                        for (i, id) in all_ids.iter().enumerate() {
                            println!("{}. {}", i + 1, id);
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to deserialize with JSON: {e:?}");
                    }
                }
            } else {
                println!("❌ R4 schema file is empty");
            }
        }
        Err(e) => {
            println!("❌ Failed to read R4 schema file: {e}");
        }
    }

    // Now test the EmbeddedModelProvider
    println!("\n🧪 Testing EmbeddedModelProvider:");

    match EmbeddedModelProvider::r4().await {
        Ok(provider) => {
            println!("✅ EmbeddedModelProvider created successfully");

            let resource_types = provider.get_available_resource_types();
            println!("Available resource types: {}", resource_types.len());

            // List the first 10 resource types
            for (i, resource_type) in resource_types.iter().take(10).enumerate() {
                println!("{}. {}", i + 1, resource_type);
            }

            if resource_types.len() > 10 {
                println!("... and {} more", resource_types.len() - 10);
            }

            // Test schema retrieval for complex types
            println!("\n🔍 Testing schema retrieval for complex types:");
            let complex_types = [
                "HumanName",
                "Period",
                "Address",
                "ContactPoint",
                "Coding",
                "CodeableConcept",
            ];

            for complex_type in &complex_types {
                match provider.get_schema_by_type(complex_type).await {
                    Some(_schema) => {
                        println!("✅ Found schema for: {complex_type}");
                    }
                    None => {
                        println!("❌ No schema found for: {complex_type}");
                    }
                }
            }

            // Test property validation on Patient.name (should involve HumanName)
            println!("\n🧪 Testing property navigation:");
            match provider.navigate_typed_path("Patient", "name").await {
                Ok(_) => println!("✅ Patient.name navigation: SUCCESS"),
                Err(e) => println!("❌ Patient.name navigation: FAILED - {e}"),
            }
        }
        Err(e) => {
            println!("❌ Failed to create EmbeddedModelProvider: {e}");
        }
    }

    Ok(())
}
