use clap::Parser;
use octofhir_canonical_manager::{CanonicalManager, FcmConfig, PackageSpec};
use octofhir_fhirschema::{FhirSchema, StructureDefinition, translate};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Clone)]
#[command(name = "schema-generator")]
#[command(about = "Generate precompiled FHIR schemas")]
struct Args {
    #[arg(
        short,
        long,
        help = "FHIR version (r4, r4b, r5, r6)",
        default_value = "r4"
    )]
    version: String,

    #[arg(
        short,
        long,
        help = "Output directory",
        default_value = "octofhir-fhirschema/precompiled_schemas"
    )]
    output: PathBuf,

    #[arg(long, help = "Generate individual schema files instead of binary")]
    individual: bool,

    #[arg(long, help = "Include only core resource types")]
    core_only: bool,

    #[arg(long, help = "Generate schemas for all FHIR versions")]
    all_versions: bool,

    #[arg(long, help = "Verbose output")]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    // Create output directory
    fs::create_dir_all(&args.output)?;

    if args.all_versions {
        println!("🔧 Generating schemas for all FHIR versions");
        println!("📂 Output directory: {}", args.output.display());

        let versions = vec!["r4", "r4b", "r5", "r6"];

        // Initialize canonical manager once for all versions
        println!("🔧 Initializing Canonical Manager...");
        let config = FcmConfig::load().await?;
        let canonical_manager = CanonicalManager::new(config).await?;

        // Collect all package specs for parallel installation
        println!("📦 Preparing package specifications for all FHIR versions...");
        let package_specs: Vec<PackageSpec> = versions
            .iter()
            .map(|version| {
                let package_info = get_package_info(version)?;
                Ok(PackageSpec {
                    name: package_info.name,
                    version: package_info.version,
                    priority: 1,
                })
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;

        // Install all packages in parallel (4-8x faster!)
        println!("📥 Installing all FHIR packages in parallel...");
        canonical_manager
            .install_packages_parallel(package_specs)
            .await?;
        println!("✅ All packages installed successfully!");

        let mut total_schemas = 0;

        for version in versions {
            println!("\n🏭 Processing FHIR version: {version}");

            let mut version_args = args.clone();
            version_args.version = version.to_string();
            version_args.all_versions = false; // Prevent recursion

            let schemas = generate_schemas_with_manager(&version_args, &canonical_manager).await?;

            if args.individual {
                save_individual_schemas(&schemas, &args.output, version).await?;
            } else {
                save_binary_schemas(&schemas, &args.output, version).await?;
            }

            println!(
                "✅ Generated {} schemas for FHIR {}",
                schemas.len(),
                version
            );
            total_schemas += schemas.len();
        }

        println!(
            "\n🎉 Successfully generated {total_schemas} total schemas for all FHIR versions!"
        );
    } else {
        println!("🔧 Generating schemas for FHIR version: {}", args.version);
        println!("📂 Output directory: {}", args.output.display());

        let schemas = generate_schemas(&args).await?;

        if args.individual {
            save_individual_schemas(&schemas, &args.output, &args.version).await?;
        } else {
            save_binary_schemas(&schemas, &args.output, &args.version).await?;
        }

        println!("✅ Generated {} schemas successfully!", schemas.len());
    }

    Ok(())
}

async fn generate_schemas(
    args: &Args,
) -> Result<HashMap<String, FhirSchema>, Box<dyn std::error::Error>> {
    let package_info = get_package_info(&args.version)?;
    println!("📦 Using FHIR package: {}", package_info.name);

    // Initialize canonical manager with default config
    println!("🔧 Initializing Canonical Manager...");
    let config = FcmConfig::load().await?;
    let canonical_manager = CanonicalManager::new(config).await?;

    // Install the FHIR package
    println!(
        "📥 Installing FHIR package: {} version {}",
        package_info.name, package_info.version
    );
    canonical_manager
        .install_package(&package_info.name, &package_info.version)
        .await?;

    generate_schemas_with_manager(args, &canonical_manager).await
}

async fn generate_schemas_with_manager(
    args: &Args,
    canonical_manager: &CanonicalManager,
) -> Result<HashMap<String, FhirSchema>, Box<dyn std::error::Error>> {
    let mut schemas = HashMap::new();

    let package_info = get_package_info(&args.version)?;
    println!("📦 Using FHIR package: {}", package_info.name);

    // Search for all StructureDefinitions in the package using the canonical manager
    println!("🔍 Discovering StructureDefinitions in package...");

    // Debug: Let's see what packages are actually available
    println!("🔍 Checking all available packages first...");
    let all_packages_result = canonical_manager
        .search()
        .await
        .limit(1000)
        .execute()
        .await?;

    let mut package_names = std::collections::HashSet::new();
    for resource in &all_packages_result.resources {
        let package_name = &resource.index.package_name;
        package_names.insert(package_name.clone());
    }

    println!("📦 Available packages:");
    for package in &package_names {
        println!("   - {package}");
    }

    println!("🎯 Looking for package: {}", package_info.name);
    if !package_names.contains(&package_info.name) {
        println!(
            "⚠️  WARNING: Package {} not found in available packages!",
            package_info.name
        );
    }

    // Debug: Let's see ALL StructureDefinitions across all packages to understand the distribution
    println!("🔍 Checking StructureDefinitions across ALL packages...");
    let all_structdefs_result = canonical_manager
        .search()
        .await
        .resource_type("StructureDefinition")
        .limit(1000)
        .execute()
        .await?;

    println!(
        "📊 Found {} total StructureDefinitions across all packages",
        all_structdefs_result.resources.len()
    );

    let mut package_counts: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for resource in &all_structdefs_result.resources {
        let package_name = &resource.index.package_name;
        let struct_name = resource
            .resource
            .content
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown")
            .to_string();

        package_counts
            .entry(package_name.clone())
            .or_default()
            .push(struct_name);
    }

    for (package, structs) in &package_counts {
        println!("📦 {}: {} StructureDefinitions", package, structs.len());
        if structs.len() <= 20 {
            // Only show details for packages with few StructureDefinitions
            for struct_name in structs {
                println!("     - {struct_name}");
            }
        } else {
            println!(
                "     First 10: {}",
                structs
                    .iter()
                    .take(10)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!("     ... and {} more", structs.len() - 10);
        }
    }

    // Use pagination to get ALL StructureDefinitions from the package
    // Note: Filtering by package name ensures FHIR version isolation, as each
    // FHIR version has a distinct package (e.g., hl7.fhir.r4.core vs hl7.fhir.r5.core)
    println!("🔍 Collecting all StructureDefinitions from package (using pagination)...");

    let mut all_structure_definitions = Vec::new();
    let mut offset = 0;
    const BATCH_SIZE: usize = 1000; // Maximum allowed by canonical manager

    loop {
        println!("   Fetching batch starting at offset {offset}...");

        let search_result = canonical_manager
            .search()
            .await
            .resource_type("StructureDefinition")
            .package(&package_info.name) // Ensures FHIR version-specific results
            .limit(BATCH_SIZE)
            .offset(offset)
            .execute()
            .await?;

        let batch_size = search_result.resources.len();
        println!("   Found {batch_size} StructureDefinitions in this batch");

        // Debug: Print first few StructureDefinition names from this batch
        if batch_size > 0 && offset == 0 {
            println!("   Sample StructureDefinitions found:");
            for (i, resource) in search_result.resources.iter().take(5).enumerate() {
                let name = resource
                    .resource
                    .content
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");
                println!("     {}. {}", i + 1, name);
            }
            if batch_size > 5 {
                println!("     ... and {} more", batch_size - 5);
            }
        }

        if batch_size == 0 {
            break; // No more results
        }

        all_structure_definitions.extend(search_result.resources);
        offset += BATCH_SIZE;

        // If we got fewer results than requested, we're done
        if batch_size < BATCH_SIZE {
            break;
        }
    }

    println!(
        "📊 Found {} StructureDefinitions in package {}",
        all_structure_definitions.len(),
        package_info.name
    );

    println!(
        "🔄 Converting {} StructureDefinitions to FhirSchemas...",
        all_structure_definitions.len()
    );

    // Convert each StructureDefinition to FhirSchema
    for resolved_resource in all_structure_definitions {
        let structure_def_json = &resolved_resource.resource.content;

        // Get the name for logging
        let type_name = structure_def_json
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown");

        if args.verbose {
            println!("   Processing: {type_name}");
        }

        match serde_json::from_value::<StructureDefinition>(structure_def_json.clone()) {
            Ok(structure_def) => {
                // Include all schemas including Extension type
                if args.verbose && structure_def.type_name == "Extension" {
                    println!("   📋 Including Extension type: {type_name}");
                }

                match translate(structure_def, None) {
                    Ok(schema) => {
                        schemas.insert(type_name.to_string(), schema);
                        if args.verbose {
                            println!("   ✅ Converted: {type_name}");
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to convert {type_name}: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("⚠️  Failed to parse StructureDefinition for {type_name}: {e}");
            }
        }
    }

    println!("✅ Successfully converted {} schemas", schemas.len());
    Ok(schemas)
}

#[derive(Debug)]
struct PackageInfo {
    name: String,
    version: String,
}

fn get_package_info(fhir_version: &str) -> Result<PackageInfo, Box<dyn std::error::Error>> {
    // Use the correct packages and versions from the fs.get-ig.org registry
    let (name, version) = match fhir_version {
        "r4" => ("hl7.fhir.r4.core".to_string(), "4.0.1".to_string()),
        "r4b" => ("hl7.fhir.r4b.core".to_string(), "4.3.0".to_string()),
        "r5" => ("hl7.fhir.r5.core".to_string(), "5.0.0".to_string()),
        "r6" => ("hl7.fhir.r6.core".to_string(), "6.0.0-ballot3".to_string()),
        _ => return Err(format!("Unsupported FHIR version: {fhir_version}").into()),
    };

    Ok(PackageInfo { name, version })
}

async fn save_binary_schemas(
    schemas: &HashMap<String, FhirSchema>,
    output_dir: &Path,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_file = output_dir.join(format!("{version}_schemas.json"));
    let serialized =
        serde_json::to_vec(schemas).map_err(|e| format!("JSON serialization error: {e}"))?;
    fs::write(&output_file, serialized)?;
    println!("💾 Saved JSON schemas to: {}", output_file.display());

    Ok(())
}

async fn save_individual_schemas(
    schemas: &HashMap<String, FhirSchema>,
    output_dir: &Path,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let schemas_dir = output_dir.join(format!("{version}_schemas"));
    fs::create_dir_all(&schemas_dir)?;

    for (name, schema) in schemas {
        let schema_file = schemas_dir.join(format!("{name}.json"));
        let json = serde_json::to_string_pretty(schema)?;
        fs::write(&schema_file, json)?;
    }

    println!(
        "📁 Saved individual schema files to: {}",
        schemas_dir.display()
    );

    Ok(())
}
