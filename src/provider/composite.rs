use async_trait::async_trait;

#[cfg(feature = "dynamic-caching")]
use std::sync::Arc;
#[cfg(feature = "dynamic-caching")]
use tokio::sync::RwLock;

use crate::core::FhirVersion;
use crate::error::Result;

#[cfg(feature = "embedded-providers")]
use crate::provider::EmbeddedModelProvider;

#[cfg(feature = "dynamic-caching")]
use crate::provider::DynamicModelProvider;

use crate::provider::FhirSchemaModelProvider;
use octofhir_fhir_model::provider::{FhirVersion as ModelProviderFhirVersion, ModelProvider};

/// Composite ModelProvider that unifies all available providers with intelligent fallback
#[derive(Debug)]
pub struct CompositeModelProvider {
    /// FHIR version this provider serves
    fhir_version: FhirVersion,

    /// Embedded provider (fast, zero I/O)
    #[cfg(feature = "embedded-providers")]
    embedded_provider: Option<EmbeddedModelProvider>,

    /// Dynamic cached provider (fast after first load)
    #[cfg(feature = "dynamic-caching")]
    dynamic_provider: Option<Arc<RwLock<DynamicModelProvider>>>,

    /// Traditional FHIR schema provider (fallback)
    fallback_provider: FhirSchemaModelProvider,
}

impl CompositeModelProvider {
    /// Create composite provider for specific FHIR version
    pub async fn new(fhir_version: FhirVersion) -> Result<Self> {
        // Initialize fallback provider (always available)
        let fallback_provider = FhirSchemaModelProvider::new(fhir_version).await?;

        // Try to initialize embedded provider
        #[cfg(feature = "embedded-providers")]
        let embedded_provider = match fhir_version {
            FhirVersion::R4 => EmbeddedModelProvider::r4().await.ok(),
            FhirVersion::R4B => EmbeddedModelProvider::r4b().await.ok(),
            FhirVersion::R5 => EmbeddedModelProvider::r5().await.ok(),
            FhirVersion::R6 => EmbeddedModelProvider::r6().await.ok(),
        };

        // Try to initialize dynamic provider
        #[cfg(feature = "dynamic-caching")]
        let dynamic_provider = DynamicModelProvider::new(fhir_version)
            .await
            .ok()
            .map(|p| Arc::new(RwLock::new(p)));

        Ok(Self {
            fhir_version,
            #[cfg(feature = "embedded-providers")]
            embedded_provider,
            #[cfg(feature = "dynamic-caching")]
            dynamic_provider,
            fallback_provider,
        })
    }

    /// Create composite provider with automatic FHIR version selection for R4
    pub async fn r4() -> Result<Self> {
        Self::new(FhirVersion::R4).await
    }

    /// Create composite provider with automatic FHIR version selection for R4B
    pub async fn r4b() -> Result<Self> {
        Self::new(FhirVersion::R4B).await
    }

    /// Create composite provider with automatic FHIR version selection for R5
    pub async fn r5() -> Result<Self> {
        Self::new(FhirVersion::R5).await
    }

    /// Create composite provider with automatic FHIR version selection for R6
    pub async fn r6() -> Result<Self> {
        Self::new(FhirVersion::R6).await
    }
}

/// Placeholder implementation that delegates everything to fallback provider
/// TODO: Implement proper fallback chain
#[async_trait]
impl ModelProvider for CompositeModelProvider {
    async fn get_type_hierarchy(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<Option<octofhir_fhir_model::type_system::TypeHierarchy>>
    {
        // Convert from FhirSchemaModelProvider TypeHierarchy to octofhir_fhir_model TypeHierarchy
        match self.fallback_provider.get_type_hierarchy(type_name).await {
            Ok(Some(schema_hierarchy)) => {
                use octofhir_fhir_model::type_system::{DerivationType, TypeHierarchy};

                let model_hierarchy = TypeHierarchy {
                    type_name: schema_hierarchy.base_type,
                    ancestors: schema_hierarchy.parent_type.clone().into_iter().collect(),
                    descendants: schema_hierarchy.child_types,
                    direct_parent: schema_hierarchy.parent_type,
                    direct_children: Vec::new(), // Would need to be computed from child_types
                    is_abstract: false,          // Default value - would need proper mapping
                    derivation: DerivationType::Specialization, // Default value
                    hierarchy_depth: 1,          // Default value - would need proper calculation
                };
                Ok(Some(model_hierarchy))
            }
            Ok(None) => Ok(None),
            Err(_e) => Err(octofhir_fhir_model::error::ModelError::generic(
                "Type hierarchy lookup failed".to_string(),
            )),
        }
    }

    async fn is_type_compatible(
        &self,
        from_type: &str,
        to_type: &str,
    ) -> octofhir_fhir_model::error::Result<bool> {
        self.fallback_provider
            .is_type_compatible(from_type, to_type)
            .await
    }

    async fn get_common_supertype(
        &self,
        types: &[String],
    ) -> octofhir_fhir_model::error::Result<Option<String>> {
        self.fallback_provider.get_common_supertype(types).await
    }

    async fn get_type_compatibility_matrix(
        &self,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::TypeCompatibilityMatrix>
    {
        self.fallback_provider.get_type_compatibility_matrix().await
    }

    async fn navigate_typed_path(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::navigation::NavigationResult> {
        // TODO: Implement proper type conversion from FhirSchemaModelProvider NavigationResult
        // to octofhir_fhir_model::navigation::NavigationResult
        use octofhir_fhir_model::navigation::{NavigationMetadata, NavigationResult};
        use octofhir_fhir_model::reflection::TypeReflectionInfo;
        use octofhir_fhir_model::type_system::CollectionInfo;

        // For now, create a minimal placeholder result with correct structure
        Ok(NavigationResult {
            result_type: TypeReflectionInfo::SimpleType {
                namespace: "FHIR".to_string(),
                name: "string".to_string(),
                base_type: Some(base_type.to_string()),
            },
            collection_info: CollectionInfo::default(),
            navigation_metadata: NavigationMetadata {
                original_path: path.to_string(),
                resolved_path: path.to_string(),
                intermediate_types: Vec::new(),
                choice_resolutions: Vec::new(),
                function_calls: Vec::new(),
                type_operations: Vec::new(),
            },
            validation_results: Vec::new(),
            performance_hints: Vec::new(),
            is_success: true,
            errors: Vec::new(),
        })
    }

    async fn validate_navigation_safety(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::navigation::PathValidation> {
        self.fallback_provider
            .validate_navigation_safety(base_type, path)
            .await
    }

    async fn get_navigation_result_type(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::reflection::TypeReflectionInfo>,
    > {
        self.fallback_provider
            .get_navigation_result_type(base_type, path)
            .await
    }

    async fn get_navigation_metadata(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::NavigationMetadata>
    {
        self.fallback_provider
            .get_navigation_metadata(base_type, path)
            .await
    }

    async fn resolve_choice_type(
        &self,
        _base_path: &str,
        context: &octofhir_fhir_model::type_system::PolymorphicContext,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::PolymorphicResolution>
    {
        // TODO: Implement proper type conversion from FhirSchemaModelProvider types
        // to octofhir_fhir_model types
        use octofhir_fhir_model::type_system::{PolymorphicResolution, ResolutionMethod};

        // For now, create a minimal placeholder result
        Ok(PolymorphicResolution {
            resolved_type: "string".to_string(),
            confidence_score: 0.5,
            resolution_method: ResolutionMethod::DefaultFallback,
            alternative_types: Vec::new(),
            resolution_context: context.clone(),
        })
    }

    async fn get_choice_expansions(
        &self,
        choice_property: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::choice_types::ChoiceExpansion>>
    {
        self.fallback_provider
            .get_choice_expansions(choice_property)
            .await
    }

    async fn infer_choice_type(
        &self,
        context: &octofhir_fhir_model::type_system::PolymorphicContext,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::choice_types::TypeInference> {
        self.fallback_provider.infer_choice_type(context).await
    }

    async fn get_choice_type_definition(
        &self,
        base_path: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::choice_types::ChoiceTypeDefinition>,
    > {
        self.fallback_provider
            .get_choice_type_definition(base_path)
            .await
    }

    async fn conforms_to_profile(
        &self,
        profile_url: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::conformance::ConformanceResult>
    {
        self.fallback_provider
            .conforms_to_profile(profile_url)
            .await
    }

    async fn analyze_expression_types(
        &self,
        expression: &str,
    ) -> octofhir_fhir_model::error::Result<
        octofhir_fhir_model::fhirpath_types::ExpressionTypeAnalysis,
    > {
        self.fallback_provider
            .analyze_expression_types(expression)
            .await
    }

    async fn validate_fhirpath_expression(
        &self,
        expression: &str,
        base_type: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::fhirpath_types::TypeCheckResult>
    {
        self.fallback_provider
            .validate_fhirpath_expression(expression, base_type)
            .await
    }

    async fn get_expression_dependencies(
        &self,
        expression: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::fhirpath_types::TypeDependency>>
    {
        self.fallback_provider
            .get_expression_dependencies(expression)
            .await
    }

    async fn get_collection_semantics(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::CollectionSemantics>
    {
        self.fallback_provider
            .get_collection_semantics(type_name)
            .await
    }

    async fn get_optimization_hints(
        &self,
        expression: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::navigation::OptimizationHint>>
    {
        self.fallback_provider
            .get_optimization_hints(expression)
            .await
    }

    async fn clear_caches(&self) -> octofhir_fhir_model::error::Result<()> {
        // TODO: Implement proper error type conversion from FhirSchemaError to ModelError
        match self.fallback_provider.clear_caches().await {
            Ok(()) => Ok(()),
            Err(_e) => Err(octofhir_fhir_model::error::ModelError::generic(
                "Cache clear failed".to_string(),
            )),
        }
    }

    async fn get_type_reflection(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::reflection::TypeReflectionInfo>,
    > {
        self.fallback_provider.get_type_reflection(type_name).await
    }

    async fn get_constraints(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::constraints::ConstraintInfo>>
    {
        self.fallback_provider.get_constraints(type_name).await
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
        // Try embedded provider first (fastest, zero I/O)
        #[cfg(feature = "embedded-providers")]
        if let Some(ref embedded) = self.embedded_provider {
            if let Ok(types) = embedded.get_supported_resource_types().await {
                if !types.is_empty() {
                    return Ok(types);
                }
            }
        }

        // Try dynamic provider next
        #[cfg(feature = "dynamic-caching")]
        if let Some(ref dynamic_arc) = self.dynamic_provider {
            if let Ok(dynamic_guard) = dynamic_arc.try_read() {
                if let Ok(types) = dynamic_guard.get_supported_resource_types().await {
                    if !types.is_empty() {
                        return Ok(types);
                    }
                }
            }
        }

        // Fallback to traditional provider
        self.fallback_provider.get_supported_resource_types().await
    }

    fn resource_type_exists(
        &self,
        resource_type: &str,
    ) -> octofhir_fhir_model::error::Result<bool> {
        // Try embedded provider first (fastest, zero I/O)
        #[cfg(feature = "embedded-providers")]
        if let Some(ref embedded) = self.embedded_provider {
            let exists = embedded.resource_type_exists(resource_type);
            return Ok(exists);
        }

        // Try dynamic provider next
        #[cfg(feature = "dynamic-caching")]
        if let Some(ref dynamic_arc) = self.dynamic_provider {
            if let Ok(dynamic_guard) = dynamic_arc.try_read() {
                if let Ok(exists) = dynamic_guard.resource_type_exists(resource_type) {
                    return Ok(exists);
                }
            }
        }

        // Fallback to traditional provider
        Ok(self.fallback_provider.resource_type_exists(resource_type))
    }
}
