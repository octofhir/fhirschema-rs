// Advanced type resolution system with choice type handling and caching

use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::core::{ResolutionContext, ResolvedType, TypeInfo};
use crate::error::{FhirSchemaError, Result};
use crate::types::choice_types::ChoiceTypeResolver;
use crate::types::type_hierarchy::TypeHierarchyBuilder;

// Debug impl manually provided below due to CanonicalManager not implementing Debug
pub struct TypeResolver {
    // LRU cache for resolved types
    type_cache: Arc<RwLock<LruCache<String, TypeInfo>>>,

    // Choice type resolver for polymorphic elements
    choice_resolver: Arc<ChoiceTypeResolver>,

    // Type hierarchy builder for complex type relationships
    hierarchy_builder: Arc<TypeHierarchyBuilder>,

    // Canonical manager for accessing FHIR definitions
    #[allow(dead_code)]
    canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,

    // Cache configuration
    cache_size: usize,
}

impl TypeResolver {
    pub async fn new(
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
    ) -> Result<Self> {
        let cache_size = NonZeroUsize::new(10000)
            .ok_or_else(|| FhirSchemaError::configuration_error("Cache size cannot be zero"))?;

        Ok(Self {
            type_cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
            choice_resolver: Arc::new(
                ChoiceTypeResolver::new(Arc::clone(&canonical_manager)).await?,
            ),
            hierarchy_builder: Arc::new(
                TypeHierarchyBuilder::new(Arc::clone(&canonical_manager)).await?,
            ),
            canonical_manager,
            cache_size: cache_size.get(),
        })
    }

    /// Create with custom cache size
    pub async fn with_cache_size(
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
        cache_size: usize,
    ) -> Result<Self> {
        let cache_size = NonZeroUsize::new(cache_size)
            .ok_or_else(|| FhirSchemaError::configuration_error("Cache size cannot be zero"))?;

        Ok(Self {
            type_cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
            choice_resolver: Arc::new(
                ChoiceTypeResolver::new(Arc::clone(&canonical_manager)).await?,
            ),
            hierarchy_builder: Arc::new(
                TypeHierarchyBuilder::new(Arc::clone(&canonical_manager)).await?,
            ),
            canonical_manager,
            cache_size: cache_size.get(),
        })
    }

    /// Resolve choice type with advanced context-aware resolution and caching
    pub async fn resolve_choice_type(
        &self,
        base_type: &str,
        choice_suffix: &str,
        context: &ResolutionContext,
    ) -> Result<Vec<ResolvedType>> {
        let cache_key = format!("choice:{}:{}:{}", base_type, choice_suffix, context.hash());

        // Check cache first
        {
            let mut cache = self.type_cache.write().await;
            if let Some(cached_info) = cache.get(&cache_key) {
                // Check if cache entry is still fresh (within 1 hour)
                if cached_info.timestamp.elapsed().as_secs() < 3600 {
                    return Ok(cached_info.resolved_types.clone());
                }
            }
        }

        // Resolve using the choice resolver
        let resolved_types = self
            .choice_resolver
            .resolve_with_context(base_type, choice_suffix, context)
            .await?;

        // Cache the result
        {
            let mut cache = self.type_cache.write().await;
            cache.put(
                cache_key,
                TypeInfo {
                    resolved_types: resolved_types.clone(),
                    timestamp: Instant::now(),
                },
            );
        }

        Ok(resolved_types)
    }

    /// Resolve a single type with hierarchy information
    pub async fn resolve_type(
        &self,
        type_name: &str,
        context: &ResolutionContext,
    ) -> Result<ResolvedType> {
        let cache_key = format!("type:{}:{}", type_name, context.hash());

        // Check cache first
        {
            let mut cache = self.type_cache.write().await;
            if let Some(cached_info) = cache.get(&cache_key) {
                if let Some(resolved_type) = cached_info.resolved_types.first() {
                    if cached_info.timestamp.elapsed().as_secs() < 3600 {
                        return Ok(resolved_type.clone());
                    }
                }
            }
        }

        // Resolve the type
        let resolved_type = self.resolve_type_internal(type_name, context).await?;

        // Cache the result
        {
            let mut cache = self.type_cache.write().await;
            cache.put(
                cache_key,
                TypeInfo {
                    resolved_types: vec![resolved_type.clone()],
                    timestamp: Instant::now(),
                },
            );
        }

        Ok(resolved_type)
    }

    /// Internal type resolution logic
    async fn resolve_type_internal(
        &self,
        type_name: &str,
        context: &ResolutionContext,
    ) -> Result<ResolvedType> {
        // Check if it's a primitive type
        if self.is_primitive_type(type_name) {
            return Ok(ResolvedType {
                type_name: type_name.to_string(),
                is_primitive: true,
                is_complex: false,
                is_resource: false,
                base_type: None,
                constraints: Vec::new(),
                metadata: HashMap::new(),
            });
        }

        // Check if it's a known resource type
        if self.is_resource_type(type_name).await? {
            return Ok(ResolvedType {
                type_name: type_name.to_string(),
                is_primitive: false,
                is_complex: true,
                is_resource: true,
                base_type: Some("Resource".to_string()),
                constraints: Vec::new(),
                metadata: HashMap::new(),
            });
        }

        // Try to resolve as complex type using canonical manager
        match self.resolve_complex_type(type_name, context).await {
            Ok(resolved_type) => Ok(resolved_type),
            Err(_) => {
                // Fallback to unknown type
                Ok(ResolvedType {
                    type_name: type_name.to_string(),
                    is_primitive: false,
                    is_complex: true,
                    is_resource: false,
                    base_type: None,
                    constraints: Vec::new(),
                    metadata: {
                        let mut meta = HashMap::new();
                        meta.insert(
                            "resolution_status".to_string(),
                            serde_json::Value::String("unresolved".to_string()),
                        );
                        meta
                    },
                })
            }
        }
    }

    /// Resolve complex type using canonical manager
    async fn resolve_complex_type(
        &self,
        type_name: &str,
        _context: &ResolutionContext,
    ) -> Result<ResolvedType> {
        // Try to get the StructureDefinition for this type
        let url = format!("http://hl7.org/fhir/StructureDefinition/{type_name}");

        // This is a placeholder - in a real implementation, we would query the canonical manager
        // for the StructureDefinition and extract type information
        Ok(ResolvedType {
            type_name: type_name.to_string(),
            is_primitive: false,
            is_complex: true,
            is_resource: false,
            base_type: Some("Element".to_string()),
            constraints: Vec::new(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert(
                    "structure_definition_url".to_string(),
                    serde_json::Value::String(url),
                );
                meta
            },
        })
    }

    /// Check if a type is a FHIR primitive type
    fn is_primitive_type(&self, type_name: &str) -> bool {
        matches!(
            type_name,
            "boolean"
                | "integer"
                | "integer64"
                | "decimal"
                | "string"
                | "uri"
                | "url"
                | "canonical"
                | "base64Binary"
                | "instant"
                | "date"
                | "dateTime"
                | "time"
                | "code"
                | "oid"
                | "id"
                | "markdown"
                | "unsignedInt"
                | "positiveInt"
                | "uuid"
        )
    }

    /// Check if a type is a FHIR resource type
    async fn is_resource_type(&self, type_name: &str) -> Result<bool> {
        // Common FHIR resource types
        let common_resources = [
            "Patient",
            "Practitioner",
            "Organization",
            "Location",
            "Observation",
            "Condition",
            "Procedure",
            "MedicationRequest",
            "DiagnosticReport",
            "Encounter",
            "Bundle",
            "OperationOutcome",
            "CapabilityStatement",
        ];

        if common_resources.contains(&type_name) {
            return Ok(true);
        }

        // For other types, we could query the canonical manager
        // This is a placeholder for now
        Ok(false)
    }

    /// Resolve types in batch for efficiency
    pub async fn resolve_types_batch(
        &self,
        type_requests: Vec<(String, ResolutionContext)>,
    ) -> Result<Vec<ResolvedType>> {
        let mut results = Vec::new();

        for (type_name, context) in type_requests {
            match self.resolve_type(&type_name, &context).await {
                Ok(resolved_type) => results.push(resolved_type),
                Err(e) => {
                    // Log error but continue with other types
                    eprintln!("Failed to resolve type '{type_name}': {e}");

                    // Push a fallback type
                    results.push(ResolvedType {
                        type_name: type_name.clone(),
                        is_primitive: false,
                        is_complex: true,
                        is_resource: false,
                        base_type: None,
                        constraints: Vec::new(),
                        metadata: {
                            let mut meta = HashMap::new();
                            meta.insert(
                                "resolution_error".to_string(),
                                serde_json::Value::String(e.to_string()),
                            );
                            meta
                        },
                    });
                }
            }
        }

        Ok(results)
    }

    /// Get type hierarchy for a given type
    pub async fn get_type_hierarchy(
        &self,
        type_name: &str,
        context: &ResolutionContext,
    ) -> Result<Vec<String>> {
        self.hierarchy_builder
            .build_hierarchy(type_name, context)
            .await
    }

    /// Check if one type is compatible with another (subtype relationship)
    pub async fn is_compatible_type(
        &self,
        source_type: &str,
        target_type: &str,
        context: &ResolutionContext,
    ) -> Result<bool> {
        // Same type is always compatible
        if source_type == target_type {
            return Ok(true);
        }

        // Get hierarchies for both types
        let source_hierarchy = self.get_type_hierarchy(source_type, context).await?;

        // Check if target_type is in the source type's hierarchy
        Ok(source_hierarchy.contains(&target_type.to_string()))
    }

    /// Clear the type cache
    pub async fn clear_cache(&self) {
        let mut cache = self.type_cache.write().await;
        cache.clear();
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.type_cache.read().await;
        (cache.len(), self.cache_size)
    }

    /// Invalidate cache entries for a specific type
    pub async fn invalidate_type(&self, type_name: &str) {
        let mut cache = self.type_cache.write().await;
        let keys_to_remove: Vec<String> = cache
            .iter()
            .filter_map(|(key, _)| {
                if key.contains(type_name) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            cache.pop(&key);
        }
    }

    /// Preload common types into cache
    pub async fn preload_common_types(&self) -> Result<()> {
        let common_types = [
            "string",
            "boolean",
            "integer",
            "decimal",
            "uri",
            "code",
            "id",
            "Patient",
            "Practitioner",
            "Organization",
            "Observation",
            "Reference",
            "Extension",
            "Meta",
            "Narrative",
            "Element",
        ];

        let context = ResolutionContext::new("preload");

        for type_name in &common_types {
            let _ = self.resolve_type(type_name, &context).await;
        }

        Ok(())
    }
}

impl TypeResolver {
    /// Create a new TypeResolver with provided canonical manager
    pub fn with_canonical_manager(
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
    ) -> Self {
        Self {
            type_cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))),
            choice_resolver: Arc::new(ChoiceTypeResolver::new_sync(canonical_manager.clone())),
            hierarchy_builder: Arc::new(TypeHierarchyBuilder::with_canonical_manager(
                canonical_manager.clone(),
            )),
            canonical_manager,
            cache_size: 1000,
        }
    }
}

impl std::fmt::Debug for TypeResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeResolver")
            .field("cache_size", &self.cache_size)
            .field("canonical_manager", &"<CanonicalManager>")
            .finish()
    }
}
