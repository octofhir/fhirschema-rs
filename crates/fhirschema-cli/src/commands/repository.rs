//! Repository management commands
//!
//! This module provides CLI commands for managing FHIRSchema repositories,
//! including initialization, schema management, synchronization, and reporting.

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use fhirschema_core::FhirSchema;
use fhirschema_repository::{
    ConfigManager, MemoryRepository, RepositoryConfig, RepositoryError, RepositoryResult,
    RepositoryType, SchemaMetadata, SchemaQuery, SchemaRepository, SchemaVersion,
};
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

#[cfg(feature = "filesystem")]
use fhirschema_repository::FileSystemRepository;

#[cfg(feature = "s3")]
use fhirschema_repository::{S3Config, S3Repository};

/// Repository management commands
#[derive(Args)]
pub struct RepositoryCommand {
    #[command(subcommand)]
    pub command: RepositorySubcommand,

    /// Repository configuration file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Repository name to use (defaults to configured default)
    #[arg(short, long, global = true)]
    pub repository: Option<String>,
}

#[derive(Subcommand)]
pub enum RepositorySubcommand {
    /// Initialize a new repository
    Init(InitCommand),
    /// Add a schema to the repository
    Add(AddCommand),
    /// Remove a schema from the repository
    Remove(RemoveCommand),
    /// List schemas in the repository
    List(ListCommand),
    /// Search for schemas
    Search(SearchCommand),
    /// Show repository status and information
    Status(StatusCommand),
    /// Synchronize with remote repositories
    Sync(SyncCommand),
    /// Generate repository reports
    Report(ReportCommand),
    /// Repository maintenance operations
    Maintenance(MaintenanceCommand),
    /// Manage repository configuration
    Config(ConfigCommand),
}

/// Initialize a new repository
#[derive(Args)]
pub struct InitCommand {
    /// Repository type (memory, filesystem, s3)
    #[arg(short, long, default_value = "filesystem")]
    pub repo_type: String,

    /// Repository name
    #[arg(short, long, default_value = "default")]
    pub name: String,

    /// Repository location (path for filesystem, bucket for S3)
    #[arg(short, long)]
    pub location: Option<String>,

    /// Force initialization even if repository exists
    #[arg(short, long)]
    pub force: bool,
}

/// Add a schema to the repository
#[derive(Args)]
pub struct AddCommand {
    /// Schema file path or URL
    pub schema: String,

    /// Schema version (optional)
    #[arg(short, long)]
    pub version: Option<String>,

    /// Schema URL/identifier (if different from file path)
    #[arg(short, long)]
    pub url: Option<String>,

    /// Force overwrite if schema already exists
    #[arg(short, long)]
    pub force: bool,

    /// Add tags to the schema
    #[arg(short, long)]
    pub tags: Vec<String>,
}

/// Remove a schema from the repository
#[derive(Args)]
pub struct RemoveCommand {
    /// Schema URL or identifier
    pub schema: String,

    /// Specific version to remove (removes all versions if not specified)
    #[arg(short, long)]
    pub version: Option<String>,

    /// Force removal without confirmation
    #[arg(short, long)]
    pub force: bool,
}

/// List schemas in the repository
#[derive(Args)]
pub struct ListCommand {
    /// Filter by name pattern
    #[arg(short, long)]
    pub name: Option<String>,

    /// Filter by URL pattern
    #[arg(short, long)]
    pub url: Option<String>,

    /// Filter by version
    #[arg(short, long)]
    pub version: Option<String>,

    /// Limit number of results
    #[arg(short, long)]
    pub limit: Option<usize>,

    /// Output format (table, json, yaml)
    #[arg(short, long, default_value = "table")]
    pub format: String,

    /// Show detailed information
    #[arg(short, long)]
    pub detailed: bool,
}

/// Search for schemas
#[derive(Args)]
pub struct SearchCommand {
    /// Search query
    pub query: String,

    /// Search in name
    #[arg(long)]
    pub in_name: bool,

    /// Search in description
    #[arg(long)]
    pub in_description: bool,

    /// Search in URL
    #[arg(long)]
    pub in_url: bool,

    /// Limit number of results
    #[arg(short, long, default_value = "10")]
    pub limit: usize,

    /// Output format (table, json, yaml)
    #[arg(short, long, default_value = "table")]
    pub format: String,
}

/// Show repository status and information
#[derive(Args)]
pub struct StatusCommand {
    /// Show detailed status
    #[arg(short, long)]
    pub detailed: bool,

    /// Output format (table, json, yaml)
    #[arg(short, long, default_value = "table")]
    pub format: String,
}

/// Synchronize with remote repositories
#[derive(Args)]
pub struct SyncCommand {
    /// Remote repository URL or name
    pub remote: Option<String>,

    /// Sync direction (pull, push, both)
    #[arg(short, long, default_value = "pull")]
    pub direction: String,

    /// Force sync even if conflicts exist
    #[arg(short, long)]
    pub force: bool,

    /// Dry run - show what would be synced
    #[arg(long)]
    pub dry_run: bool,
}

/// Generate repository reports
#[derive(Args)]
pub struct ReportCommand {
    /// Report type (usage, dependencies, health, performance)
    #[arg(short, long, default_value = "usage")]
    pub report_type: String,

    /// Output file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output format (json, yaml, html, csv)
    #[arg(short, long, default_value = "json")]
    pub format: String,

    /// Include detailed metrics
    #[arg(short, long)]
    pub detailed: bool,
}

/// Repository maintenance operations
#[derive(Args)]
pub struct MaintenanceCommand {
    #[command(subcommand)]
    pub operation: MaintenanceOperation,
}

#[derive(Subcommand)]
pub enum MaintenanceOperation {
    /// Clean up unused schemas and versions
    Cleanup(CleanupCommand),
    /// Verify repository integrity
    Verify(VerifyCommand),
    /// Optimize repository performance
    Optimize(OptimizeCommand),
    /// Backup repository
    Backup(BackupCommand),
    /// Restore repository from backup
    Restore(RestoreCommand),
}

/// Clean up unused schemas and versions
#[derive(Args)]
pub struct CleanupCommand {
    /// Remove schemas older than specified days
    #[arg(long)]
    pub older_than_days: Option<u64>,

    /// Remove unused versions
    #[arg(long)]
    pub unused_versions: bool,

    /// Dry run - show what would be cleaned
    #[arg(long)]
    pub dry_run: bool,
}

/// Verify repository integrity
#[derive(Args)]
pub struct VerifyCommand {
    /// Fix issues automatically
    #[arg(long)]
    pub fix: bool,

    /// Detailed verification output
    #[arg(short, long)]
    pub detailed: bool,
}

/// Optimize repository performance
#[derive(Args)]
pub struct OptimizeCommand {
    /// Rebuild indexes
    #[arg(long)]
    pub rebuild_indexes: bool,

    /// Compress data
    #[arg(long)]
    pub compress: bool,
}

/// Backup repository
#[derive(Args)]
pub struct BackupCommand {
    /// Backup destination path
    pub destination: PathBuf,

    /// Backup format (tar, zip, directory)
    #[arg(short, long, default_value = "tar")]
    pub format: String,

    /// Compress backup
    #[arg(short, long)]
    pub compress: bool,
}

/// Restore repository from backup
#[derive(Args)]
pub struct RestoreCommand {
    /// Backup source path
    pub source: PathBuf,

    /// Force restore even if repository exists
    #[arg(short, long)]
    pub force: bool,
}

/// Manage repository configuration
#[derive(Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub operation: ConfigOperation,
}

#[derive(Subcommand)]
pub enum ConfigOperation {
    /// Show current configuration
    Show(ShowConfigCommand),
    /// Set configuration value
    Set(SetConfigCommand),
    /// Generate default configuration file
    Generate(GenerateConfigCommand),
    /// Validate configuration
    Validate(ValidateConfigCommand),
}

/// Show current configuration
#[derive(Args)]
pub struct ShowConfigCommand {
    /// Configuration key to show (shows all if not specified)
    pub key: Option<String>,

    /// Output format (yaml, json, table)
    #[arg(short, long, default_value = "yaml")]
    pub format: String,
}

/// Set configuration value
#[derive(Args)]
pub struct SetConfigCommand {
    /// Configuration key
    pub key: String,

    /// Configuration value
    pub value: String,
}

/// Generate default configuration file
#[derive(Args)]
pub struct GenerateConfigCommand {
    /// Output file path
    #[arg(short, long, default_value = "fhirschema-config.yaml")]
    pub output: PathBuf,

    /// Force overwrite existing file
    #[arg(short, long)]
    pub force: bool,
}

/// Validate configuration
#[derive(Args)]
pub struct ValidateConfigCommand {
    /// Configuration file to validate
    pub config_file: Option<PathBuf>,
}

impl RepositoryCommand {
    pub async fn execute(&self) -> Result<()> {
        let config_manager = self.load_config_manager().await?;
        let repository = self.create_repository(&config_manager).await?;

        match &self.command {
            RepositorySubcommand::Init(cmd) => cmd.execute().await,
            RepositorySubcommand::Add(cmd) => cmd.execute(repository).await,
            RepositorySubcommand::Remove(cmd) => cmd.execute(repository).await,
            RepositorySubcommand::List(cmd) => cmd.execute(repository).await,
            RepositorySubcommand::Search(cmd) => cmd.execute(repository).await,
            RepositorySubcommand::Status(cmd) => cmd.execute(repository).await,
            RepositorySubcommand::Sync(cmd) => cmd.execute(repository).await,
            RepositorySubcommand::Report(cmd) => cmd.execute(repository).await,
            RepositorySubcommand::Maintenance(cmd) => cmd.execute(repository).await,
            RepositorySubcommand::Config(cmd) => cmd.execute(&config_manager).await,
        }
    }

    async fn load_config_manager(&self) -> Result<ConfigManager> {
        if let Some(config_path) = &self.config {
            info!("Loading configuration from: {}", config_path.display());
            ConfigManager::load_from_file(config_path)
                .context("Failed to load configuration file")
        } else {
            // Try to load from default locations
            let default_paths = [
                "fhirschema-config.yaml",
                "~/.fhirschema/config.yaml",
                "/etc/fhirschema/config.yaml",
            ];

            for path_str in &default_paths {
                let path = PathBuf::from(path_str);
                if path.exists() {
                    info!("Loading configuration from: {}", path.display());
                    return ConfigManager::load_from_file(&path)
                        .context("Failed to load configuration file");
                }
            }

            info!("No configuration file found, using default configuration");
            Ok(ConfigManager::new())
        }
    }

    async fn create_repository(&self, config_manager: &ConfigManager) -> Result<Box<dyn SchemaRepository + Send + Sync>> {
        let repo_name = self.repository.as_deref().unwrap_or(&config_manager.default_repository);
        let repo_config = config_manager.get_repository(repo_name)
            .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found in configuration", repo_name))?;

        info!("Using repository: {} (type: {})", repo_name, repo_config.repository_type);

        match repo_config.repository_type {
            RepositoryType::Memory => {
                let repo = MemoryRepository::new();
                Ok(Box::new(repo))
            }
            #[cfg(feature = "filesystem")]
            RepositoryType::FileSystem => {
                let fs_config = repo_config.filesystem_config()
                    .ok_or_else(|| anyhow::anyhow!("Invalid filesystem configuration"))?;
                let repo = FileSystemRepository::new(fs_config.clone()).await
                    .context("Failed to create filesystem repository")?;
                Ok(Box::new(repo))
            }
            #[cfg(not(feature = "filesystem"))]
            RepositoryType::FileSystem => {
                anyhow::bail!("Filesystem repository support not compiled in")
            }
            #[cfg(feature = "s3")]
            RepositoryType::S3 => {
                let s3_config = repo_config.s3_config()
                    .ok_or_else(|| anyhow::anyhow!("Invalid S3 configuration"))?;
                let repo = S3Repository::new(s3_config.clone()).await
                    .context("Failed to create S3 repository")?;
                Ok(Box::new(repo))
            }
            #[cfg(not(feature = "s3"))]
            RepositoryType::S3 => {
                anyhow::bail!("S3 repository support not compiled in")
            }
        }
    }
}

impl InitCommand {
    pub async fn execute(&self) -> Result<()> {
        info!("Initializing repository: {} (type: {})", self.name, self.repo_type);

        let repo_type: RepositoryType = self.repo_type.parse()
            .context("Invalid repository type")?;

        let config = match repo_type {
            RepositoryType::Memory => RepositoryConfig::memory(&self.name),
            RepositoryType::FileSystem => {
                let path = self.location.as_ref()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(format!("./repositories/{}", self.name)));
                RepositoryConfig::filesystem(&self.name, path)
            }
            RepositoryType::S3 => {
                let bucket = self.location.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("S3 bucket name required for S3 repository"))?;
                RepositoryConfig::s3(&self.name, bucket)
            }
        };

        // Create configuration manager and add repository
        let mut config_manager = ConfigManager::new();
        config_manager.add_repository(config)
            .context("Failed to add repository configuration")?;
        config_manager.set_default_repository(&self.name)
            .context("Failed to set default repository")?;

        // Save configuration
        let config_path = PathBuf::from("fhirschema-config.yaml");
        if config_path.exists() && !self.force {
            anyhow::bail!("Configuration file already exists. Use --force to overwrite.");
        }

        config_manager.save_to_file(&config_path)
            .context("Failed to save configuration")?;

        println!("Repository '{}' initialized successfully", self.name);
        println!("Configuration saved to: {}", config_path.display());

        Ok(())
    }
}

impl AddCommand {
    pub async fn execute(&self, repository: Box<dyn SchemaRepository + Send + Sync>) -> Result<()> {
        info!("Adding schema: {}", self.schema);

        // Load schema from file or URL
        let schema = self.load_schema().await?;

        // Determine schema URL
        let schema_url = self.url.as_ref()
            .or(schema.url.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Schema URL not specified and not found in schema"))?;

        // Parse version if provided
        let version = if let Some(version_str) = &self.version {
            Some(version_str.parse::<SchemaVersion>()
                .map_err(|e| anyhow::anyhow!("Invalid version format: {}", e))?)
        } else {
            None
        };

        // Check if schema already exists
        if !self.force {
            if repository.schema_exists(schema_url).await
                .context("Failed to check existing schema")? {
                anyhow::bail!("Schema already exists. Use --force to overwrite.");
            }
        }

        // Create metadata
        let mut metadata = SchemaMetadata {
            id: uuid::Uuid::new_v4(),
            url: schema_url.to_string(),
            name: schema.name.clone().into(),
            title: None,
            description: None,
            version: version.unwrap_or_default(),
            status: fhirschema_repository::SchemaStatus::Active,
            schema_type: fhirschema_repository::SchemaType::Resource,
            base: schema.base.clone(),
            derivation: None,
            tags: self.tags.clone(),
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            size: 0,
            checksum: String::new(),
        };

        // Store schema
        repository.store_schema(&schema, Some(metadata)).await
            .context("Failed to store schema")?;

        println!("Schema added successfully: {}", schema_url);
        if let Some(v) = version {
            println!("Version: {}", v);
        }

        Ok(())
    }

    async fn load_schema(&self) -> Result<FhirSchema> {
        if self.schema.starts_with("http://") || self.schema.starts_with("https://") {
            // Load from URL
            let response = reqwest::get(&self.schema).await
                .context("Failed to fetch schema from URL")?;
            let content = response.text().await
                .context("Failed to read response body")?;
            serde_json::from_str(&content)
                .context("Failed to parse schema JSON")
        } else {
            // Load from file
            let content = std::fs::read_to_string(&self.schema)
                .context("Failed to read schema file")?;

            if self.schema.ends_with(".yaml") || self.schema.ends_with(".yml") {
                serde_yaml::from_str(&content)
                    .context("Failed to parse schema YAML")
            } else {
                serde_json::from_str(&content)
                    .context("Failed to parse schema JSON")
            }
        }
    }
}

impl RemoveCommand {
    pub async fn execute(&self, repository: Box<dyn SchemaRepository + Send + Sync>) -> Result<()> {
        info!("Removing schema: {}", self.schema);

        let version = if let Some(version_str) = &self.version {
            Some(version_str.parse::<SchemaVersion>()
                .context("Invalid version format")?)
        } else {
            None
        };

        if !self.force {
            // Confirm removal
            if version.is_some() {
                print!("Remove version {} of schema '{}'? [y/N]: ", version.as_ref().unwrap(), self.schema);
            } else {
                print!("Remove all versions of schema '{}'? [y/N]: ", self.schema);
            }

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().to_lowercase().starts_with('y') {
                println!("Removal cancelled");
                return Ok(());
            }
        }

        let removed = repository.remove_schema(&self.schema, version.as_ref()).await
            .context("Failed to remove schema")?;

        if removed {
            println!("Schema removed successfully: {}", self.schema);
            if let Some(v) = version {
                println!("Version: {}", v);
            }
        } else {
            println!("Schema not found: {}", self.schema);
        }

        Ok(())
    }
}

impl ListCommand {
    pub async fn execute(&self, repository: Box<dyn SchemaRepository + Send + Sync>) -> Result<()> {
        info!("Listing schemas");

        let query = SchemaQuery {
            name_pattern: self.name.clone(),
            url_pattern: self.url.clone(),
            version: self.version.as_ref().and_then(|v| v.parse().ok()),
            limit: self.limit,
        };

        let schemas = repository.list_schemas(Some(&query)).await
            .context("Failed to list schemas")?;

        match self.format.as_str() {
            "json" => {
                println!("{}", serde_json::to_string_pretty(&schemas)?);
            }
            "yaml" => {
                println!("{}", serde_yaml::to_string(&schemas)?);
            }
            "table" | _ => {
                self.print_table(&schemas);
            }
        }

        Ok(())
    }

    fn print_table(&self, schemas: &[SchemaMetadata]) {
        if schemas.is_empty() {
            println!("No schemas found");
            return;
        }

        println!("{:<40} {:<20} {:<10} {:<20}", "URL", "Name", "Versions", "Updated");
        println!("{}", "-".repeat(90));

        for schema in schemas {
            let versions_str = if schema.versions.len() > 3 {
                format!("{} versions", schema.versions.len())
            } else {
                schema.versions.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            println!(
                "{:<40} {:<20} {:<10} {:<20}",
                truncate(&schema.url, 40),
                truncate(&schema.name, 20),
                versions_str,
                schema.updated_at.format("%Y-%m-%d %H:%M")
            );

            if self.detailed {
                if let Some(desc) = &schema.description {
                    println!("  Description: {}", truncate(desc, 70));
                }
                if !schema.tags.is_empty() {
                    let tags: Vec<String> = schema.tags.iter()
                        .map(|(k, v)| if v == "true" { k.clone() } else { format!("{}={}", k, v) })
                        .collect();
                    println!("  Tags: {}", tags.join(", "));
                }
                println!();
            }
        }
    }
}

impl SearchCommand {
    pub async fn execute(&self, repository: Box<dyn SchemaRepository + Send + Sync>) -> Result<()> {
        info!("Searching schemas: {}", self.query);

        let query = SchemaQuery {
            name_pattern: if self.in_name { Some(self.query.clone()) } else { None },
            url_pattern: if self.in_url { Some(self.query.clone()) } else { None },
            version: None,
            limit: Some(self.limit),
        };

        let mut schemas = repository.list_schemas(Some(&query)).await
            .context("Failed to search schemas")?;

        // Additional filtering for description search
        if self.in_description {
            schemas.retain(|schema| {
                schema.description.as_ref()
                    .map(|desc| desc.contains(&self.query))
                    .unwrap_or(false)
            });
        }

        match self.format.as_str() {
            "json" => {
                println!("{}", serde_json::to_string_pretty(&schemas)?);
            }
            "yaml" => {
                println!("{}", serde_yaml::to_string(&schemas)?);
            }
            "table" | _ => {
                self.print_search_results(&schemas);
            }
        }

        Ok(())
    }

    fn print_search_results(&self, schemas: &[SchemaMetadata]) {
        if schemas.is_empty() {
            println!("No schemas found matching '{}'", self.query);
            return;
        }

        println!("Found {} schema(s) matching '{}':", schemas.len(), self.query);
        println!();

        for (i, schema) in schemas.iter().enumerate() {
            println!("{}. {}", i + 1, schema.name);
            println!("   URL: {}", schema.url);
            if let Some(desc) = &schema.description {
                println!("   Description: {}", truncate(desc, 70));
            }
            println!("   Versions: {}", schema.versions.len());
            println!("   Updated: {}", schema.updated_at.format("%Y-%m-%d %H:%M"));
            println!();
        }
    }
}

impl StatusCommand {
    pub async fn execute(&self, repository: Box<dyn SchemaRepository + Send + Sync>) -> Result<()> {
        info!("Getting repository status");

        let metadata = repository.get_repository_metadata().await
            .context("Failed to get repository metadata")?;

        match self.format.as_str() {
            "json" => {
                println!("{}", serde_json::to_string_pretty(&metadata)?);
            }
            "yaml" => {
                println!("{}", serde_yaml::to_string(&metadata)?);
            }
            "table" | _ => {
                println!("Repository Status");
                println!("================");
                println!("Type: {}", metadata.repository_type);
                println!("Location: {}", metadata.location);
                println!("Total Schemas: {}", metadata.total_schemas);
                println!("Total Versions: {}", metadata.total_versions);
                println!("Created: {}", metadata.created_at.format("%Y-%m-%d %H:%M:%S"));
                println!("Last Updated: {}", metadata.last_updated.format("%Y-%m-%d %H:%M:%S"));

                if self.detailed {
                    // Add more detailed information
                    let schemas = repository.list_schemas(None).await
                        .context("Failed to get detailed schema information")?;

                    println!("\nSchema Summary:");
                    let mut version_counts = HashMap::new();
                    for schema in &schemas {
                        for version in &schema.versions {
                            *version_counts.entry(format!("{}.{}", version.major, version.minor)).or_insert(0) += 1;
                        }
                    }

                    for (version, count) in version_counts {
                        println!("  {}: {} schemas", version, count);
                    }
                }
            }
        }

        Ok(())
    }
}

// Placeholder implementations for remaining commands
impl SyncCommand {
    pub async fn execute(&self, _repository: Box<dyn SchemaRepository + Send + Sync>) -> Result<()> {
        println!("Sync functionality not yet implemented");
        Ok(())
    }
}

impl ReportCommand {
    pub async fn execute(&self, _repository: Box<dyn SchemaRepository + Send + Sync>) -> Result<()> {
        println!("Report functionality not yet implemented");
        Ok(())
    }
}

impl MaintenanceCommand {
    pub async fn execute(&self, _repository: Box<dyn SchemaRepository + Send + Sync>) -> Result<()> {
        println!("Maintenance functionality not yet implemented");
        Ok(())
    }
}

impl ConfigCommand {
    pub async fn execute(&self, _config_manager: &ConfigManager) -> Result<()> {
        println!("Config management functionality not yet implemented");
        Ok(())
    }
}

// Helper function to truncate strings for display
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
