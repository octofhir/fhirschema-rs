//! Download command implementation for fetching StructureDefinitions from FHIR registry.

use clap::Args;
use std::path::PathBuf;
use std::fs;
use anyhow::{Context, Result};
use fhirschema_converter::StructureDefinitionConverter;
use tracing::{info, error, warn};
use reqwest;
use serde_json::Value;
use tar::Archive;
use flate2::read::GzDecoder;
use std::io::{Read, Cursor};

/// Download StructureDefinitions from FHIR registry and convert to FHIRSchema
#[derive(Args)]
pub struct DownloadCommand {
    /// FHIR registry URL (defaults to official HL7 registry)
    #[arg(short, long, default_value = "https://packages.fhir.org")]
    pub registry: String,

    /// Package ID to download (e.g., "hl7.fhir.r4.core" or "hl7.fhir.r4.core#4.0.1")
    #[arg(short, long)]
    pub package: String,

    /// Package version (defaults to latest, can also be specified with package#version format)
    #[arg(short = 'v', long)]
    pub version: Option<String>,

    /// Output directory for downloaded and converted schemas
    #[arg(short, long, default_value = "./.output")]
    pub output: PathBuf,

    /// Output format (json, yaml, or ndjson)
    #[arg(short, long, default_value = "ndjson")]
    pub format: String,

    /// Convert to FHIRSchema format (default: true)
    #[arg(long, default_value = "true")]
    pub convert: bool,

    /// Skip download if package already exists locally
    #[arg(long)]
    pub skip_download: bool,
}

impl DownloadCommand {
    /// Execute the download command
    pub async fn execute(&self) -> Result<()> {
        // Parse package name and version from package parameter
        let (package_name, package_version) = self.parse_package_and_version()?;

        info!("Downloading package {} (version: {}) from registry {}",
              package_name, package_version.as_deref().unwrap_or("latest"), self.registry);

        // Validate format
        if !matches!(self.format.as_str(), "json" | "yaml" | "ndjson") {
            anyhow::bail!("Unsupported format: {}. Use 'json', 'yaml', or 'ndjson'", self.format);
        }

        // Create output directory
        fs::create_dir_all(&self.output)
            .with_context(|| format!("Failed to create output directory: {}", self.output.display()))?;

        // Create HTTP client
        let client = reqwest::Client::new();

        // Determine final version to download
        let final_version = if let Some(version) = package_version {
            version
        } else {
            // For now, require explicit version specification
            // TODO: Implement manifest fetching for latest version if needed
            anyhow::bail!("Package version must be specified explicitly (use package#version format)")
        };

        info!("Using package version: {}", final_version);

        // Check if package already exists and skip download if requested
        let package_id = format!("{}-{}", package_name, final_version);
        let package_dir = self.output.join("packages").join(&package_id);

        if self.skip_download && package_dir.exists() {
            info!("Package {} already exists locally, skipping download", package_id);
            println!("📦 Package {} found locally, skipping download", package_id);

            // Process existing package if it exists
            self.process_existing_package(&package_id).await?;
        } else {
            // Download package using npm-style URL format: {registry}/{package}/-/{package}-{version}.tgz
            let package_url = format!("{}/{}/-/{}-{}.tgz", self.registry, package_name, package_name, final_version);
            let package_response = client.get(&package_url)
                .send()
                .await
                .with_context(|| format!("Failed to download package from {}", package_url))?;

            if !package_response.status().is_success() {
                anyhow::bail!("Failed to download package: HTTP {}", package_response.status());
            }

            let package_data = package_response.bytes()
                .await
                .with_context(|| "Failed to read package data")?;

            // Extract and process StructureDefinitions
            self.process_package(&package_data).await?;
        }

        info!("Successfully downloaded and processed package: {}", self.package);
        println!("✓ Download completed: {}", self.output.display());

        Ok(())
    }

    /// Process existing package without downloading
    async fn process_existing_package(&self, package_id: &str) -> Result<()> {
        info!("Processing existing package: {}", package_id);

        let package_dir = self.output.join("packages").join(package_id);
        let raw_dir = package_dir.join("raw");

        if !raw_dir.exists() {
            anyhow::bail!("Package {} exists but raw files directory not found: {}", package_id, raw_dir.display());
        }

        // Read existing raw StructureDefinition files
        let structure_definitions = self.load_existing_raw_files(&raw_dir)?;

        if structure_definitions.is_empty() {
            warn!("No StructureDefinitions found in existing package: {}", package_id);
            return Ok(());
        }

        info!("Found {} existing StructureDefinitions in package", structure_definitions.len());

        if self.convert {
            self.convert_and_save_schemas(&structure_definitions, package_id).await?;
        }

        // Update package metadata
        self.save_package_metadata(&structure_definitions, package_id).await?;

        Ok(())
    }

    /// Load existing raw StructureDefinition files from directory
    fn load_existing_raw_files(&self, raw_dir: &std::path::Path) -> Result<Vec<(String, String)>> {
        let mut structure_definitions = Vec::new();

        for entry in fs::read_dir(raw_dir)
            .with_context(|| format!("Failed to read raw directory: {}", raw_dir.display()))? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                let filename = path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown.json")
                    .to_string();

                let content = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read file: {}", path.display()))?;

                // Verify it's a StructureDefinition
                if self.is_structure_definition_json(&content)? {
                    structure_definitions.push((filename, content));
                } else {
                    warn!("Skipping non-StructureDefinition file: {}", filename);
                }
            }
        }

        info!("Loaded {} StructureDefinitions from existing raw files", structure_definitions.len());
        Ok(structure_definitions)
    }

    /// Parse package name and version from package parameter
    fn parse_package_and_version(&self) -> Result<(String, Option<String>)> {
        if let Some(hash_pos) = self.package.find('#') {
            // Package has version specified: package#version
            let package_name = self.package[..hash_pos].to_string();
            let version = self.package[hash_pos + 1..].to_string();
            Ok((package_name, Some(version)))
        } else {
            // No version specified, use latest
            Ok((self.package.clone(), None))
        }
    }

    /// Determine which version to download
    fn determine_version(&self, manifest: &Value) -> Result<String> {
        if let Some(version) = &self.version {
            return Ok(version.clone());
        }

        // Get latest version from manifest
        if let Some(versions) = manifest.get("versions").and_then(|v| v.as_array()) {
            if let Some(latest) = versions.last() {
                if let Some(version_str) = latest.get("version").and_then(|v| v.as_str()) {
                    return Ok(version_str.to_string());
                }
            }
        }

        anyhow::bail!("Could not determine package version from manifest");
    }

    /// Process downloaded package and extract StructureDefinitions
    async fn process_package(&self, package_data: &[u8]) -> Result<()> {
        info!("Processing downloaded package ({} bytes)", package_data.len());

        // Parse package name and version for directory structure
        let (package_name, package_version) = self.parse_package_and_version()?;
        let package_version = package_version.unwrap_or_else(|| "latest".to_string());
        let package_id = format!("{}-{}", package_name, package_version);

        // Create structured directory layout
        self.create_output_directories(&package_id)?;

        // Save raw package data for debugging
        self.save_raw_package(package_data, &package_id).await?;

        // Extract StructureDefinitions from the package
        let structure_definitions = self.extract_structure_definitions(package_data)?;

        if structure_definitions.is_empty() {
            warn!("No StructureDefinitions found in the downloaded package");
            return Ok(());
        }

        info!("Found {} StructureDefinitions in package", structure_definitions.len());

        // Always save raw StructureDefinitions for debugging
        self.save_raw_definitions(&structure_definitions, &package_id).await?;

        if self.convert {
            self.convert_and_save_schemas(&structure_definitions, &package_id).await?;
        }

        // Save package metadata
        self.save_package_metadata(&structure_definitions, &package_id).await?;

        Ok(())
    }

    /// Extract StructureDefinitions from package data
    fn extract_structure_definitions(&self, package_data: &[u8]) -> Result<Vec<(String, String)>> {
        info!("Extracting StructureDefinitions from package archive...");

        let mut structure_definitions = Vec::new();

        // Try to extract as tar.gz first (most common format for FHIR packages)
        match self.extract_from_tar_gz(package_data) {
            Ok(definitions) => {
                info!("Successfully extracted {} files from tar.gz archive", definitions.len());
                structure_definitions.extend(definitions);
            }
            Err(e) => {
                warn!("Failed to extract as tar.gz: {}", e);
                // Try to process as raw JSON (fallback)
                if let Ok(json_content) = std::str::from_utf8(package_data) {
                    if self.is_structure_definition_json(json_content)? {
                        info!("Package data appears to be a single StructureDefinition JSON");
                        structure_definitions.push(("package.json".to_string(), json_content.to_string()));
                    }
                }
            }
        }

        if structure_definitions.is_empty() {
            warn!("No StructureDefinitions found in package data");
        } else {
            info!("Found {} StructureDefinitions in package", structure_definitions.len());
        }

        Ok(structure_definitions)
    }

    /// Extract JSON files from tar.gz archive
    fn extract_from_tar_gz(&self, package_data: &[u8]) -> Result<Vec<(String, String)>> {
        let cursor = Cursor::new(package_data);
        let gz_decoder = GzDecoder::new(cursor);
        let mut archive = Archive::new(gz_decoder);

        let mut structure_definitions = Vec::new();
        let mut processed_files = 0;
        let mut json_files_found = 0;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            processed_files += 1;

            // Skip directories
            if entry.header().entry_type().is_dir() {
                continue;
            }

            // Check if this is a JSON file
            if let Some(extension) = path.extension() {
                if extension == "json" {
                    json_files_found += 1;

                    // Capture path information before mutable borrow
                    let path_display = path.display().to_string();

                    // Read file contents
                    let mut contents = String::new();
                    match entry.read_to_string(&mut contents) {
                        Ok(_) => {
                            // Check if it's a StructureDefinition
                            match self.is_structure_definition_json(&contents) {
                                Ok(true) => {
                                    info!("Found StructureDefinition: {}", path_display);
                                    structure_definitions.push((path_display, contents));
                                }
                                Ok(false) => {
                                    // Not a StructureDefinition, skip silently
                                }
                                Err(e) => {
                                    warn!("Failed to parse JSON file {}: {}", path_display, e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to read file {}: {}", path_display, e);
                        }
                    }
                }
            }
        }

        info!("Archive extraction completed: {} files processed, {} JSON files found, {} StructureDefinitions extracted",
              processed_files, json_files_found, structure_definitions.len());

        Ok(structure_definitions)
    }

    /// Check if JSON content is a StructureDefinition
    fn is_structure_definition_json(&self, json_content: &str) -> Result<bool> {
        let json: Value = serde_json::from_str(json_content)
            .with_context(|| "Failed to parse JSON content")?;

        Ok(json.get("resourceType")
            .and_then(|v| v.as_str())
            .map_or(false, |rt| rt == "StructureDefinition"))
    }

    /// Create structured output directories for debugging
    fn create_output_directories(&self, package_id: &str) -> Result<()> {
        let base_output = &self.output;

        // Create main directories
        fs::create_dir_all(base_output.join("downloads").join("packages"))?;
        fs::create_dir_all(base_output.join("downloads").join("extracted").join(package_id))?;
        fs::create_dir_all(base_output.join("converted").join("ndjson"))?;
        fs::create_dir_all(base_output.join("converted").join("json"))?;
        fs::create_dir_all(base_output.join("converted").join("yaml"))?;
        fs::create_dir_all(base_output.join("logs"))?;
        fs::create_dir_all(base_output.join("packages").join(package_id).join("raw"))?;
        fs::create_dir_all(base_output.join("packages").join(package_id).join("converted"))?;
        fs::create_dir_all(base_output.join("packages").join(package_id).join("metadata"))?;

        info!("Created structured output directories in: {}", base_output.display());
        Ok(())
    }

    /// Save raw package data for debugging
    async fn save_raw_package(&self, package_data: &[u8], package_id: &str) -> Result<()> {
        let package_file = self.output
            .join("downloads")
            .join("packages")
            .join(format!("{}.tgz", package_id));

        fs::write(&package_file, package_data)
            .with_context(|| format!("Failed to save raw package: {}", package_file.display()))?;

        info!("Saved raw package: {}", package_file.display());
        Ok(())
    }

    /// Save package metadata for debugging
    async fn save_package_metadata(&self, definitions: &[(String, String)], package_id: &str) -> Result<()> {
        let metadata_dir = self.output.join("packages").join(package_id).join("metadata");

        // Create summary metadata
        let metadata = serde_json::json!({
            "package_id": package_id,
            "total_files": definitions.len(),
            "processed_at": chrono::Utc::now().to_rfc3339(),
            "files": definitions.iter().map(|(filename, _)| filename).collect::<Vec<_>>()
        });

        let metadata_file = metadata_dir.join("summary.json");
        fs::write(&metadata_file, serde_json::to_string_pretty(&metadata)?)
            .with_context(|| format!("Failed to save metadata: {}", metadata_file.display()))?;

        info!("Saved package metadata: {}", metadata_file.display());
        Ok(())
    }

    /// Convert StructureDefinitions to FHIRSchema and save
    async fn convert_and_save_schemas(&self, definitions: &[(String, String)], package_id: &str) -> Result<()> {
        info!("Converting {} StructureDefinitions to FHIRSchema format", definitions.len());

        let converter = StructureDefinitionConverter::new();
        let mut converted_schemas = Vec::new();
        let mut conversion_errors = Vec::new();

        for (index, (filename, definition_json)) in definitions.iter().enumerate() {
            match converter.convert(definition_json) {
                Ok(schema) => {
                    converted_schemas.push(schema);
                    if (index + 1) % 10 == 0 || index + 1 == definitions.len() {
                        info!("Converted {}/{} StructureDefinitions", index + 1, definitions.len());
                    }
                }
                Err(e) => {
                    error!("Failed to convert StructureDefinition {}: {}", filename, e);
                    conversion_errors.push((filename.clone(), e));
                }
            }
        }

        if converted_schemas.is_empty() {
            anyhow::bail!("No StructureDefinitions could be converted successfully");
        }

        // Save converted schemas
        match self.format.as_str() {
            "ndjson" => self.save_as_ndjson(&converted_schemas, package_id).await?,
            "json" => self.save_as_json(&converted_schemas, package_id).await?,
            "yaml" => self.save_as_yaml(&converted_schemas, package_id).await?,
            _ => unreachable!(),
        }

        // Report conversion results
        info!("Conversion completed: {} successful, {} failed",
              converted_schemas.len(), conversion_errors.len());

        if !conversion_errors.is_empty() {
            warn!("Failed to convert {} StructureDefinitions:", conversion_errors.len());
            for (filename, _) in &conversion_errors {
                warn!("  - {}", filename);
            }
        }

        Ok(())
    }

    /// Save raw StructureDefinitions without conversion
    async fn save_raw_definitions(&self, definitions: &[(String, String)], package_id: &str) -> Result<()> {
        for (original_filename, definition) in definitions.iter() {
            // Extract just the filename from the path for cleaner output filenames
            let clean_filename = std::path::Path::new(original_filename)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("structuredefinition.json");

            let filepath = self.output
                .join("packages")
                .join(package_id)
                .join("raw")
                .join(clean_filename);
            fs::write(&filepath, definition)
                .with_context(|| format!("Failed to write file: {}", filepath.display()))?;
        }
        Ok(())
    }

    /// Save schemas as NDJSON format
    async fn save_as_ndjson(&self, schemas: &[fhirschema_core::Schema], package_id: &str) -> Result<()> {
        let filepath = self.output
            .join("packages")
            .join(package_id)
            .join("converted")
            .join("schemas.ndjson");
        let mut content = String::new();

        for schema in schemas {
            let json_line = serde_json::to_string(schema)
                .with_context(|| "Failed to serialize schema to JSON")?;
            content.push_str(&json_line);
            content.push('\n');
        }

        fs::write(&filepath, content)
            .with_context(|| format!("Failed to write NDJSON file: {}", filepath.display()))?;

        info!("Saved {} schemas to NDJSON: {}", schemas.len(), filepath.display());
        Ok(())
    }

    /// Save schemas as JSON array
    async fn save_as_json(&self, schemas: &[fhirschema_core::Schema], package_id: &str) -> Result<()> {
        let filepath = self.output
            .join("packages")
            .join(package_id)
            .join("converted")
            .join("schemas.json");
        let content = serde_json::to_string_pretty(schemas)
            .with_context(|| "Failed to serialize schemas to JSON")?;

        fs::write(&filepath, content)
            .with_context(|| format!("Failed to write JSON file: {}", filepath.display()))?;

        info!("Saved {} schemas to JSON: {}", schemas.len(), filepath.display());
        Ok(())
    }

    /// Save schemas as YAML
    async fn save_as_yaml(&self, schemas: &[fhirschema_core::Schema], package_id: &str) -> Result<()> {
        let filepath = self.output
            .join("packages")
            .join(package_id)
            .join("converted")
            .join("schemas.yaml");
        let content = serde_yaml::to_string(schemas)
            .with_context(|| "Failed to serialize schemas to YAML")?;

        fs::write(&filepath, content)
            .with_context(|| format!("Failed to write YAML file: {}", filepath.display()))?;

        info!("Saved {} schemas to YAML: {}", schemas.len(), filepath.display());
        Ok(())
    }
}
