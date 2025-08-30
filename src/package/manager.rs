use crate::converter::{ConverterConfig, FhirSchemaConverter, StructureDefinition};
use crate::error::{FhirSchemaError, Result};
use crate::package::{
    ConversionResults, ErrorCategory, InstallOptions, InstalledPackage, PackageId,
    PackageInstallError, PackageInstallResult, PackageRegistry, PackageSpec, RegistryConfig,
};
use crate::storage::{EnhancedStorageManager, StorageConfig};
use crate::types::FhirSchema;

use async_trait::async_trait;
use chrono::Utc;
use octofhir_canonical_manager::{CanonicalManager, FcmConfig};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main package manager for FHIRSchema with O(1) schema access
pub struct FhirSchemaPackageManager {
    /// Canonical manager for package download and management
    canonical_manager: Arc<CanonicalManager>,

    /// Enhanced storage manager for schema persistence
    storage: Arc<EnhancedStorageManager>,

    /// Package registry for O(1) schema access
    registry: Arc<RwLock<PackageRegistry>>,

    /// Schema converter
    converter: Arc<FhirSchemaConverter>,

    /// Package manager configuration
    config: PackageManagerConfig,

    /// Installation progress tracking
    progress_tracker: Arc<RwLock<HashMap<String, InstallProgress>>>,
}

/// Package manager configuration
#[derive(Clone)]
pub struct PackageManagerConfig {
    pub max_concurrent_conversions: usize,
    pub registry_config: RegistryConfig,
    pub storage_config: StorageConfig,
    pub converter_config: ConverterConfig,
    pub auto_resolve_dependencies: bool,
    pub validate_after_install: bool,
    pub cleanup_on_failure: bool,
}

/// Installation progress tracking
#[derive(Debug, Clone)]
pub struct InstallProgress {
    pub package_id: PackageId,
    pub status: InstallStatus,
    pub progress_percentage: f64,
    pub current_operation: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub estimated_completion: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
}

/// Installation status
#[derive(Debug, Clone, PartialEq)]
pub enum InstallStatus {
    Pending,
    Downloading,
    Converting,
    Storing,
    Completed,
    Failed,
}

/// Model provider trait for ecosystem integration
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Get schema by canonical URL (O(1) operation)
    async fn get_schema(&self, canonical_url: &str) -> Option<Arc<FhirSchema>>;

    /// Get schemas by resource type
    async fn get_schemas_by_type(&self, resource_type: &str) -> Vec<Arc<FhirSchema>>;

    /// Resolve profile for a base type
    async fn resolve_profile(&self, base_type: &str, profile_url: &str) -> Option<Arc<FhirSchema>>;

    /// Check if a resource type is known
    async fn has_resource_type(&self, resource_type: &str) -> bool;

    /// Get all known resource types
    async fn get_resource_types(&self) -> Vec<String>;

    /// Search schemas by query
    async fn search_schemas(&self, query: &str) -> Vec<Arc<FhirSchema>>;
}

impl FhirSchemaPackageManager {
    /// Create a new package manager with the specified configurations
    pub async fn new(fcm_config: FcmConfig, config: PackageManagerConfig) -> Result<Self> {
        // Add timeout to CanonicalManager initialization to prevent hanging
        let canonical_manager = match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            CanonicalManager::new(fcm_config),
        )
        .await
        {
            Ok(Ok(cm)) => Arc::new(cm),
            Ok(Err(e)) => {
                return Err(FhirSchemaError::Initialization {
                    message: format!("Failed to initialize canonical manager: {e}"),
                });
            }
            Err(_) => {
                return Err(FhirSchemaError::Initialization {
                    message: "Canonical manager initialization timed out after 30 seconds"
                        .to_string(),
                });
            }
        };

        let storage = Arc::new(EnhancedStorageManager::new(config.storage_config.clone()));
        let registry = Arc::new(RwLock::new(PackageRegistry::new(
            config.registry_config.clone(),
        )));
        let converter = Arc::new(FhirSchemaConverter::with_config(
            config.converter_config.clone(),
        ));

        let progress_tracker = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            canonical_manager,
            storage,
            registry,
            converter,
            config,
            progress_tracker,
        })
    }

    /// Install multiple packages with dependency resolution
    pub async fn install_packages(
        &self,
        package_specs: &[PackageSpec],
        options: Option<InstallOptions>,
    ) -> Result<PackageInstallResult> {
        let options = options.unwrap_or_default();
        let start_time = std::time::Instant::now();

        // Resolve dependencies if enabled
        let all_packages = if options.skip_dependencies || !self.config.auto_resolve_dependencies {
            package_specs.to_vec()
        } else {
            self.resolve_all_dependencies(package_specs).await?
        };

        // Determine installation order - pre-allocate with known capacity
        let mut package_ids = Vec::with_capacity(all_packages.len());
        for spec in &all_packages {
            package_ids.push(PackageId::new(&spec.name, &spec.version));
        }

        let _install_order = {
            let registry = self.registry.read().await;
            registry.resolve_install_order(&package_ids)?
        };

        // Filter out already installed packages unless force is specified
        let mut packages_to_install = Vec::new();
        for spec in &all_packages {
            let package_id = PackageId::new(&spec.name, &spec.version);
            if options.force || !self.is_package_installed(&package_id).await {
                packages_to_install.push(spec);
            }
        }

        if packages_to_install.is_empty() {
            return Ok(PackageInstallResult {
                installed: Vec::new(),
                skipped: package_ids,
                failed: Vec::new(),
                conversion_results: ConversionResults::default(),
                duration: start_time.elapsed(),
            });
        }

        // Install packages sequentially - pre-allocate with known capacity
        let mut installed = Vec::with_capacity(packages_to_install.len());
        let mut failed = Vec::new(); // Unknown failure count, keep as-is
        let mut total_conversion_results = ConversionResults::default();

        // Install packages one by one sequentially
        for spec in &packages_to_install {
            let package_id = PackageId::new(&spec.name, &spec.version);

            match self.install_single_package(spec, &options).await {
                Ok((installed_package, conversion_results)) => {
                    installed.push(installed_package);
                    total_conversion_results.merge(conversion_results);
                }
                Err(e) => {
                    failed.push(PackageInstallError {
                        package_id: package_id.clone(),
                        error: e.to_string(),
                        category: self.categorize_error(&e),
                    });

                    if !options.force && self.config.cleanup_on_failure {
                        // Rollback previously installed packages
                        self.rollback_installation(&installed).await;
                        break;
                    }
                }
            }
        }

        Ok(PackageInstallResult {
            installed,
            skipped: Vec::new(),
            failed,
            conversion_results: total_conversion_results,
            duration: start_time.elapsed(),
        })
    }

    /// Install a single package
    async fn install_single_package(
        &self,
        package_spec: &PackageSpec,
        _options: &InstallOptions,
    ) -> Result<(InstalledPackage, ConversionResults)> {
        let package_id = PackageId::new(&package_spec.name, &package_spec.version);

        // Track installation progress
        self.update_progress(
            &package_id,
            InstallStatus::Downloading,
            10.0,
            "Downloading package",
        )
        .await;

        // Download package via canonical manager with timeout to prevent hanging
        match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.canonical_manager
                .install_package(&package_spec.name, &package_spec.version),
        )
        .await
        {
            Ok(Ok(())) => {
                // Successfully installed
            }
            Ok(Err(e)) => {
                return Err(FhirSchemaError::Download {
                    message: format!("Failed to download package {package_id}: {e}"),
                });
            }
            Err(_) => {
                return Err(FhirSchemaError::Download {
                    message: format!(
                        "Package download timed out after 30 seconds for {package_id}"
                    ),
                });
            }
        }

        self.update_progress(
            &package_id,
            InstallStatus::Converting,
            30.0,
            "Converting StructureDefinitions",
        )
        .await;

        // Get all StructureDefinitions from the package
        let structure_definitions = self
            .get_structure_definitions_for_package(&package_spec.name)
            .await?;

        // Convert StructureDefinitions to FhirSchemas
        let (schemas, conversion_results) = self
            .convert_structure_definitions(&structure_definitions, &package_spec.conversion_options)
            .await?;

        self.update_progress(&package_id, InstallStatus::Storing, 80.0, "Storing schemas")
            .await;

        // Create installed package record - optimize URL collection
        let mut schema_urls = Vec::with_capacity(schemas.len());
        for schema in &schemas {
            if let Some(url) = &schema.url {
                schema_urls.push(url.clone());
            }
        }

        let installed_package = InstalledPackage {
            id: package_id.clone(),
            spec: package_spec.clone(),
            install_time: Utc::now(),
            file_path: self
                .get_package_file_path(&package_spec.name, &package_spec.version)
                .await,
            checksum: self.calculate_package_checksum(&schemas).await,
            schemas: schema_urls,
            dependencies: self
                .extract_package_dependencies(&package_spec.name, &package_spec.version)
                .await,
            metadata: package_spec.metadata.clone(),
        };

        // Store schemas in enhanced storage
        for schema in &schemas {
            if let Some(url) = &schema.url {
                self.storage.put(url.clone(), schema.clone()).await?;
            }
        }

        // Register package in registry
        {
            let registry = self.registry.write().await;
            registry
                .register_package(installed_package.clone(), schemas)
                .await?;
        }

        self.update_progress(
            &package_id,
            InstallStatus::Completed,
            100.0,
            "Installation completed",
        )
        .await;

        Ok((installed_package, conversion_results))
    }

    /// Get StructureDefinitions for a package
    async fn get_structure_definitions_for_package(
        &self,
        _package_name: &str,
    ) -> Result<Vec<StructureDefinition>> {
        // Search for StructureDefinition resources in the package
        let search_results = self
            .canonical_manager
            .search()
            .await
            .resource_type("StructureDefinition")
            .execute()
            .await
            .map_err(|e| FhirSchemaError::Search {
                message: format!("Failed to search for StructureDefinitions: {e}"),
            })?;

        let mut structure_definitions = Vec::with_capacity(search_results.resources.len());

        for resource_match in search_results.resources {
            match serde_json::from_value::<StructureDefinition>(resource_match.resource.content) {
                Ok(mut structure_def) => {
                    // Filter by package name using the StructureDefinition's URL or id
                    let belongs_to_package = if let Some(url) = &structure_def.url {
                        url.as_str().contains(_package_name)
                            || url
                                .path_segments()
                                .and_then(|mut segments| segments.next_back())
                                .is_some_and(|segment| segment.contains(_package_name))
                    } else if let Some(id) = &structure_def.id {
                        id.contains(_package_name)
                    } else {
                        // If no URL or ID, include by default for backward compatibility
                        true
                    };

                    if belongs_to_package && structure_def.extract_elements().is_ok() {
                        structure_definitions.push(structure_def);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse StructureDefinition: {e}");
                }
            }
        }

        Ok(structure_definitions)
    }

    /// Convert StructureDefinitions to FhirSchemas
    async fn convert_structure_definitions(
        &self,
        structure_definitions: &[StructureDefinition],
        _conversion_options: &crate::package::ConversionOptions,
    ) -> Result<(Vec<FhirSchema>, ConversionResults)> {
        let start_time = std::time::Instant::now();

        // Use optimized conversion pipeline with streaming for large packages
        let pipeline = crate::package::pipeline::ConversionPipeline::new(
            self.converter.clone(),
            self.config.max_concurrent_conversions,
        );

        let method = if structure_definitions.len() > 100 {
            "streaming"
        } else {
            "batch"
        };

        let batch_result = if structure_definitions.len() > 100 {
            pipeline
                .convert_streaming(structure_definitions, _conversion_options)
                .await?
        } else {
            pipeline
                .convert_batch(structure_definitions, _conversion_options)
                .await?
        };

        let schemas: Vec<FhirSchema> = batch_result.schemas;
        let failed_conversions: Vec<crate::package::ConversionError> = batch_result.results.failed;

        // Log conversion method used for monitoring
        if !structure_definitions.is_empty() {
            eprintln!(
                "Converted {} schemas using {} method",
                schemas.len(),
                method
            );
        }

        let conversion_results = ConversionResults {
            total_structure_definitions: structure_definitions.len(),
            converted_schemas: schemas.len(),
            skipped: 0,
            failed: failed_conversions,
            conversion_time: start_time.elapsed(),
            performance_stats: batch_result.results.performance_stats,
        };

        Ok((schemas, conversion_results))
    }

    /// Uninstall a package
    pub async fn uninstall_package(&self, package_id: &PackageId) -> Result<bool> {
        // Remove from registry (this also removes from storage and handles dependencies)
        let removed_package = {
            let registry = self.registry.write().await;
            registry.unregister_package(package_id).await?
        };

        if let Some(package) = removed_package {
            // Remove from canonical manager if possible
            // Note: CanonicalManager might not support uninstallation

            // Clean up any cached data
            for schema_url in &package.schemas {
                let _ = self.storage.delete(schema_url).await;
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if a package is installed
    pub async fn is_package_installed(&self, package_id: &PackageId) -> bool {
        let registry = self.registry.read().await;
        registry.is_installed(package_id)
    }

    /// List all installed packages
    pub async fn list_packages(&self) -> Vec<PackageId> {
        let registry = self.registry.read().await;
        registry.list_packages()
    }

    /// Get package information
    pub async fn get_package(&self, package_id: &PackageId) -> Option<InstalledPackage> {
        let registry = self.registry.read().await;
        registry.get_package(package_id)
    }

    /// Resolve all dependencies for a set of package specs
    async fn resolve_all_dependencies(
        &self,
        package_specs: &[PackageSpec],
    ) -> Result<Vec<PackageSpec>> {
        let mut resolved_specs = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::from(package_specs.to_vec());

        while let Some(spec) = queue.pop_front() {
            let package_id = PackageId::new(&spec.name, &spec.version);

            // Skip if already processed
            if visited.contains(&package_id) {
                continue;
            }

            visited.insert(package_id.clone());
            resolved_specs.push(spec.clone());

            // Add dependencies to queue for processing
            for dep_spec in &spec.dependencies {
                let dep_id = PackageId::new(&dep_spec.name, &dep_spec.version);
                if !visited.contains(&dep_id) {
                    queue.push_back(dep_spec.clone());
                }
            }

            // Try to resolve additional dependencies from package metadata
            let metadata_deps = self
                .extract_package_dependencies(&spec.name, &spec.version)
                .await;
            for dep_id in metadata_deps {
                if !visited.contains(&dep_id) {
                    // Create a basic PackageSpec for the dependency
                    let dep_spec = PackageSpec {
                        name: dep_id.name.clone(),
                        version: dep_id.version.clone(),
                        source: crate::package::specification::PackageSource::Registry {
                            url: None,
                            auth: None,
                        },
                        dependencies: Vec::new(),
                        conversion_options: Default::default(),
                        metadata: Default::default(),
                    };
                    queue.push_back(dep_spec);
                }
            }
        }

        Ok(resolved_specs)
    }

    /// Update installation progress
    async fn update_progress(
        &self,
        package_id: &PackageId,
        status: InstallStatus,
        progress: f64,
        operation: &str,
    ) {
        let mut tracker = self.progress_tracker.write().await;

        let progress_info =
            tracker
                .entry(package_id.to_string())
                .or_insert_with(|| InstallProgress {
                    package_id: package_id.clone(),
                    status: InstallStatus::Pending,
                    progress_percentage: 0.0,
                    current_operation: String::new(),
                    started_at: Utc::now(),
                    estimated_completion: None,
                    error: None,
                });

        progress_info.status = status;
        progress_info.progress_percentage = progress;
        progress_info.current_operation = operation.to_string();

        if matches!(progress_info.status, InstallStatus::Failed) {
            // Error will be set separately if needed
        }
    }

    /// Get installation progress
    pub async fn get_progress(&self, package_id: &PackageId) -> Option<InstallProgress> {
        let tracker = self.progress_tracker.read().await;
        tracker.get(&package_id.to_string()).cloned()
    }

    /// Rollback installation of packages
    async fn rollback_installation(&self, packages: &[InstalledPackage]) {
        for package in packages.iter().rev() {
            let _ = self.uninstall_package(&package.id).await;
        }
    }

    /// Calculate checksum for package schemas
    async fn calculate_package_checksum(&self, schemas: &[FhirSchema]) -> Option<String> {
        use sha2::{Digest, Sha256};

        if schemas.is_empty() {
            return None;
        }

        let mut hasher = Sha256::new();

        // Sort schemas by URL for deterministic checksum
        let mut sorted_schemas: Vec<&FhirSchema> = schemas.iter().collect();
        sorted_schemas.sort_by(|a, b| match (&a.url, &b.url) {
            (Some(url_a), Some(url_b)) => url_a.as_str().cmp(url_b.as_str()),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        });

        // Hash each schema's canonical representation
        for schema in sorted_schemas {
            if let Ok(schema_json) = serde_json::to_string(schema) {
                hasher.update(schema_json.as_bytes());
            }
        }

        let result = hasher.finalize();
        Some(format!("{result:x}"))
    }

    /// Extract dependencies from package metadata
    async fn extract_package_dependencies(
        &self,
        package_name: &str,
        _version: &str,
    ) -> Vec<PackageId> {
        let mut dependencies = Vec::new();

        // Try to get package manifest/metadata
        // Note: get_package_info may not be available, using fallback approach

        // Fallback: check common FHIR dependencies
        if !package_name.contains("hl7.fhir.r4.core") {
            // Most FHIR packages depend on the core specification
            dependencies.push(PackageId::new("hl7.fhir.r4.core", "4.0.1"));
        }

        // Add other common dependencies based on package patterns
        if package_name.contains("us.core") {
            dependencies.push(PackageId::new("hl7.fhir.us.core", "6.1.0"));
        }

        dependencies
    }

    /// Get the file path for an installed package
    async fn get_package_file_path(
        &self,
        package_name: &str,
        version: &str,
    ) -> Option<std::path::PathBuf> {
        // Try to get the package directory from canonical manager
        // Since get_package_directory may not be available, use fallback approach
        {
            // Fallback: construct expected path based on naming conventions
            let cache_dir = std::env::var("FHIR_CACHE_DIR")
                .or_else(|_| std::env::var("HOME").map(|h| format!("{h}/.fhir/packages")))
                .unwrap_or_else(|_| "/tmp/fhir/packages".to_string());

            let package_path =
                std::path::PathBuf::from(cache_dir).join(format!("{package_name}#{version}"));

            if package_path.exists() {
                Some(package_path)
            } else {
                None
            }
        }
    }

    /// Categorize error for better reporting
    fn categorize_error(&self, error: &FhirSchemaError) -> ErrorCategory {
        match error {
            FhirSchemaError::Download { .. } => ErrorCategory::Download,
            FhirSchemaError::Parsing { .. } => ErrorCategory::Parsing,
            FhirSchemaError::Conversion { .. } => ErrorCategory::Conversion,
            FhirSchemaError::Storage { .. } => ErrorCategory::Storage,
            FhirSchemaError::Dependency { .. } => ErrorCategory::Dependency,
            FhirSchemaError::Validation { .. } => ErrorCategory::Validation,
            _ => ErrorCategory::Network,
        }
    }
}

// Implement ModelProvider trait for ecosystem integration
#[async_trait]
impl ModelProvider for FhirSchemaPackageManager {
    async fn get_schema(&self, canonical_url: &str) -> Option<Arc<FhirSchema>> {
        let registry = self.registry.read().await;
        registry.get_schema(canonical_url).await
    }

    async fn get_schemas_by_type(&self, resource_type: &str) -> Vec<Arc<FhirSchema>> {
        let registry = self.registry.read().await;
        registry.get_schemas_by_type(resource_type).await
    }

    async fn resolve_profile(&self, base_type: &str, profile_url: &str) -> Option<Arc<FhirSchema>> {
        // First try to get the profile directly
        if let Some(profile_schema) = self.get_schema(profile_url).await {
            return Some(profile_schema);
        }

        // If not found, search for profiles of the base type
        let schemas = self.get_schemas_by_type(base_type).await;
        schemas.into_iter().find(|schema| {
            schema
                .url
                .as_ref()
                .map(|url| url.as_str() == profile_url)
                .unwrap_or(false)
        })
    }

    async fn has_resource_type(&self, resource_type: &str) -> bool {
        // O(1) lookup using type registry
        let registry = self.registry.read().await;
        let guard = registry.type_registry.pin();
        if let Some(type_registry) = guard.get(&()) {
            type_registry.is_resource_type(resource_type)
        } else {
            false
        }
    }

    async fn get_resource_types(&self) -> Vec<String> {
        // O(1) access instead of iterating through schemas
        let registry = self.registry.read().await;
        let guard = registry.type_registry.pin();
        if let Some(type_registry) = guard.get(&()) {
            type_registry
                .get_all_resource_types()
                .iter()
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    async fn search_schemas(&self, query: &str) -> Vec<Arc<FhirSchema>> {
        let registry = self.registry.read().await;
        registry.search_schemas(query).await
    }
}

impl Default for PackageManagerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_conversions: 8,
            registry_config: RegistryConfig::default(),
            storage_config: StorageConfig::default(),
            converter_config: ConverterConfig::default(),
            auto_resolve_dependencies: true,
            validate_after_install: true,
            cleanup_on_failure: true,
        }
    }
}

impl std::fmt::Debug for FhirSchemaPackageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FhirSchemaPackageManager")
            .field("canonical_manager", &"Arc<CanonicalManager>")
            .field("storage", &"Arc<EnhancedStorageManager>")
            .field("registry", &"Arc<RwLock<PackageRegistry>>")
            .field("converter", &"Arc<FhirSchemaConverter>")
            .field("config", &"PackageManagerConfig")
            .finish()
    }
}

impl std::fmt::Debug for PackageManagerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackageManagerConfig")
            .field(
                "max_concurrent_conversions",
                &self.max_concurrent_conversions,
            )
            .field("registry_config", &self.registry_config)
            .field("storage_config", &"StorageConfig")
            .field("converter_config", &self.converter_config)
            .field("auto_resolve_dependencies", &self.auto_resolve_dependencies)
            .field("validate_after_install", &self.validate_after_install)
            .field("cleanup_on_failure", &self.cleanup_on_failure)
            .finish()
    }
}

// Extension trait for ConversionResults
impl ConversionResults {
    pub fn merge(&mut self, other: ConversionResults) {
        self.total_structure_definitions += other.total_structure_definitions;
        self.converted_schemas += other.converted_schemas;
        self.skipped += other.skipped;
        self.failed.extend(other.failed);
        self.conversion_time += other.conversion_time;

        // Merge performance stats
        let total_time = self.performance_stats.avg_conversion_time_ms
            + other.performance_stats.avg_conversion_time_ms;
        self.performance_stats.avg_conversion_time_ms = total_time / 2.0;

        self.performance_stats.max_conversion_time_ms = self
            .performance_stats
            .max_conversion_time_ms
            .max(other.performance_stats.max_conversion_time_ms);

        if self.performance_stats.min_conversion_time_ms == 0 {
            self.performance_stats.min_conversion_time_ms =
                other.performance_stats.min_conversion_time_ms;
        } else {
            self.performance_stats.min_conversion_time_ms = self
                .performance_stats
                .min_conversion_time_ms
                .min(other.performance_stats.min_conversion_time_ms);
        }
    }
}
