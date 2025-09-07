use clap::{Arg, Command};
use octofhir_fhirschema::core::FhirVersion;
use octofhir_fhirschema::provider::SchemaBuilder;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let matches = Command::new("schema-builder")
        .version("1.0.0")
        .author("Octofhir Team")
        .about("Build precompiled FHIR schemas for embedded providers")
        .arg(
            Arg::new("output-dir")
                .short('o')
                .long("output-dir")
                .value_name("DIR")
                .help("Output directory for precompiled schemas")
                .default_value("precompiled_schemas")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("fhir-version")
                .short('f')
                .long("fhir-version")
                .value_name("VERSION")
                .help("FHIR version to build (r4, r4b, r5, r6) or 'all'")
                .default_value("all"),
        )
        .arg(
            Arg::new("package")
                .short('p')
                .long("package")
                .value_name("PACKAGE")
                .help("FHIR package name (if not using core packages)"),
        )
        .arg(
            Arg::new("package-version")
                .long("package-version")
                .value_name("VERSION")
                .help("FHIR package version (if not using core packages)"),
        )
        .get_matches();

    let output_dir: PathBuf = matches.get_one::<PathBuf>("output-dir").unwrap().clone();
    let version_str: &String = matches.get_one("fhir-version").unwrap();

    println!("🚀 Building precompiled FHIR schemas...");
    println!("📁 Output directory: {}", output_dir.display());

    // Create output directory
    tokio::fs::create_dir_all(&output_dir).await?;

    let results = if version_str == "all" {
        println!("🔧 Building all FHIR versions...");
        SchemaBuilder::build_all_versions(&output_dir).await?
    } else {
        // Build specific version
        let fhir_version = parse_fhir_version(version_str)?;
        println!("🔧 Building FHIR {fhir_version}...");

        let builder = SchemaBuilder::new(fhir_version).await?;

        let package_name = if let Some(pkg) = matches.get_one::<String>("package") {
            pkg.clone()
        } else {
            get_default_package_name(fhir_version)
        };

        let package_version = if let Some(ver) = matches.get_one::<String>("package-version") {
            ver.clone()
        } else {
            fhir_version.package_version().to_string()
        };

        let output_file = output_dir.join(format!("{}_schemas.bin", fhir_version.short_name()));

        let result = builder
            .build_and_save_schemas(&package_name, &package_version, output_file)
            .await?;
        vec![result]
    };

    // Print summary
    println!("\n🎉 Schema building completed!");
    println!("═══════════════════════════════════════");

    let mut total_schemas = 0;
    let mut total_time = 0;
    let mut total_size = 0;

    for result in &results {
        result.print_summary();
        total_schemas += result.schema_count;
        total_time += result.build_duration_ms;
        total_size += result.file_size_bytes;
    }

    println!("═══════════════════════════════════════");
    println!(
        "📊 Total: {} schemas, {}ms, {}",
        total_schemas,
        total_time,
        format_file_size(total_size)
    );

    // Verify files can be loaded
    println!("\n🔍 Verifying generated files...");
    for result in &results {
        match tokio::fs::read(&result.output_path).await {
            Ok(data) => {
                match serde_json::from_slice::<Vec<octofhir_fhirschema::types::FhirSchema>>(&data) {
                    Ok(schemas) => {
                        println!(
                            "✅ {} - {} schemas loaded successfully",
                            result.output_path.display(),
                            schemas.len()
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "❌ {} - Failed to deserialize: {}",
                            result.output_path.display(),
                            e
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "❌ {} - Failed to read: {}",
                    result.output_path.display(),
                    e
                );
            }
        }
    }

    println!("\n✨ All done! You can now use the embedded provider with precompiled schemas.");

    Ok(())
}

fn parse_fhir_version(version_str: &str) -> Result<FhirVersion, String> {
    match version_str.to_lowercase().as_str() {
        "r4" => Ok(FhirVersion::R4),
        "r4b" => Ok(FhirVersion::R4B),
        "r5" => Ok(FhirVersion::R5),
        "r6" => Ok(FhirVersion::R6),
        _ => Err(format!("Unsupported FHIR version: {version_str}")),
    }
}

fn get_default_package_name(version: FhirVersion) -> String {
    match version {
        FhirVersion::R4 => "hl7.fhir.r4.core",
        FhirVersion::R4B => "hl7.fhir.r4b.core",
        FhirVersion::R5 => "hl7.fhir.r5.core",
        FhirVersion::R6 => "hl7.fhir.r6.core",
    }
    .to_string()
}

fn format_file_size(size: u64) -> String {
    let size = size as f64;
    if size < 1024.0 {
        format!("{} bytes", size as u64)
    } else if size < 1024.0 * 1024.0 {
        format!("{:.2} KB", size / 1024.0)
    } else if size < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} MB", size / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", size / (1024.0 * 1024.0 * 1024.0))
    }
}
