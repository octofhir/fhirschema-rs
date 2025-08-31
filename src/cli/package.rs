// New package management CLI functions using FhirSchemaPackageManager

use crate::{
    FhirSchemaPackageManager, InstallOptions, PackageId, PackageManagerConfig, PackageSpec,
};
use octofhir_canonical_manager::{FcmConfig, RegistryConfig, StorageConfig};
use std::sync::Arc;

/// Create FCM config using local .fcm directory
fn create_fcm_config(use_local_cache: bool) -> FcmConfig {
    let cache_base = if use_local_cache {
        std::env::current_dir().unwrap().join(".fcm")
    } else {
        std::env::temp_dir().join("fhirschema-cache")
    };

    FcmConfig {
        registry: RegistryConfig::default(),
        packages: vec![],
        storage: StorageConfig {
            cache_dir: cache_base.join("cache"),
            index_dir: cache_base.join("index"),
            packages_dir: cache_base.join("packages"),
            max_cache_size: "2GB".to_string(),
        },
        optimization: Default::default(),
    }
}

/// Initialize package manager
async fn create_package_manager(
    use_local_cache: bool,
) -> Result<Arc<FhirSchemaPackageManager>, Box<dyn std::error::Error>> {
    let fcm_config = create_fcm_config(use_local_cache);
    let pm_config = PackageManagerConfig::default();

    let manager = FhirSchemaPackageManager::new(fcm_config, pm_config).await?;
    Ok(Arc::new(manager))
}

/// Parse package specification from string (e.g., "hl7.fhir.r4.core@4.0.1")
fn parse_package_spec(package_str: &str) -> Result<PackageSpec, Box<dyn std::error::Error>> {
    if let Some(at_pos) = package_str.find('@') {
        let name = &package_str[..at_pos];
        let version = &package_str[at_pos + 1..];
        Ok(PackageSpec::registry(name, version))
    } else {
        Err(
            format!("Invalid package specification '{package_str}'. Use format: name@version")
                .into(),
        )
    }
}

/// Install FHIR packages
pub async fn install_packages(
    package_specs: Vec<String>,
    force: bool,
    skip_dependencies: bool,
    use_local_cache: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Installing FHIR packages...");

    let manager = create_package_manager(use_local_cache).await?;

    // Parse package specifications
    let packages: Result<Vec<_>, _> = package_specs
        .iter()
        .map(|spec| parse_package_spec(spec))
        .collect();
    let packages = packages?;

    let options = InstallOptions {
        force,
        skip_dependencies,
        timeout_seconds: 300,
        validate: true,
        allow_prerelease: false,
        hooks: Default::default(),
    };

    let start_time = std::time::Instant::now();
    let result = manager.install_packages(&packages, Some(options)).await?;
    let duration = start_time.elapsed();

    println!("\n📊 Installation Results:");
    println!("  ✅ Installed: {} packages", result.installed.len());
    println!("  ⏭️  Skipped: {} packages", result.skipped.len());
    println!("  ❌ Failed: {} packages", result.failed.len());
    println!(
        "  📄 Converted: {} schemas",
        result.conversion_results.converted_schemas
    );
    println!("  ⏱️  Duration: {:.2}s", duration.as_secs_f64());

    if !result.failed.is_empty() {
        println!("\n❌ Failed packages:");
        for failure in &result.failed {
            println!(
                "  - {}: {} ({:?})",
                failure.package_id, failure.error, failure.category
            );
        }
    }

    if !result.conversion_results.failed.is_empty() {
        println!("\n⚠️  Conversion failures:");
        for failure in &result.conversion_results.failed {
            println!(
                "  - {}: {} ({:?})",
                failure.structure_definition_url, failure.error_message, failure.error_type
            );
        }
    }

    println!("\n🎉 Installation completed!");
    Ok(())
}

/// Show schema information by canonical URL
pub async fn show_schema_by_url(
    canonical_url: &str,
    use_local_cache: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Looking up schema: {canonical_url}");

    let manager = create_package_manager(use_local_cache).await?;

    match manager.get_schema(canonical_url).await {
        Some(schema) => {
            println!("\n📋 Schema Information:");
            println!("==================");
            println!("Type: {}", schema.schema_type);
            if let Some(name) = &schema.name {
                println!("Name: {name}");
            }
            if let Some(url) = &schema.url {
                println!("URL: {url}");
            }
            if let Some(version) = &schema.version {
                println!("Version: {version}");
            }
            if let Some(title) = &schema.title {
                println!("Title: {title}");
            }
            if let Some(description) = &schema.description {
                println!("Description: {description}");
            }
            if let Some(class) = &schema.class {
                println!("Class: {class}");
            }
            if let Some(kind) = &schema.kind {
                println!("Kind: {kind}");
            }

            println!("\n📊 Statistics:");
            println!("  - Elements: {}", schema.elements.len());
            println!("  - Constraints: {}", schema.constraints.len());
            println!("  - Slicing definitions: {}", schema.slicing.len());

            if !schema.elements.is_empty() {
                println!("\n🔗 Sample Elements:");
                for (path, element) in schema.elements.iter().take(10) {
                    let types = element
                        .element_type
                        .as_ref()
                        .map(|types| {
                            types
                                .iter()
                                .map(|t| t.code.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_else(|| "unknown".to_string());
                    println!(
                        "  - {}: {} (min: {:?}, max: {:?})",
                        path, types, element.min, element.max
                    );
                }
                if schema.elements.len() > 10 {
                    println!("  ... and {} more elements", schema.elements.len() - 10);
                }
            }
        }
        None => {
            println!("❌ Schema not found: {canonical_url}");
            println!("💡 Try installing the relevant FHIR package first");
        }
    }

    Ok(())
}

/// Uninstall a FHIR package
pub async fn uninstall_package(
    package_spec: &str,
    use_local_cache: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🗑️  Uninstalling package: {package_spec}");

    let manager = create_package_manager(use_local_cache).await?;
    let spec = parse_package_spec(package_spec)?;
    let package_id = PackageId::new(&spec.name, &spec.version);

    let uninstalled = manager.uninstall_package(&package_id).await?;

    if uninstalled {
        println!("✅ Successfully uninstalled: {package_id}");
    } else {
        println!("⚠️  Package was not installed: {package_id}");
    }

    Ok(())
}

/// List installed packages using the new package manager
pub async fn list_packages_new(use_local_cache: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 Listing installed packages...");

    let manager = create_package_manager(use_local_cache).await?;
    let packages = manager.list_packages().await;

    if packages.is_empty() {
        println!("📭 No packages are currently installed");
        println!("💡 Use 'install' command to install FHIR packages");
    } else {
        println!("\n📦 Installed Packages ({}):", packages.len());
        println!("=======================");

        for package_id in packages {
            if let Some(package) = manager.get_package(&package_id).await {
                println!(
                    "📄 {} (installed: {})",
                    package_id,
                    package.install_time.format("%Y-%m-%d %H:%M:%S")
                );
                println!("   - Schemas: {}", package.schemas.len());
                if let Some(title) = &package.metadata.title {
                    println!("   - Title: {title}");
                }
                if let Some(description) = &package.metadata.description {
                    println!("   - Description: {description}");
                }
                println!();
            } else {
                println!("📄 {package_id} (metadata unavailable)");
            }
        }

        // Show resource type statistics
        let resource_types = manager.get_resource_types().await;
        if !resource_types.is_empty() {
            println!("🔍 Available Resource Types ({}):", resource_types.len());
            let mut sorted_types = resource_types;
            sorted_types.sort();

            for (i, resource_type) in sorted_types.iter().take(20).enumerate() {
                let count = manager.get_schemas_by_type(resource_type).await.len();
                print!("{resource_type}: {count} ");
                if (i + 1) % 4 == 0 {
                    println!();
                }
            }
            if sorted_types.len() > 20 {
                println!("\n... and {} more resource types", sorted_types.len() - 20);
            } else {
                println!();
            }
        }
    }

    Ok(())
}

/// Search schemas using the new package manager
pub async fn search_schemas(
    query: &str,
    use_local_cache: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Searching schemas for: '{query}'");

    let manager = create_package_manager(use_local_cache).await?;

    // Try multiple search strategies
    let mut all_results = Vec::new();

    // 1. Search by resource type
    let type_results = manager.get_schemas_by_type(query).await;
    all_results.extend(type_results);

    // 2. Full-text search
    let search_results = manager.search_schemas(query).await;
    all_results.extend(search_results);

    // Deduplicate by canonical URL
    let mut seen_urls = std::collections::HashSet::new();
    all_results.retain(|schema| {
        if let Some(url) = &schema.url {
            seen_urls.insert(url.to_string())
        } else {
            true
        }
    });

    if all_results.is_empty() {
        println!("📭 No schemas found matching: '{query}'");
        println!("💡 Try searching for resource types like 'Patient', 'Observation', etc.");
    } else {
        println!("\n🎯 Found {} matching schemas:", all_results.len());
        println!("========================");

        for (i, schema) in all_results.iter().take(50).enumerate() {
            let name = schema.name.as_deref().unwrap_or("Unknown");
            let url = schema
                .url
                .as_ref()
                .map(|u| u.to_string())
                .unwrap_or_else(|| "no-url".to_string());
            let class = schema.class.as_deref().unwrap_or("unknown");

            println!("{:2}. {} ({})", i + 1, name, schema.schema_type);
            println!("    Class: {class} | URL: {url}");
            if let Some(description) = &schema.description {
                let desc = if description.len() > 100 {
                    format!("{}...", &description[..100])
                } else {
                    description.clone()
                };
                println!("    Description: {desc}");
            }
            println!();
        }

        if all_results.len() > 50 {
            println!("... and {} more results", all_results.len() - 50);
        }
    }

    Ok(())
}
