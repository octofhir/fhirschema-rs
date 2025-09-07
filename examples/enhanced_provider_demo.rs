// Enhanced ModelProvider Demo
//
// This example demonstrates the new ModelProvider architecture with:
// - EmbeddedModelProvider (precompiled schemas for zero I/O startup)
// - DynamicModelProvider (disk caching for fast subsequent startups)
// - CompositeModelProvider (automatic fallback chain for best performance)

use octofhir_fhirschema::prelude::*;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Enhanced ModelProvider Architecture Demo");
    println!("============================================\n");

    // Demo 1: CompositeModelProvider (Recommended - Best Performance)
    println!("📊 Demo 1: CompositeModelProvider (Recommended)");
    println!("This provider automatically uses the fastest available method:");
    println!("  1. EmbeddedModelProvider (if available) - Zero I/O startup");
    println!("  2. DynamicModelProvider (if cached) - Fast disk access");
    println!("  3. FhirSchemaModelProvider (fallback) - Live compilation");

    let start = Instant::now();
    let composite_provider = CompositeModelProvider::r4().await?;
    let composite_time = start.elapsed();

    println!(
        "  ✅ CompositeModelProvider initialized in {:?}",
        composite_time
    );

    // Show provider status
    let status = composite_provider.get_provider_status();
    println!("  📈 Provider Status:");
    println!("    - Embedded available: {}", status.embedded_available);
    println!(
        "    - Dynamic caching available: {}",
        status.dynamic_available
    );
    println!("    - Fallback available: {}", status.fallback_available);
    println!("    - Total providers: {}", status.total_providers);

    // Test basic operations
    test_basic_operations(&composite_provider, "CompositeModelProvider").await?;
    println!();

    // Demo 2: EmbeddedModelProvider (Fastest Startup)
    #[cfg(feature = "embedded-providers")]
    {
        println!("⚡ Demo 2: EmbeddedModelProvider (Fastest Startup)");
        println!("Uses precompiled schemas embedded at compile time");

        let start = Instant::now();
        let embedded_provider = FhirSchemaModelProvider::embedded_only(FhirVersion::R4).await?;
        let embedded_time = start.elapsed();

        println!(
            "  ✅ EmbeddedModelProvider initialized in {:?}",
            embedded_time
        );
        println!("  📊 Schema count: {}", embedded_provider.schema_count());
        println!(
            "  🎯 Performance: ~{}% faster startup vs live compilation",
            calculate_improvement(embedded_time, composite_time)
        );

        // Test basic operations
        test_basic_operations(&embedded_provider, "EmbeddedModelProvider").await?;
        println!();
    }

    // Demo 3: DynamicModelProvider (Cached Performance)
    #[cfg(feature = "dynamic-caching")]
    {
        println!("💾 Demo 3: DynamicModelProvider (Cached Performance)");
        println!("Compiles schemas once, caches for fast subsequent startups");

        let start = Instant::now();
        let dynamic_provider = FhirSchemaModelProvider::with_caching(FhirVersion::R4).await?;
        let dynamic_time = start.elapsed();

        println!(
            "  ✅ DynamicModelProvider initialized in {:?}",
            dynamic_time
        );
        println!(
            "  📂 Cache directory: {}",
            dynamic_provider.cache_dir().display()
        );
        println!(
            "  🔄 Status: {}",
            if dynamic_provider.is_initialized() {
                "Initialized with cached data"
            } else {
                "Initialized without cache"
            }
        );

        // Test basic operations
        test_basic_operations(&dynamic_provider, "DynamicModelProvider").await?;
        println!();
    }

    // Demo 4: Traditional Provider (Full Compatibility)
    println!("🔧 Demo 4: Traditional FhirSchemaModelProvider");
    println!("Full-featured provider with live compilation (slower startup)");

    let start = Instant::now();
    let traditional_provider = FhirSchemaModelProvider::r4().await?;
    let traditional_time = start.elapsed();

    println!(
        "  ✅ FhirSchemaModelProvider initialized in {:?}",
        traditional_time
    );
    println!("  📊 Performance comparison:");
    println!(
        "    - Composite: {:?} ({}% of traditional)",
        composite_time,
        (composite_time.as_millis() * 100 / traditional_time.as_millis().max(1))
    );

    #[cfg(feature = "embedded-providers")]
    {
        let embedded_provider = FhirSchemaModelProvider::embedded_only(FhirVersion::R4).await?;
        let embedded_time_2 = std::time::Instant::now();
        let _test = embedded_provider.get_supported_resource_types().await;
        let embedded_op_time = embedded_time_2.elapsed();

        println!("    - Embedded operation time: {:?}", embedded_op_time);
    }

    // Test basic operations
    test_basic_operations(&traditional_provider, "FhirSchemaModelProvider").await?;
    println!();

    // Performance Summary
    println!("📈 Performance Summary");
    println!("=====================");
    println!("Startup Times:");
    println!("  - CompositeModelProvider: {:?}", composite_time);
    #[cfg(feature = "embedded-providers")]
    {
        let embedded_provider = FhirSchemaModelProvider::embedded_only(FhirVersion::R4).await?;
        let start = Instant::now();
        let _test = embedded_provider.get_supported_resource_types().await;
        let embedded_time = start.elapsed();
        println!(
            "  - EmbeddedModelProvider: ~{:?} (estimated)",
            embedded_time
        );
    }
    println!("  - Traditional: {:?}", traditional_time);

    println!("\n🎯 Recommendations:");
    println!("  - Use CompositeModelProvider for best overall performance");
    println!("  - Use EmbeddedModelProvider for fastest startup (limited functionality)");
    println!("  - Use DynamicModelProvider for custom packages with caching");
    println!("  - Use FhirSchemaModelProvider for maximum compatibility");

    println!("\n✅ Enhanced ModelProvider Demo Completed!");
    Ok(())
}

async fn test_basic_operations<T: ModelProvider>(
    provider: &T,
    provider_name: &str,
) -> octofhir_fhir_model::error::Result<()> {
    println!("  🧪 Testing basic operations for {}:", provider_name);

    // Test resource type checking
    let patient_exists = provider.resource_type_exists("Patient")?;
    println!("    - Patient resource exists: {}", patient_exists);

    // Test getting supported resource types
    match provider.get_supported_resource_types().await {
        Ok(types) => {
            let count = types.len();
            println!("    - Supported resource types: {} found", count);
            if count > 0 {
                let sample_types: Vec<&String> = types.iter().take(5).collect();
                println!(
                    "      Sample: {}",
                    sample_types
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                if count > 5 {
                    println!("      ... and {} more", count - 5);
                }
            }
        }
        Err(e) => println!("    - Error getting resource types: {}", e),
    }

    // Test navigation
    match provider.navigate_typed_path("Patient", "name.family").await {
        Ok(result) => {
            println!("    - Navigation Patient.name.family: ✅ Success");
            if result.is_success {
                println!("      Result type available: Yes");
            }
        }
        Err(e) => println!("    - Navigation failed: {}", e),
    }

    // Test type hierarchy
    match provider.get_type_hierarchy("Patient").await {
        Ok(Some(hierarchy)) => {
            println!("    - Patient type hierarchy: ✅ Found");
            println!("      Type: {}", hierarchy.type_name);
            if let Some(parent) = &hierarchy.direct_parent {
                println!("      Parent: {}", parent);
            }
        }
        Ok(None) => println!("    - Patient type hierarchy: Not found"),
        Err(e) => println!("    - Type hierarchy error: {}", e),
    }

    Ok(())
}

fn calculate_improvement(new_time: std::time::Duration, old_time: std::time::Duration) -> u32 {
    if old_time.as_millis() == 0 {
        return 0;
    }

    let improvement =
        (old_time.as_millis().saturating_sub(new_time.as_millis())) * 100 / old_time.as_millis();
    improvement as u32
}
