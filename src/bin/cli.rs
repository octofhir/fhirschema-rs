#[cfg(feature = "cli")]
use clap::{Parser, Subcommand};
#[cfg(feature = "cli")]
use octofhir_canonical_manager::{CanonicalManager, FcmConfig, RegistryConfig, StorageConfig};
#[cfg(feature = "cli")]
use octofhir_fhirschema::validation::ValidationSeverity;
#[cfg(feature = "cli")]
use octofhir_fhirschema::*;
#[cfg(feature = "cli")]
use std::path::PathBuf;

#[cfg(feature = "cli")]
#[derive(Parser)]
#[command(name = "octofhir-fhirschema")]
#[command(about = "A high-performance FHIR schema management and validation tool")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum Commands {
    /// Validate a FHIR schema
    Validate {
        /// Path to the schema file
        #[arg(short, long)]
        schema: PathBuf,
        /// Output format (json, text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Show schema information
    Info {
        /// Path to the schema file
        #[arg(short, long)]
        schema: PathBuf,
    },
    /// Convert StructureDefinition to FHIRSchema
    ConvertStructureDefinition {
        /// Path to the StructureDefinition file
        #[arg(short, long)]
        input: PathBuf,
        /// Output FHIRSchema file
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Download and convert FHIR packages using canonical manager
    Download {
        /// FHIR package name (e.g., hl7.fhir.r4.core)
        #[arg(short, long)]
        package: String,
        /// Package version (e.g., 4.0.1)
        #[arg(short, long)]
        version: String,
        /// Output directory for converted schemas
        #[arg(short, long)]
        output: PathBuf,
        /// Resource types to convert (comma-separated, e.g., Patient,Observation)
        #[arg(short, long)]
        resource_types: Option<String>,
        /// Use local .fcm folder for caching
        #[arg(long, default_value = "true")]
        use_local_cache: bool,
    },
    /// List installed FHIR packages
    List {
        /// Use local .fcm folder for caching
        #[arg(long, default_value = "true")]
        use_local_cache: bool,
    },
    /// Search for StructureDefinitions in installed packages
    Search {
        /// Search query (resource type or name pattern)
        #[arg(short, long)]
        query: String,
        /// Use local .fcm folder for caching
        #[arg(long, default_value = "true")]
        use_local_cache: bool,
    },
}

#[cfg(feature = "cli")]
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { schema, format } => {
            validate_schema(&schema, &format).await?;
        }
        Commands::Info { schema } => {
            show_schema_info(&schema).await?;
        }
        Commands::ConvertStructureDefinition { input, output } => {
            convert_structure_definition(&input, &output).await?;
        }
        Commands::Download {
            package,
            version,
            output,
            resource_types,
            use_local_cache,
        } => {
            download_and_convert(
                &package,
                &version,
                &output,
                resource_types.as_deref(),
                use_local_cache,
            )
            .await?;
        }
        Commands::List { use_local_cache } => {
            list_packages(use_local_cache).await?;
        }
        Commands::Search {
            query,
            use_local_cache,
        } => {
            search_structure_definitions(&query, use_local_cache).await?;
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
async fn validate_schema(
    schema_path: &PathBuf,
    format: &str,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(schema_path)?;
    let schema: FhirSchema = serde_json::from_str(&content)?;

    let validator = BasicSchemaValidator;
    let issues = validator.validate_schema(&schema)?;

    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&issues)?);
        }
        _ => {
            if issues.is_empty() {
                println!("✅ Schema validation passed");
            } else {
                println!(
                    "❌ Schema validation failed with {} issues:\n",
                    issues.len()
                );
                for issue in issues {
                    println!(
                        "  {} [{}]: {}",
                        match issue.severity {
                            ValidationSeverity::Error => "ERROR",
                            ValidationSeverity::Warning => "WARN",
                            ValidationSeverity::Information => "INFO",
                        },
                        issue.code,
                        issue.message
                    );
                    if let Some(path) = issue.path {
                        println!("    at: {path}");
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
async fn show_schema_info(
    schema_path: &PathBuf,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(schema_path)?;
    let schema: FhirSchema = serde_json::from_str(&content)?;

    println!("Schema Information:");
    println!("==================");
    println!("Type: {}", schema.schema_type);
    if let Some(name) = schema.name {
        println!("Name: {name}");
    }
    if let Some(url) = schema.url {
        println!("URL: {url}");
    }
    if let Some(version) = schema.version {
        println!("Version: {version}");
    }
    if let Some(description) = schema.description {
        println!("Description: {description}");
    }

    println!("\nElements: {}", schema.elements.len());
    for (_path, element) in schema.elements.iter().take(5) {
        println!("  - {element}");
    }
    if schema.elements.len() > 5 {
        println!("  ... and {} more", schema.elements.len() - 5);
    }

    println!("Constraints: {}", schema.constraints.len());
    println!("Slicing definitions: {}", schema.slicing.len());

    Ok(())
}

#[cfg(feature = "cli")]
async fn convert_structure_definition(
    input_path: &PathBuf,
    output_path: &PathBuf,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(input_path)?;
    let mut structure_def: StructureDefinition = serde_json::from_str(&content)?;

    // Extract elements from snapshot or differential
    structure_def.extract_elements()?;

    let converter = FhirSchemaConverter::new();
    let schema = converter.convert(&structure_def)?;

    let output_content = serde_json::to_string_pretty(&schema)?;
    std::fs::write(output_path, output_content)?;

    println!(
        "✅ Converted StructureDefinition {} to FHIRSchema {}",
        input_path.display(),
        output_path.display()
    );

    // Show some stats
    let stats = format!(
        "Generated {} elements, {} constraints, {} slicing definitions",
        schema.elements.len(),
        schema.constraints.len(),
        schema.slicing.len()
    );
    println!("📊 {stats}");

    Ok(())
}

#[cfg(feature = "cli")]
fn create_canonical_manager_config(use_local_cache: bool) -> FcmConfig {
    if use_local_cache {
        // Use local .fcm folder in current directory
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let fcm_dir = current_dir.join(".fcm");

        FcmConfig {
            registry: RegistryConfig::default(),
            packages: vec![],
            storage: StorageConfig {
                cache_dir: fcm_dir.join("cache"),
                index_dir: fcm_dir.join("index"),
                packages_dir: fcm_dir.join("packages"),
                max_cache_size: "2GB".to_string(),
            },
        }
    } else {
        // Use default system locations
        FcmConfig::default()
    }
}

#[cfg(feature = "cli")]
async fn download_and_convert(
    package: &str,
    version: &str,
    output_dir: &PathBuf,
    resource_types: Option<&str>,
    use_local_cache: bool,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!(
        "🚀 Downloading and converting FHIR package: {package}@{version}"
    );

    // Create output directory if it doesn't exist
    std::fs::create_dir_all(output_dir)?;

    // Initialize canonical manager
    println!("🔧 Initializing FHIR canonical manager...");

    let config = create_canonical_manager_config(use_local_cache);
    let canonical_manager = CanonicalManager::new(config).await?;

    // Check if package is already installed
    let packages = canonical_manager.list_packages().await?;
    let package_key = format!("{package}@{version}");

    if packages.contains(&package_key) {
        println!("📦 Package already installed: {package_key}");
        println!("🔄 Ensuring search index is up to date...");
        canonical_manager.rebuild_index().await?;
        println!("✅ Search index updated");
    } else {
        println!("📥 Installing package: {package_key}");
        canonical_manager.install_package(package, version).await?;
        println!("✅ Package installed successfully");
        println!("🔄 Rebuilding search index after installation...");
        // The install_package method should automatically rebuild the index, but let's be explicit
        canonical_manager.rebuild_index().await?;
        println!("✅ Search index rebuilt");
    }

    // Search for StructureDefinitions
    println!("🔍 Searching for StructureDefinitions...");
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

    // Filter by resource types if specified
    let target_types = resource_types.map(|types| {
        types
            .split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });

    let converter = FhirSchemaConverter::new();
    let mut converted_count = 0;

    // Track skip reasons for detailed reporting
    #[derive(Default)]
    struct SkipStats {
        parse_failed: u32,
        type_filtered: u32,
        unsupported_kind: u32,
        element_extraction_failed: u32,
        conversion_failed: u32,
    }
    let mut skip_stats = SkipStats::default();

    for resource_match in search_results.resources {
        // Skip OpenAPI/JSON Schema files (from openapi folder)
        if let Some(schema_field) = resource_match.resource.content.get("$schema") {
            if let Some(schema_str) = schema_field.as_str() {
                if schema_str.contains("json-schema.org") {
                    // This is a JSON Schema file from openapi folder, skip it
                    continue;
                }
            }
        }

        // Skip resources with JSON Schema ID pattern
        if let Some(id_field) = resource_match.resource.content.get("id") {
            if let Some(id_str) = id_field.as_str() {
                if id_str.contains("json-schema") {
                    // This is likely a JSON Schema file from openapi folder, skip it
                    continue;
                }
            }
        }

        // Parse StructureDefinition
        let structure_def_result =
            serde_json::from_value::<StructureDefinition>(resource_match.resource.content.clone());

        let mut structure_def = match structure_def_result {
            Ok(sd) => sd,
            Err(e) => {
                // Show detailed information about the resource that failed to parse
                println!("❌ Failed to parse StructureDefinition:");
                println!("   📍 Canonical URL: {}", resource_match.index.canonical_url);
                println!("   📦 Package: {}@{}", resource_match.index.package_name, resource_match.index.package_version);
                if let Some(resource_type) = resource_match.resource.content.get("resourceType") {
                    println!("   🏷️  Resource Type: {}", resource_type);
                }
                if let Some(id) = resource_match.resource.content.get("id") {
                    println!("   🆔 Resource ID: {}", id);
                }
                if let Some(name) = resource_match.resource.content.get("name") {
                    println!("   📝 Name: {}", name);
                }
                println!("   ⚠️  Parse Error: {}", e);
                println!();
                skip_stats.parse_failed += 1;
                continue;
            }
        };

        // Check if we should process this resource type
        if let Some(ref types) = target_types {
            if !types.contains(&structure_def.type_name) {
                skip_stats.type_filtered += 1;
                continue;
            }
        }

        // Process all supported StructureDefinition kinds
        let supported_kinds = ["resource", "complex-type", "primitive-type", "logical"];
        if !supported_kinds.contains(&structure_def.kind.as_str()) {
            skip_stats.unsupported_kind += 1;
            continue;
        }

        println!(
            "⚙️  Converting: {} ({})",
            structure_def.type_name, structure_def.kind
        );

        // Extract elements
        if let Err(_e) = structure_def.extract_elements() {
            skip_stats.element_extraction_failed += 1;
            continue;
        }

        // Convert to FhirSchema
        let schema = match converter.convert(&structure_def) {
            Ok(s) => s,
            Err(_e) => {
                skip_stats.conversion_failed += 1;
                continue;
            }
        };

        // Write to file
        let filename = format!("{}.fhirschema.json", structure_def.type_name.to_lowercase());
        let output_path = output_dir.join(&filename);
        let output_content = serde_json::to_string_pretty(&schema)?;

        std::fs::write(&output_path, output_content)?;
        println!("✅ Converted {} -> {}", structure_def.type_name, filename);
        converted_count += 1;
    }

    // Calculate total skipped
    let total_skipped = skip_stats.parse_failed + skip_stats.type_filtered + skip_stats.unsupported_kind + skip_stats.element_extraction_failed + skip_stats.conversion_failed;

    println!("\n🎉 Conversion completed!");
    println!("📊 Converted: {converted_count} schemas");
    println!("📊 Skipped: {total_skipped} resources");

    if total_skipped > 0 {
        println!("📋 Resources skipped and reasons:");
        if skip_stats.parse_failed > 0 {
            println!("   • {} resources failed to parse - Invalid JSON structure or malformed StructureDefinition", skip_stats.parse_failed);
        }
        if skip_stats.type_filtered > 0 {
            println!("   • {} resources filtered by type - Resource type not in the specified filter list", skip_stats.type_filtered);
        }
        if skip_stats.unsupported_kind > 0 {
            println!("   • {} resources have unsupported kind - Only StructureDefinitions with supported kinds are converted (supported: resource, complex-type, primitive-type, logical)", skip_stats.unsupported_kind);
        }
        if skip_stats.element_extraction_failed > 0 {
            println!("   • {} resources failed element extraction - Unable to process the element definitions from the StructureDefinition", skip_stats.element_extraction_failed);
        }
        if skip_stats.conversion_failed > 0 {
            println!("   • {} resources failed schema conversion - Error occurred while converting StructureDefinition to FhirSchema format", skip_stats.conversion_failed);
        }
    }

    println!("📁 Output directory: {}", output_dir.display());

    Ok(())
}

#[cfg(feature = "cli")]
async fn list_packages(
    use_local_cache: bool,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("📦 Listing installed FHIR packages...");

    let config = create_canonical_manager_config(use_local_cache);
    let canonical_manager = CanonicalManager::new(config).await?;

    let packages = canonical_manager.list_packages().await?;

    if packages.is_empty() {
        println!("No packages installed.");
    } else {
        println!("Installed packages ({}):", packages.len());
        for package in packages {
            println!("  - {package}");
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
async fn search_structure_definitions(
    query: &str,
    use_local_cache: bool,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!(
        "🔍 Searching for StructureDefinitions matching: '{query}'"
    );

    let config = create_canonical_manager_config(use_local_cache);
    let canonical_manager = CanonicalManager::new(config).await?;


    println!("🔍 Executing search for StructureDefinitions...");
    let search_results = canonical_manager
        .search()
        .await
        .resource_type("StructureDefinition")
        .execute()
        .await?;

    println!(
        "📊 Search found {} total StructureDefinition resources",
        search_results.resources.len()
    );

    let mut matches = Vec::new();
    let query_lower = query.to_lowercase();


    println!("🔍 Filtering results for query: '{query}'");

    for resource_match in search_results.resources {
        if let Ok(structure_def) =
            serde_json::from_value::<StructureDefinition>(resource_match.resource.content.clone())
        {
            // Comprehensive matching logic - search in multiple fields
            let matches_query =
                // Resource type name
                structure_def.type_name.to_lowercase().contains(&query_lower) ||
                // Resource name
                structure_def.name.as_ref().is_some_and(|n| n.to_lowercase().contains(&query_lower)) ||
                // Resource title
                structure_def.title.as_ref().is_some_and(|t| t.to_lowercase().contains(&query_lower)) ||
                // Resource description
                structure_def.description.as_ref().is_some_and(|d| d.to_lowercase().contains(&query_lower)) ||
                // Resource URL
                structure_def.url.as_ref().is_some_and(|u| u.to_string().to_lowercase().contains(&query_lower)) ||
                // Resource kind
                structure_def.kind.to_lowercase().contains(&query_lower) ||
                // Package information (via index)
                resource_match.index.package_name.to_lowercase().contains(&query_lower) ||
                resource_match.index.package_version.to_lowercase().contains(&query_lower) ||
                // Canonical URL from index
                resource_match.index.canonical_url.to_lowercase().contains(&query_lower);

            if matches_query {
                matches.push((structure_def, resource_match.index.clone()));
            }
        }
    }

    if matches.is_empty() {
        println!("No StructureDefinitions found matching '{query}'");
        println!("💡 Try searching for:");
        println!("  - Resource types (e.g., Patient, Observation)");
        println!("  - Resource names or titles");
        println!("  - Package names (e.g., us.core, r4.core)");
        println!("  - URLs or descriptions");
    } else {
        println!("Found {} matching StructureDefinitions:", matches.len());

        // Group by package for better organization
        use std::collections::HashMap;
        let mut by_package: HashMap<String, Vec<&(StructureDefinition, _)>> = HashMap::new();

        for match_item in &matches {
            let package_key = format!(
                "{}@{}",
                match_item.1.package_name, match_item.1.package_version
            );
            by_package
                .entry(package_key)
                .or_insert_with(Vec::new)
                .push(match_item);
        }

        for (package, package_matches) in by_package.iter().take(5) {
            println!("\n📦 Package: {package}");
            for (structure_def, _) in package_matches.iter().take(5) {
                println!(
                    "  - {} ({}) - {}",
                    structure_def.type_name,
                    structure_def.kind,
                    structure_def.name.as_deref().unwrap_or("no name")
                );
                if let Some(ref url) = structure_def.url {
                    println!("    URL: {url}");
                }
                if let Some(ref description) = structure_def.description {
                    let desc_preview = if description.len() > 100 {
                        format!("{}...", &description[..97])
                    } else {
                        description.clone()
                    };
                    println!("    Description: {desc_preview}");
                }
            }
            if package_matches.len() > 5 {
                println!(
                    "    ... and {} more from this package",
                    package_matches.len() - 5
                );
            }
        }

        if by_package.len() > 5 {
            println!(
                "\n... and {} more packages with matches",
                by_package.len() - 5
            );
        }

        println!("\n📊 Summary:");
        println!("  - Total matches: {}", matches.len());
        println!("  - Packages with matches: {}", by_package.len());

        // Show match reasons for first few results
        println!("\n🎯 Match details (first 3):");
        for (i, (structure_def, resource_index)) in matches.iter().take(3).enumerate() {
            println!(
                "  {}. {} from {}@{}",
                i + 1,
                structure_def.type_name,
                resource_index.package_name,
                resource_index.package_version
            );
            let mut reasons = Vec::new();
            if structure_def
                .type_name
                .to_lowercase()
                .contains(&query_lower)
            {
                reasons.push("resource type");
            }
            if structure_def
                .name
                .as_ref()
                .is_some_and(|n| n.to_lowercase().contains(&query_lower))
            {
                reasons.push("name");
            }
            if structure_def
                .description
                .as_ref()
                .is_some_and(|d| d.to_lowercase().contains(&query_lower))
            {
                reasons.push("description");
            }
            if resource_index
                .package_name
                .to_lowercase()
                .contains(&query_lower)
            {
                reasons.push("package name");
            }
            if resource_index
                .canonical_url
                .to_lowercase()
                .contains(&query_lower)
            {
                reasons.push("canonical URL");
            }
            if !reasons.is_empty() {
                println!("     Matched in: {}", reasons.join(", "));
            }
        }
    }

    Ok(())
}

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("CLI feature is not enabled. Please compile with --features cli");
    std::process::exit(1);
}
