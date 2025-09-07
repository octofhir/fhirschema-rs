// Advanced choice type handling with context-aware resolution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::{ResolutionContext, ResolvedType};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceType {
    pub base_name: String,
    pub suffix: String,
    pub allowed_types: Vec<String>,
    pub resolved_types: HashMap<String, ResolvedType>,
    pub context_patterns: Vec<ContextPattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPattern {
    pub resource_type: Option<String>,
    pub profile_url: Option<String>,
    pub path_pattern: String,
    pub preferred_types: Vec<String>,
    pub confidence: f64,
}

/// Advanced choice type resolver with context-aware resolution
// Debug impl manually provided below due to CanonicalManager not implementing Debug
pub struct ChoiceTypeResolver {
    // Cache for resolved choice types
    choice_cache: Arc<RwLock<HashMap<String, Vec<ResolvedType>>>>,

    // Canonical manager for accessing FHIR definitions
    #[allow(dead_code)]
    canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,

    // Context patterns for intelligent type resolution
    context_patterns: Arc<RwLock<Vec<ContextPattern>>>,

    // Type mapping for common choice patterns
    type_mappings: HashMap<String, Vec<String>>,
}

impl ChoiceTypeResolver {
    pub async fn new(
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
    ) -> Result<Self> {
        let mut resolver = Self {
            choice_cache: Arc::new(RwLock::new(HashMap::new())),
            canonical_manager,
            context_patterns: Arc::new(RwLock::new(Vec::new())),
            type_mappings: Self::build_default_mappings(),
        };

        resolver.initialize_context_patterns().await?;
        Ok(resolver)
    }

    /// Build default type mappings for common choice patterns
    fn build_default_mappings() -> HashMap<String, Vec<String>> {
        let mut mappings = HashMap::new();

        // Common value[x] patterns
        mappings.insert(
            "value".to_string(),
            vec![
                "boolean".to_string(),
                "integer".to_string(),
                "decimal".to_string(),
                "string".to_string(),
                "uri".to_string(),
                "url".to_string(),
                "date".to_string(),
                "dateTime".to_string(),
                "time".to_string(),
                "code".to_string(),
                "Coding".to_string(),
                "CodeableConcept".to_string(),
                "Quantity".to_string(),
                "Range".to_string(),
                "Ratio".to_string(),
                "Period".to_string(),
                "Reference".to_string(),
                "Attachment".to_string(),
            ],
        );

        // onset[x] patterns (common in clinical resources)
        mappings.insert(
            "onset".to_string(),
            vec![
                "dateTime".to_string(),
                "Age".to_string(),
                "Period".to_string(),
                "Range".to_string(),
                "string".to_string(),
            ],
        );

        // effective[x] patterns (common in observations)
        mappings.insert(
            "effective".to_string(),
            vec![
                "dateTime".to_string(),
                "Period".to_string(),
                "Timing".to_string(),
                "instant".to_string(),
            ],
        );

        // multipleBirth[x] patterns
        mappings.insert(
            "multipleBirth".to_string(),
            vec!["boolean".to_string(), "integer".to_string()],
        );

        // deceased[x] patterns
        mappings.insert(
            "deceased".to_string(),
            vec!["boolean".to_string(), "dateTime".to_string()],
        );

        mappings
    }

    /// Initialize context patterns for intelligent resolution
    async fn initialize_context_patterns(&mut self) -> Result<()> {
        let mut patterns = self.context_patterns.write().await;

        // Patient resource patterns
        patterns.push(ContextPattern {
            resource_type: Some("Patient".to_string()),
            profile_url: None,
            path_pattern: "*.multipleBirth[x]".to_string(),
            preferred_types: vec!["boolean".to_string(), "integer".to_string()],
            confidence: 0.9,
        });

        patterns.push(ContextPattern {
            resource_type: Some("Patient".to_string()),
            profile_url: None,
            path_pattern: "*.deceased[x]".to_string(),
            preferred_types: vec!["boolean".to_string(), "dateTime".to_string()],
            confidence: 0.9,
        });

        // Observation resource patterns
        patterns.push(ContextPattern {
            resource_type: Some("Observation".to_string()),
            profile_url: None,
            path_pattern: "*.value[x]".to_string(),
            preferred_types: vec![
                "Quantity".to_string(),
                "CodeableConcept".to_string(),
                "string".to_string(),
                "boolean".to_string(),
                "integer".to_string(),
                "Range".to_string(),
            ],
            confidence: 0.8,
        });

        patterns.push(ContextPattern {
            resource_type: Some("Observation".to_string()),
            profile_url: None,
            path_pattern: "*.effective[x]".to_string(),
            preferred_types: vec!["dateTime".to_string(), "Period".to_string()],
            confidence: 0.9,
        });

        // Condition resource patterns
        patterns.push(ContextPattern {
            resource_type: Some("Condition".to_string()),
            profile_url: None,
            path_pattern: "*.onset[x]".to_string(),
            preferred_types: vec![
                "dateTime".to_string(),
                "Age".to_string(),
                "Period".to_string(),
                "Range".to_string(),
            ],
            confidence: 0.8,
        });

        Ok(())
    }

    /// Resolve choice type with context-aware intelligence
    pub async fn resolve_with_context(
        &self,
        base_type: &str,
        choice_suffix: &str,
        context: &ResolutionContext,
    ) -> Result<Vec<ResolvedType>> {
        let cache_key = format!("{}:{}:{}", base_type, choice_suffix, context.hash());

        // Check cache first
        {
            let cache = self.choice_cache.read().await;
            if let Some(cached_types) = cache.get(&cache_key) {
                return Ok(cached_types.clone());
            }
        }

        // Resolve types based on context
        let resolved_types = self
            .resolve_choice_internal(base_type, choice_suffix, context)
            .await?;

        // Cache the result
        {
            let mut cache = self.choice_cache.write().await;
            cache.insert(cache_key, resolved_types.clone());
        }

        Ok(resolved_types)
    }

    /// Internal choice type resolution with context analysis
    async fn resolve_choice_internal(
        &self,
        base_type: &str,
        choice_suffix: &str,
        context: &ResolutionContext,
    ) -> Result<Vec<ResolvedType>> {
        let mut resolved_types = Vec::new();

        // If suffix is provided and not empty, resolve specific type
        if !choice_suffix.is_empty() && choice_suffix != "[x]" {
            let specific_type = self.normalize_choice_suffix(choice_suffix);
            return Ok(vec![
                self.resolve_specific_choice_type(&specific_type, context)
                    .await?,
            ]);
        }

        // Get base type mappings
        let possible_types = self.get_possible_types_for_base(base_type, context).await?;

        // Resolve each possible type
        for type_name in possible_types {
            match self.resolve_specific_choice_type(&type_name, context).await {
                Ok(resolved_type) => resolved_types.push(resolved_type),
                Err(e) => {
                    eprintln!("Failed to resolve choice type '{type_name}': {e}");
                    // Continue with other types
                }
            }
        }

        // Sort by relevance based on context patterns
        self.sort_by_context_relevance(&mut resolved_types, base_type, context)
            .await;

        Ok(resolved_types)
    }

    /// Get possible types for a base choice element
    async fn get_possible_types_for_base(
        &self,
        base_type: &str,
        context: &ResolutionContext,
    ) -> Result<Vec<String>> {
        // Check context patterns first
        let context_types = self
            .get_types_from_context_patterns(base_type, context)
            .await;
        if !context_types.is_empty() {
            return Ok(context_types);
        }

        // Fall back to default mappings
        if let Some(default_types) = self.type_mappings.get(base_type) {
            return Ok(default_types.clone());
        }

        // If no specific mapping exists, return common choice types
        Ok(vec![
            "string".to_string(),
            "boolean".to_string(),
            "integer".to_string(),
            "decimal".to_string(),
            "dateTime".to_string(),
            "CodeableConcept".to_string(),
            "Quantity".to_string(),
            "Reference".to_string(),
        ])
    }

    /// Get types from context patterns
    async fn get_types_from_context_patterns(
        &self,
        base_type: &str,
        context: &ResolutionContext,
    ) -> Vec<String> {
        let patterns = self.context_patterns.read().await;
        let mut matching_types = Vec::new();

        let path_pattern = format!("*.{base_type}[x]");

        for pattern in patterns.iter() {
            let mut matches = true;

            // Check resource type match
            if let (Some(pattern_resource), Some(context_resource)) =
                (&pattern.resource_type, &context.resource_type)
            {
                if pattern_resource != context_resource {
                    matches = false;
                }
            }

            // Check profile URL match
            if let Some(pattern_profile) = &pattern.profile_url {
                if !context.profile_urls.contains(pattern_profile) {
                    matches = false;
                }
            }

            // Check path pattern match
            if !self.matches_path_pattern(&pattern.path_pattern, &path_pattern) {
                matches = false;
            }

            if matches {
                // Weight by confidence and add to results
                for _ in 0..(pattern.confidence * 10.0) as usize {
                    matching_types.extend(pattern.preferred_types.clone());
                }
            }
        }

        // Remove duplicates and return most frequent
        let mut type_counts = HashMap::new();
        for type_name in matching_types {
            *type_counts.entry(type_name).or_insert(0) += 1;
        }

        let mut sorted_types: Vec<_> = type_counts.into_iter().collect();
        sorted_types.sort_by(|a, b| b.1.cmp(&a.1));

        sorted_types
            .into_iter()
            .map(|(type_name, _)| type_name)
            .collect()
    }

    /// Check if a path matches a pattern
    fn matches_path_pattern(&self, pattern: &str, path: &str) -> bool {
        // Simple wildcard matching - could be enhanced with regex
        if pattern.contains('*') {
            let pattern_suffix = pattern.strip_prefix("*.").unwrap_or(pattern);
            let path_suffix = path.strip_prefix("*.").unwrap_or(path);
            pattern_suffix == path_suffix
        } else {
            pattern == path
        }
    }

    /// Resolve a specific choice type
    async fn resolve_specific_choice_type(
        &self,
        type_name: &str,
        _context: &ResolutionContext,
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

        // For complex types, create a reference-based resolved type
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
                    serde_json::Value::String(format!(
                        "http://hl7.org/fhir/StructureDefinition/{type_name}"
                    )),
                );
                meta
            },
        })
    }

    /// Sort resolved types by context relevance
    async fn sort_by_context_relevance(
        &self,
        types: &mut [ResolvedType],
        _base_type: &str,
        context: &ResolutionContext,
    ) {
        let patterns = self.context_patterns.read().await;

        // Calculate relevance scores
        let mut scores = HashMap::new();

        for resolved_type in types.iter() {
            let mut score = 0.0;

            for pattern in patterns.iter() {
                if let Some(resource_type) = &context.resource_type {
                    if let Some(pattern_resource) = &pattern.resource_type {
                        if pattern_resource == resource_type
                            && pattern.preferred_types.contains(&resolved_type.type_name)
                        {
                            score += pattern.confidence;
                        }
                    }
                }
            }

            scores.insert(resolved_type.type_name.clone(), score);
        }

        // Sort by score (descending)
        types.sort_by(|a, b| {
            let score_a = scores.get(&a.type_name).unwrap_or(&0.0);
            let score_b = scores.get(&b.type_name).unwrap_or(&0.0);
            score_b
                .partial_cmp(score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Normalize choice suffix (e.g., "String" -> "string")
    fn normalize_choice_suffix(&self, suffix: &str) -> String {
        // Handle capitalization differences
        match suffix {
            "String" => "string".to_string(),
            "Boolean" => "boolean".to_string(),
            "Integer" => "integer".to_string(),
            "Decimal" => "decimal".to_string(),
            "DateTime" => "dateTime".to_string(),
            s => s.to_string(),
        }
    }

    /// Check if a type is primitive
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

    /// Add custom context pattern
    pub async fn add_context_pattern(&self, pattern: ContextPattern) {
        let mut patterns = self.context_patterns.write().await;
        patterns.push(pattern);
    }

    /// Clear the choice cache
    pub async fn clear_cache(&self) {
        let mut cache = self.choice_cache.write().await;
        cache.clear();
    }
}

// Static utility functions
impl ChoiceType {
    pub fn new(base_name: &str, suffix: &str) -> Self {
        Self {
            base_name: base_name.to_string(),
            suffix: suffix.to_string(),
            allowed_types: Vec::new(),
            resolved_types: HashMap::new(),
            context_patterns: Vec::new(),
        }
    }

    pub fn is_choice_element(path: &str) -> bool {
        path.contains("[x]")
    }

    pub fn extract_choice_base(path: &str) -> Option<String> {
        path.find("[x]").map(|pos| path[..pos].to_string())
    }

    /// Extract choice suffix from a concrete path
    pub fn extract_choice_suffix(path: &str) -> Option<String> {
        // Look for patterns like "valueString", "valueBoolean", etc.
        for base in &["value", "onset", "effective", "multipleBirth", "deceased"] {
            if path.starts_with(base) && path.len() > base.len() {
                return Some(path[base.len()..].to_string());
            }
        }
        None
    }

    /// Check if a concrete path matches a choice base
    pub fn matches_choice_base(concrete_path: &str, base_path: &str) -> bool {
        if let Some(choice_base) = Self::extract_choice_base(base_path) {
            concrete_path.starts_with(&choice_base)
        } else {
            false
        }
    }
}

impl ChoiceTypeResolver {
    /// Create a new ChoiceTypeResolver with provided canonical manager (sync version)
    pub fn new_sync(canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>) -> Self {
        Self {
            choice_cache: Arc::new(RwLock::new(HashMap::new())),
            canonical_manager,
            context_patterns: Arc::new(RwLock::new(Vec::new())),
            type_mappings: Self::build_default_mappings(),
        }
    }
}

impl std::fmt::Debug for ChoiceTypeResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChoiceTypeResolver")
            .field("canonical_manager", &"<CanonicalManager>")
            .finish()
    }
}
