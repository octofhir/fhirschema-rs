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
                let (name, ver) = get_package_info(version)?;
                Ok(PackageSpec {
                    name,
                    version: ver,
                    priority: 1,
                    url: None,
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
    let (package_name, package_version) = get_package_info(&args.version)?;
    println!("📦 Using FHIR package: {}", package_name);

    // Initialize canonical manager with default config
    println!("🔧 Initializing Canonical Manager...");
    let config = FcmConfig::load().await?;
    let canonical_manager = CanonicalManager::new(config).await?;

    // Install the FHIR package
    println!(
        "📥 Installing FHIR package: {} version {}",
        package_name, package_version
    );
    canonical_manager
        .install_package(&package_name, &package_version)
        .await?;

    generate_schemas_with_manager(args, &canonical_manager).await
}

async fn generate_schemas_with_manager(
    args: &Args,
    canonical_manager: &CanonicalManager,
) -> Result<HashMap<String, FhirSchema>, Box<dyn std::error::Error>> {
    let mut schemas = HashMap::new();

    let (package_name, _) = get_package_info(&args.version)?;

    // Collect all StructureDefinitions from the package
    println!("📦 Loading schemas from: {}", package_name);
    let package_schemas =
        collect_schemas_from_package(canonical_manager, &package_name, args.verbose).await?;

    schemas.extend(package_schemas);

    println!(
        "✅ Successfully generated {} schemas from {}",
        schemas.len(),
        package_name
    );
    Ok(schemas)
}

/// Collects all StructureDefinitions from a single package and converts them to FhirSchemas.
///
/// Queries the database directly using find_by_type_and_package to avoid
/// any caching issues that could cause deduplication.
async fn collect_schemas_from_package(
    canonical_manager: &CanonicalManager,
    package_name: &str,
    verbose: bool,
) -> Result<HashMap<String, FhirSchema>, Box<dyn std::error::Error>> {
    let mut schemas = HashMap::new();
    let mut parse_failures: Vec<(String, String)> = Vec::new();
    let mut convert_failures: Vec<(String, String)> = Vec::new();

    // Query database directly for all StructureDefinitions in this package
    let resource_indices = canonical_manager
        .find_by_type_and_package("StructureDefinition", package_name)
        .await?;

    println!(
        "   📊 Found {} StructureDefinitions in package {}",
        resource_indices.len(),
        package_name
    );

    // Convert each StructureDefinition to FhirSchema
    for resource_index in resource_indices {
        if verbose {
            eprintln!(
                "Resolving: {} (FHIR version: {})",
                resource_index.canonical_url, resource_index.fhir_version
            );
        }

        // Load the full resource content from the specific FHIR version
        let resolved = canonical_manager
            .resolve_with_fhir_version(&resource_index.canonical_url, &resource_index.fhir_version)
            .await?;
        let structure_def_json = &resolved.resource.content;

        // Debug: check if we're getting the right version
        let resource_fhir_version = structure_def_json
            .get("fhirVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let resource_version = structure_def_json
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        if verbose && resource_fhir_version != resource_index.fhir_version {
            eprintln!(
                "WARNING: Version mismatch for {} - expected FHIR {}, got FHIR {} (resource version: {})",
                resource_index.canonical_url,
                resource_index.fhir_version,
                resource_fhir_version,
                resource_version
            );
        }

        // Use 'id' as the unique key since 'name' can have collisions
        // (e.g., multiple extensions named 'replaces' with different urls)
        let schema_id = structure_def_json
            .get("id")
            .and_then(|n| n.as_str())
            .unwrap_or_else(|| {
                structure_def_json
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown")
            });

        let display_name = structure_def_json
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or(schema_id);

        if verbose {
            println!("   Processing: {} (id: {})", display_name, schema_id);
        }

        match serde_json::from_value::<StructureDefinition>(structure_def_json.clone()) {
            Ok(structure_def) => {
                if verbose && structure_def.type_name == "Extension" {
                    println!("   📋 Including Extension type: {}", display_name);
                }

                match translate(structure_def, None) {
                    Ok(schema) => {
                        schemas.insert(schema_id.to_string(), schema);
                        if verbose {
                            println!("   ✅ Converted: {} -> {}", display_name, schema_id);
                        }
                    }
                    Err(e) => {
                        convert_failures.push((schema_id.to_string(), e.to_string()));
                    }
                }
            }
            Err(e) => {
                parse_failures.push((schema_id.to_string(), e.to_string()));
            }
        }
    }

    // Report failures at the end
    if !parse_failures.is_empty() {
        println!(
            "\n   ⚠️  {} StructureDefinitions failed to parse:",
            parse_failures.len()
        );
        for (name, error) in &parse_failures {
            println!("      - {}: {}", name, error);
        }
    }

    if !convert_failures.is_empty() {
        println!(
            "\n   ⚠️  {} StructureDefinitions failed to convert:",
            convert_failures.len()
        );
        for (name, error) in &convert_failures {
            println!("      - {}: {}", name, error);
        }
    }

    Ok(schemas)
}

fn get_package_info(fhir_version: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    // Use the correct packages and versions from the FHIR registry
    match fhir_version {
        "r4" => Ok(("hl7.fhir.r4.core".to_string(), "4.0.1".to_string())),
        "r4b" => Ok(("hl7.fhir.r4b.core".to_string(), "4.3.0".to_string())),
        "r5" => Ok(("hl7.fhir.r5.core".to_string(), "5.0.0".to_string())),
        "r6" => Ok(("hl7.fhir.r6.core".to_string(), "6.0.0-ballot3".to_string())),
        _ => Err(format!("Unsupported FHIR version: {fhir_version}").into()),
    }
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
