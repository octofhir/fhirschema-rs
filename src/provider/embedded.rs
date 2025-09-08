use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::FhirVersion;
use crate::error::{FhirSchemaError, Result};
use crate::types::{FhirSchema, TypeHierarchy};
use octofhir_fhir_model::NavigationResult;
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

            Ok(Self {
                fhir_version,
                schemas,
                available_resource_types,
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

    /// Build type hierarchy for a type
    async fn build_type_hierarchy(&self, type_name: &str) -> Result<TypeHierarchy> {
        // Try to get schema for this type
        if let Some(schema) = self.get_schema_by_type(type_name).await {
            let mut properties = HashMap::new();

            // Extract properties from schema
            for (prop_name, prop) in &schema.properties {
                let is_required = schema.required.contains(prop_name);
                let is_array = prop.items.is_some();

                let cardinality = crate::provider::fhir_model_provider::Cardinality {
                    min: if is_required { 1 } else { 0 },
                    max: if is_array { None } else { Some(1) },
                };

                // Try to extract choice types from property type or reference
                let mut choice_types = Vec::new();
                if let Some(properties) = &prop.properties {
                    for prop_key in properties.keys() {
                        if prop_key != "type" {
                            choice_types.push(prop_key.clone());
                        }
                    }
                }

                properties.insert(
                    prop_name.clone(),
                    crate::provider::fhir_model_provider::PropertyInfo {
                        name: prop_name.clone(),
                        property_type: prop
                            .property_type
                            .clone()
                            .unwrap_or_else(|| "string".to_string()),
                        cardinality,
                        is_choice_type: prop_name.contains("[x]") || prop_name.ends_with("[x]"),
                        choice_types,
                    },
                );
            }

            // Extract constraints
            let _constraints: Vec<crate::provider::fhir_model_provider::ConstraintInfo> = schema
                .constraints
                .iter()
                .map(|c| crate::provider::fhir_model_provider::ConstraintInfo {
                    key: c.key.clone(),
                    severity: format!("{:?}", c.severity),
                    human: c.human.clone(),
                    expression: c.expression.clone(),
                })
                .collect();

            return Ok(TypeHierarchy {
                type_name: type_name.to_string(),
                parent_type: None,
                child_types: Vec::new(),
                interfaces: Vec::new(),
                is_abstract: false,
                depth: 0,
            });
        }

        // Fallback - create minimal hierarchy
        Ok(TypeHierarchy {
            type_name: type_name.to_string(),
            parent_type: None,
            child_types: Vec::new(),
            interfaces: Vec::new(),
            is_abstract: false,
            depth: 0,
        })
    }

    /// Navigate a typed path
    pub async fn navigate_typed_path(
        &self,
        base_type: &str,
        path: &str,
    ) -> Result<NavigationResult> {
        // Handle nested paths (contains '.')
        if path.contains('.') {
            // For nested paths, we would need full navigation engine
            // For now, return error for nested paths as they're not yet supported
            return Err(FhirSchemaError::navigation_failed(&format!(
                "Nested path navigation not yet implemented for path: {path}"
            )));
        }

        // Single property navigation
        if let Some(schema) = self.get_schema_by_type(base_type).await {
            if let Some(property) = schema.properties.get(path) {
                // Property exists - return success using octofhir_fhir_model::NavigationResult
                use octofhir_fhir_model::reflection::TypeReflectionInfo;

                let target_type = property
                    .property_type
                    .clone()
                    .unwrap_or_else(|| "string".to_string());

                let result_type = TypeReflectionInfo::simple_type("FHIR", &target_type);
                Ok(NavigationResult::success(result_type))
            } else {
                // Property doesn't exist on this type - return error
                Err(FhirSchemaError::navigation_failed(&format!(
                    "Property '{path}' does not exist on type '{base_type}'"
                )))
            }
        } else {
            // Base type not found - return error
            Err(FhirSchemaError::type_not_found(base_type))
        }
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
        match self.build_type_hierarchy(type_name).await {
            Ok(hierarchy) => {
                use octofhir_fhir_model::type_system::{
                    DerivationType, TypeHierarchy as ModelTypeHierarchy,
                };

                let model_hierarchy = ModelTypeHierarchy {
                    type_name: hierarchy.type_name.clone(),
                    ancestors: hierarchy.parent_type.clone().into_iter().collect(),
                    descendants: Vec::new(),
                    direct_parent: hierarchy.parent_type.clone(),
                    direct_children: hierarchy.child_types.clone(),
                    is_abstract: false,
                    derivation: DerivationType::Specialization,
                    hierarchy_depth: 1,
                };
                Ok(Some(model_hierarchy))
            }
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
        // Basic compatibility check
        if from_type == to_type {
            return Ok(true);
        }

        // Check if from_type is a subtype of to_type
        if let Ok(hierarchy) = self.build_type_hierarchy(from_type).await {
            if let Some(parent) = hierarchy.parent_type {
                if parent == to_type {
                    return Ok(true);
                }
            }
        }

        // Basic FHIR hierarchy rules
        if to_type == "Resource" {
            return Ok(true); // Everything derives from Resource
        }

        if to_type == "DomainResource" && from_type != "Resource" {
            return Ok(true); // Most resources derive from DomainResource
        }

        Ok(false)
    }

    async fn get_common_supertype(
        &self,
        types: &[String],
    ) -> octofhir_fhir_model::error::Result<Option<String>> {
        if types.is_empty() {
            return Ok(None);
        }

        if types.len() == 1 {
            return Ok(Some(types[0].clone()));
        }

        // For embedded provider, use simple logic
        // In practice, this would analyze type hierarchies
        Ok(Some("Resource".to_string()))
    }

    async fn get_type_compatibility_matrix(
        &self,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::TypeCompatibilityMatrix>
    {
        use octofhir_fhir_model::type_system::TypeCompatibilityMatrix;
        // Return empty matrix - would be populated with FHIR type rules
        Ok(TypeCompatibilityMatrix::new())
    }

    // Navigation operations
    async fn navigate_typed_path(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::navigation::NavigationResult> {
        match self.navigate_typed_path(base_type, path).await {
            Ok(_result) => {
                use octofhir_fhir_model::{
                    navigation::NavigationResult as ModelNavigationResult,
                    reflection::TypeReflectionInfo,
                };
                Ok(ModelNavigationResult::success(
                    TypeReflectionInfo::simple_type("FHIR", base_type),
                ))
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
        // Basic validation
        if self.resource_type_exists(base_type) {
            Ok(octofhir_fhir_model::navigation::PathValidation::success(
                format!("{base_type}.{path}"),
            ))
        } else {
            let mut validation =
                octofhir_fhir_model::navigation::PathValidation::new(format!("{base_type}.{path}"));
            validation
                .validation_errors
                .push(octofhir_fhir_model::navigation::ValidationError {
                    error_code: "TYPE_NOT_FOUND".to_string(),
                    message: format!("Type '{base_type}' not found in embedded schemas"),
                    location: octofhir_fhir_model::navigation::PathLocation {
                        segment_index: 0,
                        character_position: 0,
                        segment_name: base_type.to_string(),
                    },
                    severity: octofhir_fhir_model::navigation::ConstraintSeverity::Error,
                });
            Ok(validation)
        }
    }

    async fn get_navigation_result_type(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::reflection::TypeReflectionInfo>,
    > {
        if let Ok(_result) = self.navigate_typed_path(base_type, path).await {
            Ok(Some(
                octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type("FHIR", "string"),
            ))
        } else {
            Ok(None)
        }
    }

    async fn get_navigation_metadata(
        &self,
        base_type: &str,
        path: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::NavigationMetadata>
    {
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
                operation_cost: 0.1, // Very fast for embedded
                is_cacheable: true,
                cache_key: Some(format!("embedded-{base_type}-{path}")),
                memory_estimate: Some(64),
            },
        })
    }

    // Stub implementations for remaining methods
    async fn resolve_choice_type(
        &self,
        _base_path: &str,
        _context: &octofhir_fhir_model::type_system::PolymorphicContext,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::PolymorphicResolution>
    {
        use octofhir_fhir_model::type_system::{PolymorphicResolution, ResolutionMethod};
        Ok(PolymorphicResolution {
            resolved_type: "string".to_string(),
            confidence_score: 0.5,
            resolution_method: ResolutionMethod::DefaultFallback,
            alternative_types: Vec::new(),
            resolution_context: _context.clone(),
        })
    }

    async fn get_choice_expansions(
        &self,
        _choice_property: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::choice_types::ChoiceExpansion>>
    {
        Ok(Vec::new())
    }

    async fn infer_choice_type(
        &self,
        _context: &octofhir_fhir_model::type_system::PolymorphicContext,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::choice_types::TypeInference> {
        use octofhir_fhir_model::choice_types::{InferenceContext, TypeInference};
        Ok(TypeInference {
            inference_rules: Vec::new(),
            confidence_threshold: 0.5,
            inference_context: InferenceContext {
                polymorphic_context: Some(_context.clone()),
                analyzed_value: None,
                resource_context: Some("Resource".to_string()),
                historical_usage: std::collections::HashMap::new(),
            },
            statistical_model: None,
        })
    }

    async fn get_choice_type_definition(
        &self,
        _base_path: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::choice_types::ChoiceTypeDefinition>,
    > {
        Ok(None)
    }

    async fn conforms_to_profile(
        &self,
        profile_url: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::conformance::ConformanceResult>
    {
        Ok(octofhir_fhir_model::conformance::ConformanceResult::new(
            profile_url,
            "EmbeddedModelProvider",
        ))
    }

    async fn analyze_expression_types(
        &self,
        expression: &str,
    ) -> octofhir_fhir_model::error::Result<
        octofhir_fhir_model::fhirpath_types::ExpressionTypeAnalysis,
    > {
        Ok(octofhir_fhir_model::fhirpath_types::ExpressionTypeAnalysis::new(expression))
    }

    async fn validate_fhirpath_expression(
        &self,
        _expression: &str,
        _base_type: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::fhirpath_types::TypeCheckResult>
    {
        Ok(octofhir_fhir_model::fhirpath_types::TypeCheckResult::success())
    }

    async fn get_expression_dependencies(
        &self,
        _expression: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::fhirpath_types::TypeDependency>>
    {
        Ok(Vec::new())
    }

    async fn get_collection_semantics(
        &self,
        _type_name: &str,
    ) -> octofhir_fhir_model::error::Result<octofhir_fhir_model::type_system::CollectionSemantics>
    {
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

    async fn get_optimization_hints(
        &self,
        _expression: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::navigation::OptimizationHint>>
    {
        Ok(Vec::new())
    }

    async fn clear_caches(&self) -> octofhir_fhir_model::error::Result<()> {
        // No-op for embedded provider - nothing to clear
        Ok(())
    }

    async fn get_type_reflection(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<
        Option<octofhir_fhir_model::reflection::TypeReflectionInfo>,
    > {
        use octofhir_fhir_model::reflection::TypeReflectionInfo;

        if !self.resource_type_exists(type_name) {
            return Ok(None);
        }

        // Create basic FHIR resource properties for the given type
        let elements = self.create_common_resource_properties(type_name);

        Ok(Some(TypeReflectionInfo::ClassInfo {
            namespace: "FHIR".to_string(),
            name: type_name.to_string(),
            base_type: Some("Resource".to_string()),
            elements,
        }))
    }

    async fn get_constraints(
        &self,
        type_name: &str,
    ) -> octofhir_fhir_model::error::Result<Vec<octofhir_fhir_model::constraints::ConstraintInfo>>
    {
        if let Ok(_hierarchy) = self.build_type_hierarchy(type_name).await {
            let constraints: Vec<octofhir_fhir_model::constraints::ConstraintInfo> = Vec::new();
            Ok(constraints)
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

impl EmbeddedModelProvider {
    /// Create common FHIR resource properties for a given resource type
    fn create_common_resource_properties(
        &self,
        resource_type: &str,
    ) -> Vec<octofhir_fhir_model::reflection::ElementInfo> {
        use octofhir_fhir_model::reflection::{ElementInfo, TypeReflectionInfo};

        let mut elements = Vec::new();

        // Add common properties that most FHIR resources have
        let common_properties = match resource_type {
            "Patient" => vec![
                ("name", "HumanName", 0, None),
                ("gender", "code", 0, Some(1)),
                ("birthDate", "date", 0, Some(1)),
                ("address", "Address", 0, None),
                ("telecom", "ContactPoint", 0, None),
                ("identifier", "Identifier", 0, None),
                ("active", "boolean", 0, Some(1)),
                ("deceased", "boolean", 0, Some(1)),
                ("maritalStatus", "CodeableConcept", 0, Some(1)),
                ("contact", "PatientContact", 0, None),
                ("communication", "PatientCommunication", 0, None),
                ("generalPractitioner", "Reference", 0, None),
                ("managingOrganization", "Reference", 0, Some(1)),
                ("link", "PatientLink", 0, None),
            ],
            "Observation" => vec![
                ("status", "code", 1, Some(1)),
                ("code", "CodeableConcept", 1, Some(1)),
                ("subject", "Reference", 0, Some(1)),
                ("value", "Quantity", 0, Some(1)),
                ("valueQuantity", "Quantity", 0, Some(1)),
                ("valueString", "string", 0, Some(1)),
                ("valueBoolean", "boolean", 0, Some(1)),
                ("effectiveDateTime", "dateTime", 0, Some(1)),
                ("issued", "instant", 0, Some(1)),
                ("performer", "Reference", 0, None),
                ("interpretation", "CodeableConcept", 0, None),
                ("bodySite", "CodeableConcept", 0, Some(1)),
                ("method", "CodeableConcept", 0, Some(1)),
                ("component", "ObservationComponent", 0, None),
            ],
            "Procedure" => vec![
                ("status", "code", 1, Some(1)),
                ("code", "CodeableConcept", 1, Some(1)),
                ("subject", "Reference", 1, Some(1)),
                ("performedDateTime", "dateTime", 0, Some(1)),
                ("performer", "Reference", 0, None),
                ("location", "Reference", 0, Some(1)),
                ("bodySite", "CodeableConcept", 0, None),
                ("outcome", "CodeableConcept", 0, Some(1)),
                ("reasonCode", "CodeableConcept", 0, None),
                ("note", "Annotation", 0, None),
            ],
            _ => vec![
                // Generic resource properties
                ("id", "id", 0, Some(1)),
                ("meta", "Meta", 0, Some(1)),
                ("implicitRules", "uri", 0, Some(1)),
                ("language", "code", 0, Some(1)),
                ("text", "Narrative", 0, Some(1)),
                ("extension", "Extension", 0, None),
                ("modifierExtension", "Extension", 0, None),
            ],
        };

        for (name, type_name, min, max) in common_properties {
            elements.push(ElementInfo {
                name: name.to_string(),
                type_info: TypeReflectionInfo::SimpleType {
                    namespace: "FHIR".to_string(),
                    name: type_name.to_string(),
                    base_type: None,
                },
                min_cardinality: min,
                max_cardinality: max,
                is_summary: name == "id" || name == "meta",
                is_modifier: false,
                documentation: None,
            });
        }

        elements
    }
}
