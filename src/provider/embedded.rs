use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::FhirVersion;
use crate::error::{FhirSchemaError, Result};
use crate::types::FhirSchema;
use octofhir_fhir_model::provider::{FhirVersion as ModelProviderFhirVersion, ModelProvider};

// Include generated precompiled schemas
#[cfg(feature = "embedded-providers")]
include!("embedded_schemas.rs");

/// Embedded ModelProvider with precompiled schemas for fast startup
#[derive(Debug)]
pub struct EmbeddedModelProvider {
    fhir_version: FhirVersion,
    schemas: HashMap<String, FhirSchema>,
    /// O(1) resource type existence check
    available_resource_types: Arc<papaya::HashMap<String, ()>>,
    /// Delegate for advanced operations like navigation and inheritance
    fhir_schema_provider: crate::provider::FhirSchemaModelProvider,
}

impl EmbeddedModelProvider {
    /// Create embedded provider for FHIR R4
    pub async fn r4() -> Result<Self> {
        Self::new(FhirVersion::R4).await
    }

    /// Create embedded provider for FHIR R4B
    pub async fn r4b() -> Result<Self> {
        Self::new(FhirVersion::R4B).await
    }

    /// Create embedded provider for FHIR R5
    pub async fn r5() -> Result<Self> {
        Self::new(FhirVersion::R5).await
    }

    /// Create embedded provider for FHIR R6
    pub async fn r6() -> Result<Self> {
        Self::new(FhirVersion::R6).await
    }

    /// Create embedded provider for specific FHIR version
    async fn new(fhir_version: FhirVersion) -> Result<Self> {
        #[cfg(feature = "embedded-providers")]
        {
            let schemas = Self::load_embedded_schemas(fhir_version)?;
            let available_resource_types = Arc::new(papaya::HashMap::new());

            // Initialize resource types from embedded schemas
            {
                let guard = available_resource_types.pin();
                let mut extracted_count = 0;
                for schema in schemas.values() {
                    if let Some(resource_type) = Self::extract_resource_type_from_schema(schema) {
                        guard.insert(resource_type.clone(), ());
                        extracted_count += 1;
                        if extracted_count <= 5 {
                            tracing::debug!(
                                "Extracted resource type: {} from schema title: {:?}, id: {:?}",
                                resource_type,
                                schema.title,
                                schema.id
                            );
                        }
                    } else if extracted_count <= 5 {
                        tracing::debug!(
                            "Failed to extract resource type from schema title: {:?}, id: {:?}",
                            schema.title,
                            schema.id
                        );
                    }
                }
                tracing::info!(
                    "Loaded {} schemas, extracted {} resource types",
                    schemas.len(),
                    extracted_count
                );
            }

            // Create FhirSchemaModelProvider for delegation
            let mut fhir_schema_provider =
                crate::provider::FhirSchemaModelProvider::new(fhir_version).await?;

            // CRITICAL: Share the embedded schemas and resource types with the FhirSchemaModelProvider
            // This ensures both providers have access to the same resource types and schemas
            fhir_schema_provider
                .register_embedded_schemas(&schemas, &available_resource_types)
                .await?;

            Ok(Self {
                fhir_version,
                schemas,
                available_resource_types,
                fhir_schema_provider,
            })
        }

        #[cfg(not(feature = "embedded-providers"))]
        {
            Err(FhirSchemaError::configuration_error(
                "EmbeddedModelProvider requires 'embedded-providers' feature to be enabled",
            ))
        }
    }

    /// Load embedded schemas for a FHIR version
    #[cfg(feature = "embedded-providers")]
    fn load_embedded_schemas(fhir_version: FhirVersion) -> Result<HashMap<String, FhirSchema>> {
        let version_str = fhir_version.short_name();

        // Get precompiled schema data
        let schema_data = schemas::get_schemas(version_str).ok_or_else(|| {
            eprintln!("❌ No embedded schemas available for FHIR version: {version_str}");
            FhirSchemaError::configuration_error(&format!(
                "No embedded schemas available for FHIR version: {version_str}"
            ))
        })?;

        // Deserialize schemas
        if schema_data.is_empty() {
            // Empty placeholder - return minimal set of schemas
            return Ok(Self::create_minimal_schemas(fhir_version));
        }

        let schemas: Vec<FhirSchema> = serde_json::from_slice(schema_data)
            .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))?;

        // Create HashMap indexed by schema URL/ID
        let mut schema_map = HashMap::new();
        for schema in schemas {
            if let Some(id) = &schema.id {
                schema_map.insert(id.clone(), schema);
            } else if let Some(title) = &schema.title {
                // Fallback to using title as key
                let url = format!("http://hl7.org/fhir/StructureDefinition/{title}");
                schema_map.insert(url, schema);
            }
        }

        if schema_map.is_empty() {
            return Ok(Self::create_minimal_schemas(fhir_version));
        }
        Ok(schema_map)
    }

    /// Create minimal schemas as fallback
    fn create_minimal_schemas(fhir_version: FhirVersion) -> HashMap<String, FhirSchema> {
        let mut schemas = HashMap::new();

        // Create minimal Resource schema
        let resource_schema = FhirSchema {
            schema_type: "object".to_string(),
            properties: HashMap::new(),
            required: Vec::new(),
            additional_properties: Some(false),
            json_schema_version: Some("https://json-schema.org/draft/2020-12/schema".to_string()),
            title: Some("Resource".to_string()),
            description: Some("Base Resource".to_string()),
            id: Some("http://hl7.org/fhir/StructureDefinition/Resource".to_string()),
            constraints: Vec::new(),
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert(
                    "resourceType".to_string(),
                    serde_json::Value::String("Resource".to_string()),
                );
                metadata.insert(
                    "fhirVersion".to_string(),
                    serde_json::Value::String(fhir_version.to_string()),
                );
                metadata
            },
        };

        schemas.insert(
            "http://hl7.org/fhir/StructureDefinition/Resource".to_string(),
            resource_schema,
        );

        // Create minimal DomainResource schema
        let domain_resource_schema = FhirSchema {
            schema_type: "object".to_string(),
            properties: HashMap::new(),
            required: Vec::new(),
            additional_properties: Some(false),
            json_schema_version: Some("https://json-schema.org/draft/2020-12/schema".to_string()),
            title: Some("DomainResource".to_string()),
            description: Some("Base DomainResource".to_string()),
            id: Some("http://hl7.org/fhir/StructureDefinition/DomainResource".to_string()),
            constraints: Vec::new(),
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert(
                    "resourceType".to_string(),
                    serde_json::Value::String("DomainResource".to_string()),
                );
                metadata.insert(
                    "fhirVersion".to_string(),
                    serde_json::Value::String(fhir_version.to_string()),
                );
                metadata
            },
        };

        schemas.insert(
            "http://hl7.org/fhir/StructureDefinition/DomainResource".to_string(),
            domain_resource_schema,
        );

        schemas
    }

    /// Extract resource type from schema metadata
    fn extract_resource_type_from_schema(schema: &FhirSchema) -> Option<String> {
        // Check schema title
        if let Some(title) = &schema.title {
            if title.chars().next().unwrap_or('a').is_uppercase() {
                return Some(title.clone());
            }
        }

        // Check schema ID for resource type extraction
        if let Some(id) = &schema.id {
            if let Some(captures) = id.strip_prefix("http://hl7.org/fhir/StructureDefinition/") {
                if !captures.is_empty() && captures.chars().next().unwrap_or('a').is_uppercase() {
                    return Some(captures.to_string());
                }
            }
        }

        // Check metadata
        if let Some(resource_type_value) = schema.metadata.get("resourceType") {
            if let Some(resource_type) = resource_type_value.as_str() {
                if resource_type.chars().next().unwrap_or('a').is_uppercase() {
                    return Some(resource_type.to_string());
                }
            }
        }

        None
    }

    /// Get a schema by URL
    pub async fn get_schema(&self, url: &str) -> Option<&FhirSchema> {
        self.schemas.get(url)
    }

    /// Get schema by resource type
    pub async fn get_schema_by_type(&self, resource_type: &str) -> Option<&FhirSchema> {
        let url = format!("http://hl7.org/fhir/StructureDefinition/{resource_type}");
        self.get_schema(&url).await
    }

    /// List all available schemas
    pub async fn list_schemas(&self) -> Vec<String> {
        self.schemas.keys().cloned().collect()
    }

    /// Check if resource type exists
    pub fn resource_type_exists(&self, resource_type: &str) -> bool {
        let guard = self.available_resource_types.pin();
        guard.contains_key(resource_type)
    }

    /// Get all available resource types
    pub fn get_available_resource_types(&self) -> Vec<String> {
        let guard = self.available_resource_types.pin();
        guard.keys().cloned().collect()
    }

    /// Get FHIR version
    pub fn fhir_version(&self) -> FhirVersion {
        self.fhir_version
    }

    /// Get number of loaded schemas
    pub fn schema_count(&self) -> usize {
        self.schemas.len()
    }

    /// Check if this provider supports a specific resource type
    pub fn supports_resource_type(&self, resource_type: &str) -> bool {
        self.resource_type_exists(resource_type)
    }
}

// Forward ModelProvider trait implementation to the full provider
// In practice, we might implement this directly or use composition
#[async_trait]
impl ModelProvider for EmbeddedModelProvider {
    // Core type operations
    async fn get_type_hierarchy(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<Option<octofhir_fhir_model::type_system::TypeHierarchy>>
    {
        // Delegate to FhirSchemaModelProvider and convert result
        match self
            .fhir_schema_provider
            .get_type_hierarchy(type_name)
            .await
        {
            Ok(Some(hierarchy)) => {
                use octofhir_fhir_model::type_system::{DerivationType, TypeHierarchy};
                let model_hierarchy = TypeHierarchy {
                    type_name: hierarchy.base_type,
                    ancestors: hierarchy.parent_type.clone().into_iter().collect(),
                    descendants: hierarchy.child_types,
                    direct_parent: hierarchy.parent_type,
                    direct_children: Vec::new(),
                    is_abstract: false, // Default value
                    derivation: DerivationType::Specialization,
                    hierarchy_depth: 1, // Default value
                };
                Ok(Some(model_hierarchy))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(octofhir_fhir_model::error::ModelError::generic(
                e.to_string(),
            )),
        }
    }

    async fn is_type_compatible(
        &self,
        from_type: &str,
        to_type: &str,
    ) -> octofhir_fhir_model::error::Result<bool> {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .is_type_compatible(from_type, to_type)
            .await
    }

    async fn get_common_supertype(
        &self,
        types: &[String],
    ) -> octofhir_fhir_model::error::Result<Option<String>> {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider.get_common_supertype(types).await
    }

    async fn get_type_compatibility_matrix(
        &self,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::TypeCompatibilityMatrix>
    {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .get_type_compatibility_matrix()
            .await
    }

    // Navigation operations
    async fn navigate_typed_path(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::navigation::NavigationResult> {
        // Delegate to FhirSchemaModelProvider and convert result
        match self
            .fhir_schema_provider
            .navigate_typed_path(base_type, path)
            .await
        {
            Ok(fhir_result) => {
                use octofhir_fhir_model::{
                    navigation::{NavigationMetadata, NavigationResult},
                    reflection::TypeReflectionInfo,
                    type_system::CollectionInfo,
                };
                let result_type = if fhir_result.is_array {
                    // For arrays, create ListType with the element type
                    let element_type =
                        TypeReflectionInfo::simple_type("FHIR", &fhir_result.element_type);
                    TypeReflectionInfo::list_type(element_type)
                } else {
                    // For scalars, create SimpleType
                    TypeReflectionInfo::simple_type("FHIR", &fhir_result.target_type)
                };
                Ok(NavigationResult {
                    result_type,
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
            Err(e) => Err(octofhir_fhir_model::error::ModelError::generic(
                e.to_string(),
            )),
        }
    }

    async fn validate_navigation_safety(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::navigation::PathValidation> {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
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
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .get_navigation_result_type(base_type, path)
            .await
    }

    async fn get_navigation_metadata(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::NavigationMetadata>
    {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .get_navigation_metadata(base_type, path)
            .await
    }

    // Stub implementations for remaining methods
    async fn resolve_choice_type(
        &self,
        base_path: &str,
        context: &octofhir_fhir_model::type_system::PolymorphicContext,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::PolymorphicResolution>
    {
        // Convert context and delegate to FhirSchemaModelProvider
        let fhir_context = crate::core::types::ResolutionContext {
            base_path: base_path.to_string(),
            resource_type: Some(context.base_type.clone()),
            profile_urls: Vec::new(),
            discriminator_paths: Vec::new(),
            metadata: std::collections::HashMap::new(),
        };

        match self
            .fhir_schema_provider
            .resolve_choice_type(base_path, &fhir_context)
            .await
        {
            Ok(fhir_result) => {
                use octofhir_fhir_model::type_system::{PolymorphicResolution, ResolutionMethod};
                Ok(PolymorphicResolution {
                    resolved_type: fhir_result.resolved_type,
                    confidence_score: fhir_result.confidence,
                    resolution_method: ResolutionMethod::DefaultFallback,
                    alternative_types: fhir_result
                        .alternatives
                        .into_iter()
                        .map(|alt| octofhir_fhir_model::type_system::AlternativeType {
                            type_name: alt.type_name,
                            confidence: alt.confidence,
                            reasoning: alt.reason,
                        })
                        .collect(),
                    resolution_context: context.clone(),
                })
            }
            Err(e) => Err(octofhir_fhir_model::error::ModelError::generic(
                e.to_string(),
            )),
        }
    }

    async fn get_choice_expansions(
        &self,
        choice_property: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::choice_types::ChoiceExpansion>>
    {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .get_choice_expansions(choice_property)
            .await
    }

    async fn infer_choice_type(
        &self,
        context: &octofhir_fhir_model::type_system::PolymorphicContext,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::choice_types::TypeInference> {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider.infer_choice_type(context).await
    }

    async fn get_choice_type_definition(
        &self,
        base_path: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::choice_types::ChoiceTypeDefinition>,
    > {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .get_choice_type_definition(base_path)
            .await
    }

    async fn conforms_to_profile(
        &self,
        profile_url: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::conformance::ConformanceResult>
    {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .conforms_to_profile(profile_url)
            .await
    }

    async fn analyze_expression_types(
        &self,
        expression: &str,
    ) -> octofhir_fhir_model::error::Result<
        octofhir_fhir_model::fhirpath_types::ExpressionTypeAnalysis,
    > {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .analyze_expression_types(expression)
            .await
    }

    async fn validate_fhirpath_expression(
        &self,
        expression: &str,
        base_type: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::fhirpath_types::TypeCheckResult>
    {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .validate_fhirpath_expression(expression, base_type)
            .await
    }

    async fn get_expression_dependencies(
        &self,
        expression: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::fhirpath_types::TypeDependency>>
    {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .get_expression_dependencies(expression)
            .await
    }

    async fn get_collection_semantics(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::CollectionSemantics>
    {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .get_collection_semantics(type_name)
            .await
    }

    async fn get_optimization_hints(
        &self,
        expression: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::navigation::OptimizationHint>>
    {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .get_optimization_hints(expression)
            .await
    }

    async fn clear_caches(&self) -> octofhir_fhir_model::error::Result<()> {
        // Delegate to FhirSchemaModelProvider and convert error type
        match self.fhir_schema_provider.clear_caches().await {
            Ok(()) => Ok(()),
            Err(e) => Err(octofhir_fhir_model::error::ModelError::generic(
                e.to_string(),
            )),
        }
    }

    async fn get_type_reflection(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::reflection::TypeReflectionInfo>,
    > {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider
            .get_type_reflection(type_name)
            .await
    }

    async fn get_constraints(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::constraints::ConstraintInfo>>
    {
        // Delegate to FhirSchemaModelProvider
        self.fhir_schema_provider.get_constraints(type_name).await
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
        Ok(self.get_available_resource_types())
    }

    fn resource_type_exists(
        &self,
        resource_type: &str,
    ) -> octofhir_fhir_model::error::Result<bool> {
        let guard = self.available_resource_types.pin();
        Ok(guard.contains_key(resource_type))
    }

    async fn refresh_resource_types(&self) -> octofhir_fhir_model::error::Result<()> {
        // No-op for embedded provider - types are static
        Ok(())
    }
}
