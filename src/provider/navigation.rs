use lru::LruCache;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::{FhirSchemaManager, ResolutionContext};
use crate::error::{FhirSchemaError, Result};
use crate::provider::fhir_model_provider::{
    Cardinality, ChoiceResolution, NavigationResult, PathSegment,
};
use crate::types::TypeResolver;

#[derive(Debug)]
pub struct NavigationEngine {
    type_resolver: Arc<TypeResolver>,
    schema_manager: Arc<FhirSchemaManager>,
    path_cache: Arc<RwLock<LruCache<String, CachedNavigation>>>,
    optimization_cache: Arc<RwLock<LruCache<String, OptimizationHint>>>,
}

#[derive(Debug, Clone)]
struct CachedNavigation {
    result: NavigationResult,
    timestamp: std::time::Instant,
}

#[derive(Debug, Clone)]
struct OptimizationHint {
    #[allow(dead_code)]
    common_patterns: Vec<String>,
    #[allow(dead_code)]
    performance_score: f64,
    #[allow(dead_code)]
    last_used: std::time::Instant,
}

#[derive(Debug)]
struct PathParsingResult {
    segments: Vec<String>,
    #[allow(dead_code)]
    has_choice_types: bool,
    #[allow(dead_code)]
    has_array_access: bool,
    #[allow(dead_code)]
    complexity_score: u32,
}

impl NavigationEngine {
    pub async fn new(
        type_resolver: Arc<TypeResolver>,
        schema_manager: Arc<FhirSchemaManager>,
    ) -> Result<Self> {
        Ok(Self {
            type_resolver,
            schema_manager,
            path_cache: Arc::new(RwLock::new(LruCache::new(1000.try_into().unwrap()))),
            optimization_cache: Arc::new(RwLock::new(LruCache::new(500.try_into().unwrap()))),
        })
    }

    /// Advanced navigation with path optimization
    pub async fn navigate_with_optimization(
        &self,
        base_type: &str,
        path: &str,
    ) -> Result<NavigationResult> {
        let cache_key = format!("{base_type}:{path}");

        // Check cache first
        {
            let mut cache = self.path_cache.write().await;
            if let Some(cached) = cache.get(&cache_key) {
                // Check if cache is still fresh (5 minutes)
                if cached.timestamp.elapsed().as_secs() < 300 {
                    return Ok(cached.result.clone());
                }
            }
        }

        // Parse and optimize path
        let parsing_result = self.parse_path(path)?;
        let optimization_hint = self.get_optimization_hint(base_type, path).await;

        // Navigate the path
        let result = self
            .navigate_parsed_path(base_type, &parsing_result, &optimization_hint)
            .await?;

        // Cache the result
        {
            let mut cache = self.path_cache.write().await;
            cache.put(
                cache_key,
                CachedNavigation {
                    result: result.clone(),
                    timestamp: std::time::Instant::now(),
                },
            );
        }

        Ok(result)
    }

    /// Parse path into segments and analyze complexity
    fn parse_path(&self, path: &str) -> Result<PathParsingResult> {
        let segments: Vec<String> = path
            .split('.')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if segments.is_empty() {
            return Err(FhirSchemaError::navigation_failed("Empty path provided"));
        }

        let has_choice_types = segments.iter().any(|s| s.contains("[x]"));
        let has_array_access = segments
            .iter()
            .any(|s| s.contains('[') && s.contains(']') && !s.contains("[x]"));

        // Calculate complexity score
        let mut complexity_score = segments.len() as u32;
        if has_choice_types {
            complexity_score += 2;
        }
        if has_array_access {
            complexity_score += 1;
        }

        // Add complexity for deep nesting
        complexity_score += (segments.len() / 3) as u32;

        Ok(PathParsingResult {
            segments,
            has_choice_types,
            has_array_access,
            complexity_score,
        })
    }

    /// Get or create optimization hints for path patterns
    async fn get_optimization_hint(&self, base_type: &str, path: &str) -> OptimizationHint {
        let hint_key = format!("{}:{}", base_type, self.extract_pattern(path));

        {
            let mut cache = self.optimization_cache.write().await;
            if let Some(hint) = cache.get(&hint_key) {
                return hint.clone();
            }
        }

        // Create new optimization hint
        let hint = OptimizationHint {
            common_patterns: self.extract_common_patterns(path),
            performance_score: self.calculate_performance_score(path),
            last_used: std::time::Instant::now(),
        };

        {
            let mut cache = self.optimization_cache.write().await;
            cache.put(hint_key, hint.clone());
        }

        hint
    }

    /// Navigate parsed path with optimization
    async fn navigate_parsed_path(
        &self,
        base_type: &str,
        parsing_result: &PathParsingResult,
        _optimization_hint: &OptimizationHint,
    ) -> Result<NavigationResult> {
        let mut current_type = base_type.to_string();
        let mut path_segments = Vec::new();
        let mut confidence = 1.0;
        let mut choice_resolution = None;

        for (index, segment) in parsing_result.segments.iter().enumerate() {
            // Handle choice types
            if segment.contains("[x]") {
                let context = ResolutionContext::new(base_type).with_resource_type(&current_type);

                let resolved_types = self
                    .type_resolver
                    .resolve_choice_type(&current_type, "[x]", &context)
                    .await?;

                if let Some(resolved) = resolved_types.first() {
                    current_type = resolved.type_name.clone();
                    confidence *= 0.8; // Fixed confidence for choice types

                    choice_resolution = Some(ChoiceResolution {
                        resolved_type: current_type.clone(),
                        confidence: 0.8,
                        alternatives: vec![],
                        context_used: context,
                        resolution_path: parsing_result.segments[..=index].join("."),
                    });
                } else {
                    return Err(FhirSchemaError::navigation_failed(&format!(
                        "Could not resolve choice type in segment: {segment}"
                    )));
                }
            }
            // Handle array access
            else if segment.contains('[') && segment.contains(']') {
                let base_segment = segment.split('[').next().unwrap_or(segment);
                current_type = self
                    .resolve_property_type(&current_type, base_segment)
                    .await?;
                confidence *= 0.95; // Slight reduction for array access
            }
            // Handle regular properties
            else {
                current_type = self.resolve_property_type(&current_type, segment).await?;
            }

            // Create path segment info
            let segment_info = PathSegment {
                name: segment.clone(),
                segment_type: current_type.clone(),
                is_array: segment.contains('[') && segment.contains(']'),
                cardinality: Cardinality {
                    min: 0, // TODO: Get actual cardinality from schema
                    max: if segment.contains('[') { None } else { Some(1) },
                },
            };

            path_segments.push(segment_info);
        }

        Ok(NavigationResult {
            target_type: current_type,
            is_valid_path: true,
            path_segments,
            choice_resolution,
            confidence,
        })
    }

    /// Resolve the type of a property within a parent type
    async fn resolve_property_type(
        &self,
        parent_type: &str,
        property_name: &str,
    ) -> Result<String> {
        let context = ResolutionContext::new(parent_type);

        match self
            .schema_manager
            .infer_element_type(parent_type, property_name, &context)
            .await
        {
            Ok(resolved_type) => Ok(resolved_type),
            Err(_) => {
                // Fallback: try to get schema and extract property type
                if let Some(schema) = self.schema_manager.get_schema_by_type(parent_type).await? {
                    for (prop_name, prop) in &schema.properties {
                        if prop_name == property_name {
                            return Ok(prop
                                .property_type
                                .clone()
                                .unwrap_or_else(|| "string".to_string()));
                        }
                    }
                }

                Err(FhirSchemaError::navigation_failed(&format!(
                    "Property '{property_name}' not found in type '{parent_type}'"
                )))
            }
        }
    }

    /// Extract pattern for optimization caching
    fn extract_pattern(&self, path: &str) -> String {
        // Replace specific values with patterns
        path.replace("[0]", "[n]")
            .replace("[1]", "[n]")
            .replace("[2]", "[n]")
            .chars()
            .take(50) // Limit pattern length
            .collect()
    }

    /// Extract common path patterns for optimization
    fn extract_common_patterns(&self, path: &str) -> Vec<String> {
        let mut patterns = Vec::new();

        // Common FHIR patterns
        if path.contains(".value[x]") {
            patterns.push("choice_type_value".to_string());
        }
        if path.contains(".extension") {
            patterns.push("extension_navigation".to_string());
        }
        if path.contains(".coding") {
            patterns.push("coding_navigation".to_string());
        }
        if path.contains("[0]") {
            patterns.push("first_element_access".to_string());
        }

        patterns
    }

    /// Calculate performance score for optimization
    fn calculate_performance_score(&self, path: &str) -> f64 {
        let mut score = 1.0;

        // Reduce score for complex paths
        let segment_count = path.split('.').count();
        score -= (segment_count as f64) * 0.05;

        // Reduce score for choice types (more expensive)
        if path.contains("[x]") {
            score -= 0.2;
        }

        // Reduce score for array access
        if path.contains('[') && path.contains(']') {
            score -= 0.1;
        }

        score.max(0.1) // Minimum score
    }

    /// Clear navigation caches
    pub async fn clear_caches(&self) {
        let mut path_cache = self.path_cache.write().await;
        let mut opt_cache = self.optimization_cache.write().await;
        path_cache.clear();
        opt_cache.clear();
    }

    /// Get navigation statistics
    pub async fn get_navigation_stats(&self) -> (usize, usize) {
        let path_cache = self.path_cache.read().await;
        let opt_cache = self.optimization_cache.read().await;
        (path_cache.len(), opt_cache.len())
    }
}
