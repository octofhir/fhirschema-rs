use std::sync::Arc;

use crate::conversion::ConversionEngine;
use crate::core::{ConversionResult, FhirSchemaConfig, ValidationResult};
use crate::error::Result;
use crate::storage::{SchemaCache, SchemaStorage};
use crate::types::{FhirSchema, PathNavigator, TypeResolver};
use crate::validation::ValidationEngine;

pub struct FhirSchemaManager {
    canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
    conversion_engine: Arc<ConversionEngine>,
    validation_engine: Arc<ValidationEngine>,
    storage: Arc<dyn SchemaStorage>,
    cache: Arc<SchemaCache>,
    type_resolver: Arc<TypeResolver>,
    path_navigator: Arc<PathNavigator>,
    config: FhirSchemaConfig,
}

impl FhirSchemaManager {
    pub async fn new(
        config: FhirSchemaConfig,
        canonical_manager: octofhir_canonical_manager::CanonicalManager,
    ) -> Result<Self> {
        let canonical_manager = Arc::new(canonical_manager);

        let storage: Arc<dyn SchemaStorage> = Arc::new(crate::storage::MemoryStorage::new());

        let cache = Arc::new(SchemaCache::new(&config.cache_config).await?);

        // Initialize advanced type system components
        let type_resolver = Arc::new(TypeResolver::new(Arc::clone(&canonical_manager)).await?);

        let path_navigator = Arc::new(
            PathNavigator::new(Arc::clone(&type_resolver), Arc::clone(&canonical_manager)).await?,
        );

        let conversion_engine = Arc::new(
            ConversionEngine::new(Arc::clone(&canonical_manager), &config.performance_config)
                .await?,
        );

        let validation_engine = Arc::new(
            ValidationEngine::new(
                Arc::clone(&canonical_manager),
                Arc::clone(&storage),
                &config.performance_config,
            )
            .await?,
        );

        Ok(Self {
            canonical_manager,
            conversion_engine,
            validation_engine,
            storage,
            cache,
            type_resolver,
            path_navigator,
            config,
        })
    }

    pub async fn convert_structure_definition(
        &self,
        structure_def: serde_json::Value,
    ) -> Result<ConversionResult> {
        self.conversion_engine.convert_single(structure_def).await
    }

    pub async fn convert_batch(
        &self,
        structure_defs: Vec<serde_json::Value>,
    ) -> Result<Vec<ConversionResult>> {
        self.conversion_engine.convert_batch(structure_defs).await
    }

    pub async fn get_schema(&self, url: &str) -> Result<Option<FhirSchema>> {
        if let Some(cached) = self.cache.get(url).await {
            return Ok(Some(cached));
        }

        if let Some(schema) = self.storage.get_schema(url).await? {
            self.cache.insert(url, &schema).await?;
            return Ok(Some(schema));
        }

        Ok(None)
    }

    pub async fn get_schema_by_type(&self, type_name: &str) -> Result<Option<FhirSchema>> {
        let url = format!("http://hl7.org/fhir/StructureDefinition/{type_name}");
        self.get_schema(&url).await
    }

    pub async fn store_schema(&self, url: &str, schema: FhirSchema) -> Result<()> {
        self.storage.store_schema(url, schema.clone()).await?;
        self.cache.insert(url, &schema).await?;
        Ok(())
    }

    pub async fn validate_resource<T: serde::Serialize>(
        &self,
        resource: &T,
        schema_url: Option<&str>,
    ) -> Result<ValidationResult> {
        if !self.config.enable_validation {
            return Ok(ValidationResult::valid());
        }

        self.validation_engine
            .validate_resource(resource, schema_url)
            .await
    }

    pub async fn validate_batch<T: serde::Serialize>(
        &self,
        resources: Vec<T>,
        schema_url: Option<&str>,
    ) -> Result<Vec<ValidationResult>> {
        if !self.config.enable_validation {
            return Ok(resources
                .iter()
                .map(|_| ValidationResult::valid())
                .collect());
        }

        self.validation_engine
            .validate_batch(resources, schema_url)
            .await
    }

    pub async fn list_schemas(&self) -> Result<Vec<String>> {
        self.storage.list_schemas().await
    }

    pub async fn delete_schema(&self, url: &str) -> Result<()> {
        self.storage.delete_schema(url).await?;
        self.cache.remove(url).await;
        Ok(())
    }

    pub async fn clear_cache(&self) -> Result<()> {
        self.cache.clear().await;
        Ok(())
    }

    pub fn config(&self) -> &FhirSchemaConfig {
        &self.config
    }

    pub fn canonical_manager(&self) -> &octofhir_canonical_manager::CanonicalManager {
        &self.canonical_manager
    }

    /// Access the type resolver for advanced type operations
    pub fn type_resolver(&self) -> &TypeResolver {
        &self.type_resolver
    }

    /// Access the path navigator for FHIR path operations
    pub fn path_navigator(&self) -> &PathNavigator {
        &self.path_navigator
    }

    /// Resolve choice types with context
    pub async fn resolve_choice_type(
        &self,
        base_type: &str,
        choice_suffix: &str,
        context: &crate::core::ResolutionContext,
    ) -> Result<Vec<crate::core::ResolvedType>> {
        self.type_resolver
            .resolve_choice_type(base_type, choice_suffix, context)
            .await
    }

    /// Navigate and validate FHIR paths
    pub async fn navigate_path(
        &self,
        path: &str,
        context: &crate::core::ResolutionContext,
    ) -> Result<crate::types::PathNavigationResult> {
        self.path_navigator.navigate_path(path, context).await
    }

    /// Infer type from element name and context
    pub async fn infer_element_type(
        &self,
        parent_type: &str,
        element_name: &str,
        context: &crate::core::ResolutionContext,
    ) -> Result<String> {
        self.path_navigator
            .infer_element_type(parent_type, element_name, context)
            .await
    }

    /// Check type compatibility (subtype relationships)
    pub async fn is_compatible_type(
        &self,
        source_type: &str,
        target_type: &str,
        context: &crate::core::ResolutionContext,
    ) -> Result<bool> {
        self.type_resolver
            .is_compatible_type(source_type, target_type, context)
            .await
    }

    /// Get type hierarchy for a given type
    pub async fn get_type_hierarchy(
        &self,
        type_name: &str,
        context: &crate::core::ResolutionContext,
    ) -> Result<Vec<String>> {
        self.type_resolver
            .get_type_hierarchy(type_name, context)
            .await
    }

    /// Clear all caches (including type system caches)
    pub async fn clear_all_caches(&self) -> Result<()> {
        self.cache.clear().await;
        self.type_resolver.clear_cache().await;
        self.path_navigator.clear_caches().await;
        Ok(())
    }

    /// Get comprehensive cache statistics
    pub async fn get_cache_stats(&self) -> Result<serde_json::Value> {
        let type_resolver_stats = self.type_resolver.get_cache_stats().await;
        let path_navigator_stats = self.path_navigator.get_navigation_stats().await;

        Ok(serde_json::json!({
            "type_resolver": {
                "entries": type_resolver_stats.0,
                "capacity": type_resolver_stats.1
            },
            "path_navigator": {
                "navigation_entries": path_navigator_stats.0,
                "inference_entries": path_navigator_stats.1
            }
        }))
    }
}

impl std::fmt::Debug for FhirSchemaManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FhirSchemaManager")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
