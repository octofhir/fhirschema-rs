use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::package::registry::SchemaIndex;
use crate::types::Element;
use crate::{ChoiceTypeExpander, ConverterConfig, Result};

/// High-performance path resolver with caching
pub struct PathResolver {
    /// Schema index for type lookups
    schema_index: Arc<SchemaIndex>,

    /// Cache for resolved paths (path_key -> resolution result)
    resolution_cache: Arc<RwLock<HashMap<PathQuery, PathResolution>>>,

    /// Pre-computed common paths for O(1) access
    common_paths: Arc<RwLock<HashMap<String, PathMetadata>>>,

    /// Statistics for cache performance
    metrics: Arc<RwLock<PathResolverMetrics>>,

    /// Choice type expander for handling choice types
    choice_expander: ChoiceTypeExpander,
}

/// Path query for caching
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PathQuery {
    pub base_type: String,
    pub path: String,
    pub context: Option<String>, // For context-dependent resolution
}

/// Path resolution result
#[derive(Debug, Clone)]
pub struct PathResolution {
    pub target_type: String,
    pub is_collection: bool,
    pub cardinality: PathCardinality,
    pub element_info: ElementInfo,
    pub resolution_steps: Vec<PathStep>,
    pub is_choice_type: bool,
    pub choice_info: Option<ChoiceTypeInfo>,
}

/// Individual step in path resolution
#[derive(Debug, Clone)]
pub struct PathStep {
    pub segment: String,
    pub from_type: String,
    pub to_type: String,
    pub is_collection: bool,
    pub element_name: String,
}

/// Pre-computed path metadata
#[derive(Debug, Clone)]
pub struct PathMetadata {
    pub path: String,
    pub target_type: String,
    pub depth: u32,
    pub is_common: bool,
    pub usage_frequency: f64,
}

/// Performance metrics
#[derive(Debug, Default)]
pub struct PathResolverMetrics {
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_resolutions: u64,
    pub avg_resolution_time_ns: u64,
    pub max_path_depth: u32,
}

/// Cardinality information for path resolution
#[derive(Debug, Clone)]
pub struct PathCardinality {
    pub min: u32,
    pub max: Option<u32>, // None for unbounded (*)
}

/// Element information for path resolution
#[derive(Debug, Clone)]
pub struct ElementInfo {
    pub path: String,
    pub element_type: String,
    pub cardinality: PathCardinality,
    pub is_collection: bool,
    pub is_choice_type: bool,
    pub choice_type_options: Option<Vec<String>>,
    pub constraints: Vec<ConstraintInfo>,
    pub binding: Option<BindingInfo>,
    pub definition: Option<String>,
    pub short_description: Option<String>,
}

/// Constraint information
#[derive(Debug, Clone)]
pub struct ConstraintInfo {
    pub key: String,
    pub severity: String,
    pub human_description: String,
    pub fhirpath_expression: String,
    pub source: Option<String>,
}

/// Binding information
#[derive(Debug, Clone)]
pub struct BindingInfo {
    pub strength: String,
    pub value_set: Option<String>,
    pub description: Option<String>,
}

/// Choice type information
#[derive(Debug, Clone)]
pub struct ChoiceTypeInfo {
    pub base_path: String,
    pub expanded_path: String,
    pub actual_type: String,
}

impl PathResolver {
    pub fn new(schema_index: Arc<SchemaIndex>) -> Self {
        Self {
            schema_index,
            resolution_cache: Arc::new(RwLock::new(HashMap::new())),
            common_paths: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(PathResolverMetrics::default())),
            choice_expander: ChoiceTypeExpander::new(&ConverterConfig::default()),
        }
    }

    /// Resolve a path with full caching
    pub async fn resolve_path(&self, base_type: &str, path: &str) -> Option<PathResolution> {
        let query = PathQuery {
            base_type: base_type.to_string(),
            path: path.to_string(),
            context: None,
        };

        self.resolve_with_cache(&query).await
    }

    /// Resolve path with context (e.g., for where() clauses)
    pub async fn resolve_path_with_context(
        &self,
        base_type: &str,
        path: &str,
        context: &str,
    ) -> Option<PathResolution> {
        let query = PathQuery {
            base_type: base_type.to_string(),
            path: path.to_string(),
            context: Some(context.to_string()),
        };

        self.resolve_with_cache(&query).await
    }

    /// Get all available paths for a type (for autocomplete)
    pub async fn get_available_paths(&self, type_name: &str) -> Vec<String> {
        let Some(schema) = self.schema_index.get_schema_by_type(type_name).await else {
            return Vec::new();
        };

        let mut paths = Vec::new();
        for element_path in schema.elements.keys() {
            // Convert internal paths to FHIRPath syntax
            if let Some(fhirpath) = self.convert_to_fhirpath(element_path) {
                paths.push(fhirpath);
            }
        }

        // Add computed paths (e.g., nested combinations)
        paths.extend(self.generate_nested_paths(type_name).await);

        paths.sort();
        paths.dedup();
        paths
    }

    /// Pre-compute common paths for performance
    pub async fn precompute_common_paths(&self, types: &[String]) -> Result<()> {
        let common_patterns = [
            "id",
            "text",
            "status",
            "code",
            "value",
            "name",
            "identifier",
            "name.given",
            "name.family",
            "telecom.value",
            "address.city",
            "entry.resource",
            "coding.system",
            "coding.code",
        ];

        for type_name in types {
            for pattern in &common_patterns {
                if let Some(resolution) = self.resolve_path_uncached(type_name, pattern).await {
                    let metadata = PathMetadata {
                        path: pattern.to_string(),
                        target_type: resolution.target_type.clone(),
                        depth: pattern.matches('.').count() as u32,
                        is_common: true,
                        usage_frequency: 1.0, // Would be computed from usage data
                    };

                    let key = format!("{type_name}.{pattern}");
                    let mut common_paths = self.common_paths.write().await;
                    common_paths.insert(key, metadata);
                }
            }
        }

        Ok(())
    }

    /// Get cache statistics
    pub async fn get_metrics(&self) -> PathResolverMetrics {
        let metrics = self.metrics.read().await;
        PathResolverMetrics {
            cache_hits: metrics.cache_hits,
            cache_misses: metrics.cache_misses,
            total_resolutions: metrics.total_resolutions,
            avg_resolution_time_ns: metrics.avg_resolution_time_ns,
            max_path_depth: metrics.max_path_depth,
        }
    }

    /// Clear all caches
    pub async fn clear_caches(&self) {
        self.resolution_cache.write().await.clear();
        self.common_paths.write().await.clear();

        let mut metrics = self.metrics.write().await;
        *metrics = PathResolverMetrics::default();
    }

    // === PRIVATE IMPLEMENTATION ===

    async fn resolve_with_cache(&self, query: &PathQuery) -> Option<PathResolution> {
        let start_time = std::time::Instant::now();

        // Check cache first
        {
            let cache = self.resolution_cache.read().await;
            if let Some(cached) = cache.get(query) {
                self.update_metrics(true, start_time).await;
                return Some(cached.clone());
            }
        }

        // Resolve uncached
        let resolution = self
            .resolve_path_uncached(&query.base_type, &query.path)
            .await?;

        // Cache result
        {
            let mut cache = self.resolution_cache.write().await;
            cache.insert(query.clone(), resolution.clone());
        }

        self.update_metrics(false, start_time).await;
        Some(resolution)
    }

    async fn resolve_path_uncached(&self, base_type: &str, path: &str) -> Option<PathResolution> {
        let segments: Vec<&str> = path.split('.').collect();

        if segments.is_empty() {
            return None;
        }

        // Handle single segment (simple case)
        if segments.len() == 1 {
            return self.resolve_single_segment(base_type, segments[0]).await;
        }

        // Handle multi-segment path (complex case)
        self.resolve_multi_segment_path(base_type, &segments).await
    }

    async fn resolve_single_segment(
        &self,
        base_type: &str,
        segment: &str,
    ) -> Option<PathResolution> {
        let schema = self.schema_index.get_schema_by_type(base_type).await?;

        // Direct element lookup
        if let Some(element) = schema.elements.get(segment) {
            return self.element_to_resolution(base_type, segment, element);
        }

        // Check for choice type expansion
        if let Some(choice_resolution) = self.resolve_choice_segment(base_type, segment).await {
            return Some(choice_resolution);
        }

        None
    }

    async fn resolve_multi_segment_path(
        &self,
        base_type: &str,
        segments: &[&str],
    ) -> Option<PathResolution> {
        let mut current_type = base_type.to_string();
        let mut resolution_steps = Vec::new();
        let mut is_collection = false;

        for (i, segment) in segments.iter().enumerate() {
            let step_resolution = self.resolve_single_segment(&current_type, segment).await?;

            // Build resolution step
            let step = PathStep {
                segment: segment.to_string(),
                from_type: current_type.clone(),
                to_type: step_resolution.target_type.clone(),
                is_collection: step_resolution.is_collection,
                element_name: segment.to_string(),
            };

            resolution_steps.push(step);

            // Update for next iteration
            current_type = step_resolution.target_type.clone();
            if step_resolution.is_collection {
                is_collection = true;
            }

            // For the final segment, return complete resolution
            if i == segments.len() - 1 {
                return Some(PathResolution {
                    target_type: current_type,
                    is_collection,
                    cardinality: step_resolution.cardinality,
                    element_info: step_resolution.element_info,
                    resolution_steps,
                    is_choice_type: step_resolution.is_choice_type,
                    choice_info: step_resolution.choice_info,
                });
            }
        }

        None
    }

    async fn resolve_choice_segment(
        &self,
        base_type: &str,
        segment: &str,
    ) -> Option<PathResolution> {
        // Check if segment matches a choice type pattern
        if let Some(base_path) = self.choice_expander.extract_choice_base_path(segment) {
            let schema = self.schema_index.get_schema_by_type(base_type).await?;

            if let Some(base_element) = schema.elements.get(&base_path) {
                // This is a choice type expansion
                let actual_type = self.extract_type_from_choice_path(segment)?;

                return Some(PathResolution {
                    target_type: actual_type.clone(),
                    is_collection: self.is_collection_type(&actual_type),
                    cardinality: PathCardinality {
                        min: base_element.min.unwrap_or(0),
                        max: self.parse_max_cardinality(&base_element.max),
                    },
                    element_info: self.build_element_info(segment, &actual_type, base_element),
                    resolution_steps: vec![],
                    is_choice_type: true,
                    choice_info: Some(ChoiceTypeInfo {
                        base_path,
                        expanded_path: segment.to_string(),
                        actual_type,
                    }),
                });
            }
        }

        None
    }

    fn element_to_resolution(
        &self,
        _base_type: &str,
        path: &str,
        element: &Element,
    ) -> Option<PathResolution> {
        let target_type = self.extract_element_type(element)?;
        let is_collection = self.is_collection_from_cardinality(element);

        Some(PathResolution {
            target_type: target_type.clone(),
            is_collection,
            cardinality: PathCardinality {
                min: element.min.unwrap_or(0),
                max: self.parse_max_cardinality(&element.max),
            },
            element_info: ElementInfo {
                path: path.to_string(),
                element_type: target_type,
                cardinality: PathCardinality {
                    min: element.min.unwrap_or(0),
                    max: self.parse_max_cardinality(&element.max),
                },
                is_collection,
                is_choice_type: element
                    .element_type
                    .as_ref()
                    .map(|types| types.len() > 1)
                    .unwrap_or(false),
                choice_type_options: None,
                constraints: element
                    .constraints
                    .iter()
                    .map(|c| ConstraintInfo {
                        key: c.key.clone(),
                        severity: c.severity.clone(),
                        human_description: c.human.clone(),
                        fhirpath_expression: c.expression.clone(),
                        source: None,
                    })
                    .collect(),
                binding: element.binding.as_ref().map(|b| BindingInfo {
                    strength: b.strength.clone(),
                    value_set: b.value_set.as_ref().map(|u| u.to_string()),
                    description: b.description.clone(),
                }),
                definition: element.definition.clone(),
                short_description: element.short.clone(),
            },
            resolution_steps: vec![],
            is_choice_type: false,
            choice_info: None,
        })
    }

    // Helper methods...
    fn extract_element_type(&self, element: &Element) -> Option<String> {
        element
            .element_type
            .as_ref()?
            .first()
            .map(|t| t.code.clone())
    }

    fn is_collection_from_cardinality(&self, element: &Element) -> bool {
        element
            .max
            .as_ref()
            .map(|max| max == "*" || max.parse::<u32>().unwrap_or(1) > 1)
            .unwrap_or(false)
    }

    fn parse_max_cardinality(&self, max: &Option<String>) -> Option<u32> {
        max.as_ref().and_then(|m| {
            if m == "*" {
                None // Unbounded
            } else {
                m.parse().ok()
            }
        })
    }

    fn is_collection_type(&self, _type_name: &str) -> bool {
        // For now, assume primitive types are not collections
        // This could be enhanced with more sophisticated logic
        false
    }

    fn extract_type_from_choice_path(&self, path: &str) -> Option<String> {
        // Extract type from choice path like "valueString" -> "string"
        for type_name in ChoiceTypeExpander::get_common_choice_types() {
            let capitalized = self.choice_expander.capitalize_first(type_name);
            if path.ends_with(&capitalized) {
                return Some(type_name.to_string());
            }
        }
        None
    }

    fn build_element_info(
        &self,
        path: &str,
        element_type: &str,
        base_element: &Element,
    ) -> ElementInfo {
        ElementInfo {
            path: path.to_string(),
            element_type: element_type.to_string(),
            cardinality: PathCardinality {
                min: base_element.min.unwrap_or(0),
                max: self.parse_max_cardinality(&base_element.max),
            },
            is_collection: self.is_collection_from_cardinality(base_element),
            is_choice_type: true,
            choice_type_options: None,
            constraints: base_element
                .constraints
                .iter()
                .map(|c| ConstraintInfo {
                    key: c.key.clone(),
                    severity: c.severity.clone(),
                    human_description: c.human.clone(),
                    fhirpath_expression: c.expression.clone(),
                    source: None,
                })
                .collect(),
            binding: base_element.binding.as_ref().map(|b| BindingInfo {
                strength: b.strength.clone(),
                value_set: b.value_set.as_ref().map(|u| u.to_string()),
                description: b.description.clone(),
            }),
            definition: base_element.definition.clone(),
            short_description: base_element.short.clone(),
        }
    }

    fn convert_to_fhirpath(&self, internal_path: &str) -> Option<String> {
        // Convert internal element paths to FHIRPath syntax
        // For now, just return as-is, but this could be enhanced
        Some(internal_path.to_string())
    }

    async fn generate_nested_paths(&self, _type_name: &str) -> Vec<String> {
        // Generate common nested path combinations
        // For now, return empty, but this could be enhanced with actual logic
        Vec::new()
    }

    async fn update_metrics(&self, cache_hit: bool, start_time: std::time::Instant) {
        let mut metrics = self.metrics.write().await;
        metrics.total_resolutions += 1;

        if cache_hit {
            metrics.cache_hits += 1;
        } else {
            metrics.cache_misses += 1;
        }

        let resolution_time = start_time.elapsed().as_nanos() as u64;
        metrics.avg_resolution_time_ns =
            (metrics.avg_resolution_time_ns * (metrics.total_resolutions - 1) + resolution_time)
                / metrics.total_resolutions;
    }
}

impl PathCardinality {
    pub fn new(min: u32, max: Option<u32>) -> Self {
        Self { min, max }
    }

    pub fn is_required(&self) -> bool {
        self.min > 0
    }

    pub fn is_unbounded(&self) -> bool {
        self.max.is_none()
    }

    pub fn is_collection(&self) -> bool {
        self.max.is_none() || self.max.unwrap_or(0) > 1
    }
}
