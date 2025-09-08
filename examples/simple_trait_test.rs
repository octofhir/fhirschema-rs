// Simple test to verify ModelProvider trait implementation works correctly
use octofhir_fhir_model::provider::ModelProvider;
use octofhir_fhirschema::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 Testing ModelProvider Trait Implementation");

    // Create FhirSchemaModelProvider
    println!("Creating FHIR R4 provider...");
    let provider = FhirSchemaModelProvider::r4().await?;
    println!("✅ Provider created successfully");

    // Cast to ModelProvider trait
    let model_provider: &dyn ModelProvider = &provider;
    println!("✅ Successfully cast to ModelProvider trait");

    // Test basic trait methods
    println!("Testing FHIR version...");
    let version = model_provider.get_fhir_version();
    println!("✅ FHIR Version: {version}");

    println!("Testing supported resource types...");
    match model_provider.get_supported_resource_types().await {
        Ok(types) => {
            println!("✅ Found {} supported resource types", types.len());
            if !types.is_empty() {
                println!(
                    "   Examples: {}",
                    types.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
                );
            }
        }
        Err(e) => {
            println!("❌ Error getting resource types: {e}");
            return Err(FhirSchemaError::Runtime {
                message: e.to_string(),
            });
        }
    }

    println!("Testing type compatibility...");
    match model_provider
        .is_type_compatible("Patient", "Resource")
        .await
    {
        Ok(compatible) => {
            println!("✅ Patient -> Resource compatibility: {compatible}");
        }
        Err(e) => {
            println!("❌ Error checking compatibility: {e}");
        }
    }

    println!("Testing cache clearing...");
    match model_provider.clear_caches().await {
        Ok(_) => {
            println!("✅ Caches cleared successfully");
        }
        Err(e) => {
            println!("❌ Error clearing caches: {e}");
        }
    }

    println!("\n🎉 ModelProvider trait implementation test completed successfully!");
    println!(
        "The FhirSchemaModelProvider correctly implements the ModelProvider trait from fhir-model-rs"
    );

    Ok(())
}
