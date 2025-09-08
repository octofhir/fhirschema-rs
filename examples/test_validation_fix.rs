use octofhir_fhirschema::provider::EmbeddedModelProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing Property Validation Fix");
    println!("==================================\n");

    let provider = EmbeddedModelProvider::r4().await?;

    println!("📊 Schema Statistics:");
    println!("Total schemas: {}", provider.schema_count());
    println!(
        "Resource types: {}",
        provider.get_available_resource_types().len()
    );

    // Test complex types are now present
    println!("\n🎯 Testing Complex Type Schemas:");
    let critical_types = [
        "HumanName",
        "Period",
        "Address",
        "ContactPoint",
        "Coding",
        "CodeableConcept",
    ];

    let mut found_count = 0;
    for complex_type in &critical_types {
        match provider.get_schema_by_type(complex_type).await {
            Some(schema) => {
                println!(
                    "✅ Found {}: {} properties",
                    complex_type,
                    schema.properties.len()
                );
                found_count += 1;
            }
            None => {
                println!("❌ Missing: {complex_type}");
            }
        }
    }

    println!(
        "\n📈 Complex Types Found: {}/{}",
        found_count,
        critical_types.len()
    );

    if found_count == critical_types.len() {
        println!("🎉 SUCCESS: All critical complex types are now present!");
    } else {
        println!("⚠️  PARTIAL: Some complex types still missing");
    }

    // Test the original failing case
    println!("\n🎯 Original Failing Case Test:");
    match provider.get_schema_by_type("Patient").await {
        Some(patient_schema) => {
            if patient_schema.properties.contains_key("name") {
                println!("✅ Step 1: Patient has 'name' property");

                match provider.get_schema_by_type("HumanName").await {
                    Some(name_schema) => {
                        if name_schema.properties.contains_key("given") {
                            println!("✅ Step 2: HumanName has 'given' property");
                            println!(
                                "🎉 RESULT: Patient.name.given should now validate correctly!"
                            );
                        } else {
                            println!("❌ Step 2: HumanName missing 'given' property");
                        }
                    }
                    None => {
                        println!("❌ Step 2: HumanName schema not found");
                    }
                }
            } else {
                println!("❌ Step 1: Patient missing 'name' property");
            }
        }
        None => {
            println!("❌ Step 1: Patient schema not found");
        }
    }

    Ok(())
}
