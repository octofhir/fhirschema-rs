use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::choice_type_info::ResolvedChoiceType;
use crate::{FhirSchema, Result};

/// Advanced choice type resolver with caching
pub struct AdvancedChoiceTypeResolver {
    choice_registry: Arc<RwLock<HashMap<String, ChoiceTypeDefinition>>>,
    resolution_cache: Arc<RwLock<HashMap<ChoiceQuery, ResolvedChoiceType>>>,
    #[allow(dead_code)]
    type_resolver: Arc<TypeResolver>,
}

#[derive(Debug, Clone)]
pub struct ChoiceTypeDefinition {
    pub base_path: String,
    pub possible_types: Vec<String>,
    pub expansions: HashMap<String, String>, // type -> expanded_path
    pub metadata: ChoiceMetadata,
}

#[derive(Debug, Clone)]
pub struct ChoiceMetadata {
    pub is_required: bool,
    pub common_constraints: Vec<String>,
    pub usage_frequency: HashMap<String, f64>, // type -> frequency
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ChoiceQuery {
    pub base_path: String,
    pub value_type: Option<String>,
    pub context: Option<String>,
}

/// Simple type resolver for choice type resolution
pub struct TypeResolver {
    type_definitions: HashMap<String, TypeDefinition>,
}

#[derive(Debug, Clone)]
pub struct TypeDefinition {
    pub name: String,
    pub is_primitive: bool,
    pub properties: HashMap<String, String>,
}

impl AdvancedChoiceTypeResolver {
    pub fn new() -> Self {
        Self {
            choice_registry: Arc::new(RwLock::new(HashMap::new())),
            resolution_cache: Arc::new(RwLock::new(HashMap::new())),
            type_resolver: Arc::new(TypeResolver::new()),
        }
    }

    /// Build resolver from schema collection
    pub async fn build_from_schemas(&mut self, schemas: &[Arc<FhirSchema>]) -> Result<()> {
        for schema in schemas {
            self.analyze_schema_choice_types(schema).await?;
        }
        Ok(())
    }

    /// Resolve choice type with caching
    pub async fn resolve_choice_cached(&self, query: &ChoiceQuery) -> Option<ResolvedChoiceType> {
        // Check cache first
        {
            let cache = self.resolution_cache.read().await;
            if let Some(cached) = cache.get(query) {
                return Some(cached.clone());
            }
        }

        // Resolve and cache
        if let Some(resolved) = self.resolve_choice_uncached(query).await {
            let mut cache = self.resolution_cache.write().await;
            cache.insert(query.clone(), resolved.clone());
            Some(resolved)
        } else {
            None
        }
    }

    /// Get choice definition by base path
    pub async fn get_choice_definition(&self, base_path: &str) -> Option<ChoiceTypeDefinition> {
        let registry = self.choice_registry.read().await;
        registry.get(base_path).cloned()
    }

    /// Register a choice type definition
    pub async fn register_choice_type(&self, definition: ChoiceTypeDefinition) {
        let mut registry = self.choice_registry.write().await;
        registry.insert(definition.base_path.clone(), definition);
    }

    /// Clear resolution cache
    pub async fn clear_cache(&self) {
        let mut cache = self.resolution_cache.write().await;
        cache.clear();
    }

    async fn resolve_choice_uncached(&self, query: &ChoiceQuery) -> Option<ResolvedChoiceType> {
        let registry = self.choice_registry.read().await;
        let definition = registry.get(&query.base_path)?;

        if let Some(value_type) = &query.value_type {
            // Direct type resolution
            if definition.possible_types.contains(value_type) {
                Some(ResolvedChoiceType::new(&query.base_path, value_type))
            } else {
                None
            }
        } else {
            // Context-based resolution or return most common
            self.resolve_from_context(definition, &query.context).await
        }
    }

    async fn resolve_from_context(
        &self,
        definition: &ChoiceTypeDefinition,
        context: &Option<String>,
    ) -> Option<ResolvedChoiceType> {
        // If we have context, try to infer the most likely type
        if let Some(_ctx) = context {
            // For now, return the most frequent type based on usage frequency
            let most_frequent = definition
                .metadata
                .usage_frequency
                .iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(type_name, _)| type_name.clone());

            if let Some(type_name) = most_frequent {
                return Some(ResolvedChoiceType::new(&definition.base_path, &type_name));
            }
        }

        // Fallback to first available type
        definition
            .possible_types
            .first()
            .map(|type_name| ResolvedChoiceType::new(&definition.base_path, type_name))
    }

    async fn analyze_schema_choice_types(&mut self, schema: &FhirSchema) -> Result<()> {
        for (path, element) in &schema.elements {
            if path.ends_with("[x]") {
                let possible_types = if let Some(element_types) = &element.element_type {
                    element_types.iter().map(|et| et.code.clone()).collect()
                } else {
                    Vec::new()
                };

                let base_path_without_choice = path.trim_end_matches("[x]");
                let mut expansions = HashMap::new();

                for type_code in &possible_types {
                    let capitalized_type = capitalize_first(type_code);
                    let expanded_path = format!("{base_path_without_choice}{capitalized_type}");
                    expansions.insert(type_code.clone(), expanded_path);
                }

                let metadata = ChoiceMetadata {
                    is_required: element.min.unwrap_or(0) > 0,
                    common_constraints: element.constraints.iter().map(|c| c.key.clone()).collect(),
                    usage_frequency: HashMap::new(), // Would be populated from usage statistics
                };

                let definition = ChoiceTypeDefinition {
                    base_path: path.clone(),
                    possible_types,
                    expansions,
                    metadata,
                };

                self.register_choice_type(definition).await;
            }
        }
        Ok(())
    }
}

impl Default for AdvancedChoiceTypeResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeResolver {
    pub fn new() -> Self {
        let mut type_definitions = HashMap::new();

        // Add primitive types
        for type_name in &[
            "boolean",
            "integer",
            "string",
            "decimal",
            "uri",
            "url",
            "canonical",
            "base64Binary",
            "instant",
            "date",
            "dateTime",
            "time",
            "code",
            "oid",
            "id",
            "markdown",
            "unsignedInt",
            "positiveInt",
            "uuid",
        ] {
            type_definitions.insert(
                type_name.to_string(),
                TypeDefinition {
                    name: type_name.to_string(),
                    is_primitive: true,
                    properties: HashMap::new(),
                },
            );
        }

        // Add complex types
        for type_name in &[
            "Address",
            "Age",
            "Annotation",
            "Attachment",
            "CodeableConcept",
            "Coding",
            "ContactPoint",
            "Count",
            "Distance",
            "Duration",
            "HumanName",
            "Identifier",
            "Money",
            "Period",
            "Quantity",
            "Range",
            "Ratio",
            "Reference",
            "SampledData",
            "Signature",
            "Timing",
        ] {
            type_definitions.insert(
                type_name.to_string(),
                TypeDefinition {
                    name: type_name.to_string(),
                    is_primitive: false,
                    properties: HashMap::new(),
                },
            );
        }

        Self { type_definitions }
    }

    pub fn is_primitive(&self, type_name: &str) -> bool {
        self.type_definitions
            .get(type_name)
            .map(|def| def.is_primitive)
            .unwrap_or(false)
    }

    pub fn get_type_definition(&self, type_name: &str) -> Option<&TypeDefinition> {
        self.type_definitions.get(type_name)
    }
}

impl Default for TypeResolver {
    fn default() -> Self {
        Self::new()
    }
}

// Helper function
fn capitalize_first(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }

    let mut chars: Vec<char> = s.chars().collect();
    chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
    chars.into_iter().collect()
}
