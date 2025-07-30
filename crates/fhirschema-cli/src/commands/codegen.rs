//! Code generation commands
//!
//! This module provides CLI commands for generating code from FHIRSchema definitions,
//! supporting multiple target languages including TypeScript and Rust.

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;
use tracing::{debug, info};

#[cfg(feature = "codegen")]
use fhirschema_codegen::{
    CodegenConfig, GenerationContext, LanguageTarget,
    generator::GeneratorRegistry,
    typescript::TypeScriptGenerator,
};

/// Code generation commands
#[derive(Args)]
pub struct CodegenCommand {
    #[command(subcommand)]
    pub command: CodegenSubcommand,
}

#[derive(Subcommand)]
pub enum CodegenSubcommand {
    /// Generate code from FHIRSchema files
    Generate(GenerateCommand),
    /// List available code generators
    List(ListCommand),
    /// Show information about a specific generator
    Info(InfoCommand),
}

/// Generate code from FHIRSchema files
#[derive(Args)]
pub struct GenerateCommand {
    /// Input FHIRSchema files or directories
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,

    /// Target language for code generation
    #[arg(short, long, default_value = "typescript")]
    pub target: TargetLanguage,

    /// Output directory for generated code
    #[arg(short, long, default_value = "./generated")]
    pub output: PathBuf,

    /// Overwrite existing files
    #[arg(long)]
    pub overwrite: bool,

    /// Generate interfaces only (TypeScript)
    #[arg(long)]
    pub interfaces_only: bool,

    /// Include JSDoc comments (TypeScript)
    #[arg(long, default_value = "true")]
    pub include_docs: bool,

    /// Generate index files
    #[arg(long, default_value = "true")]
    pub generate_index: bool,

    /// Create subdirectories for organization
    #[arg(long, default_value = "true")]
    pub create_subdirs: bool,

    /// Format generated code
    #[arg(long, default_value = "true")]
    pub format: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// List available code generators
#[derive(Args)]
pub struct ListCommand {
    /// Show detailed information
    #[arg(short, long)]
    pub detailed: bool,
}

/// Show information about a specific generator
#[derive(Args)]
pub struct InfoCommand {
    /// Generator name
    pub generator: String,
}

/// Target language for code generation
#[derive(Debug, Clone, ValueEnum)]
pub enum TargetLanguage {
    /// TypeScript interfaces and classes
    #[value(name = "typescript", alias = "type-script")]
    TypeScript,
    /// Rust structs and enums
    Rust,
    /// JSON Schema
    #[value(name = "json-schema", alias = "jsonschema")]
    JsonSchema,
}

impl From<TargetLanguage> for LanguageTarget {
    fn from(target: TargetLanguage) -> Self {
        match target {
            TargetLanguage::TypeScript => LanguageTarget::TypeScript,
            TargetLanguage::Rust => LanguageTarget::Rust,
            TargetLanguage::JsonSchema => LanguageTarget::JsonSchema,
        }
    }
}

impl CodegenCommand {
    pub async fn execute(&self) -> Result<()> {
        match &self.command {
            CodegenSubcommand::Generate(cmd) => cmd.execute().await,
            CodegenSubcommand::List(cmd) => cmd.execute().await,
            CodegenSubcommand::Info(cmd) => cmd.execute().await,
        }
    }
}

impl GenerateCommand {
    pub async fn execute(&self) -> Result<()> {
        #[cfg(not(feature = "codegen"))]
        {
            anyhow::bail!("Code generation functionality not enabled. Enable the 'codegen' feature.");
        }

        #[cfg(feature = "codegen")]
        {
            info!("Starting code generation");
            debug!("Target: {:?}", self.target);
            debug!("Output: {}", self.output.display());
            debug!("Inputs: {:?}", self.inputs);

            // Load schemas from input files
            let schemas = self.load_schemas().await?;
            info!("Loaded {} schema(s)", schemas.len());

            // Create configuration
            let config = self.create_config()?;

            // Create generation context
            let context = GenerationContext::new(schemas, config);

            // Create generator registry and register generators
            let mut registry = GeneratorRegistry::new();
            self.register_generators(&mut registry)?;

            // Generate code
            let generator_name = match self.target {
                TargetLanguage::TypeScript => "typescript",
                TargetLanguage::Rust => "rust",
                TargetLanguage::JsonSchema => "json-schema",
            };

            let files = registry.generate(generator_name, &context)
                .context("Failed to generate code")?;

            info!("Generated {} file(s)", files.len());

            // Write files to disk
            self.write_files(&files).await?;

            println!("Code generation completed successfully!");
            println!("Generated {} files in {}", files.len(), self.output.display());

            Ok(())
        }
    }

    #[cfg(feature = "codegen")]
    async fn load_schemas(&self) -> Result<Vec<fhirschema_core::Schema>> {
        let mut schemas = Vec::new();

        for input in &self.inputs {
            if input.is_file() {
                let file_schemas = self.load_schemas_from_file(input).await?;
                schemas.extend(file_schemas);
            } else if input.is_dir() {
                let dir_schemas = self.load_schemas_from_dir(input).await?;
                schemas.extend(dir_schemas);
            } else {
                anyhow::bail!("Input path does not exist: {}", input.display());
            }
        }

        if schemas.is_empty() {
            anyhow::bail!("No schemas found in input paths");
        }

        Ok(schemas)
    }

    #[cfg(feature = "codegen")]
    async fn load_schemas_from_file(&self, path: &PathBuf) -> Result<Vec<fhirschema_core::Schema>> {
        let content = tokio::fs::read_to_string(path).await
            .context(format!("Failed to read schema file: {}", path.display()))?;

        if path.extension().and_then(|s| s.to_str()) == Some("yaml") ||
           path.extension().and_then(|s| s.to_str()) == Some("yml") {
            let schema: fhirschema_core::Schema = serde_yaml::from_str(&content)
                .context("Failed to parse YAML schema")?;
            Ok(vec![schema])
        } else {
            // Try parsing as single JSON first
            match serde_json::from_str::<fhirschema_core::Schema>(&content) {
                Ok(schema) => Ok(vec![schema]),
                Err(_) => {
                    // If that fails, try parsing as NDJSON (multiple schemas, one per line)
                    let mut schemas = Vec::new();
                    for (line_num, line) in content.lines().enumerate() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        let schema: fhirschema_core::Schema = serde_json::from_str(line)
                            .context(format!("Failed to parse JSON schema at line {}", line_num + 1))?;
                        schemas.push(schema);
                    }
                    if schemas.is_empty() {
                        anyhow::bail!("No valid schemas found in file");
                    }
                    Ok(schemas)
                }
            }
        }
    }

    #[cfg(feature = "codegen")]
    async fn load_schema_file(&self, path: &PathBuf) -> Result<fhirschema_core::Schema> {
        let schemas = self.load_schemas_from_file(path).await?;
        if schemas.len() != 1 {
            anyhow::bail!("Expected single schema, found {}", schemas.len());
        }
        Ok(schemas.into_iter().next().unwrap())
    }

    #[cfg(feature = "codegen")]
    async fn load_schemas_from_dir(&self, dir: &PathBuf) -> Result<Vec<fhirschema_core::Schema>> {
        let mut schemas = Vec::new();
        let mut entries = tokio::fs::read_dir(dir).await
            .context(format!("Failed to read directory: {}", dir.display()))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if matches!(ext, "json" | "yaml" | "yml") {
                        match self.load_schema_file(&path).await {
                            Ok(schema) => schemas.push(schema),
                            Err(e) => {
                                if self.verbose {
                                    eprintln!("Warning: Failed to load {}: {}", path.display(), e);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(schemas)
    }

    #[cfg(feature = "codegen")]
    fn create_config(&self) -> Result<CodegenConfig> {
        let mut config = match self.target {
            TargetLanguage::TypeScript => CodegenConfig::typescript(),
            TargetLanguage::Rust => CodegenConfig::rust(),
            TargetLanguage::JsonSchema => CodegenConfig::default(),
        };

        // Configure output settings
        config.output.output_dir = self.output.clone();
        config.output.overwrite = self.overwrite;
        config.output.create_subdirs = self.create_subdirs;
        config.output.format_code = self.format;

        // Configure TypeScript-specific settings
        if matches!(self.target, TargetLanguage::TypeScript) {
            config.typescript.interfaces_only = self.interfaces_only;
            config.typescript.include_jsdoc = self.include_docs;
            config.typescript.generate_index = self.generate_index;
        }

        Ok(config)
    }

    #[cfg(feature = "codegen")]
    fn register_generators(&self, registry: &mut GeneratorRegistry) -> Result<()> {
        // Register TypeScript generator
        registry.register(Box::new(TypeScriptGenerator::new()));

        // TODO: Register other generators when implemented
        // registry.register(Box::new(RustGenerator::new()));
        // registry.register(Box::new(JsonSchemaGenerator::new()));

        Ok(())
    }

    #[cfg(feature = "codegen")]
    async fn write_files(&self, files: &[fhirschema_codegen::GeneratedFile]) -> Result<()> {
        // Create output directory if it doesn't exist
        tokio::fs::create_dir_all(&self.output).await
            .context("Failed to create output directory")?;

        for file in files {
            let full_path = if file.path.is_absolute() {
                file.path.clone()
            } else {
                self.output.join(&file.path)
            };

            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await
                    .context(format!("Failed to create directory: {}", parent.display()))?;
            }

            // Check if file exists and we're not overwriting
            if full_path.exists() && !self.overwrite {
                if self.verbose {
                    println!("Skipping existing file: {}", full_path.display());
                }
                continue;
            }

            // Write file
            tokio::fs::write(&full_path, &file.content).await
                .context(format!("Failed to write file: {}", full_path.display()))?;

            if self.verbose {
                println!("Generated: {}", full_path.display());
            }
        }

        Ok(())
    }
}

impl ListCommand {
    pub async fn execute(&self) -> Result<()> {
        #[cfg(not(feature = "codegen"))]
        {
            anyhow::bail!("Code generation functionality not enabled. Enable the 'codegen' feature.");
        }

        #[cfg(feature = "codegen")]
        {
            println!("Available code generators:");
            println!();

            let mut registry = GeneratorRegistry::new();
            registry.register(Box::new(TypeScriptGenerator::new()));

            let generators = registry.list_generators();

            if generators.is_empty() {
                println!("No generators available");
                return Ok(());
            }

            for generator_name in generators {
                if let Some(generator) = registry.get(generator_name) {
                    println!("  {}", generator_name);
                    if self.detailed {
                        println!("    Extensions: {}", generator.file_extensions().join(", "));
                    }
                }
            }

            Ok(())
        }
    }
}

impl InfoCommand {
    pub async fn execute(&self) -> Result<()> {
        #[cfg(not(feature = "codegen"))]
        {
            anyhow::bail!("Code generation functionality not enabled. Enable the 'codegen' feature.");
        }

        #[cfg(feature = "codegen")]
        {
            let mut registry = GeneratorRegistry::new();
            registry.register(Box::new(TypeScriptGenerator::new()));

            if let Some(generator) = registry.get(&self.generator) {
                println!("Generator: {}", self.generator);
                println!("File extensions: {}", generator.file_extensions().join(", "));
                // TODO: Add more detailed information when available
            } else {
                anyhow::bail!("Generator '{}' not found", self.generator);
            }

            Ok(())
        }
    }
}
