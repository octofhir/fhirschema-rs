use octofhir_fhir_model::provider::ModelProvider;
use octofhir_fhirschema::provider::EmbeddedModelProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Diagnosing Schema Loading Issue");
    println!("===================================\n");

    // First check - what happens when we try to load the embedded provider?
    println!("🧪 Testing EmbeddedModelProvider creation:");
    match EmbeddedModelProvider::r4().await {
        Ok(provider) => {
            println!("✅ EmbeddedModelProvider created successfully");
            println!("Schema count: {}", provider.schema_count());

            let resource_types = provider.get_available_resource_types();
            println!("Available resource types: {}", resource_types.len());

            // List the first few resource types
            println!("\nFirst 10 resource types:");
            for (i, resource_type) in resource_types.iter().take(10).enumerate() {
                println!("{}. {}", i + 1, resource_type);
            }

            if resource_types.len() > 10 {
                println!("... and {} more", resource_types.len() - 10);
            }

            // Test basic schema lookups
            println!("\n🔍 Testing schema lookups:");

            // Test 1: Look for Patient schema
            match provider.get_schema_by_type("Patient").await {
                Some(schema) => {
                    println!("✅ Found Patient schema");
                    println!("   ID: {:?}", schema.id);
                    println!("   Title: {:?}", schema.title);
                    println!("   Properties count: {}", schema.properties.len());

                    // Check if Patient has expected properties
                    let expected_props = ["name", "gender", "birthDate", "address"];
                    for prop in &expected_props {
                        if schema.properties.contains_key(*prop) {
                            println!("   ✅ Has property: {prop}");
                        } else {
                            println!("   ❌ Missing property: {prop}");
                        }
                    }
                }
                None => {
                    println!("❌ Patient schema not found");
                }
            }

            // Test 2: Look for complex type schemas
            println!("\n🎯 Checking for complex type schemas:");
            let complex_types = [
                "HumanName",
                "Period",
                "Address",
                "ContactPoint",
                "Coding",
                "CodeableConcept",
                "Identifier",
                "Reference",
            ];

            for complex_type in &complex_types {
                match provider.get_schema_by_type(complex_type).await {
                    Some(schema) => {
                        println!("✅ Found {complex_type}");
                        println!("   ID: {:?}", schema.id);
                        println!("   Properties: {}", schema.properties.len());
                    }
                    None => {
                        println!("❌ Missing {complex_type}");
                    }
                }
            }

            // Test 3: Property navigation test
            println!("\n🧪 Testing property navigation:");
            match provider.navigate_typed_path("Patient", "name").await {
                Ok(_) => println!("✅ Patient.name navigation: SUCCESS"),
                Err(e) => {
                    println!("❌ Patient.name navigation: FAILED");
                    println!("   Error: {e}");
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to create EmbeddedModelProvider");
            println!("   Error: {e}");
        }
    }

    println!("\n📊 Binary File Analysis:");
    // Check the actual precompiled schema file
    match std::fs::read(
        "/Users/alexanderstreltsov/work/octofhir/fhirschema/precompiled_schemas/r4_schemas.bin",
    ) {
        Ok(data) => {
            println!("✅ R4 schema file exists: {} bytes", data.len());

            if !data.is_empty() {
                println!(
                    "First 20 bytes (hex): {:02x?}",
                    &data[..std::cmp::min(20, data.len())]
                );
                println!(
                    "First 20 bytes (char): {:?}",
                    data[..std::cmp::min(20, data.len())]
                        .iter()
                        .map(|&b| if b.is_ascii_graphic() { b as char } else { '.' })
                        .collect::<String>()
                );

                // Check if it looks like JSON
                if data.starts_with(b"[") || data.starts_with(b"{") {
                    println!("📝 File appears to start with JSON");
                } else {
                    println!("💾 File appears to be binary data (not JSON)");
                }

                // Try to detect if this is bincode or JSON
                if let Ok(s) = std::str::from_utf8(&data[..std::cmp::min(100, data.len())]) {
                    if s.trim_start().starts_with('[') || s.trim_start().starts_with('{') {
                        println!("🔍 Detected: Likely JSON format");
                    } else {
                        println!("🔍 Detected: Not JSON, likely binary format");
                    }
                } else {
                    println!("🔍 Detected: Binary data (not UTF-8 text)");
                }
            } else {
                println!("📭 File is empty");
            }
        }
        Err(e) => {
            println!("❌ Failed to read R4 schema file: {e}");
        }
    }

    Ok(())
}
