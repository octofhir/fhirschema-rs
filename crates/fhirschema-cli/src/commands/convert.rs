//! Convert command implementation.

use clap::Args;
use std::path::PathBuf;
use std::fs;
use anyhow::{Context, Result};
use fhirschema_converter::StructureDefinitionConverter;
use tracing::{info, error};

/// Convert FHIR StructureDefinition to FHIRSchema
#[derive(Args)]
pub struct ConvertCommand {
    /// Input StructureDefinition file or directory path
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output FHIRSchema file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output format (json, yaml, or ndjson)
    #[arg(short, long, default_value = "yaml")]
    pub format: String,

    /// Process entire directory (IG-wide conversion)
    #[arg(long)]
    pub batch: bool,
}

impl ConvertCommand {
    /// Execute the convert command
    pub fn execute(&self) -> Result<()> {
        info!("Converting {} to FHIRSchema format: {}",
              self.input.display(),
              self.format);

        // Validate input exists
        if !self.input.exists() {
            anyhow::bail!("Input path does not exist: {}", self.input.display());
        }

        // Validate format
        if !matches!(self.format.as_str(), "json" | "yaml" | "ndjson") {
            anyhow::bail!("Unsupported format: {}. Use 'json', 'yaml', or 'ndjson'", self.format);
        }

        if self.batch || self.input.is_dir() {
            self.execute_batch_conversion()
        } else {
            self.execute_single_conversion()
        }
    }

    /// Execute single file conversion
    fn execute_single_conversion(&self) -> Result<()> {
        // Read input file
        let input_content = fs::read_to_string(&self.input)
            .with_context(|| format!("Failed to read input file: {}", self.input.display()))?;

        // Create converter and convert
        let converter = StructureDefinitionConverter::new();
        let schema = converter.convert(&input_content)
            .with_context(|| "Failed to convert StructureDefinition to FHIRSchema")?;

        // Serialize to requested format
        let output_content = match self.format.as_str() {
            "json" => serde_json::to_string_pretty(&schema)
                .with_context(|| "Failed to serialize schema to JSON")?,
            "yaml" => serde_yaml::to_string(&schema)
                .with_context(|| "Failed to serialize schema to YAML")?,
            "ndjson" => {
                let json_line = serde_json::to_string(&schema)
                    .with_context(|| "Failed to serialize schema to JSON")?;
                format!("{}\n", json_line)
            },
            _ => unreachable!(), // Already validated above
        };

        // Determine output path
        let output_path = match &self.output {
            Some(path) => path.clone(),
            None => {
                let mut output_path = self.input.clone();
                let extension = match self.format.as_str() {
                    "json" => "fhirschema.json",
                    "yaml" => "fhirschema.yaml",
                    "ndjson" => "fhirschema.ndjson",
                    _ => unreachable!(),
                };
                output_path.set_extension(extension);
                output_path
            }
        };

        // Write output file
        fs::write(&output_path, output_content)
            .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;

        info!("Successfully converted to: {}", output_path.display());
        println!("✓ Conversion completed: {}", output_path.display());

        Ok(())
    }

    /// Execute batch conversion of directory
    fn execute_batch_conversion(&self) -> Result<()> {
        if !self.input.is_dir() {
            anyhow::bail!("Batch conversion requires a directory input: {}", self.input.display());
        }

        info!("Starting batch conversion of directory: {}", self.input.display());

        // Find all JSON files in the directory
        let json_files = self.find_structure_definition_files()?;

        if json_files.is_empty() {
            anyhow::bail!("No StructureDefinition JSON files found in directory: {}", self.input.display());
        }

        info!("Found {} StructureDefinition files to convert", json_files.len());

        // Convert all files
        let converter = StructureDefinitionConverter::new();
        let mut converted_schemas = Vec::new();
        let mut conversion_errors = Vec::new();

        for json_file in &json_files {
            match self.convert_single_file(&converter, json_file) {
                Ok(schema) => {
                    converted_schemas.push(schema);
                    info!("Successfully converted: {}", json_file.display());
                }
                Err(e) => {
                    error!("Failed to convert {}: {}", json_file.display(), e);
                    conversion_errors.push((json_file.clone(), e));
                }
            }
        }

        if converted_schemas.is_empty() {
            anyhow::bail!("No StructureDefinitions could be converted successfully");
        }

        // Save converted schemas
        self.save_batch_results(&converted_schemas)?;

        // Report results
        println!("✓ Batch conversion completed:");
        println!("  - Successfully converted: {}", converted_schemas.len());
        println!("  - Failed conversions: {}", conversion_errors.len());

        if !conversion_errors.is_empty() {
            println!("  - Failed files:");
            for (file, _) in &conversion_errors {
                println!("    - {}", file.display());
            }
        }

        Ok(())
    }

    /// Find StructureDefinition JSON files in directory
    fn find_structure_definition_files(&self) -> Result<Vec<PathBuf>> {
        let mut json_files = Vec::new();

        for entry in fs::read_dir(&self.input)
            .with_context(|| format!("Failed to read directory: {}", self.input.display()))? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                // Check if it's a StructureDefinition by reading the file
                if self.is_structure_definition_file(&path)? {
                    json_files.push(path);
                }
            }
        }

        Ok(json_files)
    }

    /// Check if a JSON file contains a StructureDefinition
    fn is_structure_definition_file(&self, path: &PathBuf) -> Result<bool> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Parse JSON and check resourceType
        let json: serde_json::Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON in file: {}", path.display()))?;

        Ok(json.get("resourceType")
            .and_then(|v| v.as_str())
            .map_or(false, |rt| rt == "StructureDefinition"))
    }

    /// Convert a single file to schema
    fn convert_single_file(&self, converter: &StructureDefinitionConverter, path: &PathBuf) -> Result<fhirschema_core::Schema> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        converter.convert(&content)
            .with_context(|| format!("Failed to convert file: {}", path.display()))
    }

    /// Save batch conversion results
    fn save_batch_results(&self, schemas: &[fhirschema_core::Schema]) -> Result<()> {
        let output_path = match &self.output {
            Some(path) => path.clone(),
            None => {
                let extension = match self.format.as_str() {
                    "json" => "schemas.json",
                    "yaml" => "schemas.yaml",
                    "ndjson" => "schemas.ndjson",
                    _ => unreachable!(),
                };
                self.input.join(extension)
            }
        };

        let output_content = match self.format.as_str() {
            "json" => serde_json::to_string_pretty(schemas)
                .with_context(|| "Failed to serialize schemas to JSON")?,
            "yaml" => serde_yaml::to_string(schemas)
                .with_context(|| "Failed to serialize schemas to YAML")?,
            "ndjson" => {
                let mut content = String::new();
                for schema in schemas {
                    let json_line = serde_json::to_string(schema)
                        .with_context(|| "Failed to serialize schema to JSON")?;
                    content.push_str(&json_line);
                    content.push('\n');
                }
                content
            },
            _ => unreachable!(),
        };

        fs::write(&output_path, output_content)
            .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;

        info!("Saved {} schemas to: {}", schemas.len(), output_path.display());
        println!("  - Output file: {}", output_path.display());

        Ok(())
    }
}
