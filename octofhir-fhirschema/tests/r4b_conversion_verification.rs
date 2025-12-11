//! R4B Conversion Verification Tests
//!
//! This module implements comprehensive conversion verification for all R4B FHIR resources.
//! It downloads official R4B StructureDefinitions using the canonical manager and converts
//! them using the translate() function, ensuring all 568 resources convert successfully.

use futures::stream::{self, StreamExt};
use octofhir_canonical_manager::CanonicalManager;
use octofhir_fhirschema::{translate, FhirSchema, StructureDefinition};
use std::time::{Duration, Instant};

/// Result of converting a single StructureDefinition
#[derive(Debug, Clone)]
pub struct ConversionResult {
    pub resource_name: String,
    pub success: bool,
    pub schema: Option<FhirSchema>,
    pub error: Option<String>,
    pub conversion_time_ms: u64,
}

impl ConversionResult {
    pub fn success(resource_name: String, schema: FhirSchema, elapsed: Duration) -> Self {
        Self {
            resource_name,
            success: true,
            schema: Some(schema),
            error: None,
            conversion_time_ms: elapsed.as_millis() as u64,
        }
    }

    pub fn failure(resource_name: String, error: String, elapsed: Duration) -> Self {
        Self {
            resource_name,
            success: false,
            schema: None,
            error: Some(error),
            conversion_time_ms: elapsed.as_millis() as u64,
        }
    }
}

/// Summary statistics for batch conversion
#[derive(Debug)]
pub struct ConversionSummary {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub failed_resources: Vec<String>,
    pub total_time_ms: u64,
}

impl ConversionSummary {
    pub fn from_results(results: &[ConversionResult]) -> Self {
        let successful = results.iter().filter(|r| r.success).count();
        let failed_resources: Vec<String> = results
            .iter()
            .filter(|r| !r.success)
            .map(|r| r.resource_name.clone())
            .collect();
        let total_time_ms = results.iter().map(|r| r.conversion_time_ms).sum();

        Self {
            total: results.len(),
            successful,
            failed: results.len() - successful,
            failed_resources,
            total_time_ms,
        }
    }

    pub fn print_summary(&self) {
        println!("\n=== R4B Conversion Summary ===");
        println!("Total resources: {}", self.total);
        println!("Successfully converted: {}", self.successful);
        println!("Failed: {}", self.failed);
        println!("Total time: {}ms", self.total_time_ms);
        if self.total > 0 {
            println!(
                "Average time per resource: {}ms",
                self.total_time_ms / self.total as u64
            );
        }
    }

    pub fn print_failures(&self, results: &[ConversionResult]) {
        if !self.failed_resources.is_empty() {
            println!("\n=== Failed Resources ===");
            for name in &self.failed_resources {
                if let Some(result) = results.iter().find(|r| &r.resource_name == name) {
                    if let Some(err) = &result.error {
                        println!("  - {}: {}", name, err);
                    }
                }
            }
        }
    }
}

/// Load all R4B StructureDefinitions using the canonical manager
pub async fn load_r4b_structure_definitions(
) -> Result<Vec<StructureDefinition>, Box<dyn std::error::Error>> {
    println!("Initializing CanonicalManager with test database...");

    // Use a test-specific database directory to avoid schema conflicts
    let test_dir = tempfile::tempdir()?;
    let mut config = octofhir_canonical_manager::FcmConfig::default();
    config.storage.cache_dir = test_dir.path().join("cache");
    config.storage.packages_dir = test_dir.path().join("packages");

    let manager = octofhir_canonical_manager::CanonicalManager::new(config).await?;

    println!("Installing R4B core package (hl7.fhir.r4b.core@4.3.0)...");
    manager
        .install_package("hl7.fhir.r4b.core", "4.3.0")
        .await?;

    println!("Verifying package installation...");
    let packages = manager.list_packages().await?;
    println!("Installed packages: {:?}", packages);

    println!("Testing resolve with known URL...");
    match manager.resolve("http://hl7.org/fhir/StructureDefinition/Patient").await {
        Ok(resource) => println!("✓ Resolve works! Found: {:?}", resource.metadata.name),
        Err(e) => println!("✗ Resolve failed: {}", e),
    }

    println!("Searching for all resources (no filters)...");
    let test_results = manager
        .search()
        .await
        .limit(10)
        .execute()
        .await?;
    println!("Test search returned {} resources", test_results.resources.len());
    if !test_results.resources.is_empty() {
        println!("Sample resource: {:?}", test_results.resources[0].index.resource_type);
    }

    println!("Searching for R4B StructureDefinitions...");
    let all_results = manager
        .search()
        .await
        .resource_type("StructureDefinition")
        .limit(2000)
        .execute()
        .await?;

    println!("Found {} total StructureDefinitions across all packages", all_results.resources.len());

    // Filter for R4B package manually
    let results = all_results.resources
        .into_iter()
        .filter(|r| {
            r.index.package_name == "hl7.fhir.r4b.core" && r.index.package_version == "4.3.0"
        })
        .collect::<Vec<_>>();

    println!("Found {} R4B StructureDefinitions after filtering", results.len());

    // Parse JSON to StructureDefinition
    let mut structure_defs = Vec::new();
    for resource_match in results {
        let content = &resource_match.resource.content;
        match serde_json::from_value::<StructureDefinition>(content.clone()) {
            Ok(sd) => structure_defs.push(sd),
            Err(e) => {
                eprintln!(
                    "Warning: Failed to parse StructureDefinition {}: {}",
                    content
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown"),
                    e
                );
            }
        }
    }

    Ok(structure_defs)
}

/// Convert all StructureDefinitions in parallel
pub async fn convert_all_parallel(
    structure_defs: Vec<StructureDefinition>,
    parallelism: usize,
) -> Vec<ConversionResult> {
    println!(
        "Converting {} StructureDefinitions with {} parallel workers...",
        structure_defs.len(),
        parallelism
    );

    stream::iter(structure_defs)
        .map(|sd| async move {
            let name = sd.name.clone();
            let start = Instant::now();

            match translate(sd, None) {
                Ok(schema) => ConversionResult::success(name, schema, start.elapsed()),
                Err(e) => ConversionResult::failure(name, e.to_string(), start.elapsed()),
            }
        })
        .buffer_unordered(parallelism)
        .collect()
        .await
}

#[tokio::test]
async fn test_r4b_all_resources_convert() {
    // Downloads all R4B StructureDefinitions from hl7.fhir.r4b.core@4.3.0
    // and verifies they convert successfully using translate()

    // Load R4B StructureDefinitions via CanonicalManager
    let structure_defs = load_r4b_structure_definitions()
        .await
        .expect("Failed to load R4B StructureDefinitions");

    println!("Loaded {} StructureDefinitions", structure_defs.len());

    // Note: R4B has 568 total schemas, but some may be primitives or non-StructureDefinition resources
    // We expect at least 500 StructureDefinitions
    assert!(
        structure_defs.len() >= 500,
        "Expected at least 500 R4B StructureDefinitions, got {}",
        structure_defs.len()
    );

    // Convert all in parallel (use 8 threads for optimal performance)
    let results = convert_all_parallel(structure_defs, 8).await;

    // Generate and print summary
    let summary = ConversionSummary::from_results(&results);
    summary.print_summary();
    summary.print_failures(&results);

    // All R4B resources should convert successfully (slice handling fixed)
    assert_eq!(
        summary.failed, 0,
        "Expected zero conversion failures, but got {} failures out of {} resources",
        summary.failed,
        summary.total
    );

    // Ensure we processed all expected R4B resources
    assert!(
        summary.total >= 650,
        "Expected at least 650 R4B resources, got {}",
        summary.total
    );

    println!("\n✅ All R4B resources converted successfully!");
}

#[tokio::test]
async fn test_r4b_sample_resources_convert() {
    // Tests conversion of specific key R4B resources including new ones

    // Test a few key R4B resources individually for detailed debugging
    let test_resources = vec![
        "Patient",
        "Observation",
        "SubscriptionTopic",
        "SubscriptionStatus",
        "MedicinalProductDefinition",
    ];

    println!("Loading all R4B StructureDefinitions via CanonicalManager...");
    let structure_defs = load_r4b_structure_definitions()
        .await
        .expect("Failed to load R4B StructureDefinitions");

    println!("Loaded {} StructureDefinitions\n", structure_defs.len());

    for resource_name in test_resources {
        println!("Testing conversion of: {}", resource_name);

        // Find the StructureDefinition by name in the cached list
        let sd = structure_defs
            .iter()
            .find(|sd| sd.name == resource_name)
            .unwrap_or_else(|| panic!("StructureDefinition not found for {}", resource_name))
            .clone();

        let start = Instant::now();
        let schema = translate(sd, None).unwrap_or_else(|e| {
            panic!(
                "Failed to convert StructureDefinition for {}: {}",
                resource_name, e
            )
        });
        let elapsed = start.elapsed();

        println!("  ✅ Converted in {}ms", elapsed.as_millis());
        println!("  Schema name: {}", schema.name);
        println!("  Schema type: {}", schema.type_name);

        assert_eq!(schema.name, resource_name);
    }
}

#[tokio::test]
async fn test_r4b_conversion_performance() {
    // Benchmarks parallel conversion performance

    // Benchmark conversion performance
    let structure_defs = load_r4b_structure_definitions()
        .await
        .expect("Failed to load R4B StructureDefinitions");

    let count = structure_defs.len();
    let start = Instant::now();

    let results = convert_all_parallel(structure_defs, 8).await;
    let total_elapsed = start.elapsed();

    let summary = ConversionSummary::from_results(&results);

    println!("\n=== Performance Metrics ===");
    println!("Total resources: {}", count);
    println!("Total time: {:?}", total_elapsed);
    println!(
        "Average per resource: {}ms",
        total_elapsed.as_millis() / count as u128
    );
    println!("Throughput: {:.2} resources/sec", count as f64 / total_elapsed.as_secs_f64());

    // Performance assertion: should complete in reasonable time
    // With 8 parallel workers, expect < 30 seconds for ~650 resources
    assert!(
        total_elapsed < Duration::from_secs(30),
        "Conversion took too long: {:?}",
        total_elapsed
    );

    // All conversions should succeed
    assert_eq!(
        summary.failed, 0,
        "Expected zero conversion failures, but got {} failures",
        summary.failed
    );
}
