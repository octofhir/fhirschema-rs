//! Schema Compiler - transforms FhirSchema into CompiledSchema.
//!
//! The compiler resolves inheritance chains, merges schemas, and expands
//! all nested types inline for fast validation without runtime lookups.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_recursion::async_recursion;

use super::SchemaProvider;
use crate::types::{FhirSchema, FhirSchemaConstraint, FhirSchemaElement, FhirSchemaSlicing};

use super::compiled::{
    BindingStrength, CompiledBinding, CompiledConstraint, CompiledDiscriminator, CompiledElement,
    CompiledSchema, CompiledSlice, CompiledSlicing, CompiledTypeInfo, ConstraintSeverity,
    DiscriminatorType, PrimitiveType, SchemaKind, SharedCompiledSchema, SlicingRules,
    is_primitive_type,
};

/// Error during schema compilation
#[derive(Debug, Clone)]
pub struct CompileError {
    pub message: String,
    pub schema_name: Option<String>,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.schema_name {
            write!(f, "Failed to compile schema '{}': {}", name, self.message)
        } else {
            write!(f, "Compile error: {}", self.message)
        }
    }
}

impl std::error::Error for CompileError {}

/// Schema compiler with caching
pub struct SchemaCompiler {
    /// Schema provider for loading raw schemas
    schema_provider: Arc<dyn SchemaProvider>,
    /// Cache of compiled schemas
    compiled_cache: moka::future::Cache<String, SharedCompiledSchema>,
}

impl SchemaCompiler {
    /// Create a new schema compiler
    pub fn new(schema_provider: Arc<dyn SchemaProvider>) -> Self {
        Self {
            schema_provider,
            // Cache ~500 compiled schemas (covers most FHIR types)
            compiled_cache: moka::future::Cache::new(500),
        }
    }

    /// Get or compile a schema by name/URL
    #[async_recursion]
    pub async fn compile(&self, schema_name: &str) -> Result<SharedCompiledSchema, CompileError> {
        // Check cache first
        if let Some(cached) = self.compiled_cache.get(schema_name).await {
            return Ok(cached);
        }

        // Compile and cache
        let compiled = self.compile_internal(schema_name).await?;
        let arc = Arc::new(compiled);
        self.compiled_cache
            .insert(schema_name.to_string(), arc.clone())
            .await;
        Ok(arc)
    }

    /// Internal compilation logic
    #[async_recursion]
    async fn compile_internal(&self, schema_name: &str) -> Result<CompiledSchema, CompileError> {
        // 1. Load base schema
        let schema = self
            .schema_provider
            .get_schema(schema_name)
            .await
            .ok_or_else(|| CompileError {
                message: format!("Schema not found: {}", schema_name),
                schema_name: Some(schema_name.to_string()),
            })?;

        // 2. Resolve inheritance chain and merge
        let chain = self.resolve_chain(&schema).await?;
        let merged = self.merge_chain(&chain);

        // 3. Recursively expand all element types
        let elements = self.expand_elements(merged.elements.as_ref()).await?;

        // 4. Collect all constraints from the chain
        let constraints = self.collect_constraints(&chain);

        // 5. Build required/excluded sets
        let required: HashSet<String> = merged
            .required
            .as_ref()
            .map(|r| r.iter().cloned().collect())
            .unwrap_or_default();

        let excluded: HashSet<String> = merged
            .excluded
            .as_ref()
            .map(|e| e.iter().cloned().collect())
            .unwrap_or_default();

        Ok(CompiledSchema {
            url: schema.url.clone(),
            name: schema.name.clone(),
            elements,
            constraints,
            required,
            excluded,
            is_resource: schema.kind == "resource",
            kind: SchemaKind::parse(&schema.kind),
        })
    }

    /// Resolve inheritance chain from base to derived
    async fn resolve_chain(
        &self,
        schema: &FhirSchema,
    ) -> Result<Vec<Arc<FhirSchema>>, CompileError> {
        let mut chain = vec![Arc::new(schema.clone())];
        let mut current = schema.clone();
        let mut visited = HashSet::new();
        visited.insert(schema.url.clone());

        // Follow base references
        while let Some(base_url) = &current.base {
            if visited.contains(base_url) {
                // Cycle detected
                break;
            }
            visited.insert(base_url.clone());

            if let Some(base_schema) = self.schema_provider.get_schema_by_url(base_url).await {
                chain.push(base_schema.clone());
                current = (*base_schema).clone();
            } else {
                // Base not found, stop here
                break;
            }
        }

        // Reverse so base comes first
        chain.reverse();
        Ok(chain)
    }

    /// Merge inheritance chain into single schema
    fn merge_chain(&self, chain: &[Arc<FhirSchema>]) -> FhirSchema {
        if chain.is_empty() {
            return FhirSchema::default();
        }

        let mut merged = (*chain[0]).clone();

        for schema in chain.iter().skip(1) {
            merged = self.merge_schemas(&merged, schema);
        }

        merged
    }

    /// Merge two schemas (base + overlay)
    fn merge_schemas(&self, base: &FhirSchema, overlay: &FhirSchema) -> FhirSchema {
        let mut result = base.clone();

        // Overlay takes precedence for metadata
        result.url = overlay.url.clone();
        result.name = overlay.name.clone();
        if overlay.version.is_some() {
            result.version = overlay.version.clone();
        }

        // Merge elements
        if let Some(overlay_elements) = &overlay.elements {
            let mut merged_elements = result.elements.unwrap_or_default();
            for (key, element) in overlay_elements {
                if let Some(base_element) = merged_elements.get(key) {
                    merged_elements.insert(key.clone(), self.merge_elements(base_element, element));
                } else {
                    merged_elements.insert(key.clone(), element.clone());
                }
            }
            result.elements = Some(merged_elements);
        }

        // Union required elements
        if let Some(overlay_required) = &overlay.required {
            let mut required = result.required.unwrap_or_default();
            required.extend(overlay_required.iter().cloned());
            result.required = Some(required);
        }

        // Union excluded elements
        if let Some(overlay_excluded) = &overlay.excluded {
            let mut excluded = result.excluded.unwrap_or_default();
            excluded.extend(overlay_excluded.iter().cloned());
            result.excluded = Some(excluded);
        }

        // Union constraints (overlay takes precedence for same key)
        if let Some(overlay_constraints) = &overlay.constraint {
            let mut constraints = result.constraint.unwrap_or_default();
            for (key, constraint) in overlay_constraints {
                constraints.insert(key.clone(), constraint.clone());
            }
            result.constraint = Some(constraints);
        }

        result
    }

    /// Merge two elements
    fn merge_elements(
        &self,
        base: &FhirSchemaElement,
        overlay: &FhirSchemaElement,
    ) -> FhirSchemaElement {
        let mut result = base.clone();

        // Overlay cardinality
        if overlay.min.is_some() {
            result.min = overlay.min;
        }
        if overlay.max.is_some() {
            result.max = overlay.max;
        }
        if overlay.array.is_some() {
            result.array = overlay.array;
        }

        // Overlay binding
        if overlay.binding.is_some() {
            result.binding = overlay.binding.clone();
        }

        // Overlay pattern
        if overlay.pattern.is_some() {
            result.pattern = overlay.pattern.clone();
        }

        // Overlay must_support
        if overlay.must_support.is_some() {
            result.must_support = overlay.must_support;
        }

        // Overlay refers (reference targets)
        if overlay.refers.is_some() {
            result.refers = overlay.refers.clone();
        }

        // Merge nested elements
        if let Some(overlay_nested) = &overlay.elements {
            let mut nested = result.elements.unwrap_or_default();
            for (key, element) in overlay_nested {
                if let Some(base_element) = nested.get(key) {
                    nested.insert(key.clone(), self.merge_elements(base_element, element));
                } else {
                    nested.insert(key.clone(), element.clone());
                }
            }
            result.elements = Some(nested);
        }

        // Union constraints
        if let Some(overlay_constraints) = &overlay.constraint {
            let mut constraints = result.constraint.unwrap_or_default();
            for (key, constraint) in overlay_constraints {
                constraints.insert(key.clone(), constraint.clone());
            }
            result.constraint = Some(constraints);
        }

        result
    }

    /// Recursively expand element types inline
    #[async_recursion]
    async fn expand_elements(
        &self,
        elements: Option<&HashMap<String, FhirSchemaElement>>,
    ) -> Result<HashMap<String, CompiledElement>, CompileError> {
        let Some(elements) = elements else {
            return Ok(HashMap::new());
        };

        let mut result = HashMap::new();

        for (name, element) in elements {
            let compiled = self.expand_element(name, element).await?;
            result.insert(name.clone(), compiled);
        }

        Ok(result)
    }

    /// Expand a single element
    #[async_recursion]
    async fn expand_element(
        &self,
        name: &str,
        element: &FhirSchemaElement,
    ) -> Result<CompiledElement, CompileError> {
        let type_info = self.determine_type_info(element);
        let mut children = HashMap::new();

        // Expand nested elements based on type
        match &type_info {
            CompiledTypeInfo::BackboneElement | CompiledTypeInfo::Complex => {
                // If element has inline nested elements (BackboneElement)
                if let Some(nested) = &element.elements {
                    children = Box::pin(self.expand_elements(Some(nested))).await?;
                }
                // If element has a type, recursively compile that type's elements
                else if let Some(type_name) = &element.type_name
                    && !is_primitive_type(type_name)
                    && type_name != "Resource"
                    && type_name != "Reference"
                {
                    // Get compiled schema for this type
                    if let Ok(type_schema) = self.compile(type_name).await {
                        children = type_schema.elements.clone();
                    }
                }
            }
            _ => {
                // Primitives, References, Resources don't have children to expand
            }
        }

        // Extract constraints
        let constraints = self.extract_element_constraints(element);

        // Extract binding
        let binding = element.binding.as_ref().map(|b| CompiledBinding {
            value_set: b.value_set.clone().unwrap_or_default(),
            strength: BindingStrength::parse(&b.strength),
            description: b.binding_name.clone(),
        });

        // Compile slicing if present
        let slicing = element.slicing.as_ref().map(|s| self.compile_slicing(s));

        Ok(CompiledElement {
            name: name.to_string(),
            type_info,
            is_array: element.array.unwrap_or(false),
            min: element.min.unwrap_or(0),
            max: element.max,
            children,
            binding,
            reference_targets: element.refers.clone(),
            constraints,
            pattern: element.pattern.as_ref().map(|p| p.value.clone()),
            choices: element.choices.clone(),
            slicing,
            short: element.short.clone(),
            must_support: element.must_support.unwrap_or(false),
            is_modifier: element.is_modifier.unwrap_or(false),
        })
    }

    /// Determine the type info for an element
    fn determine_type_info(&self, element: &FhirSchemaElement) -> CompiledTypeInfo {
        // Check for inline elements first (BackboneElement)
        if element.elements.is_some() {
            return CompiledTypeInfo::BackboneElement;
        }

        let Some(type_name) = &element.type_name else {
            return CompiledTypeInfo::Complex;
        };

        // Check for primitive
        if let Some(ptype) = PrimitiveType::parse(type_name) {
            return CompiledTypeInfo::Primitive(ptype);
        }

        // Check for special types
        match type_name.as_str() {
            "Reference" => CompiledTypeInfo::Reference,
            "Resource" => CompiledTypeInfo::Resource,
            "Extension" => CompiledTypeInfo::Extension,
            "BackboneElement" => CompiledTypeInfo::BackboneElement,
            _ => CompiledTypeInfo::Complex,
        }
    }

    /// Extract constraints from element
    fn extract_element_constraints(&self, element: &FhirSchemaElement) -> Vec<CompiledConstraint> {
        let Some(constraints) = &element.constraint else {
            return Vec::new();
        };

        constraints
            .iter()
            .map(|(key, c)| self.convert_constraint(key, c))
            .collect()
    }

    /// Collect all constraints from inheritance chain
    fn collect_constraints(&self, chain: &[Arc<FhirSchema>]) -> Vec<CompiledConstraint> {
        let mut result = Vec::new();

        for schema in chain {
            if let Some(constraints) = &schema.constraint {
                for (key, constraint) in constraints {
                    result.push(self.convert_constraint(key, constraint));
                }
            }
        }

        result
    }

    /// Convert FhirSchemaConstraint to CompiledConstraint
    fn convert_constraint(
        &self,
        key: &str,
        constraint: &FhirSchemaConstraint,
    ) -> CompiledConstraint {
        CompiledConstraint {
            key: key.to_string(),
            expression: constraint.expression.clone(),
            human: constraint.human.clone(),
            severity: ConstraintSeverity::parse(&constraint.severity),
        }
    }

    /// Compile slicing definition
    fn compile_slicing(&self, slicing: &FhirSchemaSlicing) -> CompiledSlicing {
        // Compile discriminators
        let discriminators = slicing
            .discriminator
            .as_ref()
            .map(|discs| {
                discs
                    .iter()
                    .map(|d| CompiledDiscriminator {
                        discriminator_type: DiscriminatorType::parse(&d.type_name),
                        path: d.path.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Compile slices
        let slices = slicing
            .slices
            .as_ref()
            .map(|slice_map| {
                slice_map
                    .iter()
                    .map(|(name, slice_def)| {
                        let compiled_slice = CompiledSlice {
                            name: name.clone(),
                            match_value: slice_def.match_value.clone(),
                            min: slice_def.min,
                            max: slice_def.max,
                            // TODO: compile nested schema if needed
                            schema: None,
                        };
                        (name.clone(), compiled_slice)
                    })
                    .collect()
            })
            .unwrap_or_default();

        CompiledSlicing {
            rules: SlicingRules::parse(slicing.rules.as_deref().unwrap_or("open")),
            ordered: slicing.ordered.unwrap_or(false),
            discriminators,
            slices,
        }
    }
}

impl Default for FhirSchema {
    fn default() -> Self {
        Self {
            url: String::new(),
            version: None,
            name: String::new(),
            type_name: String::new(),
            kind: "complex-type".to_string(),
            derivation: None,
            base: None,
            abstract_type: None,
            class: String::new(),
            description: None,
            package_name: None,
            package_version: None,
            package_id: None,
            package_meta: None,
            elements: None,
            required: None,
            excluded: None,
            extensions: None,
            constraint: None,
            primitive_type: None,
            choices: None,
        }
    }
}
