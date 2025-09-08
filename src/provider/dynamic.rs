use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::{FhirVersion, ResolutionContext};
use crate::error::{FhirSchemaError, Result};
use crate::provider::FhirSchemaModelProvider;
use crate::storage::{DiskStorage, DiskStorageConfig};
use crate::types::FhirSchema;
use crate::utils::{PackageFingerprint, fingerprint::generate_package_fingerprint};
use octofhir_fhir_model::provider::{FhirVersion as ModelProviderFhirVersion, ModelProvider};

/// Dynamic ModelProvider with disk caching for fast subsequent startups
#[derive(Debug)]
pub struct DynamicModelProvider {
    /// FHIR version this provider supports
    fhir_version: FhirVersion,
    /// Disk storage for caching compiled schemas
    disk_storage: Arc<RwLock<DiskStorage>>,
    /// Optional fallback to full provider for cache misses
    fallback_provider: Option<Arc<FhirSchemaModelProvider>>,
    /// Cache directory path
    cache_dir: PathBuf,
    /// Current package fingerprint (if loaded)
    current_fingerprint: Option<PackageFingerprint>,
}

#[derive(Debug, Clone)]
pub struct DynamicProviderConfig {
    /// Automatically create fallback provider on cache miss
    pub auto_fallback: bool,
    /// Enable disk compression
    pub enable_compression: bool,
    /// Enable binary serialization
    pub binary_serialization: bool,
    /// Package loading timeout
    pub package_timeout: std::time::Duration,
}

impl Default for DynamicProviderConfig {
    fn default() -> Self {
        Self {
            auto_fallback: true,
            enable_compression: true,
            binary_serialization: cfg!(feature = "embedded-providers"),
            package_timeout: std::time::Duration::from_secs(60),
        }
    }
}

impl DynamicModelProvider {
    /// Create dynamic provider for FHIR R4
    pub async fn r4() -> Result<Self> {
        Self::new(FhirVersion::R4).await
    }

    /// Create dynamic provider for FHIR R4B
    pub async fn r4b() -> Result<Self> {
        Self::new(FhirVersion::R4B).await
    }

    /// Create dynamic provider for FHIR R5  
    pub async fn r5() -> Result<Self> {
        Self::new(FhirVersion::R5).await
    }

    /// Create dynamic provider for FHIR R6
    pub async fn r6() -> Result<Self> {
        Self::new(FhirVersion::R6).await
    }

    /// Create dynamic provider with custom configuration
    pub async fn with_config(
        fhir_version: FhirVersion,
        config: DynamicProviderConfig,
    ) -> Result<Self> {
        let cache_dir = DiskStorage::default_cache_dir()?
            .join("dynamic")
            .join(fhir_version.short_name());

        let disk_config = DiskStorageConfig {
            enable_compression: config.enable_compression,
            binary_serialization: config.binary_serialization,
            ..Default::default()
        };

        let disk_storage = Arc::new(RwLock::new(
            DiskStorage::with_config(&cache_dir, disk_config).await?,
        ));

        Ok(Self {
            fhir_version,
            disk_storage,
            fallback_provider: None,
            cache_dir,
            current_fingerprint: None,
        })
    }

    /// Create dynamic provider for a specific FHIR version  
    pub async fn new(fhir_version: FhirVersion) -> Result<Self> {
        Self::with_config(fhir_version, DynamicProviderConfig::default()).await
    }

    /// Load or compile schemas for a FHIR package
    pub async fn load_package(
        &mut self,
        package_id: &str,
        package_version: &str,
    ) -> Result<PackageFingerprint> {
        // First try to load from canonical manager to get StructureDefinitions
        let canonical_manager = octofhir_canonical_manager::CanonicalManager::new(
            octofhir_canonical_manager::FcmConfig::default(),
        )
        .await
        .map_err(|e| FhirSchemaError::conversion_failed("CanonicalManager", &e.to_string()))?;

        // Install the package
        canonical_manager
            .install_package(package_id, package_version)
            .await
            .map_err(|e| {
                FhirSchemaError::conversion_failed("Package Installation", &e.to_string())
            })?;

        // Get all StructureDefinitions from the package
        let structure_definitions = self
            .extract_structure_definitions(&canonical_manager, package_id)
            .await?;

        // Generate fingerprint from StructureDefinitions
        let serialized = serde_json::to_vec(&structure_definitions)
            .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))?;
        let fingerprint = generate_package_fingerprint(package_id, package_version, &serialized);

        // Check if we have this exact version cached
        {
            let storage = self.disk_storage.read().await;
            if storage.is_package_cached(&fingerprint).await {
                #[cfg(feature = "tracing")]
                tracing::info!(
                    "Package {}@{} found in cache ({})",
                    package_id,
                    package_version,
                    fingerprint.short_hash()
                );

                self.current_fingerprint = Some(fingerprint.clone());
                return Ok(fingerprint);
            }
        }

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Compiling and caching package {}@{} ({})",
            package_id,
            package_version,
            fingerprint.short_hash()
        );

        // Not in cache - compile schemas
        let schemas = self
            .compile_schemas(Arc::new(canonical_manager), &structure_definitions)
            .await?;

        // Store in cache
        {
            let mut storage = self.disk_storage.write().await;
            storage
                .store_package(package_id, package_version, schemas)
                .await?;
        }

        self.current_fingerprint = Some(fingerprint.clone());
        Ok(fingerprint)
    }

    /// Extract StructureDefinitions from a package
    async fn extract_structure_definitions(
        &self,
        canonical_manager: &octofhir_canonical_manager::CanonicalManager,
        package_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        // Get core resource types for this FHIR version
        let core_resource_types = self.get_core_resource_types();
        let mut structure_defs = Vec::new();

        for resource_type in &core_resource_types {
            let structure_definition_url =
                format!("http://hl7.org/fhir/StructureDefinition/{resource_type}");

            match canonical_manager.resolve(&structure_definition_url).await {
                Ok(resolved_resource) => {
                    structure_defs.push(resolved_resource.resource.content);
                }
                Err(_e) => {
                    #[cfg(feature = "tracing")]
                    tracing::warn!(
                        "Failed to resolve StructureDefinition for {}: {}",
                        resource_type,
                        _e
                    );
                }
            }
        }

        if structure_defs.is_empty() {
            return Err(FhirSchemaError::conversion_failed(
                package_id,
                "No StructureDefinition resources could be resolved from the package",
            ));
        }

        Ok(structure_defs)
    }

    /// Compile StructureDefinitions to FhirSchemas
    async fn compile_schemas(
        &self,
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
        structure_definitions: &[serde_json::Value],
    ) -> Result<Vec<FhirSchema>> {
        // Create ConversionEngine
        let conversion_engine = crate::conversion::ConversionEngine::new(
            canonical_manager,
            &crate::core::PerformanceConfig::default(),
        )
        .await?;

        // Convert StructureDefinitions to schemas
        let conversion_results = conversion_engine
            .convert_batch(structure_definitions.to_vec())
            .await?;

        let mut schemas = Vec::new();
        for result in conversion_results {
            if result.success {
                if let Some(schema) = result.schema {
                    schemas.push(schema);
                }
            }
        }

        if schemas.is_empty() {
            return Err(FhirSchemaError::conversion_failed(
                "batch",
                "No schemas were successfully converted",
            ));
        }

        Ok(schemas)
    }

    /// Get core resource types for this FHIR version
    fn get_core_resource_types(&self) -> Vec<&'static str> {
        match self.fhir_version {
            FhirVersion::R4 | FhirVersion::R4B => vec![
                "Resource",
                "DomainResource",
                "Patient",
                "Practitioner",
                "Organization",
                "Bundle",
                "Observation",
                "Condition",
                "Medication",
                "MedicationRequest",
                "Encounter",
                "DiagnosticReport",
                "Procedure",
                "Immunization",
                "AllergyIntolerance",
            ],
            FhirVersion::R5 | FhirVersion::R6 => vec![
                "Resource",
                "DomainResource",
                "Patient",
                "Practitioner",
                "Organization",
                "Bundle",
                "Observation",
                "Condition",
                "Medication",
                "MedicationRequest",
                "Encounter",
                "DiagnosticReport",
                "Procedure",
                "Immunization",
                "AllergyIntolerance",
                "Subscription",
                "SubscriptionStatus",
                "SubscriptionTopic",
            ],
        }
    }

    /// Get cached schemas for current package
    async fn get_cached_schemas(&self) -> Result<Option<Vec<FhirSchema>>> {
        if let Some(ref fingerprint) = self.current_fingerprint {
            let mut storage = self.disk_storage.write().await;
            storage.load_package(fingerprint).await
        } else {
            Ok(None)
        }
    }

    /// Initialize with standard FHIR package
    pub async fn initialize(&mut self) -> Result<()> {
        let (package_name, package_version) = match self.fhir_version {
            FhirVersion::R4 => ("hl7.fhir.r4.core", "4.0.1"),
            FhirVersion::R4B => ("hl7.fhir.r4b.core", "4.3.0"),
            FhirVersion::R5 => ("hl7.fhir.r5.core", "5.0.0"),
            FhirVersion::R6 => ("hl7.fhir.r6.core", "6.0.0-ballot3"),
        };

        self.load_package(package_name, package_version).await?;
        Ok(())
    }

    /// Get FHIR version
    pub fn fhir_version(&self) -> FhirVersion {
        self.fhir_version
    }

    /// Get cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Clear all cached data
    pub async fn clear_cache(&mut self) -> Result<()> {
        let mut storage = self.disk_storage.write().await;
        storage.clear_cache().await?;
        self.current_fingerprint = None;
        Ok(())
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.current_fingerprint.is_some()
    }
}

// Implement ModelProvider trait by delegating to fallback or using cached data
#[async_trait]
impl ModelProvider for DynamicModelProvider {
    async fn get_type_hierarchy(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<Option<octofhir_fhir_model::type_system::TypeHierarchy>>
    {
        // Try to use cached data first
        if let Some(schemas) = self.get_cached_schemas().await.ok().flatten() {
            for schema in &schemas {
                if let Some(title) = &schema.title {
                    if title == type_name {
                        // Build hierarchy from cached schema
                        // This would be a simplified version
                        use octofhir_fhir_model::type_system::{DerivationType, TypeHierarchy};

                        return Ok(Some(TypeHierarchy {
                            type_name: type_name.to_string(),
                            ancestors: vec!["Resource".to_string()],
                            descendants: Vec::new(),
                            direct_parent: Some("Resource".to_string()),
                            direct_children: Vec::new(),
                            is_abstract: false,
                            derivation: DerivationType::Specialization,
                            hierarchy_depth: 1,
                        }));
                    }
                }
            }
        }

        // Fallback to full provider if available
        if let Some(ref fallback) = self.fallback_provider {
            match fallback.get_type_hierarchy(type_name).await {
                Ok(Some(local_hierarchy)) => {
                    // Convert from local TypeHierarchy to octofhir_fhir_model TypeHierarchy
                    use octofhir_fhir_model::type_system::{DerivationType, TypeHierarchy};

                    let model_hierarchy = TypeHierarchy {
                        type_name: local_hierarchy.base_type,
                        ancestors: local_hierarchy.parent_type.clone().into_iter().collect(),
                        descendants: local_hierarchy.child_types,
                        direct_parent: local_hierarchy.parent_type,
                        direct_children: Vec::new(),
                        is_abstract: false,
                        derivation: DerivationType::Specialization,
                        hierarchy_depth: 1,
                    };
                    Ok(Some(model_hierarchy))
                }
                Ok(None) => Ok(None),
                Err(_) => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    async fn is_type_compatible(
        &self,
        from_type: &str,
        to_type: &str,
    ) -> octofhir_fhir_model::error::Result<bool> {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.is_type_compatible(from_type, to_type).await
        } else {
            // Simple compatibility check
            Ok(from_type == to_type || to_type == "Resource")
        }
    }

    async fn get_common_supertype(
        &self,
        types: &[String],
    ) -> octofhir_fhir_model::error::Result<Option<String>> {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_common_supertype(types).await
        } else {
            Ok(Some("Resource".to_string()))
        }
    }

    async fn get_type_compatibility_matrix(
        &self,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::TypeCompatibilityMatrix>
    {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_type_compatibility_matrix().await
        } else {
            Ok(octofhir_fhir_model::type_system::TypeCompatibilityMatrix::new())
        }
    }

    async fn navigate_typed_path(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::navigation::NavigationResult> {
        if let Some(ref fallback) = self.fallback_provider {
            match fallback.navigate_typed_path(base_type, path).await {
                Ok(local_result) => {
                    // Convert from local NavigationResult to octofhir_fhir_model NavigationResult
                    let reflection_info =
                        octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type(
                            "FHIR",
                            &local_result.target_type,
                        );
                    Ok(octofhir_fhir_model::navigation::NavigationResult::success(
                        reflection_info,
                    ))
                }
                Err(_) => {
                    // Fallback
                    let reflection_info =
                        octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type(
                            "FHIR", "string",
                        );
                    Ok(octofhir_fhir_model::navigation::NavigationResult::success(
                        reflection_info,
                    ))
                }
            }
        } else {
            use octofhir_fhir_model::{
                navigation::NavigationResult, reflection::TypeReflectionInfo,
            };
            Ok(NavigationResult::success(TypeReflectionInfo::simple_type(
                "FHIR", "string",
            )))
        }
    }

    async fn validate_navigation_safety(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::navigation::PathValidation> {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.validate_navigation_safety(base_type, path).await
        } else {
            Ok(octofhir_fhir_model::navigation::PathValidation::success(
                format!("{base_type}.{path}"),
            ))
        }
    }

    async fn get_navigation_result_type(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::reflection::TypeReflectionInfo>,
    > {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_navigation_result_type(base_type, path).await
        } else {
            Ok(Some(
                octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type("FHIR", "string"),
            ))
        }
    }

    async fn get_navigation_metadata(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::NavigationMetadata>
    {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_navigation_metadata(base_type, path).await
        } else {
            use octofhir_fhir_model::type_system::{NavigationMetadata, PerformanceMetadata};

            Ok(NavigationMetadata {
                path: format!("{base_type}.{path}"),
                source_type: base_type.to_string(),
                target_type: "string".to_string(),
                intermediate_types: vec![base_type.to_string()],
                collection_info: Default::default(),
                polymorphic_resolution: None,
                navigation_warnings: Vec::new(),
                performance_metadata: PerformanceMetadata {
                    operation_cost: 0.2, // Fast but not as fast as embedded
                    is_cacheable: true,
                    cache_key: Some(format!("dynamic-{base_type}-{path}")),
                    memory_estimate: Some(128),
                },
            })
        }
    }

    // Delegate remaining methods to fallback provider
    async fn resolve_choice_type(
        &self,
        base_path: &str,
        context: &octofhir_fhir_model::type_system::PolymorphicContext,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::PolymorphicResolution>
    {
        if let Some(ref fallback) = self.fallback_provider {
            // Convert context from octofhir_fhir_model to local ResolutionContext
            let local_context = ResolutionContext::new(base_path);

            match fallback
                .resolve_choice_type(base_path, &local_context)
                .await
            {
                Ok(choice_resolution) => {
                    // Convert from local ChoiceResolution to octofhir_fhir_model PolymorphicResolution
                    Ok(octofhir_fhir_model::type_system::PolymorphicResolution {
                        resolved_type: choice_resolution.resolved_type,
                        confidence_score: choice_resolution.confidence,
                        resolution_method:
                            octofhir_fhir_model::type_system::ResolutionMethod::DefaultFallback,
                        alternative_types: Vec::new(),
                        resolution_context: context.clone(),
                    })
                }
                Err(_) => {
                    // Fallback to default resolution
                    Ok(octofhir_fhir_model::type_system::PolymorphicResolution {
                        resolved_type: "string".to_string(),
                        confidence_score: 0.5,
                        resolution_method:
                            octofhir_fhir_model::type_system::ResolutionMethod::DefaultFallback,
                        alternative_types: Vec::new(),
                        resolution_context: context.clone(),
                    })
                }
            }
        } else {
            use octofhir_fhir_model::type_system::{PolymorphicResolution, ResolutionMethod};
            Ok(PolymorphicResolution {
                resolved_type: "string".to_string(),
                confidence_score: 0.5,
                resolution_method: ResolutionMethod::DefaultFallback,
                alternative_types: Vec::new(),
                resolution_context: context.clone(),
            })
        }
    }

    async fn get_choice_expansions(
        &self,
        choice_property: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::choice_types::ChoiceExpansion>>
    {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_choice_expansions(choice_property).await
        } else {
            Ok(Vec::new())
        }
    }

    async fn infer_choice_type(
        &self,
        context: &octofhir_fhir_model::type_system::PolymorphicContext,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::choice_types::TypeInference> {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.infer_choice_type(context).await
        } else {
            use octofhir_fhir_model::choice_types::{InferenceContext, TypeInference};
            Ok(TypeInference {
                inference_rules: Vec::new(),
                confidence_threshold: 0.5,
                inference_context: InferenceContext {
                    polymorphic_context: Some(context.clone()),
                    analyzed_value: None,
                    resource_context: Some("Resource".to_string()),
                    historical_usage: std::collections::HashMap::new(),
                },
                statistical_model: None,
            })
        }
    }

    async fn get_choice_type_definition(
        &self,
        base_path: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::choice_types::ChoiceTypeDefinition>,
    > {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_choice_type_definition(base_path).await
        } else {
            Ok(None)
        }
    }

    async fn conforms_to_profile(
        &self,
        profile_url: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::conformance::ConformanceResult>
    {
        Ok(octofhir_fhir_model::conformance::ConformanceResult::new(
            profile_url,
            "DynamicModelProvider",
        ))
    }

    async fn analyze_expression_types(
        &self,
        expression: &str,
    ) -> octofhir_fhir_model::error::Result<
        octofhir_fhir_model::fhirpath_types::ExpressionTypeAnalysis,
    > {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.analyze_expression_types(expression).await
        } else {
            Ok(octofhir_fhir_model::fhirpath_types::ExpressionTypeAnalysis::new(expression))
        }
    }

    async fn validate_fhirpath_expression(
        &self,
        expression: &str,
        base_type: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::fhirpath_types::TypeCheckResult>
    {
        if let Some(ref fallback) = self.fallback_provider {
            fallback
                .validate_fhirpath_expression(expression, base_type)
                .await
        } else {
            Ok(octofhir_fhir_model::fhirpath_types::TypeCheckResult::success())
        }
    }

    async fn get_expression_dependencies(
        &self,
        expression: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::fhirpath_types::TypeDependency>>
    {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_expression_dependencies(expression).await
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_collection_semantics(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::CollectionSemantics>
    {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_collection_semantics(type_name).await
        } else {
            use octofhir_fhir_model::type_system::{
                CollectionSemantics, EmptyBehavior, IndexingType, SingletonEvaluation,
            };
            Ok(CollectionSemantics {
                is_ordered: true,
                allows_duplicates: true,
                indexing_type: IndexingType::ZeroBased,
                empty_behavior: EmptyBehavior::Propagate,
                singleton_evaluation: SingletonEvaluation::Automatic,
            })
        }
    }

    async fn get_optimization_hints(
        &self,
        expression: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::navigation::OptimizationHint>>
    {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_optimization_hints(expression).await
        } else {
            Ok(Vec::new())
        }
    }

    async fn clear_caches(&self) -> octofhir_fhir_model::error::Result<()> {
        if let Some(ref fallback) = self.fallback_provider {
            match fallback.clear_caches().await {
                Ok(()) => Ok(()),
                Err(_) => Err(octofhir_fhir_model::error::ModelError::generic(
                    "Cache clear failed".to_string(),
                )),
            }
        } else {
            Ok(())
        }
    }

    async fn get_type_reflection(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::reflection::TypeReflectionInfo>,
    > {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_type_reflection(type_name).await
        } else {
            Ok(Some(
                octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type("FHIR", type_name),
            ))
        }
    }

    async fn get_constraints(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::constraints::ConstraintInfo>>
    {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_constraints(type_name).await
        } else {
            Ok(Vec::new())
        }
    }

    fn get_fhir_version(&self) -> ModelProviderFhirVersion {
        match self.fhir_version {
            FhirVersion::R4 => ModelProviderFhirVersion::R4,
            FhirVersion::R4B => ModelProviderFhirVersion::R4B,
            FhirVersion::R5 => ModelProviderFhirVersion::R5,
            FhirVersion::R6 => ModelProviderFhirVersion::R5, // Map R6 to R5 for now
        }
    }

    async fn get_supported_resource_types(
        &self,
    ) -> octofhir_fhir_model::error::Result<Vec<String>> {
        if let Some(ref fallback) = self.fallback_provider {
            fallback.get_supported_resource_types().await
        } else {
            Ok(self
                .get_core_resource_types()
                .iter()
                .map(|s| s.to_string())
                .collect())
        }
    }

    fn resource_type_exists(
        &self,
        resource_type: &str,
    ) -> octofhir_fhir_model::error::Result<bool> {
        if let Some(ref fallback) = self.fallback_provider {
            Ok(fallback.resource_type_exists(resource_type))
        } else {
            Ok(self.get_core_resource_types().contains(&resource_type))
        }
    }

    async fn refresh_resource_types(&self) -> octofhir_fhir_model::error::Result<()> {
        if let Some(ref fallback) = self.fallback_provider {
            match fallback.refresh_resource_types().await {
                Ok(()) => Ok(()),
                Err(_) => Err(octofhir_fhir_model::error::ModelError::generic(
                    "Resource type refresh failed".to_string(),
                )),
            }
        } else {
            Ok(())
        }
    }
}
