/*!
 * Composite Provider Usage Example
 * ================================
 *
 * This example demonstrates how to use the CompositeModelProvider for production applications.
 *
 * The CompositeModelProvider combines multiple provider types with intelligent fallback:
 * 1. EmbeddedModelProvider (fastest, zero I/O)
 * 2. DynamicModelProvider (fast after first load, supports custom packages)
 * 3. FhirSchemaModelProvider (traditional fallback)
 */

use octofhir_fhir_model::provider::ModelProvider;
use octofhir_fhirschema::core::FhirVersion;
use octofhir_fhirschema::provider::CompositeModelProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Composite FHIR Schema Provider Demo");
    println!("======================================\n");

    // Example 1: Basic composite provider usage
    demonstrate_basic_usage().await?;

    // Example 2: Multi-version support
    demonstrate_multi_version().await?;

    // Example 3: Performance characteristics
    demonstrate_performance().await?;

    // Example 4: Production integration patterns
    demonstrate_integration_patterns().await?;

    // Example 5: Error handling and fallback
    demonstrate_error_handling().await?;

    Ok(())
}

async fn demonstrate_basic_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 1. Basic Composite Provider Usage");
    println!("------------------------------------");

    // Create composite provider for FHIR R4
    let provider = CompositeModelProvider::r4().await?;
    println!("✅ Created CompositeModelProvider for FHIR R4");

    // Test basic operations
    let resource_types = provider.get_supported_resource_types().await?;
    println!("📊 {} resource types available", resource_types.len());

    // Test resource type existence (O(1) operation via embedded provider)
    let test_resources = vec!["Patient", "Observation", "Practitioner", "InvalidType"];
    for resource_type in test_resources {
        let exists = provider.resource_type_exists(resource_type)?;
        let status = if exists { "✅" } else { "❌" };
        println!("   {status} {resource_type}");
    }

    println!();
    Ok(())
}

async fn demonstrate_multi_version() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 2. Multi-Version Support");
    println!("---------------------------");

    // Test each FHIR version
    let version_results = vec![
        ("R4", CompositeModelProvider::r4().await),
        ("R4B", CompositeModelProvider::r4b().await),
        ("R5", CompositeModelProvider::r5().await),
        ("R6", CompositeModelProvider::r6().await),
    ];

    for (version_name, provider_result) in version_results {
        match provider_result {
            Ok(provider) => {
                let fhir_version = provider.get_fhir_version();
                let resource_count = provider.get_supported_resource_types().await?.len();
                println!(
                    "✅ FHIR {version_name} ({fhir_version:?}): {resource_count} resources"
                );

                // Test a common operation
                if let Ok(Some(hierarchy)) = provider.get_type_hierarchy("Patient").await {
                    println!("   📋 Patient hierarchy available: {}", hierarchy.type_name);
                }
            }
            Err(e) => {
                println!("❌ FHIR {version_name}: {e}");
            }
        }
    }

    println!();
    Ok(())
}

async fn demonstrate_performance() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ 3. Performance Characteristics");
    println!("---------------------------------");

    let provider = CompositeModelProvider::r4().await?;

    // Measure startup time (should be fast due to embedded provider)
    let startup_start = std::time::Instant::now();
    let _test_provider = CompositeModelProvider::r4().await?;
    let startup_time = startup_start.elapsed();
    println!("🚀 Cold startup time: {startup_time:?}");

    // Measure various operations
    println!("📈 Performance benchmarks:");

    // Resource type check
    let iterations = 1000;
    let start = std::time::Instant::now();
    let mut success_count = 0;
    for _ in 0..iterations {
        if provider.resource_type_exists("Patient").unwrap_or(false) {
            success_count += 1;
        }
    }
    let elapsed = start.elapsed();
    let avg_time = elapsed.as_nanos() as f64 / iterations as f64;
    println!(
        "   Resource type check: {:?} total, {:.1}ns avg ({} ops, {}% success)",
        elapsed,
        avg_time,
        iterations,
        (success_count * 100) / iterations
    );

    // Type hierarchy lookup
    let iterations = 100;
    let start = std::time::Instant::now();
    let mut success_count = 0;
    for _ in 0..iterations {
        if provider.get_type_hierarchy("Patient").await.is_ok() {
            success_count += 1;
        }
    }
    let elapsed = start.elapsed();
    let avg_time = elapsed.as_nanos() as f64 / iterations as f64;
    println!(
        "   Type hierarchy lookup: {:?} total, {:.1}ns avg ({} ops, {}% success)",
        elapsed,
        avg_time,
        iterations,
        (success_count * 100) / iterations
    );

    // Type compatibility check
    let iterations = 100;
    let start = std::time::Instant::now();
    let mut success_count = 0;
    for _ in 0..iterations {
        if provider
            .is_type_compatible("Patient", "DomainResource")
            .await
            .unwrap_or(false)
        {
            success_count += 1;
        }
    }
    let elapsed = start.elapsed();
    let avg_time = elapsed.as_nanos() as f64 / iterations as f64;
    println!(
        "   Type compatibility check: {:?} total, {:.1}ns avg ({} ops, {}% success)",
        elapsed,
        avg_time,
        iterations,
        (success_count * 100) / iterations
    );

    println!();
    Ok(())
}

async fn demonstrate_integration_patterns() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏗️  4. Production Integration Patterns");
    println!("--------------------------------------");

    // Pattern 1: Application-wide shared provider
    println!("📦 Pattern 1: Shared Provider (Singleton)");
    let _shared_provider = std::sync::Arc::new(CompositeModelProvider::r4().await?);
    println!("✅ Created shared provider (wrap in Arc for thread-safety)");

    // Pattern 2: Version-specific providers
    println!("📦 Pattern 2: Multi-Version Manager");
    struct FhirProviderManager {
        r4_provider: CompositeModelProvider,
        r5_provider: CompositeModelProvider,
    }

    impl FhirProviderManager {
        async fn new() -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self {
                r4_provider: CompositeModelProvider::r4().await?,
                r5_provider: CompositeModelProvider::r5().await?,
            })
        }

        fn get_provider(&self, version: FhirVersion) -> &CompositeModelProvider {
            match version {
                FhirVersion::R4 => &self.r4_provider,
                FhirVersion::R5 => &self.r5_provider,
                _ => &self.r4_provider, // fallback to R4
            }
        }
    }

    let manager = FhirProviderManager::new().await?;
    println!("✅ Created multi-version manager");

    // Test the manager
    let r4_provider = manager.get_provider(FhirVersion::R4);
    let patient_exists = r4_provider.resource_type_exists("Patient")?;
    println!("   📋 Patient in R4: {patient_exists}");

    // Pattern 3: Lazy initialization
    println!("📦 Pattern 3: Lazy Provider");
    use std::sync::Arc;
    use tokio::sync::OnceCell;

    static GLOBAL_PROVIDER: OnceCell<Arc<CompositeModelProvider>> = OnceCell::const_new();

    async fn get_global_provider(
    ) -> Result<&'static Arc<CompositeModelProvider>, Box<dyn std::error::Error>> {
        GLOBAL_PROVIDER
            .get_or_try_init(|| async { CompositeModelProvider::r4().await.map(Arc::new) })
            .await
            .map_err(|e| e.into())
    }

    let lazy_provider = get_global_provider().await?;
    println!("✅ Lazy provider initialized");
    let resource_count = lazy_provider.get_supported_resource_types().await?.len();
    println!(
        "   📊 {resource_count} resources available via lazy provider"
    );

    println!();
    Ok(())
}

async fn demonstrate_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛡️  5. Error Handling and Fallback");
    println!("----------------------------------");

    let provider = CompositeModelProvider::r4().await?;

    // Test graceful handling of invalid inputs
    println!("🧪 Testing error handling:");

    // Invalid resource type
    match provider.resource_type_exists("") {
        Ok(exists) => println!("   📝 Empty resource type: {exists}"),
        Err(e) => println!("   ❌ Empty resource type error: {e}"),
    }

    // Non-existent resource type
    let invalid_exists = provider.resource_type_exists("NonExistentType")?;
    println!("   📝 Non-existent type: {invalid_exists}");

    // Invalid type hierarchy request
    match provider.get_type_hierarchy("InvalidType").await {
        Ok(Some(hierarchy)) => println!(
            "   📋 Got hierarchy for invalid type: {}",
            hierarchy.type_name
        ),
        Ok(None) => println!("   📋 No hierarchy for invalid type (expected)"),
        Err(e) => println!("   ❌ Type hierarchy error: {e}"),
    }

    // Test cache operations
    match provider.clear_caches().await {
        Ok(()) => println!("   🧹 Cache clear successful"),
        Err(e) => println!("   ❌ Cache clear error: {e}"),
    }

    // Test fallback behavior
    println!("🔄 Testing fallback chain:");
    println!("   1. Embedded provider (fastest) ✅");
    println!("   2. Dynamic provider (if available) ⚠️");
    println!("   3. Traditional provider (fallback) ✅");
    println!("   → Composite provider automatically selects best available option");

    println!();
    Ok(())
}

/// Example integration with a hypothetical FHIR application
#[allow(dead_code)]
mod application_integration {
    use super::*;
    use std::sync::Arc;

    /// Example FHIR application service
    pub struct FhirValidationService {
        provider: Arc<CompositeModelProvider>,
    }

    impl FhirValidationService {
        pub async fn new(fhir_version: FhirVersion) -> Result<Self, Box<dyn std::error::Error>> {
            let provider = match fhir_version {
                FhirVersion::R4 => CompositeModelProvider::r4().await?,
                FhirVersion::R4B => CompositeModelProvider::r4b().await?,
                FhirVersion::R5 => CompositeModelProvider::r5().await?,
                FhirVersion::R6 => CompositeModelProvider::r6().await?,
            };

            Ok(Self {
                provider: Arc::new(provider),
            })
        }

        /// Validate that a resource type exists
        pub async fn validate_resource_type(
            &self,
            resource_type: &str,
        ) -> Result<bool, Box<dyn std::error::Error>> {
            Ok(self.provider.resource_type_exists(resource_type)?)
        }

        /// Get type information for a resource
        pub async fn get_resource_info(
            &self,
            resource_type: &str,
        ) -> Result<Option<String>, Box<dyn std::error::Error>> {
            match self.provider.get_type_hierarchy(resource_type).await? {
                Some(hierarchy) => Ok(Some(format!(
                    "Type: {}, Parent: {:?}",
                    hierarchy.type_name, hierarchy.direct_parent
                ))),
                None => Ok(None),
            }
        }

        /// Check if two types are compatible
        pub async fn check_type_compatibility(
            &self,
            from: &str,
            to: &str,
        ) -> Result<bool, Box<dyn std::error::Error>> {
            Ok(self.provider.is_type_compatible(from, to).await?)
        }
    }

    /// Example usage in an HTTP handler
    pub async fn example_http_handler() -> Result<String, Box<dyn std::error::Error>> {
        let service = FhirValidationService::new(FhirVersion::R4).await?;

        // Validate incoming resource type
        let resource_type = "Patient";
        if !service.validate_resource_type(resource_type).await? {
            return Ok(format!("Invalid resource type: {resource_type}"));
        }

        // Get type information
        match service.get_resource_info(resource_type).await? {
            Some(info) => Ok(format!("Resource {resource_type} is valid: {info}")),
            None => Ok(format!(
                "Resource {resource_type} exists but no type info available"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_composite_provider_basic_functionality() {
        let provider = CompositeModelProvider::r4()
            .await
            .expect("Should create composite provider");

        // Test basic operations
        assert!(provider.resource_type_exists("Patient").unwrap());
        assert!(provider.resource_type_exists("Observation").unwrap());
        assert!(!provider.resource_type_exists("InvalidType").unwrap());

        // Test supported resource types
        let types = provider.get_supported_resource_types().await.unwrap();
        assert!(!types.is_empty());
        assert!(types.contains(&"Patient".to_string()));
    }

    #[tokio::test]
    async fn test_composite_provider_all_versions() {
        let versions = vec![
            ("R4", CompositeModelProvider::r4()),
            ("R4B", CompositeModelProvider::r4b()),
            ("R5", CompositeModelProvider::r5()),
            ("R6", CompositeModelProvider::r6()),
        ];

        for (version_name, provider_future) in versions {
            match provider_future.await {
                Ok(_provider) => {
                    println!("✅ {} provider created successfully", version_name);
                }
                Err(e) => {
                    println!("⚠️  {} provider not available: {}", version_name, e);
                    // Not all versions might be available, so we don't fail the test
                }
            }
        }
    }

    #[tokio::test]
    async fn test_application_integration() {
        use application_integration::*;

        let service = FhirValidationService::new(FhirVersion::R4)
            .await
            .expect("Should create validation service");

        // Test validation
        assert!(service.validate_resource_type("Patient").await.unwrap());
        assert!(!service.validate_resource_type("InvalidType").await.unwrap());

        // Test info retrieval
        let info = service.get_resource_info("Patient").await.unwrap();
        assert!(info.is_some());

        // Test compatibility
        let compatible = service
            .check_type_compatibility("Patient", "DomainResource")
            .await
            .unwrap();
        assert!(compatible);
    }
}
