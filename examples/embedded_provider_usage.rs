/*!
 * Embedded Provider Usage Example
 * ===============================
 *
 * This example demonstrates how to use the EmbeddedModelProvider with precompiled schemas
 * for lightning-fast startup times in FHIR applications.
 *
 * The EmbeddedModelProvider includes precompiled FHIR schemas directly in the binary,
 * providing zero I/O startup with O(1) resource type lookups.
 */

use octofhir_fhir_model::provider::ModelProvider;
use octofhir_fhirschema::provider::EmbeddedModelProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Embedded FHIR Schema Provider Demo");
    println!("====================================\n");

    // Example 1: Create providers for different FHIR versions
    demonstrate_version_support().await?;

    // Example 2: Fast resource type checking
    demonstrate_resource_type_checking().await?;

    // Example 3: Type hierarchy queries
    demonstrate_type_hierarchy().await?;

    // Example 4: Schema retrieval
    demonstrate_schema_retrieval().await?;

    // Example 5: Performance comparison
    demonstrate_performance().await?;

    Ok(())
}

async fn demonstrate_version_support() -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 1. FHIR Version Support");
    println!("---------------------------");

    // Create providers for each supported FHIR version
    let versions = vec![
        ("R4", EmbeddedModelProvider::r4().await),
        ("R4B", EmbeddedModelProvider::r4b().await),
        ("R5", EmbeddedModelProvider::r5().await),
        ("R6", EmbeddedModelProvider::r6().await),
    ];

    for (version_name, provider_result) in versions {
        match provider_result {
            Ok(provider) => {
                let supported_types = provider.get_supported_resource_types().await?;
                println!(
                    "✅ FHIR {}: {} resource types available",
                    version_name,
                    supported_types.len()
                );

                // Show first few resource types
                let sample_types: Vec<_> = supported_types.iter().take(5).cloned().collect();
                println!("   Sample types: {sample_types:?}");
            }
            Err(e) => {
                println!("❌ FHIR {version_name}: {e}");
            }
        }
    }

    println!();
    Ok(())
}

async fn demonstrate_resource_type_checking() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ 2. Lightning-Fast Resource Type Checking");
    println!("-------------------------------------------");

    let provider = EmbeddedModelProvider::r4().await?;

    // Test various resource types
    let test_types = vec![
        "Patient",
        "Observation",
        "Practitioner",
        "Organization",
        "InvalidType",
        "CustomResource",
        "Bundle",
        "OperationOutcome",
    ];

    for resource_type in test_types {
        let exists = provider.resource_type_exists(resource_type);
        let status = if exists { "✅" } else { "❌" };
        println!(
            "   {} {}: {}",
            status,
            resource_type,
            if exists { "Valid" } else { "Unknown" }
        );
    }

    // Demonstrate O(1) performance
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = provider.resource_type_exists("Patient");
    }
    let elapsed = start.elapsed();
    println!(
        "   🏃‍♂️ 1000 lookups in {:?} ({:.2}ns per lookup)",
        elapsed,
        elapsed.as_nanos() as f64 / 1000.0
    );

    println!();
    Ok(())
}

async fn demonstrate_type_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏗️  3. Type Hierarchy Navigation");
    println!("--------------------------------");

    let provider = EmbeddedModelProvider::r4().await?;

    // Get type hierarchy for Patient
    if let Some(hierarchy) = provider.get_type_hierarchy("Patient").await? {
        println!("   📊 Patient Type Hierarchy:");
        println!("      Type: {}", hierarchy.type_name);
        println!("      Ancestors: {:?}", hierarchy.ancestors);
        println!("      Parent: {:?}", hierarchy.direct_parent);
        println!("      Children: {:?}", hierarchy.direct_children);
        println!("      Abstract: {}", hierarchy.is_abstract);
    }

    // Test type compatibility
    let compatibility_tests = vec![
        ("Patient", "DomainResource"),
        ("Patient", "Resource"),
        ("Patient", "Observation"),
        ("Observation", "DomainResource"),
    ];

    println!("   🔗 Type Compatibility:");
    for (from_type, to_type) in compatibility_tests {
        let compatible = provider.is_type_compatible(from_type, to_type).await?;
        let status = if compatible { "✅" } else { "❌" };
        println!(
            "      {status} {from_type} -> {to_type}: {compatible}"
        );
    }

    println!();
    Ok(())
}

async fn demonstrate_schema_retrieval() -> Result<(), Box<dyn std::error::Error>> {
    println!("📄 4. Schema Retrieval");
    println!("----------------------");

    let provider = EmbeddedModelProvider::r4().await?;

    // Get schema for Patient resource
    if let Some(schema) = provider.get_schema_by_type("Patient").await {
        println!("   📋 Patient Schema:");
        println!(
            "      ID: {}",
            schema.id.as_ref().unwrap_or(&"<no id>".to_string())
        );
        println!(
            "      Title: {}",
            schema.title.as_ref().unwrap_or(&"<no title>".to_string())
        );
        println!("      Type: {}", schema.schema_type);
        println!("      Properties: {}", schema.properties.len());
        println!("      Constraints: {}", schema.constraints.len());

        // Show some properties
        let property_names: Vec<_> = schema.properties.keys().take(5).cloned().collect();
        println!("      Sample properties: {property_names:?}");
    }

    // List all available schemas
    let schemas = provider.list_schemas().await;
    println!("   📚 Available Schemas: {} total", schemas.len());

    println!();
    Ok(())
}

async fn demonstrate_performance() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏃‍♂️ 5. Performance Characteristics");
    println!("----------------------------------");

    // Measure startup time
    let startup_start = std::time::Instant::now();
    let provider = EmbeddedModelProvider::r4().await?;
    let startup_time = startup_start.elapsed();

    println!("   ⚡ Startup time: {startup_time:?}");
    println!(
        "   💾 Memory footprint: ~{} schemas loaded",
        provider.schema_count()
    );

    // Measure various operations
    println!("   📈 Operation benchmarks:");

    // Resource type check
    let start = std::time::Instant::now();
    let mut success_count = 0;
    for _ in 0..100 {
        if provider.resource_type_exists("Patient") {
            success_count += 1;
        }
    }
    let elapsed = start.elapsed();
    println!(
        "      Resource type check: {elapsed:?} (100 ops, {success_count}% success)"
    );

    // Schema retrieval
    let start = std::time::Instant::now();
    let mut success_count = 0;
    for _ in 0..100 {
        if provider.get_schema_by_type("Patient").await.is_some() {
            success_count += 1;
        }
    }
    let elapsed = start.elapsed();
    println!(
        "      Schema retrieval: {elapsed:?} (100 ops, {success_count}% success)"
    );

    // Type hierarchy
    let start = std::time::Instant::now();
    let mut success_count = 0;
    for _ in 0..100 {
        if provider.get_type_hierarchy("Patient").await.is_ok() {
            success_count += 1;
        }
    }
    let elapsed = start.elapsed();
    println!(
        "      Type hierarchy: {elapsed:?} (100 ops, {success_count}% success)"
    );

    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embedded_provider_basic_functionality() {
        let provider = EmbeddedModelProvider::r4()
            .await
            .expect("Should create R4 provider");

        // Test basic functionality
        assert!(provider.resource_type_exists("Patient"));
        assert!(provider.resource_type_exists("Observation"));
        assert!(!provider.resource_type_exists("NonExistentResource"));

        // Test schema retrieval
        let schema = provider.get_schema_by_type("Patient").await;
        assert!(schema.is_some());

        // Test type hierarchy
        let hierarchy = provider.get_type_hierarchy("Patient").await.unwrap();
        assert!(hierarchy.is_some());
    }

    #[tokio::test]
    async fn test_all_fhir_versions() {
        let versions = vec![
            EmbeddedModelProvider::r4().await,
            EmbeddedModelProvider::r4b().await,
            EmbeddedModelProvider::r5().await,
            EmbeddedModelProvider::r6().await,
        ];

        for (i, provider_result) in versions.into_iter().enumerate() {
            match provider_result {
                Ok(provider) => {
                    let version_names = vec!["R4", "R4B", "R5", "R6"];
                    println!("✅ FHIR {} provider working", version_names[i]);

                    // Basic functionality test
                    assert!(provider.schema_count() > 0);
                }
                Err(e) => {
                    // Some versions might not have schemas available
                    println!("⚠️  FHIR version {} not available: {}", i, e);
                }
            }
        }
    }
}
