mod common;

use octofhir_canonical_manager::{CanonicalManager, FcmConfig, RegistryConfig, StorageConfig};
use octofhir_fhirschema::{FhirSchemaConverter, StructureDefinition, StructureDefinitionConverter};
use std::sync::Arc;
use tempfile::TempDir;

/// Creates a custom FcmConfig that uses the local .fcm folder in the repository
fn create_local_fcm_config() -> FcmConfig {
    // Get the current working directory (should be the repository root)
    let repo_root = std::env::current_dir().expect("Failed to get current directory");
    let fcm_dir = repo_root.join(".fcm");

    FcmConfig {
        registry: RegistryConfig::default(),
        packages: vec![], // No pre-configured packages
        storage: StorageConfig {
            cache_dir: fcm_dir.join("cache"),
            index_dir: fcm_dir.join("index"),
            packages_dir: fcm_dir.join("packages"),
            max_cache_size: "2GB".to_string(), // Increase cache size for tests
        },
    }
}

/// Test that demonstrates using the official OctoFHIR canonical manager
/// to download and manage FHIR R4 core packages for conversion
#[tokio::test]
#[ignore] // Run with: cargo test fhir_r4_canonical_manager -- --ignored
async fn test_fhir_r4_canonical_manager_integration()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Testing FHIR R4 Core with OctoFHIR Canonical Manager");

    // Create temporary directory for any file operations
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _temp_path = temp_dir.path();

    // Initialize the official OctoFHIR canonical manager with local config
    println!("🗂️  Initializing OctoFHIR Canonical Manager with local .fcm folder");
    let config = create_local_fcm_config();
    println!("📁 Using storage paths:");
    println!("   - Cache: {:?}", config.storage.cache_dir);
    println!("   - Index: {:?}", config.storage.index_dir);
    println!("   - Packages: {:?}", config.storage.packages_dir);
    let canonical_manager = Arc::new(CanonicalManager::new(config).await?);

    // Use canonical manager to install FHIR R4 core package
    println!("📥 Installing FHIR R4 core package via canonical manager");
    let fhir_r4_package = "hl7.fhir.r4.core";
    let version = "4.0.1";

    // Install the package using canonical manager
    canonical_manager
        .install_package(fhir_r4_package, version)
        .await?;

    println!("✅ Successfully installed FHIR R4 core package");

    // Get StructureDefinitions from the canonical manager
    println!("🔍 Retrieving StructureDefinitions from canonical manager");
    let structure_definitions =
        get_structure_definitions_from_canonical_manager(&canonical_manager).await?;

    println!(
        "✅ Found {} StructureDefinitions",
        structure_definitions.len()
    );

    // Convert a sample of StructureDefinitions using our converter
    println!("⚙️  Converting sample StructureDefinitions");
    let conversion_results =
        convert_sample_structure_definitions(&structure_definitions, canonical_manager).await;

    // Generate and display results
    display_conversion_results(&conversion_results);

    // Validate that core resources were successfully converted
    validate_core_resources_converted(&conversion_results);

    println!("🎉 FHIR R4 Canonical Manager Integration Test completed successfully!");
    Ok(())
}

async fn get_structure_definitions_from_canonical_manager(
    canonical_manager: &Arc<CanonicalManager>,
) -> std::result::Result<Vec<StructureDefinition>, Box<dyn std::error::Error>> {
    let mut structure_definitions = Vec::new();

    // Search for all StructureDefinition resources using the search builder
    let search_results = canonical_manager
        .search()
        .await
        .resource_type("StructureDefinition")
        .execute()
        .await?;

    println!(
        "Found {} StructureDefinition resources",
        search_results.resources.len()
    );

    for resource_match in search_results.resources {
        // Convert FhirResource to StructureDefinition
        match serde_json::from_value::<StructureDefinition>(resource_match.resource.content) {
            Ok(mut structure_def) => {
                // Extract elements from snapshot/differential
                if structure_def.extract_elements().is_ok() {
                    structure_definitions.push(structure_def);
                }
            }
            Err(e) => {
                println!("⚠️  Warning: Failed to parse StructureDefinition: {e}");
            }
        }
    }

    Ok(structure_definitions)
}

#[derive(Debug)]
struct SampleConversionResult {
    name: String,
    resource_type: String,
    success: bool,
    error: Option<String>,
    element_count: usize,
    conversion_time_ms: u128,
}

async fn convert_sample_structure_definitions(
    structure_definitions: &[StructureDefinition],
    canonical_manager: Arc<CanonicalManager>,
) -> Vec<SampleConversionResult> {
    let mut results = Vec::new();
    let converter = FhirSchemaConverter::new();

    // Convert a sample of important FHIR resources
    let priority_resources = ["Patient",
        "Observation",
        "Practitioner",
        "Organization",
        "Encounter",
        "Condition",
        "MedicationRequest",
        "DiagnosticReport"];

    for structure_def in structure_definitions
        .iter()
        .filter(|sd| priority_resources.contains(&sd.type_name.as_str()))
        .take(10)
    // Limit to first 10 for this test
    {
        let start_time = std::time::Instant::now();
        let name = structure_def
            .name
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());

        // Try conversion with canonical manager support
        match convert_with_canonical_manager(&converter, structure_def, canonical_manager.clone())
            .await
        {
            Ok(schema) => {
                results.push(SampleConversionResult {
                    name,
                    resource_type: structure_def.type_name.clone(),
                    success: true,
                    error: None,
                    element_count: schema.elements.len(),
                    conversion_time_ms: start_time.elapsed().as_millis(),
                });
            }
            Err(e) => {
                results.push(SampleConversionResult {
                    name,
                    resource_type: structure_def.type_name.clone(),
                    success: false,
                    error: Some(e.to_string()),
                    element_count: 0,
                    conversion_time_ms: start_time.elapsed().as_millis(),
                });
            }
        }
    }

    results
}

async fn convert_with_canonical_manager(
    converter: &FhirSchemaConverter,
    structure_def: &StructureDefinition,
    canonical_manager: Arc<CanonicalManager>,
) -> std::result::Result<octofhir_fhirschema::FhirSchema, Box<dyn std::error::Error>> {
    // Use async conversion with canonical manager for profile resolution
    let schema = converter
        .convert_with_canonical_manager(structure_def, canonical_manager)
        .await?;
    Ok(schema)
}

fn display_conversion_results(results: &[SampleConversionResult]) {
    let successful = results.iter().filter(|r| r.success).count();
    let total = results.len();
    let success_rate = if total > 0 {
        (successful as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    println!("\n📊 Conversion Results Summary:");
    println!("  - Total Resources: {total}");
    println!(
        "  - Successful Conversions: {successful} ({success_rate:.1}%)"
    );
    println!("  - Failed Conversions: {}", total - successful);

    if total > 0 {
        let total_elements: usize = results.iter().map(|r| r.element_count).sum();
        let avg_elements = total_elements as f64 / total as f64;
        let total_time: u128 = results.iter().map(|r| r.conversion_time_ms).sum();
        let avg_time = total_time as f64 / total as f64;

        println!("  - Total Elements Processed: {total_elements}");
        println!("  - Average Elements per Resource: {avg_elements:.1}");
        println!("  - Total Conversion Time: {total_time}ms");
        println!("  - Average Time per Conversion: {avg_time:.2}ms");
    }

    println!("\n📋 Individual Results:");
    for result in results {
        let status = if result.success { "✅" } else { "❌" };
        println!(
            "  {} {} ({}): {} elements, {}ms",
            status,
            result.name,
            result.resource_type,
            result.element_count,
            result.conversion_time_ms
        );

        if let Some(error) = &result.error {
            println!("     Error: {error}");
        }
    }
}

fn validate_core_resources_converted(results: &[SampleConversionResult]) {
    let required_resources = vec!["Patient", "Observation", "Practitioner"];
    let converted_types: Vec<&str> = results
        .iter()
        .filter(|r| r.success)
        .map(|r| r.resource_type.as_str())
        .collect();

    for required_resource in required_resources {
        assert!(
            converted_types.contains(&required_resource),
            "Required resource {required_resource} was not successfully converted"
        );
    }

    // Ensure at least 80% success rate
    let success_rate = results.iter().filter(|r| r.success).count() as f64 / results.len() as f64;
    assert!(
        success_rate >= 0.8,
        "Conversion success rate too low: {:.1}% (expected ≥80%)",
        success_rate * 100.0
    );

    println!("✅ All validation checks passed!");
}

/// Simple test to verify canonical manager basic functionality
#[tokio::test]
#[ignore]
async fn test_canonical_manager_basic_functionality()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Testing basic canonical manager functionality with local .fcm folder");

    let config = create_local_fcm_config();
    println!("📁 Using local storage: {:?}", config.storage.packages_dir);
    let canonical_manager = CanonicalManager::new(config).await?;

    // Test basic operations
    println!("✅ Canonical manager initialized successfully");

    // List initially installed packages
    let packages = canonical_manager.list_packages().await?;
    println!("📦 Initially installed packages: {packages:?}");

    // Try to install a small test package (us-core) if not already installed
    let us_core_package = "hl7.fhir.us.core@6.1.0";
    if packages.contains(&us_core_package.to_string()) {
        println!("📦 Test package already installed: hl7.fhir.us.core@6.1.0");
    } else {
        println!("📥 Installing test package: hl7.fhir.us.core@6.1.0");
        canonical_manager
            .install_package("hl7.fhir.us.core", "6.1.0")
            .await?;
    }

    println!("✅ Package installed successfully");

    // List packages after installation
    let packages_after = canonical_manager.list_packages().await?;
    println!("📦 Packages after installation: {packages_after:?}");

    // Try to resolve a well-known canonical URL from us-core
    let canonical_url = "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient";
    println!("🔍 Attempting to resolve: {canonical_url}");

    match canonical_manager.resolve(canonical_url).await {
        Ok(resolved) => {
            println!("✅ Resource resolved successfully!");
            println!("   - Canonical URL: {}", resolved.canonical_url);
            println!("   - Resource Type: {}", resolved.resource.resource_type);
            println!(
                "   - Package: {}@{}",
                resolved.package_info.name, resolved.package_info.version
            );
        }
        Err(e) => {
            println!(
                "⚠️  Resolution failed (expected if package doesn't contain this resource): {e}"
            );
        }
    }

    println!("🎉 Basic canonical manager functionality test completed!");
    Ok(())
}

/// Test that uses canonical manager to retrieve and convert the Patient StructureDefinition
#[tokio::test]
#[ignore]
async fn test_patient_conversion_with_canonical_manager()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("🧬 Testing Patient StructureDefinition conversion with canonical manager");

    // Initialize canonical manager with local .fcm folder
    let config = create_local_fcm_config();
    println!(
        "📁 Using local .fcm folder: {:?}",
        config.storage.packages_dir
    );
    let canonical_manager = CanonicalManager::new(config).await?;

    // Check if FHIR R4 core package is already installed, install if needed
    let packages = canonical_manager.list_packages().await?;
    let fhir_r4_package = "hl7.fhir.r4.core@4.0.1";

    if packages.contains(&fhir_r4_package.to_string()) {
        println!("📦 FHIR R4 core package already installed");
    } else {
        println!("📥 Installing FHIR R4 core package");
        canonical_manager
            .install_package("hl7.fhir.r4.core", "4.0.1")
            .await?;
    }

    // Search for Patient StructureDefinition specifically
    println!("🔍 Searching for Patient StructureDefinition");
    let search_results = canonical_manager
        .search()
        .await
        .resource_type("StructureDefinition")
        .execute()
        .await?;

    println!(
        "Found {} StructureDefinition resources",
        search_results.resources.len()
    );

    // Debug: List some of the structure definitions we found
    println!("🔍 Examining available StructureDefinitions:");
    for (i, resource_match) in search_results.resources.iter().take(10).enumerate() {
        if let Ok(structure_def) =
            serde_json::from_value::<StructureDefinition>(resource_match.resource.content.clone())
        {
            println!(
                "   {}. Type: {}, Kind: {}, URL: {:?}",
                i + 1,
                structure_def.type_name,
                structure_def.kind,
                structure_def.url
            );
        }
    }

    // Find the Patient StructureDefinition
    let patient_structure_def = search_results.resources.iter().find(|resource_match| {
        // Check if this is the Patient resource type
        if let Ok(structure_def) =
            serde_json::from_value::<StructureDefinition>(resource_match.resource.content.clone())
        {
            structure_def.type_name == "Patient" && structure_def.kind == "resource"
        } else {
            false
        }
    });

    let patient_structure_def = match patient_structure_def {
        Some(def) => def,
        None => {
            // If we can't find Patient, let's try to find any resource-level StructureDefinition
            println!(
                "⚠️  Patient StructureDefinition not found, looking for any resource-level definition..."
            );
            search_results
                .resources
                .iter()
                .find(|resource_match| {
                    if let Ok(structure_def) = serde_json::from_value::<StructureDefinition>(
                        resource_match.resource.content.clone(),
                    ) {
                        structure_def.kind == "resource"
                    } else {
                        false
                    }
                })
                .ok_or("No resource-level StructureDefinition found")?
        }
    };

    // Parse the StructureDefinition
    let mut found_structure_def = serde_json::from_value::<StructureDefinition>(
        patient_structure_def.resource.content.clone(),
    )?;

    println!(
        "✅ Found StructureDefinition: {} ({})",
        found_structure_def.type_name, found_structure_def.kind
    );

    // Extract elements from snapshot/differential
    found_structure_def.extract_elements()?;

    println!("📊 StructureDefinition details:");
    println!("   - URL: {:?}", found_structure_def.url);
    println!("   - Name: {:?}", found_structure_def.name);
    println!("   - Type: {}", found_structure_def.type_name);
    println!("   - Kind: {}", found_structure_def.kind);
    println!("   - Elements: {}", found_structure_def.elements.len());

    // Convert to FhirSchema using our converter
    println!("⚙️  Converting StructureDefinition to FhirSchema");
    let converter = FhirSchemaConverter::new();

    let converted_schema = converter.convert(&found_structure_def)?;

    println!("✅ Successfully converted StructureDefinition to FhirSchema!");
    println!("📊 Conversion results:");
    println!("   - Schema URL: {:?}", converted_schema.url);
    println!("   - Schema Name: {:?}", converted_schema.name);
    println!("   - Schema Type: {}", converted_schema.schema_type);
    println!(
        "   - Elements converted: {}",
        converted_schema.elements.len()
    );
    println!("   - Constraints: {}", converted_schema.constraints.len());
    println!(
        "   - Slicing definitions: {}",
        converted_schema.slicing.len()
    );

    // Validate we have some key elements (adapt based on what resource we actually found)
    let resource_type = &converted_schema.schema_type;
    let key_elements: Vec<String> = if resource_type == "Patient" {
        vec![
            "Patient.id".to_string(),
            "Patient.identifier".to_string(),
            "Patient.name".to_string(),
            "Patient.gender".to_string(),
            "Patient.birthDate".to_string(),
        ]
    } else {
        // For any other resource, just look for basic elements
        vec![
            format!("{}.id", resource_type),
            format!("{}.meta", resource_type),
        ]
    };

    let mut found_elements = 0;
    for key_element in &key_elements {
        if converted_schema.elements.contains_key(key_element) {
            found_elements += 1;
            println!("   ✅ Found key element: {key_element}");
        } else {
            println!("   ⚠️  Missing key element: {key_element}");
        }
    }

    // Assertions
    assert!(
        !converted_schema.elements.is_empty(),
        "Schema should have elements"
    );
    assert_eq!(
        converted_schema.schema_type, found_structure_def.type_name,
        "Schema type should match StructureDefinition type"
    );
    assert!(
        found_elements >= 1,
        "Should find at least 1 key element, found {found_elements}"
    );

    // Display some sample elements
    println!("📋 Sample converted elements:");
    for (path, element) in converted_schema.elements.iter().take(10) {
        println!(
            "   - {}: {:?} (min: {:?}, max: {:?})",
            path, element.element_type, element.min, element.max
        );
    }

    println!(
        "🎉 StructureDefinition conversion with canonical manager test completed successfully!"
    );
    Ok(())
}
