use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::core::{FhirSchemaConfig, FhirSchemaManager, FhirVersion, ResolutionContext};
use crate::error::{FhirSchemaError, Result};
use crate::provider::cache::ModelProviderCache;
use crate::provider::navigation::NavigationEngine;
use crate::types::TypeResolver;

// Import the ModelProvider trait from fhir-model-rs
use octofhir_fhir_model::provider::{FhirVersion as ModelProviderFhirVersion, ModelProvider};

// Import required types from fhir-model-rs
use octofhir_fhir_model::{
    choice_types::{ChoiceExpansion, ChoiceTypeDefinition, TypeInference},
    conformance::ConformanceResult,
    constraints::ConstraintInfo as ModelConstraintInfo,
    error::{ModelError, Result as ModelResult},
    fhirpath_types::{ExpressionTypeAnalysis, TypeCheckResult, TypeDependency},
    navigation::{
        NavigationResult as ModelNavigationResult, OptimizationHint as ModelOptimizationHint,
        PathValidation,
    },
    reflection::TypeReflectionInfo,
    type_system::{
        CollectionSemantics, NavigationMetadata, PolymorphicContext, PolymorphicResolution,
        TypeCompatibilityMatrix, TypeHierarchy as ModelTypeHierarchy,
    },
};

#[derive(Debug)]
pub struct FhirSchemaModelProvider {
    schema_manager: Arc<FhirSchemaManager>,
    type_resolver: Arc<TypeResolver>,
    navigation_engine: Arc<NavigationEngine>,
    cache: Arc<ModelProviderCache>,
    fhir_version: FhirVersion,
    /// IMPORTANT: O(1) resource type existence check - populated from converted schemas, not hardcoded
    /// Using papaya for high-performance thread-safe access
    available_resource_types: Arc<papaya::HashMap<String, ()>>,
}

#[derive(Debug, Clone)]
pub struct TypeHierarchy {
    pub base_type: String,
    pub parent_type: Option<String>,
    pub child_types: Vec<String>,
    pub properties: HashMap<String, PropertyInfo>,
    pub constraints: Vec<ConstraintInfo>,
}

#[derive(Debug, Clone)]
pub struct PropertyInfo {
    pub name: String,
    pub property_type: String,
    pub cardinality: Cardinality,
    pub is_choice_type: bool,
    pub choice_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Cardinality {
    pub min: u32,
    pub max: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ConstraintInfo {
    pub key: String,
    pub severity: String,
    pub human: String,
    pub expression: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NavigationResult {
    pub target_type: String,
    pub is_valid_path: bool,
    pub path_segments: Vec<PathSegment>,
    pub choice_resolution: Option<ChoiceResolution>,
    pub confidence: f64,
    /// Whether the result type is an array/collection
    pub is_array: bool,
    /// Element type for arrays (same as target_type for scalars)
    pub element_type: String,
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub name: String,
    pub segment_type: String,
    pub is_array: bool,
    pub cardinality: Cardinality,
}

#[derive(Debug, Clone)]
pub struct ChoiceResolution {
    pub resolved_type: String,
    pub confidence: f64,
    pub alternatives: Vec<AlternativeType>,
    pub context_used: ResolutionContext,
    pub resolution_path: String,
}

#[derive(Debug, Clone)]
pub struct AlternativeType {
    pub type_name: String,
    pub confidence: f64,
    pub reason: String,
}

impl FhirSchemaModelProvider {
    /// Create a new FHIR R4 model provider
    ///
    /// This is the main entry point for end users. Simply call:
    /// ```rust,no_run
    /// use octofhir_fhirschema::prelude::*;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    ///     let provider = FhirSchemaModelProvider::r4().await?;
    ///     // Use the provider...
    ///     Ok(())
    /// }
    /// ```
    pub async fn r4() -> Result<Self> {
        Self::new(FhirVersion::R4).await
    }

    /// Create a new FHIR R5 model provider
    pub async fn r5() -> Result<Self> {
        Self::new(FhirVersion::R5).await
    }

    /// Create a new FHIR R4B model provider
    pub async fn r4b() -> Result<Self> {
        Self::new(FhirVersion::R4B).await
    }

    /// Create a new FHIR R6 model provider
    pub async fn r6() -> Result<Self> {
        Self::new(FhirVersion::R6).await
    }

    /// Create an embedded-only model provider for fastest startup
    /// This will only use precompiled schemas, no live compilation
    #[cfg(feature = "embedded-providers")]
    pub async fn embedded_only(
        fhir_version: FhirVersion,
    ) -> Result<crate::provider::EmbeddedModelProvider> {
        use crate::provider::EmbeddedModelProvider;
        match fhir_version {
            FhirVersion::R4 => EmbeddedModelProvider::r4().await,
            FhirVersion::R4B => EmbeddedModelProvider::r4b().await,
            FhirVersion::R5 => EmbeddedModelProvider::r5().await,
            FhirVersion::R6 => EmbeddedModelProvider::r6().await,
        }
    }

    /// Create a dynamic caching provider for fast subsequent startups
    #[cfg(feature = "dynamic-caching")]
    pub async fn with_caching(
        fhir_version: FhirVersion,
    ) -> Result<crate::provider::DynamicModelProvider> {
        use crate::provider::DynamicModelProvider;
        let mut provider = match fhir_version {
            FhirVersion::R4 => DynamicModelProvider::r4().await?,
            FhirVersion::R4B => DynamicModelProvider::r4b().await?,
            FhirVersion::R5 => DynamicModelProvider::r5().await?,
            FhirVersion::R6 => DynamicModelProvider::r6().await?,
        };

        provider.initialize().await?;
        Ok(provider)
    }

    /// Create a composite provider with all optimizations (recommended)
    pub async fn composite(
        fhir_version: FhirVersion,
    ) -> Result<crate::provider::CompositeModelProvider> {
        use crate::provider::CompositeModelProvider;
        match fhir_version {
            FhirVersion::R4 => CompositeModelProvider::r4().await,
            FhirVersion::R4B => CompositeModelProvider::r4b().await,
            FhirVersion::R5 => CompositeModelProvider::r5().await,
            FhirVersion::R6 => CompositeModelProvider::r6().await,
        }
    }

    // Constructor - now public for composite provider
    pub async fn new(fhir_version: FhirVersion) -> Result<Self> {
        let config = FhirSchemaConfig::for_version(fhir_version);

        // Create the canonical manager first
        let canonical_manager_instance = octofhir_canonical_manager::CanonicalManager::new(
            octofhir_canonical_manager::FcmConfig::default(),
        )
        .await
        .map_err(|e| FhirSchemaError::conversion_failed("CanonicalManager", &e.to_string()))?;

        // FhirSchemaManager will take ownership, but we also need to share it for loading schemas
        // So we need to create two instances (not ideal, but necessary with current architecture)
        let _canonical_manager_for_loading = octofhir_canonical_manager::CanonicalManager::new(
            octofhir_canonical_manager::FcmConfig::default(),
        )
        .await
        .map_err(|e| FhirSchemaError::conversion_failed("CanonicalManager", &e.to_string()))?;

        let schema_manager =
            Arc::new(FhirSchemaManager::new(config, canonical_manager_instance).await?);

        let canonical_manager_for_type_resolver =
            octofhir_canonical_manager::CanonicalManager::new(
                octofhir_canonical_manager::FcmConfig::default(),
            )
            .await
            .map_err(|e| FhirSchemaError::conversion_failed("CanonicalManager", &e.to_string()))?;

        let type_resolver =
            Arc::new(TypeResolver::new(Arc::new(canonical_manager_for_type_resolver)).await?);
        let navigation_engine = Arc::new(
            NavigationEngine::new(Arc::clone(&type_resolver), Arc::clone(&schema_manager)).await?,
        );

        let cache = Arc::new(ModelProviderCache::new());
        let available_resource_types = Arc::new(papaya::HashMap::new());

        let provider = Self {
            schema_manager,
            type_resolver,
            navigation_engine,
            cache,
            fhir_version,
            available_resource_types,
        };

        // CRITICAL: Initialize core FHIR resource types for fast CLI startup
        // provider.initialize_core_resource_types().await?;

        // Initialize resource types from converted schemas
        provider.initialize_resource_types().await?;

        Ok(provider)
    }

    /// Register embedded schemas and resource types from EmbeddedModelProvider
    /// This ensures the FhirSchemaModelProvider has access to the same schemas and resource types
    pub async fn register_embedded_schemas(
        &mut self,
        schemas: &HashMap<String, crate::types::FhirSchema>,
        embedded_resource_types: &Arc<papaya::HashMap<String, ()>>,
    ) -> Result<()> {
        // Copy all resource types from embedded provider
        {
            let embedded_guard = embedded_resource_types.pin();
            let our_guard = self.available_resource_types.pin();

            for (resource_type, _) in embedded_guard.iter() {
                our_guard.insert(resource_type.clone(), ());

                if std::env::var("FHIRPATH_DEBUG_PERF").is_ok() {
                    eprintln!(
                        "🔧 RESOURCE TYPE SHARING: Copied resource type {resource_type} from embedded provider"
                    );
                }
            }
        }

        // Add embedded schemas to our schema manager
        for (schema_id, schema) in schemas {
            self.schema_manager
                .store_schema(schema_id, schema.clone())
                .await?;

            if std::env::var("FHIRPATH_DEBUG_PERF").is_ok() {
                eprintln!("🔧 SCHEMA SHARING: Registered embedded schema {schema_id}");
            }
        }

        Ok(())
    }

    /// Load core FHIR resource schemas following the proper flow:
    /// 1. Load packages from canonical manager
    /// 2. Convert them to FhirSchema using the conversion engine (NOT manual creation!)
    /// 3. Store converted schemas in storage
    /// 4. Extract resource types and update HashMap (done in initialize_resource_types)
    #[allow(dead_code)]
    async fn load_core_fhir_schemas_with_converter(
        &self,
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
    ) -> Result<()> {
        // Step 1: Get core package info for this FHIR version
        let (package_name, package_version) = match self.fhir_version {
            FhirVersion::R4 => ("hl7.fhir.r4.core", "4.0.1"),
            FhirVersion::R5 => ("hl7.fhir.r5.core", "5.0.0"),
            FhirVersion::R4B => ("hl7.fhir.r4b.core", "4.3.0"),
            FhirVersion::R6 => ("hl7.fhir.r6.core", "6.0.0-ballot3"),
        };

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Loading FHIR package: {}@{} with ConversionEngine",
            package_name,
            package_version
        );

        // Step 1: Install the core FHIR package using the shared canonical manager
        canonical_manager
            .install_package(package_name, package_version)
            .await
            .map_err(|e| {
                FhirSchemaError::conversion_failed("Package Installation", &e.to_string())
            })?;

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully installed package {}@{}",
            package_name,
            package_version
        );

        // Step 2: Create ConversionEngine to convert FHIR StructureDefinitions to FhirSchemas
        let conversion_engine = crate::conversion::ConversionEngine::new(
            Arc::clone(&canonical_manager),
            &crate::core::PerformanceConfig::default(),
        )
        .await?;

        // Step 2: Get all StructureDefinition resources from the package and convert them
        let core_resource_types = match self.fhir_version {
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
                "CarePlan",
                "Goal",
                "ServiceRequest",
                "Device",
                "Location",
                "Specimen",
                "Coverage",
                "Account",
                "Person",
                "RelatedPerson",
                "Group",
                "HealthcareService",
                "Endpoint",
                "PractitionerRole",
                "Schedule",
                "Slot",
                "Appointment",
                "AppointmentResponse",
                "Flag",
                "List",
                "Composition",
                "DocumentReference",
                "Binary",
                "Media",
                "Communication",
                "CommunicationRequest",
                "Task",
                "Provenance",
                "AuditEvent",
                "Consent",
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
                "CarePlan",
                "Goal",
                "ServiceRequest",
                "Device",
                "Location",
                "Specimen",
                "Coverage",
                "Account",
                "Person",
                "RelatedPerson",
                "Group",
                "HealthcareService",
                "Endpoint",
                "PractitionerRole",
                "Schedule",
                "Slot",
                "Appointment",
                "AppointmentResponse",
                "Flag",
                "List",
                "Composition",
                "DocumentReference",
                "Binary",
                "Media",
                "Communication",
                "CommunicationRequest",
                "Task",
                "Provenance",
                "AuditEvent",
                "Consent",
                "Subscription",
                "SubscriptionStatus",
                "SubscriptionTopic",
            ],
        };

        let mut structure_defs = Vec::new();

        // Step 2: Collect all StructureDefinition resources for batch conversion
        for resource_type in core_resource_types {
            let structure_definition_url =
                format!("http://hl7.org/fhir/StructureDefinition/{resource_type}");

            // Try to resolve the StructureDefinition from the loaded package
            match canonical_manager.resolve(&structure_definition_url).await {
                Ok(resolved_resource) => {
                    #[cfg(feature = "tracing")]
                    tracing::debug!("Resolved StructureDefinition for {}", resource_type);

                    // Add to batch for conversion
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
                "batch",
                "No StructureDefinition resources could be resolved from the package",
            ));
        }

        // Step 3: Use ConversionEngine to convert FHIR StructureDefinitions to FhirSchemas
        #[cfg(feature = "tracing")]
        tracing::info!(
            "Converting {} StructureDefinitions using ConversionEngine",
            structure_defs.len()
        );

        let conversion_results = conversion_engine.convert_batch(structure_defs).await?;

        let mut loaded_count = 0;

        // Step 4: Store the converted schemas in the schema manager's storage
        for result in conversion_results {
            if result.success {
                if let Some(schema) = result.schema {
                    let schema_url = schema.id.clone().unwrap_or_else(|| "unknown".to_string());

                    // Store the properly converted schema
                    self.schema_manager
                        .store_schema(&schema_url, schema)
                        .await?;
                    loaded_count += 1;

                    #[cfg(feature = "tracing")]
                    tracing::debug!("Stored converted schema: {}", schema_url);
                }
            } else {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    "Conversion failed for a StructureDefinition: {:?}",
                    result.errors
                );
            }
        }

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully converted and loaded {} FHIR resource schemas from package {}@{} using ConversionEngine",
            loaded_count,
            package_name,
            package_version
        );

        Ok(())
    }

    /// Enhanced type hierarchy with caching
    pub async fn get_type_hierarchy(&self, type_name: &str) -> Result<Option<TypeHierarchy>> {
        if let Some(cached) = self.cache.get_hierarchy(type_name).await {
            return Ok(Some(cached));
        }

        let schema = self
            .schema_manager
            .get_schema_by_type(type_name)
            .await?
            .ok_or_else(|| FhirSchemaError::type_not_found(type_name))?;

        let hierarchy = self.build_type_hierarchy(&schema, type_name).await?;
        self.cache.cache_hierarchy(type_name, &hierarchy).await?;

        Ok(Some(hierarchy))
    }

    /// Advanced navigation with path optimization
    pub async fn navigate_typed_path(
        &self,
        base_type: &str,
        path: &str,
    ) -> Result<NavigationResult> {
        self.navigation_engine
            .navigate_with_optimization(base_type, path)
            .await
    }

    /// Choice type resolution with inference
    pub async fn resolve_choice_type(
        &self,
        base_path: &str,
        context: &ResolutionContext,
    ) -> Result<ChoiceResolution> {
        let resolved_types = self
            .type_resolver
            .resolve_choice_type(base_path, "[x]", context)
            .await?;

        if resolved_types.is_empty() {
            return Err(FhirSchemaError::type_resolution_failed(
                base_path,
                "No types resolved",
            ));
        }

        let primary = &resolved_types[0];
        let alternatives = resolved_types
            .clone()
            .into_iter()
            .skip(1)
            .map(|rt| AlternativeType {
                type_name: rt.type_name,
                confidence: 0.5, // Fixed confidence for alternatives
                reason: "Alternative type".to_string(),
            })
            .collect();

        Ok(ChoiceResolution {
            resolved_type: primary.type_name.clone(),
            confidence: 0.95, // Fixed confidence for primary
            alternatives,
            context_used: context.clone(),
            resolution_path: base_path.to_string(),
        })
    }

    async fn build_type_hierarchy(
        &self,
        schema: &crate::types::FhirSchema,
        type_name: &str,
    ) -> Result<TypeHierarchy> {
        let mut properties = HashMap::new();

        // First pass: detect choice types by analyzing property patterns
        let choice_type_info = self.extract_choice_types_from_schema(schema);

        // Extract properties from schema
        for (prop_name, prop) in &schema.properties {
            let is_required = schema.required.contains(prop_name);
            let is_array = prop.items.is_some();

            let cardinality = Cardinality {
                min: if is_required { 1 } else { 0 },
                max: if is_array { None } else { Some(1) },
            };

            // Determine if this is a choice type and get its variants
            let (is_choice_type, choice_types) = if let Some((_base_name, variants)) =
                choice_type_info
                    .iter()
                    .find(|(base_name, _)| *base_name == prop_name)
            {
                // This is a choice type base
                (true, variants.iter().cloned().collect())
            } else if choice_type_info
                .values()
                .any(|variants| variants.contains(prop_name))
            {
                // This is a concrete choice type variant, skip it in favor of the base
                continue;
            } else {
                // Regular property
                (false, Vec::new())
            };

            properties.insert(
                if is_choice_type {
                    format!("{prop_name}[x]") // Add [x] suffix for choice types
                } else {
                    prop_name.clone()
                },
                PropertyInfo {
                    name: prop_name.clone(),
                    property_type: prop
                        .property_type
                        .clone()
                        .unwrap_or_else(|| "Element".to_string()), // Choice types default to Element
                    cardinality,
                    is_choice_type,
                    choice_types,
                },
            );
        }

        // Extract constraints
        let constraints = schema
            .constraints
            .iter()
            .map(|c| ConstraintInfo {
                key: c.key.clone(),
                severity: format!("{:?}", c.severity),
                human: c.human.clone(),
                expression: c.expression.clone(),
            })
            .collect();

        // Get parent type from schema metadata (if available)
        let parent_type = None; // TODO: Extract from schema metadata if available

        // TODO: Implement child type discovery
        let child_types = Vec::new();

        Ok(TypeHierarchy {
            base_type: type_name.to_string(),
            parent_type,
            child_types,
            properties,
            constraints,
        })
    }

    /// Extract choice type information directly from schema metadata
    fn extract_choice_types_from_schema(
        &self,
        schema: &crate::types::FhirSchema,
    ) -> HashMap<String, HashSet<String>> {
        let mut choice_types = HashMap::new();

        // Look for properties that are explicitly marked as choice types in the schema
        for (property_name, property) in &schema.properties {
            // Skip underscore properties (metadata)
            if property_name.starts_with('_') {
                continue;
            }

            // Check if this property is marked as a choice type in the metadata
            if let Some(metadata_value) = property.metadata.get("is_choice_type") {
                if let Some(is_choice) = metadata_value.as_bool() {
                    if is_choice {
                        // Extract the allowed choice types from metadata
                        if let Some(choice_types_value) = property.metadata.get("choice_types") {
                            if let Some(choice_types_array) = choice_types_value.as_array() {
                                let mut variants = HashSet::new();

                                for choice_type in choice_types_array {
                                    if let Some(type_name) = choice_type.as_str() {
                                        // Create concrete property name: base + TypeName
                                        // e.g., "value" + "Quantity" = "valueQuantity"
                                        let concrete_property =
                                            format!("{property_name}{type_name}");
                                        variants.insert(concrete_property);
                                    }
                                }

                                if !variants.is_empty() {
                                    choice_types.insert(property_name.clone(), variants);
                                }
                            }
                        }
                    }
                }
            }
        }

        choice_types
    }

    /// Get schema manager reference
    pub fn schema_manager(&self) -> &FhirSchemaManager {
        &self.schema_manager
    }

    /// Get type resolver reference
    pub fn type_resolver(&self) -> &TypeResolver {
        &self.type_resolver
    }

    /// Get navigation engine reference
    pub fn navigation_engine(&self) -> &NavigationEngine {
        &self.navigation_engine
    }

    /// Get FHIR version
    pub fn fhir_version(&self) -> FhirVersion {
        self.fhir_version
    }

    /// Clear all caches
    pub async fn clear_caches(&self) -> Result<()> {
        self.cache.clear().await?;
        self.schema_manager.clear_all_caches().await?;
        Ok(())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> Result<serde_json::Value> {
        let provider_stats = self.cache.get_stats().await;
        let manager_stats = self.schema_manager.get_cache_stats().await?;
        let resource_types_count = {
            let guard = self.available_resource_types.pin();
            guard.len()
        };

        Ok(serde_json::json!({
            "model_provider": provider_stats,
            "schema_manager": manager_stats,
            "cached_resource_types": resource_types_count
        }))
    }

    /// IMPORTANT: Initialize available resource types from converted FHIR schemas
    /// This extracts all resource types from the current schema storage - NO HARDCODING
    async fn initialize_resource_types(&self) -> Result<()> {
        // Clear existing resource types
        {
            let guard = self.available_resource_types.pin();
            guard.clear();
        }

        // Get all schema URLs from storage
        let schema_urls = self.schema_manager.list_schemas().await?;

        for url in schema_urls {
            // Extract resource type from FHIR StructureDefinition URL pattern
            // e.g., "http://hl7.org/fhir/StructureDefinition/Patient" -> "Patient"
            if let Some(resource_type) = Self::extract_resource_type_from_url(&url) {
                let guard = self.available_resource_types.pin();
                guard.insert(resource_type, ());
            } else if let Some(schema) = self.schema_manager.get_schema(&url).await? {
                // Also try to extract from schema metadata if URL pattern doesn't work
                if let Some(resource_type) = Self::extract_resource_type_from_schema(&schema) {
                    let guard = self.available_resource_types.pin();
                    guard.insert(resource_type, ());
                }
            }
        }

        Ok(())
    }

    /// Extract resource type from StructureDefinition URL
    /// IMPORTANT: This extracts data from schemas, not hardcoded
    pub fn extract_resource_type_from_url(url: &str) -> Option<String> {
        // Standard FHIR StructureDefinition URL pattern: http://hl7.org/fhir/StructureDefinition/{ResourceType}
        if let Some(captures) = url.strip_prefix("http://hl7.org/fhir/StructureDefinition/") {
            if !captures.is_empty() && captures.chars().next().unwrap_or('a').is_uppercase() {
                return Some(captures.to_string());
            }
        }

        // Handle other URL patterns that might contain resource types
        if url.contains("StructureDefinition/") {
            if let Some(start) = url.rfind("StructureDefinition/") {
                let resource_type = &url[start + "StructureDefinition/".len()..];
                if !resource_type.is_empty()
                    && resource_type.chars().next().unwrap_or('a').is_uppercase()
                {
                    return Some(resource_type.to_string());
                }
            }
        }

        None
    }

    /// Extract resource type from schema metadata
    /// IMPORTANT: This extracts data from converted schemas, not hardcoded
    fn extract_resource_type_from_schema(schema: &crate::types::FhirSchema) -> Option<String> {
        // Check schema title
        if let Some(title) = &schema.title {
            if title.chars().next().unwrap_or('a').is_uppercase() {
                return Some(title.clone());
            }
        }

        // Check schema ID
        if let Some(id) = &schema.id {
            if let Some(resource_type) = Self::extract_resource_type_from_url(id) {
                return Some(resource_type);
            }
        }

        // Check metadata for resource type information
        if let Some(resource_type_value) = schema.metadata.get("resourceType") {
            if let Some(resource_type) = resource_type_value.as_str() {
                if resource_type.chars().next().unwrap_or('a').is_uppercase() {
                    return Some(resource_type.to_string());
                }
            }
        }

        None
    }

    /// O(1) check if a resource type exists in the converted schemas
    /// IMPORTANT: This uses data extracted from schemas, not hardcoded lists
    pub fn resource_type_exists(&self, resource_type: &str) -> bool {
        let guard = self.available_resource_types.pin();
        guard.contains_key(resource_type)
    }

    /// Get all available resource types from converted schemas
    /// IMPORTANT: This returns data extracted from schemas, not hardcoded lists
    pub fn get_available_resource_types(&self) -> Vec<String> {
        let guard = self.available_resource_types.pin();
        guard.keys().cloned().collect()
    }

    /// Refresh resource types cache from current schema storage
    /// IMPORTANT: This re-extracts data from schemas, ensuring no stale hardcoded data
    pub async fn refresh_resource_types(&self) -> Result<()> {
        self.initialize_resource_types().await
    }
}

// ============================================================================
// ModelProvider Trait Implementation
// ============================================================================

#[async_trait]
impl ModelProvider for FhirSchemaModelProvider {
    // ========================================================================
    // Core Type Operations
    // ========================================================================

    async fn get_type_hierarchy(&self, type_name: &str) -> ModelResult<Option<ModelTypeHierarchy>> {
        match self.get_type_hierarchy(type_name).await {
            Ok(Some(hierarchy)) => {
                // Convert our TypeHierarchy to fhir-model-rs TypeHierarchy
                let model_hierarchy = self.convert_to_model_type_hierarchy(hierarchy).await?;
                Ok(Some(model_hierarchy))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }

    async fn is_type_compatible(&self, from_type: &str, to_type: &str) -> ModelResult<bool> {
        let context = ResolutionContext::new(from_type);
        match self
            .schema_manager
            .is_compatible_type(from_type, to_type, &context)
            .await
        {
            Ok(compatible) => Ok(compatible),
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }

    async fn get_common_supertype(&self, types: &[String]) -> ModelResult<Option<String>> {
        // Handle edge cases
        if types.is_empty() {
            return Ok(None);
        }

        if types.len() == 1 {
            return Ok(Some(types[0].clone()));
        }

        // Use type hierarchy analysis to find common supertype
        let context = ResolutionContext::new(&types[0]);

        // Get hierarchies for all types and find intersection
        let mut hierarchies: Vec<Vec<String>> = Vec::new();
        for type_name in types {
            match self
                .type_resolver
                .get_type_hierarchy(type_name, &context)
                .await
            {
                Ok(hierarchy) => hierarchies.push(hierarchy),
                Err(_) => return Ok(Some("Element".to_string())), // Fallback on error
            }
        }

        if hierarchies.is_empty() {
            return Ok(None);
        }

        // Find common types in all hierarchies
        let first_hierarchy = &hierarchies[0];
        for type_name in first_hierarchy {
            let is_common = hierarchies.iter().all(|h| h.contains(type_name));
            if is_common {
                return Ok(Some(type_name.clone()));
            }
        }

        // If no common type found, fallback to Element
        Ok(Some("Element".to_string()))
    }

    async fn get_type_compatibility_matrix(&self) -> ModelResult<TypeCompatibilityMatrix> {
        use octofhir_fhir_model::type_system::{
            ConversionFunction, ConversionInfo, ConversionType,
        };

        let matrix = TypeCompatibilityMatrix::new();
        let context = ResolutionContext::new("Unknown");

        // Get all supported resource types to build compatibility rules
        // IMPORTANT: Using extracted resource types from schemas, not hardcoded
        let resource_types = self.get_available_resource_types();

        // Build basic FHIR type compatibility rules
        let fhir_primitives = vec![
            "boolean", "integer", "decimal", "string", "date", "dateTime", "time",
        ];

        // Add implicit conversions for compatible primitive types
        {
            let conversion_guard = matrix.conversions.guard();
            let implicit_guard = matrix.implicit_conversions.guard();

            for from_type in &fhir_primitives {
                let mut compatible_types = Vec::new();

                match *from_type {
                    "integer" => {
                        compatible_types.push("decimal".to_string());

                        let conversion_info = ConversionInfo {
                            conversion_type: ConversionType::Implicit,
                            conversion_function: None,
                            data_loss_possible: false,
                            validation_rules: Vec::new(),
                            performance_cost: 0.1,
                        };

                        matrix.conversions.insert(
                            ("integer".to_string(), "decimal".to_string()),
                            conversion_info,
                            &conversion_guard,
                        );
                    }
                    "decimal" => {
                        compatible_types.push("integer".to_string()); // With potential data loss

                        let conversion_info = ConversionInfo {
                            conversion_type: ConversionType::Explicit,
                            conversion_function: None,
                            data_loss_possible: true,
                            validation_rules: Vec::new(),
                            performance_cost: 0.2,
                        };

                        matrix.conversions.insert(
                            ("decimal".to_string(), "integer".to_string()),
                            conversion_info,
                            &conversion_guard,
                        );
                    }
                    "date" => {
                        compatible_types.push("dateTime".to_string());

                        let conversion_info = ConversionInfo {
                            conversion_type: ConversionType::Implicit,
                            conversion_function: None,
                            data_loss_possible: false,
                            validation_rules: Vec::new(),
                            performance_cost: 0.1,
                        };

                        matrix.conversions.insert(
                            ("date".to_string(), "dateTime".to_string()),
                            conversion_info,
                            &conversion_guard,
                        );
                    }
                    _ => {}
                }

                if !compatible_types.is_empty() {
                    matrix.implicit_conversions.insert(
                        from_type.to_string(),
                        compatible_types.clone(),
                        &implicit_guard,
                    );
                }
            }
        }

        // Add function-based conversions for all primitives to string
        {
            let function_guard = matrix.function_conversions.guard();
            let string_conversions: Vec<ConversionFunction> = fhir_primitives
                .iter()
                .map(|_type_name| ConversionFunction {
                    function_name: "toString".to_string(),
                    target_type: "string".to_string(),
                    can_fail: false,
                    validation_requirements: Vec::new(),
                })
                .collect();

            for primitive in &fhir_primitives {
                if *primitive != "string" {
                    matrix.function_conversions.insert(
                        primitive.to_string(),
                        string_conversions.clone(),
                        &function_guard,
                    );
                }
            }
        }

        // Add type hierarchy compatibility for resource types
        for resource_type in &resource_types {
            match self
                .type_resolver
                .get_type_hierarchy(resource_type, &context)
                .await
            {
                Ok(hierarchy) => {
                    let mut compatible_types = Vec::new();
                    let mut conversion_entries = Vec::new();

                    // Collect parent types as implicit conversions (subtype to supertype)
                    for parent_type in hierarchy.iter().skip(1) {
                        // Skip self
                        compatible_types.push(parent_type.clone());

                        let conversion_info = ConversionInfo {
                            conversion_type: ConversionType::Implicit,
                            conversion_function: None,
                            data_loss_possible: false,
                            validation_rules: Vec::new(),
                            performance_cost: 0.0, // No cost for type upcast
                        };

                        conversion_entries.push((
                            (resource_type.clone(), parent_type.clone()),
                            conversion_info,
                        ));
                    }

                    // Now insert all the collected data
                    {
                        let conversion_guard = matrix.conversions.guard();
                        for ((from, to), info) in conversion_entries {
                            matrix
                                .conversions
                                .insert((from, to), info, &conversion_guard);
                        }
                    }

                    if !compatible_types.is_empty() {
                        let implicit_guard = matrix.implicit_conversions.guard();
                        matrix.implicit_conversions.insert(
                            resource_type.clone(),
                            compatible_types,
                            &implicit_guard,
                        );
                    }
                }
                Err(_) => continue, // Skip types we can't resolve
            }
        }

        Ok(matrix)
    }

    // ========================================================================
    // Navigation Operations
    // ========================================================================

    async fn navigate_typed_path(
        &self,
        base_type: &str,
        path: &str,
    ) -> ModelResult<ModelNavigationResult> {
        match FhirSchemaModelProvider::navigate_typed_path(self, base_type, path).await {
            Ok(result) => {
                // Convert our NavigationResult to fhir-model-rs NavigationResult
                let type_info = if result.is_array {
                    // For arrays, create ListType with the element type
                    let element_type =
                        TypeReflectionInfo::simple_type("FHIR", &result.element_type);
                    TypeReflectionInfo::list_type(element_type)
                } else {
                    // For scalars, create SimpleType
                    TypeReflectionInfo::simple_type("FHIR", &result.target_type)
                };
                Ok(ModelNavigationResult::success(type_info))
            }
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }

    async fn validate_navigation_safety(
        &self,
        base_type: &str,
        path: &str,
    ) -> ModelResult<PathValidation> {
        match self.navigate_typed_path(base_type, path).await {
            Ok(result) => {
                if result.is_valid_path {
                    Ok(PathValidation::success(format!("{base_type}.{path}")))
                } else {
                    let mut validation = PathValidation::new(format!("{base_type}.{path}"));
                    validation.validation_errors.push(
                        octofhir_fhir_model::navigation::ValidationError {
                            error_code: "PATH_NOT_FOUND".to_string(),
                            message: "Path navigation failed".to_string(),
                            location: octofhir_fhir_model::navigation::PathLocation {
                                segment_index: 0,
                                character_position: 0,
                                segment_name: path.to_string(),
                            },
                            severity: octofhir_fhir_model::navigation::ConstraintSeverity::Error,
                        },
                    );
                    Ok(validation)
                }
            }
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }

    async fn get_navigation_result_type(
        &self,
        base_type: &str,
        path: &str,
    ) -> ModelResult<Option<TypeReflectionInfo>> {
        match self.navigate_typed_path(base_type, path).await {
            Ok(result) => {
                let type_info = TypeReflectionInfo::simple_type("FHIR", &result.target_type);
                Ok(Some(type_info))
            }
            Err(_) => Ok(None),
        }
    }

    async fn get_navigation_metadata(
        &self,
        base_type: &str,
        path: &str,
    ) -> ModelResult<NavigationMetadata> {
        match self.navigate_typed_path(base_type, path).await {
            Ok(result) => {
                Ok(
                    NavigationMetadata {
                        path: format!("{base_type}.{path}"),
                        source_type: base_type.to_string(),
                        target_type: result.target_type,
                        intermediate_types: result
                            .path_segments
                            .iter()
                            .map(|seg| seg.segment_type.clone())
                            .collect(),
                        collection_info: Default::default(),
                        polymorphic_resolution: result.choice_resolution.map(|cr| {
                            PolymorphicResolution {
                    resolved_type: cr.resolved_type,
                    confidence_score: cr.confidence,
                    resolution_method:
                        octofhir_fhir_model::type_system::ResolutionMethod::ContextInference,
                    alternative_types: cr
                        .alternatives
                        .into_iter()
                        .map(|alt| octofhir_fhir_model::type_system::AlternativeType {
                            type_name: alt.type_name,
                            confidence: alt.confidence,
                            reasoning: alt.reason,
                        })
                        .collect(),
                    resolution_context: PolymorphicContext {
                        current_path: format!("{base_type}.{path}"),
                        base_type: base_type.to_string(),
                        available_types: Vec::new(),
                        constraints: Vec::new(),
                        inference_hints: Vec::new(),
                        resolution_strategy:
                            octofhir_fhir_model::type_system::ResolutionStrategy::FirstMatch,
                        metadata: HashMap::new(),
                    },
                }
                        }),
                        navigation_warnings: Vec::new(),
                        performance_metadata:
                            octofhir_fhir_model::type_system::PerformanceMetadata {
                                operation_cost: 1.0,
                                is_cacheable: true,
                                cache_key: Some(format!("{base_type}-{path}")),
                                memory_estimate: Some(256),
                            },
                    },
                )
            }
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }

    // ========================================================================
    // Choice Type Operations
    // ========================================================================

    async fn resolve_choice_type(
        &self,
        base_path: &str,
        context: &PolymorphicContext,
    ) -> ModelResult<PolymorphicResolution> {
        let resolution_context = ResolutionContext::new(&context.base_type);

        match self
            .resolve_choice_type(base_path, &resolution_context)
            .await
        {
            Ok(choice_resolution) => Ok(PolymorphicResolution {
                resolved_type: choice_resolution.resolved_type,
                confidence_score: choice_resolution.confidence,
                resolution_method:
                    octofhir_fhir_model::type_system::ResolutionMethod::ContextInference,
                alternative_types: choice_resolution
                    .alternatives
                    .into_iter()
                    .map(|alt| octofhir_fhir_model::type_system::AlternativeType {
                        type_name: alt.type_name,
                        confidence: alt.confidence,
                        reasoning: alt.reason,
                    })
                    .collect(),
                resolution_context: context.clone(),
            }),
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }

    async fn get_choice_expansions(
        &self,
        choice_property: &str,
    ) -> ModelResult<Vec<ChoiceExpansion>> {
        // Extract base property name by removing [x] suffix
        let base_property = choice_property.trim_end_matches("[x]");

        // Get choice type expansions from type resolver with better context
        let context = ResolutionContext::new("FHIR");
        match self
            .type_resolver
            .resolve_choice_type("Resource", choice_property, &context)
            .await
        {
            Ok(resolved_types) => {
                // Create comprehensive forward and reverse mappings
                let mut forward_mappings = HashMap::new();
                let mut reverse_mappings = HashMap::new();
                let mut expanded_paths = Vec::new();

                for rt in &resolved_types {
                    // Create property name following FHIR naming conventions
                    let property_name = if rt.type_name.chars().next().unwrap_or('a').is_uppercase()
                    {
                        // Complex types like CodeableConcept -> valueCodeableConcept
                        format!("{}{}", base_property, rt.type_name)
                    } else {
                        // Primitive types like string -> valueString (capitalize first letter)
                        let mut chars = rt.type_name.chars();
                        match chars.next() {
                            None => format!("{}{}", base_property, rt.type_name),
                            Some(first) => format!(
                                "{}{}{}",
                                base_property,
                                first.to_uppercase().collect::<String>(),
                                chars.as_str()
                            ),
                        }
                    };

                    forward_mappings.insert(rt.type_name.clone(), property_name.clone());
                    reverse_mappings.insert(property_name.clone(), rt.type_name.clone());

                    // Create navigation paths for each expansion using actual schema data
                    expanded_paths.push(octofhir_fhir_model::choice_types::ExpandedPath {
                        path: property_name.clone(),
                        target_type: rt.type_name.clone(),
                        type_info: octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type(
                            "FHIR",
                            &rt.type_name,
                        ),
                        path_constraints: Vec::new(), // Will be populated from schema if available
                        cardinality: octofhir_fhir_model::type_system::Cardinality::optional(),
                    });
                }

                // Create expansion context with metadata
                let mut expansion_metadata = HashMap::new();
                expansion_metadata.insert(
                    "total_expansions".to_string(),
                    resolved_types.len().to_string(),
                );
                expansion_metadata.insert("base_property".to_string(), base_property.to_string());

                let expansion_context = octofhir_fhir_model::choice_types::ExpansionContext {
                    resource_type: Some("Resource".to_string()),
                    profile: None,
                    extension_context: None,
                    metadata: expansion_metadata,
                };

                let expansion = ChoiceExpansion {
                    choice_property: choice_property.to_string(),
                    forward_mappings,
                    reverse_mappings,
                    expanded_paths,
                    expansion_context,
                };

                Ok(vec![expansion])
            }
            Err(_e) => {
                // Return empty expansion for graceful degradation
                let mut fallback_metadata = HashMap::new();
                fallback_metadata.insert(
                    "error".to_string(),
                    "Failed to resolve choice type".to_string(),
                );
                fallback_metadata.insert("base_property".to_string(), base_property.to_string());

                let expansion = ChoiceExpansion {
                    choice_property: choice_property.to_string(),
                    forward_mappings: HashMap::new(),
                    reverse_mappings: HashMap::new(),
                    expanded_paths: Vec::new(),
                    expansion_context: octofhir_fhir_model::choice_types::ExpansionContext {
                        resource_type: Some("Unknown".to_string()),
                        profile: None,
                        extension_context: None,
                        metadata: fallback_metadata,
                    },
                };
                Ok(vec![expansion])
            }
        }
    }

    async fn infer_choice_type(&self, context: &PolymorphicContext) -> ModelResult<TypeInference> {
        let mut inference_rules = Vec::new();
        let mut type_scores: HashMap<String, f64> = HashMap::new();

        // Rule 1: Type frequency analysis based on available types in context
        if !context.available_types.is_empty() {
            let base_score = 1.0 / context.available_types.len() as f64;
            for available_type in &context.available_types {
                type_scores.insert(available_type.clone(), base_score);
            }

            inference_rules.push(octofhir_fhir_model::choice_types::InferenceRule {
                rule_id: "type_frequency".to_string(),
                pattern: "available_types_analysis".to_string(),
                inferred_type: "string".to_string(), // Default inferred type
                confidence_weight: 0.3,
                applicable_contexts: vec!["type_frequency".to_string()],
                metadata: HashMap::new(),
            });
        }

        // Rule 2: Constraint-based inference from schema
        if !context.constraints.is_empty() {
            for constraint in &context.constraints {
                // Boost score for types that match constraint applicable types
                for applicable_type in &constraint.applicable_types {
                    if let Some(current_score) = type_scores.get(applicable_type) {
                        type_scores.insert(applicable_type.clone(), current_score + 0.4);
                    } else {
                        type_scores.insert(applicable_type.clone(), 0.4);
                    }
                }
            }

            let _constraint_types: Vec<String> = context
                .constraints
                .iter()
                .flat_map(|c| c.applicable_types.iter().cloned())
                .collect();

            inference_rules.push(octofhir_fhir_model::choice_types::InferenceRule {
                rule_id: "constraint_matching".to_string(),
                pattern: "constraint_based_boost".to_string(),
                inferred_type: "Quantity".to_string(), // Default inferred type for constraints
                confidence_weight: 0.4,
                applicable_contexts: vec!["constraint_matching".to_string()],
                metadata: HashMap::new(),
            });
        }

        // Rule 3: Pattern analysis based on current path
        let mut pattern_boosts = HashMap::new();

        // Extract property patterns from current path and apply boosts accordingly
        if context.current_path.contains("value") {
            // Get possible value types from available types rather than hardcoding
            for available_type in &context.available_types {
                if [
                    "string",
                    "boolean",
                    "integer",
                    "decimal",
                    "dateTime",
                    "Quantity",
                    "CodeableConcept",
                ]
                .contains(&available_type.as_str())
                {
                    pattern_boosts.insert(available_type.clone(), 0.2);
                }
            }
        }

        if context.current_path.contains("effective") {
            for available_type in &context.available_types {
                if ["dateTime", "Period", "Timing", "instant"].contains(&available_type.as_str()) {
                    pattern_boosts.insert(available_type.clone(), 0.3);
                }
            }
        }

        if context.current_path.contains("onset") {
            for available_type in &context.available_types {
                if ["dateTime", "Age", "Period", "Range", "string"]
                    .contains(&available_type.as_str())
                {
                    pattern_boosts.insert(available_type.clone(), 0.25);
                }
            }
        }

        // Apply pattern boosts to available types only
        for (pattern_type, boost) in pattern_boosts {
            if context.available_types.contains(&pattern_type) {
                if let Some(current_score) = type_scores.get(&pattern_type) {
                    type_scores.insert(pattern_type.clone(), current_score + boost);
                } else {
                    type_scores.insert(pattern_type, boost);
                }
            }
        }

        if !type_scores.is_empty() {
            inference_rules.push(octofhir_fhir_model::choice_types::InferenceRule {
                rule_id: "path_pattern_analysis".to_string(),
                pattern: "path_context_boost".to_string(),
                inferred_type: "CodeableConcept".to_string(), // Default inferred type for patterns
                confidence_weight: 0.3,
                applicable_contexts: vec!["path_patterns".to_string()],
                metadata: HashMap::new(),
            });
        }

        // Build statistical model from computed scores
        let statistical_model = if !type_scores.is_empty() {
            // Normalize scores to probabilities
            let total_score: f64 = type_scores.values().sum();
            let mut probabilities = HashMap::new();
            for (type_name, score) in &type_scores {
                probabilities.insert(type_name.clone(), score / total_score);
            }

            Some(octofhir_fhir_model::choice_types::StatisticalModel {
                model_type: "context_aware_inference".to_string(),
                parameters: probabilities.clone(),
                training_statistics: octofhir_fhir_model::choice_types::TrainingStatistics {
                    sample_count: type_scores.len(),
                    type_frequencies: probabilities.clone(),
                    pattern_success_rates: HashMap::new(),
                    last_training_date: None,
                },
                performance_metrics: HashMap::new(),
            })
        } else {
            None
        };

        // Create inference context from available data
        let mut historical_usage = HashMap::new();
        for available_type in &context.available_types {
            historical_usage.insert(available_type.clone(), 0.5); // Default usage frequency
        }

        let inference_context = octofhir_fhir_model::choice_types::InferenceContext {
            polymorphic_context: Some(context.clone()),
            analyzed_value: None,
            resource_context: Some(context.base_type.clone()),
            historical_usage,
        };

        // Determine confidence threshold based on available types count
        let confidence_threshold = match context.available_types.len() {
            0 => 0.1,
            1 => 0.4,
            2..=3 => 0.6,
            _ => 0.8,
        };

        Ok(TypeInference {
            inference_rules,
            confidence_threshold,
            inference_context,
            statistical_model,
        })
    }

    async fn get_choice_type_definition(
        &self,
        base_path: &str,
    ) -> ModelResult<Option<ChoiceTypeDefinition>> {
        // Check if this is a choice type property
        if !base_path.contains("[x]") {
            return Ok(None);
        }

        let base_property = base_path.replace("[x]", "");
        let path_parts: Vec<&str> = base_path.split('.').collect();

        // Extract resource type from path if available
        let resource_type = path_parts.first().unwrap_or(&"Resource");

        // Try to get schema information for the resource type
        match self.schema_manager.get_schema_by_type(resource_type).await {
            Ok(Some(schema)) => {
                // Look for the choice property in the schema properties
                let mut possible_types = Vec::new();
                let mut constraints = Vec::new();
                let mut expansion_rules = Vec::new();

                // Search through schema properties for choice types
                // Look for properties that follow the pattern [property][Type]
                for (property_name, property) in &schema.properties {
                    if property_name.starts_with(&base_property)
                        && *property_name != base_property
                        && property_name.len() > base_property.len()
                    {
                        // Extract the type from the property name
                        let type_suffix = &property_name[base_property.len()..];

                        // Add as a possible type if it looks like a valid type
                        if type_suffix.chars().next().is_some_and(|c| c.is_uppercase()) {
                            let expanded_property = format!("{base_property}{type_suffix}");
                            possible_types.push(octofhir_fhir_model::choice_types::ChoiceTypeOption {
                                type_name: type_suffix.to_string(),
                                expanded_property,
                                type_info: octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type("FHIR", type_suffix),
                                usage_frequency: 0.5, // Default frequency
                                compatibility_rules: Vec::new(),
                            });
                        }

                        // Extract constraints from property definition
                        for constraint in &property.constraints {
                            constraints.push(octofhir_fhir_model::choice_types::ChoiceConstraint {
                                constraint_id: constraint.key.clone(),
                                constraint_type: match constraint.severity {
                                    crate::types::schema::ConstraintSeverity::Error => octofhir_fhir_model::choice_types::ChoiceConstraintType::Cardinality,
                                    crate::types::schema::ConstraintSeverity::Warning => octofhir_fhir_model::choice_types::ChoiceConstraintType::ContextSpecific,
                                    crate::types::schema::ConstraintSeverity::Information => octofhir_fhir_model::choice_types::ChoiceConstraintType::TypeHierarchy,
                                },
                                expression: constraint.expression.clone().unwrap_or_else(|| constraint.human.clone()),
                                error_message: constraint.human.clone(),
                                applicable_contexts: vec!["FHIR".to_string()],
                            });
                        }
                    }
                }

                // If we found choice type properties, they're already in ChoiceTypeOption format
                if possible_types.is_empty() {
                    // Try to get types from type resolver if available
                    let context = ResolutionContext::new(resource_type);
                    if let Ok(resolved_types) = self
                        .type_resolver
                        .resolve_choice_type(
                            resource_type,
                            &format!("{base_property}[x]"),
                            &context,
                        )
                        .await
                    {
                        possible_types = resolved_types.into_iter().map(|rt| {
                            let expanded_property = format!("{}{}", base_property, rt.type_name);
                            octofhir_fhir_model::choice_types::ChoiceTypeOption {
                                type_name: rt.type_name.clone(),
                                expanded_property,
                                type_info: octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type("FHIR", &rt.type_name),
                                usage_frequency: 0.5,
                                compatibility_rules: Vec::new(),
                            }
                        }).collect();
                    }
                }

                // Create expansion rules based on FHIR conventions
                expansion_rules.push(octofhir_fhir_model::choice_types::ExpansionRule {
                    rule_id: "naming_convention".to_string(),
                    source_pattern: format!("{base_property}[x]"),
                    target_pattern: format!("{base_property}{{type}}"),
                    priority: 100,
                    applicable_contexts: vec!["FHIR".to_string()],
                });

                expansion_rules.push(octofhir_fhir_model::choice_types::ExpansionRule {
                    rule_id: "type_casing".to_string(),
                    source_pattern: "[x]".to_string(),
                    target_pattern: "{TypeName}".to_string(),
                    priority: 90,
                    applicable_contexts: vec!["FHIR".to_string()],
                });

                // Create resolution metadata with schema information
                let mut performance_hints = HashMap::new();
                performance_hints.insert("schema_source".to_string(), "fhir_schema".to_string());
                performance_hints
                    .insert("types_found".to_string(), possible_types.len().to_string());

                let resolution_metadata =
                    octofhir_fhir_model::choice_types::ChoiceResolutionMetadata {
                        default_strategy:
                            octofhir_fhir_model::choice_types::ResolutionStrategy::ContextAware,
                        confidence_threshold: if !possible_types.is_empty() { 0.9 } else { 0.5 },
                        allow_ambiguous: true,
                        fallback_type: None,
                        performance_hints,
                    };

                Ok(Some(ChoiceTypeDefinition {
                    base_path: base_property,
                    choice_property: base_path.to_string(),
                    possible_types,
                    expansion_rules,
                    resolution_metadata,
                    constraints,
                }))
            }
            Ok(None) => {
                // No schema found, try using the type resolver to get dynamic types
                let base_property = base_path.replace("[x]", "");
                let context = ResolutionContext::new(resource_type);

                let mut possible_types = Vec::new();
                if let Ok(resolved_types) = self
                    .type_resolver
                    .resolve_choice_type(resource_type, &format!("{base_property}[x]"), &context)
                    .await
                {
                    possible_types = resolved_types.into_iter().map(|rt| {
                        let expanded_property = format!("{}{}", base_property, rt.type_name);
                        octofhir_fhir_model::choice_types::ChoiceTypeOption {
                            type_name: rt.type_name.clone(),
                            expanded_property,
                            type_info: octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type("FHIR", &rt.type_name),
                            usage_frequency: 0.5,
                            compatibility_rules: Vec::new(),
                        }
                    }).collect();
                }

                let expansion_rules = vec![octofhir_fhir_model::choice_types::ExpansionRule {
                    rule_id: "fallback_naming_convention".to_string(),
                    source_pattern: format!("{base_property}[x]"),
                    target_pattern: format!("{base_property}{{type}}"),
                    priority: 50,
                    applicable_contexts: vec!["fallback".to_string()],
                }];

                let mut performance_hints = HashMap::new();
                performance_hints.insert("fallback_resolution".to_string(), "true".to_string());
                performance_hints.insert("base_property".to_string(), base_property.clone());
                performance_hints.insert(
                    "types_from_resolver".to_string(),
                    possible_types.len().to_string(),
                );

                let resolution_metadata =
                    octofhir_fhir_model::choice_types::ChoiceResolutionMetadata {
                        default_strategy:
                            octofhir_fhir_model::choice_types::ResolutionStrategy::FirstMatch,
                        confidence_threshold: if !possible_types.is_empty() { 0.7 } else { 0.1 },
                        allow_ambiguous: false,
                        fallback_type: Some("string".to_string()),
                        performance_hints,
                    };

                Ok(Some(ChoiceTypeDefinition {
                    base_path: base_property,
                    choice_property: base_path.to_string(),
                    possible_types,
                    expansion_rules,
                    resolution_metadata,
                    constraints: Vec::new(),
                }))
            }
            Err(e) => Err(ModelError::generic(format!(
                "Failed to retrieve schema: {e}"
            ))),
        }
    }

    // ========================================================================
    // FHIRPath Functions
    // ========================================================================

    async fn conforms_to_profile(&self, profile_url: &str) -> ModelResult<ConformanceResult> {
        Ok(ConformanceResult::new(
            profile_url,
            "FhirSchemaModelProvider",
        ))
    }

    async fn analyze_expression_types(
        &self,
        expression: &str,
    ) -> ModelResult<ExpressionTypeAnalysis> {
        Ok(ExpressionTypeAnalysis::new(expression))
    }

    async fn validate_fhirpath_expression(
        &self,
        _expression: &str,
        _base_type: &str,
    ) -> ModelResult<TypeCheckResult> {
        Ok(TypeCheckResult::success())
    }

    async fn get_expression_dependencies(
        &self,
        _expression: &str,
    ) -> ModelResult<Vec<TypeDependency>> {
        Ok(Vec::new())
    }

    // ========================================================================
    // Advanced Operations
    // ========================================================================

    async fn get_collection_semantics(&self, type_name: &str) -> ModelResult<CollectionSemantics> {
        use octofhir_fhir_model::type_system::{EmptyBehavior, IndexingType, SingletonEvaluation};

        // Get type hierarchy to understand the type structure
        match self.get_type_hierarchy(type_name).await {
            Ok(Some(hierarchy)) => {
                // Analyze the type to determine collection semantics
                let is_array_type = type_name.contains("[]")
                    || type_name.ends_with("List")
                    || type_name.ends_with("Collection");

                // Check if this type has properties that indicate collection behavior
                let has_array_properties = hierarchy.properties.values().any(|prop| {
                    prop.cardinality.max.is_none() || prop.cardinality.max.unwrap_or(1) > 1
                });

                // FHIR-specific collection semantics
                let collection_semantics = if is_array_type || has_array_properties {
                    // Array types or types with array properties
                    CollectionSemantics {
                        is_ordered: true,                         // FHIR collections maintain order
                        allows_duplicates: true, // FHIR generally allows duplicates
                        indexing_type: IndexingType::ZeroBased, // FHIRPath uses 0-based indexing
                        empty_behavior: EmptyBehavior::Propagate, // Empty collections propagate
                        singleton_evaluation: SingletonEvaluation::Automatic, // Auto-unwrap singletons
                    }
                } else {
                    // Scalar types
                    CollectionSemantics {
                        is_ordered: false,                          // Single values don't have order
                        allows_duplicates: false, // Single values can't have duplicates
                        indexing_type: IndexingType::NotIndexable, // Scalars aren't indexable
                        empty_behavior: EmptyBehavior::TreatAsNull, // Empty scalars are null
                        singleton_evaluation: SingletonEvaluation::Explicit, // Scalars require explicit handling
                    }
                };

                // Override semantics for specific FHIR types
                let adjusted_semantics = match type_name {
                    // Bundle entries are ordered and indexable
                    "Bundle" => CollectionSemantics {
                        is_ordered: true,
                        allows_duplicates: true,
                        indexing_type: IndexingType::ZeroBased,
                        empty_behavior: EmptyBehavior::Propagate,
                        singleton_evaluation: SingletonEvaluation::Automatic,
                    },
                    // CodeableConcepts allow multiple codings
                    "CodeableConcept" => CollectionSemantics {
                        is_ordered: true,
                        allows_duplicates: false, // Usually unique codings
                        indexing_type: IndexingType::ZeroBased,
                        empty_behavior: EmptyBehavior::Propagate,
                        singleton_evaluation: SingletonEvaluation::Automatic,
                    },
                    // Identifiers in lists
                    "Identifier" => CollectionSemantics {
                        is_ordered: true,
                        allows_duplicates: false, // Identifiers should be unique
                        indexing_type: IndexingType::ZeroBased,
                        empty_behavior: EmptyBehavior::Propagate,
                        singleton_evaluation: SingletonEvaluation::Automatic,
                    },
                    // Primitive types are scalars
                    "boolean" | "integer" | "decimal" | "string" | "date" | "dateTime" | "time" => {
                        CollectionSemantics {
                            is_ordered: false,
                            allows_duplicates: false,
                            indexing_type: IndexingType::NotIndexable,
                            empty_behavior: EmptyBehavior::TreatAsNull,
                            singleton_evaluation: SingletonEvaluation::Explicit,
                        }
                    }
                    _ => collection_semantics,
                };

                Ok(adjusted_semantics)
            }
            Ok(None) => {
                // Unknown type - use conservative defaults
                Ok(CollectionSemantics {
                    is_ordered: true,
                    allows_duplicates: true,
                    indexing_type: IndexingType::ZeroBased,
                    empty_behavior: EmptyBehavior::Propagate,
                    singleton_evaluation: SingletonEvaluation::Automatic,
                })
            }
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }

    async fn get_optimization_hints(
        &self,
        _expression: &str,
    ) -> ModelResult<Vec<ModelOptimizationHint>> {
        Ok(Vec::new())
    }

    async fn clear_caches(&self) -> ModelResult<()> {
        match self.clear_caches().await {
            Ok(_) => Ok(()),
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }

    // ========================================================================
    // Core Information Methods
    // ========================================================================

    async fn get_type_reflection(
        &self,
        type_name: &str,
    ) -> ModelResult<Option<TypeReflectionInfo>> {
        match self.get_type_hierarchy(type_name).await {
            Ok(Some(hierarchy)) => {
                // Create ClassInfo with elements from the schema for proper polymorphic resolution
                let elements = self.convert_hierarchy_to_elements(&hierarchy).await?;
                Ok(Some(TypeReflectionInfo::class_type(
                    "FHIR", type_name, elements,
                )))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }

    async fn get_constraints(&self, type_name: &str) -> ModelResult<Vec<ModelConstraintInfo>> {
        match self.get_type_hierarchy(type_name).await {
            Ok(Some(hierarchy)) => {
                let constraints = hierarchy
                    .constraints
                    .into_iter()
                    .map(|c| ModelConstraintInfo {
                        key: c.key,
                        severity: octofhir_fhir_model::constraints::ConstraintSeverity::Error,
                        human: c.human,
                        expression: c.expression.unwrap_or_default(), // Handle Option<String>
                        xpath: None,                                  // No xpath for now
                        source: None,                                 // No source for now
                        metadata: HashMap::new(),                     // Empty metadata
                    })
                    .collect();
                Ok(constraints)
            }
            Ok(None) => Ok(Vec::new()),
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }

    fn get_fhir_version(&self) -> ModelProviderFhirVersion {
        match self.fhir_version {
            FhirVersion::R4 => ModelProviderFhirVersion::R4,
            FhirVersion::R5 => ModelProviderFhirVersion::R5,
            FhirVersion::R4B => ModelProviderFhirVersion::R4B,
            FhirVersion::R6 => ModelProviderFhirVersion::R5, // Map R6 to R5 for now
        }
    }

    async fn get_supported_resource_types(&self) -> ModelResult<Vec<String>> {
        // IMPORTANT: Return resource types extracted from converted FHIR schemas, not hardcoded list
        Ok(self.get_available_resource_types())
    }

    fn resource_type_exists(&self, resource_type: &str) -> ModelResult<bool> {
        // IMPORTANT: O(1) check using papaya HashMap - data extracted from schemas, not hardcoded
        Ok(self.resource_type_exists(resource_type))
    }

    async fn refresh_resource_types(&self) -> ModelResult<()> {
        // IMPORTANT: Re-extract resource types from current schema storage
        match self.refresh_resource_types().await {
            Ok(_) => Ok(()),
            Err(e) => Err(ModelError::generic(e.to_string())),
        }
    }
}

impl FhirSchemaModelProvider {
    /// Convert internal TypeHierarchy to fhir-model-rs TypeHierarchy
    async fn convert_to_model_type_hierarchy(
        &self,
        hierarchy: TypeHierarchy,
    ) -> ModelResult<ModelTypeHierarchy> {
        Ok(ModelTypeHierarchy {
            type_name: hierarchy.base_type,
            ancestors: hierarchy
                .parent_type
                .as_ref()
                .into_iter()
                .cloned()
                .collect(),
            descendants: Vec::new(), // TODO: Collect all descendants
            direct_parent: hierarchy.parent_type,
            direct_children: hierarchy.child_types,
            is_abstract: false, // TODO: Determine if abstract
            derivation: octofhir_fhir_model::type_system::DerivationType::Specialization, // Default derivation
            hierarchy_depth: 1, // TODO: Calculate actual depth
        })
    }

    /// Convert TypeHierarchy to ElementInfo vector for ClassInfo creation
    async fn convert_hierarchy_to_elements(
        &self,
        hierarchy: &TypeHierarchy,
    ) -> ModelResult<Vec<octofhir_fhir_model::reflection::ElementInfo>> {
        let mut elements = Vec::new();

        // Convert properties from hierarchy to ElementInfo
        for (property_name, property_info) in &hierarchy.properties {
            if property_info.is_choice_type {
                // For choice types, create elements for all concrete choice variants
                // The polymorphic resolution logic expects concrete property names like "valueQuantity", "valueString", etc.
                for choice_variant in &property_info.choice_types {
                    let element = octofhir_fhir_model::reflection::ElementInfo::new(
                        choice_variant,
                        octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type(
                            "FHIR", "Element",
                        ),
                    )
                    .with_cardinality(property_info.cardinality.min, property_info.cardinality.max);

                    elements.push(element);
                }
            } else {
                // Regular non-choice property
                let element = octofhir_fhir_model::reflection::ElementInfo::new(
                    property_name,
                    octofhir_fhir_model::reflection::TypeReflectionInfo::simple_type(
                        "FHIR",
                        &property_info.property_type,
                    ),
                )
                .with_cardinality(property_info.cardinality.min, property_info.cardinality.max);

                elements.push(element);
            }
        }

        Ok(elements)
    }
}
