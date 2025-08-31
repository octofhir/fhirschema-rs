use crate::converter::{ConverterConfig, FhirSchemaConverter, StructureDefinition};
use crate::error::{FhirSchemaError, Result};
use crate::package::{
    ConversionResults, ErrorCategory, InstallOptions, InstalledPackage, PackageId,
    PackageInstallError, PackageInstallResult, PackageRegistry, PackageSpec, RegistryConfig,
};
use crate::storage::{EnhancedStorageManager, StorageConfig};
use crate::types::{
    BridgeCardinality, BridgeConstraintInfo, BridgeRegistryMetrics, BridgeValidationError,
    BridgeValidationResult, ElementInfo, FhirSchema, PathResolver, PropertyInfo,
};
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

    /// Path resolver for element path resolution
    path_resolver: Arc<PathResolver>,
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

        // Create path resolver using the registry's schema index
        let schema_index = registry.read().await.get_schema_index().clone();
        let path_resolver = Arc::new(PathResolver::new(schema_index));

        Ok(Self {
            canonical_manager,
            storage,
            registry,
            converter,
            config,
            progress_tracker,
            path_resolver,
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

    // ============================================================================
    // BRIDGE SUPPORT METHODS
    // These methods provide comprehensive support for external bridge libraries
    // ============================================================================

    // === SCHEMA ACCESS METHODS ===

    /// Get schema by canonical URL - O(1) operation
    pub async fn get_schema(&self, canonical_url: &str) -> Option<Arc<FhirSchema>> {
        let registry = self.registry.read().await;
        registry.get_schema(canonical_url).await
    }

    /// Get all schemas for a resource type
    pub async fn get_schemas_by_type(&self, resource_type: &str) -> Vec<Arc<FhirSchema>> {
        let registry = self.registry.read().await;
        registry.get_schemas_by_type(resource_type).await
    }

    /// Get schema by type name (first match)
    pub async fn get_schema_by_type(&self, type_name: &str) -> Option<Arc<FhirSchema>> {
        self.get_schemas_by_type(type_name).await.into_iter().next()
    }

    /// Resolve profile for a base type
    pub async fn resolve_profile(
        &self,
        base_type: &str,
        profile_url: &str,
    ) -> Option<Arc<FhirSchema>> {
        // First try direct URL lookup
        if let Some(schema) = self.get_schema(profile_url).await {
            return Some(schema);
        }

        // Then search in base type schemas
        let base_schemas = self.get_schemas_by_type(base_type).await;
        base_schemas
            .into_iter()
            .find(|s| s.url.as_ref().map(|u| u.as_str()) == Some(profile_url))
    }

    /// Search schemas by query string
    pub async fn search_schemas(&self, query: &str) -> Vec<Arc<FhirSchema>> {
        let registry = self.registry.read().await;
        registry.search_schemas(query).await
    }

    // === RESOURCE TYPE METHODS ===

    /// Check if a resource type is known - O(1) operation
    pub async fn has_resource_type(&self, resource_type: &str) -> bool {
        let registry = self.registry.read().await;
        let guard = registry.type_registry.pin();
        if let Some(type_registry) = guard.get(&()) {
            type_registry.is_resource_type(resource_type)
        } else {
            false
        }
    }

    /// Get all known resource types - O(1) operation
    pub async fn get_resource_types(&self) -> Vec<String> {
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

    /// Check if type is primitive - O(1) operation
    pub async fn is_primitive_type(&self, type_name: &str) -> bool {
        let registry = self.registry.read().await;
        let guard = registry.type_registry.pin();
        if let Some(type_registry) = guard.get(&()) {
            type_registry.is_primitive_type(type_name)
        } else {
            false
        }
    }

    /// Check if type is complex - O(1) operation
    pub async fn is_complex_type(&self, type_name: &str) -> bool {
        let registry = self.registry.read().await;
        let guard = registry.type_registry.pin();
        if let Some(type_registry) = guard.get(&()) {
            type_registry.is_complex_type(type_name)
        } else {
            false
        }
    }

    /// Get base type for inheritance
    pub async fn get_base_type(&self, type_name: &str) -> Option<String> {
        let registry = self.registry.read().await;
        let guard = registry.type_registry.pin();
        if let Some(type_registry) = guard.get(&()) {
            type_registry.get_base_type(type_name)
        } else {
            None
        }
    }

    /// Check if one type is subtype of another
    pub async fn is_subtype_of(&self, child_type: &str, parent_type: &str) -> bool {
        let registry = self.registry.read().await;
        let guard = registry.type_registry.pin();
        if let Some(type_registry) = guard.get(&()) {
            type_registry.is_subtype_of(child_type, parent_type)
        } else {
            false
        }
    }

    // === CHOICE TYPES METHODS ===

    /// Get all possible choice type expansions for a base path
    pub async fn get_choice_type_options(&self, base_path: &str) -> Vec<String> {
        if !base_path.ends_with("[x]") {
            return Vec::new();
        }

        // Try to find schemas that contain this choice type
        let schemas = self.search_schemas(base_path).await;

        let mut options = std::collections::HashSet::new();

        for schema in schemas {
            if let Some(element) = schema.elements.get(base_path) {
                if let Some(element_types) = &element.element_type {
                    for element_type in element_types {
                        options.insert(element_type.code.clone());
                    }
                }
            }
        }

        options.into_iter().collect()
    }

    /// Resolve choice type from base path and actual type
    pub async fn resolve_choice_type(&self, base_path: &str, value_type: &str) -> Option<String> {
        if base_path.ends_with("[x]") {
            let base_without_choice = base_path.trim_end_matches("[x]");
            let capitalized_type = capitalize_first_letter(value_type);
            Some(format!("{base_without_choice}{capitalized_type}"))
        } else {
            None
        }
    }

    /// Check if a path is a choice type expansion
    pub async fn is_choice_type_expansion(&self, path: &str) -> bool {
        // Check if this path could be a choice type expansion
        for type_name in &[
            "String",
            "Integer",
            "Boolean",
            "DateTime",
            "Code",
            "Uri",
            "Coding",
            "CodeableConcept",
            "Reference",
            "Quantity",
        ] {
            if let Some(base_without_suffix) = path.strip_suffix(type_name) {
                let potential_base = format!("{base_without_suffix}[x]");
                if self
                    .get_choice_type_options(&potential_base)
                    .await
                    .contains(&type_name.to_lowercase())
                {
                    return true;
                }
            }
        }
        false
    }

    /// Get the base path for a choice type expansion
    pub async fn get_choice_type_base(&self, expanded_path: &str) -> Option<String> {
        for type_name in &[
            "String",
            "Integer",
            "Boolean",
            "DateTime",
            "Code",
            "Uri",
            "Coding",
            "CodeableConcept",
            "Reference",
            "Quantity",
        ] {
            if let Some(base_without_suffix) = expanded_path.strip_suffix(type_name) {
                let potential_base = format!("{base_without_suffix}[x]");
                if self
                    .get_choice_type_options(&potential_base)
                    .await
                    .contains(&type_name.to_lowercase())
                {
                    return Some(potential_base);
                }
            }
        }
        None
    }

    // === PATH RESOLUTION METHODS ===

    /// Resolve element path within a type
    pub async fn resolve_element_path(&self, base_type: &str, path: &str) -> Option<ElementInfo> {
        self.path_resolver
            .resolve_path(base_type, path)
            .await
            .map(|res| res.element_info)
    }

    /// Get element cardinality for a specific path
    pub async fn get_element_cardinality(
        &self,
        type_name: &str,
        path: &str,
    ) -> Option<BridgeCardinality> {
        let resolution = self.path_resolver.resolve_path(type_name, path).await?;
        Some(BridgeCardinality {
            min: resolution.cardinality.min,
            max: resolution.cardinality.max,
        })
    }

    /// Check if a path exists in a type
    pub async fn has_element_path(&self, type_name: &str, path: &str) -> bool {
        self.path_resolver
            .resolve_path(type_name, path)
            .await
            .is_some()
    }

    /// Get all available paths for a type (for auto-completion)
    pub async fn get_available_paths(&self, type_name: &str) -> Vec<String> {
        self.path_resolver.get_available_paths(type_name).await
    }

    /// Get element type for a specific path
    pub async fn get_element_type(&self, base_type: &str, path: &str) -> Option<String> {
        self.path_resolver
            .resolve_path(base_type, path)
            .await
            .map(|res| res.target_type)
    }

    // === TYPE REFLECTION METHODS ===

    /// Get all properties/elements for a type
    pub async fn get_type_properties(&self, type_name: &str) -> Vec<PropertyInfo> {
        let Some(schema) = self.get_schema_by_type(type_name).await else {
            return Vec::new();
        };

        schema
            .elements
            .iter()
            .map(|(path, element)| PropertyInfo {
                name: path.clone(),
                element_type: element
                    .element_type
                    .as_ref()
                    .and_then(|types| types.first())
                    .map(|t| t.code.clone())
                    .unwrap_or_default(),
                cardinality: BridgeCardinality {
                    min: element.min.unwrap_or(0),
                    max: element
                        .max
                        .as_ref()
                        .and_then(|m| if m == "*" { None } else { m.parse().ok() }),
                },
                is_collection: element
                    .max
                    .as_ref()
                    .map(|m| m == "*" || m.parse::<u32>().unwrap_or(1) > 1)
                    .unwrap_or(false),
                is_required: element.min.unwrap_or(0) > 0,
                is_choice_type: element
                    .element_type
                    .as_ref()
                    .map(|types| types.len() > 1)
                    .unwrap_or(false),
                definition: element.definition.clone(),
            })
            .collect()
    }

    // === CONSTRAINT METHODS ===

    /// Get all constraints for a type
    pub async fn get_type_constraints(&self, type_name: &str) -> Vec<BridgeConstraintInfo> {
        let Some(schema) = self.get_schema_by_type(type_name).await else {
            return Vec::new();
        };

        let mut constraints = Vec::new();

        // Schema-level constraints
        for constraint in &schema.constraints {
            constraints.push(BridgeConstraintInfo {
                key: constraint.key.clone(),
                severity: constraint.severity.clone(),
                human_description: constraint.human.clone(),
                fhirpath_expression: constraint.expression.clone(),
                source: Some(schema.name.clone().unwrap_or_default()),
                xpath: constraint.xpath.clone(),
                requires_fhirpath: self.is_complex_fhirpath_expression(&constraint.expression),
            });
        }

        // Element-level constraints
        for element in schema.elements.values() {
            for constraint in &element.constraints {
                constraints.push(BridgeConstraintInfo {
                    key: constraint.key.clone(),
                    severity: constraint.severity.clone(),
                    human_description: constraint.human.clone(),
                    fhirpath_expression: constraint.expression.clone(),
                    source: Some(schema.name.clone().unwrap_or_default()),
                    xpath: constraint.xpath.clone(),
                    requires_fhirpath: self.is_complex_fhirpath_expression(&constraint.expression),
                });
            }
        }

        constraints
    }

    /// Get constraints for a specific element path
    pub async fn get_element_constraints(
        &self,
        type_name: &str,
        path: &str,
    ) -> Vec<BridgeConstraintInfo> {
        let Some(schema) = self.get_schema_by_type(type_name).await else {
            return Vec::new();
        };

        if let Some(element) = schema.elements.get(path) {
            element
                .constraints
                .iter()
                .map(|c| BridgeConstraintInfo {
                    key: c.key.clone(),
                    severity: c.severity.clone(),
                    human_description: c.human.clone(),
                    fhirpath_expression: c.expression.clone(),
                    source: Some(format!("{type_name}.{path}")),
                    xpath: c.xpath.clone(),
                    requires_fhirpath: self.is_complex_fhirpath_expression(&c.expression),
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Validate constraint expression syntax
    pub async fn validate_constraint_expression(&self, expression: &str) -> BridgeValidationResult {
        // Simple validation - could be enhanced with FHIRPath parser
        if expression.is_empty() {
            return BridgeValidationResult::invalid(vec![BridgeValidationError::new(
                "Expression cannot be empty".to_string(),
                "empty-expression".to_string(),
            )]);
        }

        BridgeValidationResult::valid()
    }

    // === UTILITY METHODS ===

    /// Get registry metrics and statistics
    pub async fn get_registry_metrics(&self) -> BridgeRegistryMetrics {
        let registry = self.registry.read().await;

        // Get basic stats from registry
        let stats = registry.get_stats();
        let path_metrics = self.path_resolver.get_metrics().await;

        BridgeRegistryMetrics {
            total_schemas: stats.total_schemas,
            resource_types: stats.total_schemas, // Approximate
            profiles: 0,                         // Would need more sophisticated counting
            extensions: 0,                       // Would need more sophisticated counting
            memory_usage_bytes: (stats.memory_usage_mb * 1024.0 * 1024.0) as u64,
            index_rebuild_time_ms: 0, // Would need tracking
            cache_stats: crate::types::BridgeCacheStats {
                schema_cache_hits: 0,   // Would need tracking
                schema_cache_misses: 0, // Would need tracking
                path_cache_hits: path_metrics.cache_hits,
                path_cache_misses: path_metrics.cache_misses,
                type_cache_hits: 0,   // Would need tracking
                type_cache_misses: 0, // Would need tracking
            },
        }
    }

    /// Refresh/rebuild internal caches and indexes
    pub async fn rebuild_indexes(&self) -> Result<()> {
        // Rebuild type registry from current schemas
        self.registry.write().await.rebuild_type_registry().await?;

        // Rebuild path resolver common paths
        let resource_types = self.get_resource_types().await;
        self.path_resolver
            .precompute_common_paths(&resource_types)
            .await?;

        Ok(())
    }

    // === PRIVATE HELPER METHODS ===

    /// Check if a FHIRPath expression is complex and requires full evaluation
    fn is_complex_fhirpath_expression(&self, expression: &str) -> bool {
        // Simple heuristic to detect complex expressions
        expression.contains("where(")
            || expression.contains("select(")
            || expression.contains("all(")
            || expression.contains("any(")
            || expression.contains("implies")
            || expression.contains("and ")
            || expression.contains(" or ")
            || expression.contains("extension(")
    }
}

// Helper function for choice type resolution
fn capitalize_first_letter(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }
    let mut chars: Vec<char> = s.chars().collect();
    chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
    chars.into_iter().collect()
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
