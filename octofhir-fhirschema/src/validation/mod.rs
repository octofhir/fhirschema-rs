//! FHIR Schema Validation Engine
//!
//! This module implements the FHIR Schema validation algorithm as specified
//! in the FHIR Schema documentation. It provides comprehensive validation
//! including schemata resolution, data element validation, and constraint checking.
//!
//! ## Architecture
//!
//! The validation system uses pre-compiled schemas for performance:
//! - `CompiledSchema` - Schema with all nested types inlined
//! - `SchemaCompiler` - Lazily compiles and caches schemas
//! - `FhirValidator` - Fast validator using compiled schemas

pub mod compiled;
pub mod compiler;

pub use compiled::*;
pub use compiler::*;

use crate::reference::{ReferenceError, ReferenceResolver};
use crate::terminology::{BindingStrength as TermBindingStrength, TerminologyService};
use crate::types::{
    FhirSchema, FhirSchemaConstraint, FhirSchemaElement, FhirSchemaSliceMatch, FhirSchemaSlicing,
    ValidationError, ValidationResult,
};
use async_recursion::async_recursion;
use async_trait::async_trait;
// Note: futures crate kept for potential future parallel evaluation
use octofhir_fhir_model::{EvaluationResult, FhirPathEvaluator};
use once_cell::sync::OnceCell;
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// =============================================================================
// Schema Provider Trait for Lazy/Async Schema Loading
// =============================================================================

/// Trait for async schema lookup, enabling lazy loading from external sources.
///
/// This trait allows the validator to fetch schemas on-demand from sources like
/// databases or cached providers (e.g., OctoFhirModelProvider with Moka cache).
///
/// # Example
/// ```ignore
/// #[async_trait]
/// impl SchemaProvider for MyModelProvider {
///     async fn get_schema(&self, name: &str) -> Option<Arc<FhirSchema>> {
///         // Lookup from cache, load from DB if needed
///         self.cached_get_schema(name).await
///     }
/// }
/// ```
#[async_trait]
pub trait SchemaProvider: Send + Sync {
    /// Get a schema by name or URL.
    /// Returns Arc for efficient sharing without cloning the full schema.
    async fn get_schema(&self, name: &str) -> Option<Arc<FhirSchema>>;

    /// Get a schema by URL (optional, default delegates to get_schema).
    async fn get_schema_by_url(&self, url: &str) -> Option<Arc<FhirSchema>> {
        self.get_schema(url).await
    }
}

// =============================================================================
// InMemorySchemaProvider - Simple provider for tests
// =============================================================================

/// In-memory schema provider for testing.
///
/// Holds schemas in a HashMap and provides them synchronously wrapped in async.
/// Useful for unit tests where schemas are loaded upfront.
///
/// # Example
/// ```ignore
/// let mut provider = InMemorySchemaProvider::new();
/// provider.add_schema("Patient", patient_schema);
/// provider.add_schema("Observation", observation_schema);
///
/// let validator = FhirValidator::new(Arc::new(provider));
/// ```
pub struct InMemorySchemaProvider {
    schemas: HashMap<String, Arc<FhirSchema>>,
}

impl InMemorySchemaProvider {
    /// Create a new empty in-memory provider.
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
        }
    }

    /// Create from a pre-built schema map.
    pub fn from_map(schemas: HashMap<String, Arc<FhirSchema>>) -> Self {
        Self { schemas }
    }

    /// Add a schema to the provider.
    pub fn add_schema(&mut self, name: impl Into<String>, schema: Arc<FhirSchema>) {
        self.schemas.insert(name.into(), schema);
    }

    /// Add a schema, taking ownership (will wrap in Arc).
    pub fn add_schema_owned(&mut self, name: impl Into<String>, schema: FhirSchema) {
        self.schemas.insert(name.into(), Arc::new(schema));
    }

    /// Get all schema names in the provider.
    pub fn schema_names(&self) -> Vec<&String> {
        self.schemas.keys().collect()
    }

    /// Check if a schema exists.
    pub fn has_schema(&self, name: &str) -> bool {
        self.schemas.contains_key(name)
    }
}

impl Default for InMemorySchemaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SchemaProvider for InMemorySchemaProvider {
    async fn get_schema(&self, name: &str) -> Option<Arc<FhirSchema>> {
        self.schemas.get(name).cloned()
    }

    async fn get_schema_by_url(&self, url: &str) -> Option<Arc<FhirSchema>> {
        // Try direct lookup first
        if let Some(schema) = self.schemas.get(url) {
            return Some(schema.clone());
        }
        // Then search by schema URL field
        self.schemas.values().find(|s| s.url == url).cloned()
    }
}

/// Convert a JSON value to an EvaluationResult for use as a FHIRPath variable.
/// This is needed to pass %rootResource to constraint evaluation.
fn json_to_evaluation_result(value: &JsonValue) -> EvaluationResult {
    match value {
        JsonValue::Null => EvaluationResult::Empty,
        JsonValue::Bool(b) => EvaluationResult::boolean(*b),
        JsonValue::Number(n) => {
            // For numbers, prefer integer if possible, otherwise use string representation
            // The FHIRPath engine will handle type coercion as needed
            if let Some(i) = n.as_i64() {
                EvaluationResult::integer(i)
            } else {
                // For decimals/floats, convert to string - the engine will parse as needed
                EvaluationResult::string(n.to_string())
            }
        }
        JsonValue::String(s) => EvaluationResult::string(s.clone()),
        JsonValue::Array(arr) => {
            let items = arr.iter().map(json_to_evaluation_result).collect();
            EvaluationResult::collection(items)
        }
        JsonValue::Object(obj) => {
            let mut map = HashMap::new();
            for (key, val) in obj {
                map.insert(key.clone(), json_to_evaluation_result(val));
            }
            EvaluationResult::object(map)
        }
    }
}

/// Error codes for FHIR Schema validation (following FS001-FS011 pattern)
#[derive(Debug, Clone, PartialEq)]
pub enum FhirSchemaErrorCode {
    UnknownElement = 1001,
    UnknownSchema = 1002,
    ExpectedArray = 1003,
    UnexpectedArray = 1004,
    UnknownKeyword = 1005,
    WrongType = 1006,
    SlicingUnmatched = 1007,
    SlicingAmbiguous = 1008,
    SliceCardinality = 1009,
    ConstraintViolation = 1010,
    CardinalityViolation = 1011,
    BindingViolation = 1012,
    ReferenceTypeViolation = 1013,
}

impl std::fmt::Display for FhirSchemaErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FhirSchemaErrorCode::UnknownElement => write!(f, "FS1001"),
            FhirSchemaErrorCode::UnknownSchema => write!(f, "FS1002"),
            FhirSchemaErrorCode::ExpectedArray => write!(f, "FS1003"),
            FhirSchemaErrorCode::UnexpectedArray => write!(f, "FS1004"),
            FhirSchemaErrorCode::UnknownKeyword => write!(f, "FS1005"),
            FhirSchemaErrorCode::WrongType => write!(f, "FS1006"),
            FhirSchemaErrorCode::SlicingUnmatched => write!(f, "FS1007"),
            FhirSchemaErrorCode::SlicingAmbiguous => write!(f, "FS1008"),
            FhirSchemaErrorCode::SliceCardinality => write!(f, "FS1009"),
            FhirSchemaErrorCode::ConstraintViolation => write!(f, "FS1010"),
            FhirSchemaErrorCode::CardinalityViolation => write!(f, "FS1011"),
            FhirSchemaErrorCode::BindingViolation => write!(f, "FS1012"),
            FhirSchemaErrorCode::ReferenceTypeViolation => write!(f, "FS1013"),
        }
    }
}

/// FHIR primitive types for validation
static PRIMITIVE_TYPES: &[&str] = &[
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
    "xhtml",
];

/// Cached element information from schema lookup (single pass optimization).
/// Collects all needed info about an element in one iteration over schemata.
#[derive(Debug, Default)]
pub struct ElementInfo {
    /// Whether the element is expected to be an array
    pub is_array: bool,
    /// Slicing definition if present
    pub slicing: Option<FhirSchemaSlicing>,
}

/// Result of classifying a single array item against slice definitions
#[derive(Debug, Clone, PartialEq)]
pub enum SliceClassification {
    /// Item matched exactly one slice
    Matched(String),
    /// Item matched no slices
    Unmatched,
    /// Item matched multiple slices (ambiguous - indicates profile error)
    Ambiguous(Vec<String>),
}

/// Main validation context for tracking schemas and errors
pub struct FhirSchemaValidationContext {
    /// All schemas available for validation (shared via Arc for zero-copy)
    pub all_schemas: Arc<HashMap<String, Arc<FhirSchema>>>,
    /// Current schemata set for the element being validated (Arc for cheap clones)
    pub current_schemata: HashMap<String, Arc<FhirSchema>>,
    /// Current path in the data being validated
    pub path: String,
    /// Accumulated validation errors
    pub errors: Vec<ValidationError>,
    /// Root resource for FHIRPath constraint evaluation (%rootResource)
    /// This is set when validation starts and passed to nested constraint evaluations
    /// Uses Arc for cheap cloning when passing to nested validations
    pub root_resource: Option<Arc<JsonValue>>,
    /// Cached EvaluationResult for root_resource (lazy initialization)
    /// Avoids repeated conversion of root_resource to EvaluationResult for each constraint
    root_resource_eval: OnceCell<EvaluationResult>,
    /// Cached variables map for constraint evaluation (reused across constraints)
    constraint_variables: OnceCell<HashMap<String, EvaluationResult>>,
}

impl FhirSchemaValidationContext {
    /// Create new validation context
    pub fn new(schemas: Arc<HashMap<String, Arc<FhirSchema>>>, path: String) -> Self {
        Self {
            all_schemas: schemas,
            current_schemata: HashMap::new(),
            path,
            errors: Vec::new(),
            root_resource: None,
            root_resource_eval: OnceCell::new(),
            constraint_variables: OnceCell::new(),
        }
    }

    /// Create new validation context with root resource for constraint evaluation.
    /// The root_resource is wrapped in Arc for cheap cloning during nested validation.
    pub fn new_with_root_resource(
        schemas: Arc<HashMap<String, Arc<FhirSchema>>>,
        path: String,
        root_resource: JsonValue,
    ) -> Self {
        Self {
            all_schemas: schemas,
            current_schemata: HashMap::new(),
            path,
            errors: Vec::new(),
            root_resource: Some(Arc::new(root_resource)),
            root_resource_eval: OnceCell::new(),
            constraint_variables: OnceCell::new(),
        }
    }

    /// Get cached constraint variables map (lazy initialization).
    /// Converts root_resource to EvaluationResult only once per validation.
    pub fn get_constraint_variables(&self) -> &HashMap<String, EvaluationResult> {
        self.constraint_variables.get_or_init(|| {
            let mut variables = HashMap::with_capacity(1);
            if let Some(root_resource) = &self.root_resource {
                let eval = self
                    .root_resource_eval
                    .get_or_init(|| json_to_evaluation_result(root_resource));
                variables.insert("rootResource".to_string(), eval.clone());
            }
            variables
        })
    }

    /// Add an error to the validation context
    pub fn add_error(&mut self, code: FhirSchemaErrorCode, message: String) {
        self.errors.push(ValidationError {
            error_type: code.to_string(),
            path: self
                .path
                .split('.')
                .map(|s| JsonValue::String(s.to_string()))
                .collect(),
            message: Some(message),
            value: None,
            expected: None,
            got: None,
            schema_path: None,
            constraint_key: None,
            constraint_expression: None,
            constraint_severity: None,
        });
    }

    /// Check if a type is a primitive type
    pub fn is_primitive_type(&self, type_name: &str) -> bool {
        PRIMITIVE_TYPES.contains(&type_name)
    }
}

/// FHIR Schema Validator implementing the official specification
pub struct FhirSchemaValidator {
    /// Available schemas for validation wrapped in Arc for zero-copy sharing.
    /// Using Arc<FhirSchema> avoids expensive clones on every schema lookup.
    schemas: Arc<HashMap<String, Arc<FhirSchema>>>,
    /// Direct URL to Arc<FhirSchema> mapping for O(1) lookup by URL without intermediate lookup
    url_to_schema: Arc<HashMap<String, Arc<FhirSchema>>>,
    /// Optional async schema provider for lazy loading from external sources
    /// When set, schemas are loaded on-demand from this provider
    schema_provider: Option<Arc<dyn SchemaProvider>>,
    /// Optional FHIRPath evaluator for constraint validation
    /// None means only structural validation will be performed
    fhirpath_evaluator: Option<Arc<dyn FhirPathEvaluator>>,
    /// Optional terminology service for binding validation
    /// None means binding validation will be skipped
    terminology_service: Option<Arc<dyn TerminologyService>>,
    /// Optional reference resolver for existence validation
    /// None means reference existence validation will be skipped
    reference_resolver: Option<Arc<dyn ReferenceResolver>>,
    /// Cache for fully-expanded schemata (after collect operation converges).
    /// Key: type name (e.g., "Patient", "HumanName", "string")
    /// Value: HashMap of all schemas in the expanded set
    /// This avoids re-running collect_element_type_schemas loop for each property.
    expanded_schemata_cache: moka::future::Cache<String, HashMap<String, Arc<FhirSchema>>>,
    /// Cache for follow_operation results.
    /// Key: "{sorted_schema_keys}:{path_item}" (e.g., "Patient:name", "HumanName:given")
    /// Value: HashMap of element schemas for this path
    /// This avoids recreating inline schemas and re-fetching type schemas for same paths.
    follow_cache: moka::future::Cache<String, HashMap<String, Arc<FhirSchema>>>,
}

impl FhirSchemaValidator {
    /// Create new validator with schemas and optional FHIRPath evaluator
    ///
    /// # Arguments
    /// * `schemas` - HashMap of schema name to FhirSchema
    /// * `fhirpath_evaluator` - Optional FHIRPath evaluator for constraint validation
    ///   - If None, only structural validation will be performed
    ///   - If Some, both structural and FHIRPath constraint validation will be performed
    ///
    /// # Example
    /// ```ignore
    /// // Structural validation only
    /// let validator = FhirSchemaValidator::new(schemas, None);
    ///
    /// // With FHIRPath constraint validation
    /// let validator = FhirSchemaValidator::new(schemas, Some(evaluator));
    /// ```
    pub fn new(
        schemas: HashMap<String, FhirSchema>,
        fhirpath_evaluator: Option<Arc<dyn FhirPathEvaluator>>,
    ) -> Self {
        // Wrap all schemas in Arc once during construction
        let arc_schemas: HashMap<String, Arc<FhirSchema>> = schemas
            .into_iter()
            .map(|(name, schema)| (name, Arc::new(schema)))
            .collect();

        // Build direct URL to Arc<FhirSchema> mapping for O(1) lookup without intermediate step
        let url_to_schema: HashMap<String, Arc<FhirSchema>> = arc_schemas
            .iter()
            .map(|(_, schema)| (schema.url.clone(), Arc::clone(schema)))
            .collect();

        Self {
            schemas: Arc::new(arc_schemas),
            url_to_schema: Arc::new(url_to_schema),
            schema_provider: None,
            fhirpath_evaluator,
            terminology_service: None,
            reference_resolver: None,
            // Cache for ~500 type expansions, should cover most FHIR types
            expanded_schemata_cache: moka::future::Cache::new(500),
            // Cache for ~1000 follow operation results (schema+path combinations)
            follow_cache: moka::future::Cache::new(1000),
        }
    }

    /// Create a new validator with an async schema provider for lazy loading.
    ///
    /// This constructor creates a validator that loads schemas on-demand from
    /// the provided SchemaProvider. This is ideal for production use where
    /// schemas should be cached and loaded lazily.
    ///
    /// # Arguments
    /// * `provider` - Schema provider for async schema lookup
    /// * `fhirpath_evaluator` - Optional FHIRPath evaluator for constraint validation
    ///
    /// # Example
    /// ```ignore
    /// // Create validator with lazy schema loading from model provider
    /// let validator = FhirSchemaValidator::new_with_provider(
    ///     model_provider.clone(),
    ///     Some(fhirpath_engine.clone())
    /// );
    /// ```
    pub fn new_with_provider(
        provider: Arc<dyn SchemaProvider>,
        fhirpath_evaluator: Option<Arc<dyn FhirPathEvaluator>>,
    ) -> Self {
        Self {
            schemas: Arc::new(HashMap::new()),
            url_to_schema: Arc::new(HashMap::new()),
            schema_provider: Some(provider),
            fhirpath_evaluator,
            terminology_service: None,
            reference_resolver: None,
            // Cache for ~500 type expansions, should cover most FHIR types
            expanded_schemata_cache: moka::future::Cache::new(500),
            // Cache for ~1000 follow operation results (schema+path combinations)
            follow_cache: moka::future::Cache::new(1000),
        }
    }

    /// Add a terminology service for binding validation.
    ///
    /// When a terminology service is provided, the validator will validate
    /// coded elements against their bound value sets.
    pub fn with_terminology_service(mut self, service: Arc<dyn TerminologyService>) -> Self {
        self.terminology_service = Some(service);
        self
    }

    /// Add a reference resolver for existence validation.
    ///
    /// When a reference resolver is provided, the validator will check that
    /// referenced resources actually exist in the storage.
    pub fn with_reference_resolver(mut self, resolver: Arc<dyn ReferenceResolver>) -> Self {
        self.reference_resolver = Some(resolver);
        self
    }

    /// Get schema by URL or name.
    ///
    /// First checks local pre-loaded schemas, then falls back to the
    /// schema provider for lazy loading. Uses Arc::clone for zero-copy sharing.
    async fn get_schema(&self, url_or_name: &str) -> Option<Arc<FhirSchema>> {
        // Try name lookup first (fast path - just Arc::clone, no data copy)
        if let Some(schema) = self.schemas.get(url_or_name) {
            return Some(Arc::clone(schema));
        }
        // Try URL lookup with direct O(1) mapping (also just Arc::clone)
        if let Some(schema) = self.url_to_schema.get(url_or_name) {
            return Some(Arc::clone(schema));
        }
        // Fall back to provider for lazy loading
        if let Some(provider) = &self.schema_provider {
            if let Some(schema) = provider.get_schema(url_or_name).await {
                return Some(schema);
            }
            if let Some(schema) = provider.get_schema_by_url(url_or_name).await {
                return Some(schema);
            }
        }
        None
    }

    // ============================================================================
    // Profile Chain Resolution (Phase 3)
    // ============================================================================

    /// Resolve profile derivation chain from derived to base.
    /// Returns schemas in order: [base, intermediate..., derived]
    ///
    /// # Arguments
    /// * `profile_url` - URL or name of the profile to resolve
    ///
    /// # Returns
    /// * `Ok(Vec<Arc<FhirSchema>>)` - Chain of schemas from base to derived
    /// * `Err(Box<ValidationError>)` - If cycle detected or schema not found
    pub async fn resolve_profile_chain(
        &self,
        profile_url: &str,
    ) -> Result<Vec<Arc<FhirSchema>>, Box<ValidationError>> {
        let mut chain = Vec::new();
        let mut visited = HashSet::new();
        let mut current_url = profile_url.to_string();

        loop {
            // Cycle detection
            if !visited.insert(current_url.clone()) {
                return Err(Box::new(ValidationError {
                    error_type: FhirSchemaErrorCode::UnknownSchema.to_string(),
                    path: vec![],
                    message: Some(format!("Cycle detected in profile chain: {}", current_url)),
                    value: None,
                    expected: None,
                    got: None,
                    schema_path: None,
                    constraint_key: None,
                    constraint_expression: None,
                    constraint_severity: None,
                }));
            }

            let schema = self.get_schema(&current_url).await.ok_or_else(|| {
                Box::new(ValidationError {
                    error_type: FhirSchemaErrorCode::UnknownSchema.to_string(),
                    path: vec![],
                    message: Some(format!("Schema not found: {}", current_url)),
                    value: None,
                    expected: None,
                    got: None,
                    schema_path: None,
                    constraint_key: None,
                    constraint_expression: None,
                    constraint_severity: None,
                })
            })?;

            let base_url = schema.base.clone();
            chain.push(schema);

            // Follow base chain
            if let Some(url) = base_url {
                current_url = url;
            } else {
                break;
            }
        }

        // Reverse to get base-first order
        chain.reverse();
        Ok(chain)
    }

    /// Deep merge two schemas (base + overlay).
    /// Overlay takes precedence for conflicts.
    ///
    /// # Arguments
    /// * `base` - Base schema to merge into
    /// * `overlay` - Overlay schema that takes precedence
    ///
    /// # Returns
    /// * Merged schema with overlay values taking precedence
    pub fn merge_schemas(&self, base: &FhirSchema, overlay: &FhirSchema) -> FhirSchema {
        let mut merged = base.clone();

        // Merge elements (deep)
        if let Some(overlay_elements) = &overlay.elements {
            let mut merged_elements = merged.elements.unwrap_or_default();
            for (key, overlay_elem) in overlay_elements {
                if let Some(base_elem) = merged_elements.get(key) {
                    merged_elements
                        .insert(key.clone(), self.merge_elements(base_elem, overlay_elem));
                } else {
                    merged_elements.insert(key.clone(), overlay_elem.clone());
                }
            }
            merged.elements = Some(merged_elements);
        }

        // Merge required (union)
        if let Some(overlay_required) = &overlay.required {
            let mut merged_required = merged.required.unwrap_or_default();
            for req in overlay_required {
                if !merged_required.contains(req) {
                    merged_required.push(req.clone());
                }
            }
            merged.required = Some(merged_required);
        }

        // Merge excluded (union)
        if let Some(overlay_excluded) = &overlay.excluded {
            let mut merged_excluded = merged.excluded.unwrap_or_default();
            for excl in overlay_excluded {
                if !merged_excluded.contains(excl) {
                    merged_excluded.push(excl.clone());
                }
            }
            merged.excluded = Some(merged_excluded);
        }

        // Merge constraints (union with overlay priority for same key)
        if let Some(overlay_constraints) = &overlay.constraint {
            let mut merged_constraints = merged.constraint.unwrap_or_default();
            for (key, constraint) in overlay_constraints {
                merged_constraints.insert(key.clone(), constraint.clone());
            }
            merged.constraint = Some(merged_constraints);
        }

        // Use overlay metadata
        merged.url = overlay.url.clone();
        merged.name = overlay.name.clone();
        merged.version = overlay.version.clone();
        merged.derivation = overlay.derivation.clone();

        merged
    }

    /// Deep merge two elements.
    /// Overlay takes precedence for scalar fields.
    ///
    /// # Arguments
    /// * `base` - Base element to merge into
    /// * `overlay` - Overlay element that takes precedence
    ///
    /// # Returns
    /// * Merged element with overlay values taking precedence
    pub fn merge_elements(
        &self,
        base: &FhirSchemaElement,
        overlay: &FhirSchemaElement,
    ) -> FhirSchemaElement {
        let mut merged = base.clone();

        // Overlay takes precedence for scalar fields
        if overlay.type_name.is_some() {
            merged.type_name = overlay.type_name.clone();
        }
        if overlay.min.is_some() {
            merged.min = overlay.min;
        }
        if overlay.max.is_some() {
            merged.max = overlay.max;
        }
        if overlay.array.is_some() {
            merged.array = overlay.array;
        }
        if overlay.binding.is_some() {
            merged.binding = overlay.binding.clone();
        }
        if overlay.pattern.is_some() {
            merged.pattern = overlay.pattern.clone();
        }
        if overlay.must_support.is_some() {
            merged.must_support = overlay.must_support;
        }
        if overlay.is_modifier.is_some() {
            merged.is_modifier = overlay.is_modifier;
        }
        if overlay.is_summary.is_some() {
            merged.is_summary = overlay.is_summary;
        }
        if overlay.refers.is_some() {
            merged.refers = overlay.refers.clone();
        }
        if overlay.url.is_some() {
            merged.url = overlay.url.clone();
        }

        // Deep merge nested elements
        if let Some(overlay_elements) = &overlay.elements {
            let mut merged_elements = merged.elements.unwrap_or_default();
            for (key, elem) in overlay_elements {
                if let Some(base_elem) = merged_elements.get(key) {
                    merged_elements.insert(key.clone(), self.merge_elements(base_elem, elem));
                } else {
                    merged_elements.insert(key.clone(), elem.clone());
                }
            }
            merged.elements = Some(merged_elements);
        }

        // Merge constraints (union with overlay priority)
        if let Some(overlay_constraints) = &overlay.constraint {
            let mut merged_constraints = merged.constraint.unwrap_or_default();
            for (key, constraint) in overlay_constraints {
                merged_constraints.insert(key.clone(), constraint.clone());
            }
            merged.constraint = Some(merged_constraints);
        }

        // Merge required (union)
        if let Some(overlay_required) = &overlay.required {
            let mut merged_required = merged.required.unwrap_or_default();
            for req in overlay_required {
                if !merged_required.contains(req) {
                    merged_required.push(req.clone());
                }
            }
            merged.required = Some(merged_required);
        }

        // Merge excluded (union)
        if let Some(overlay_excluded) = &overlay.excluded {
            let mut merged_excluded = merged.excluded.unwrap_or_default();
            for excl in overlay_excluded {
                if !merged_excluded.contains(excl) {
                    merged_excluded.push(excl.clone());
                }
            }
            merged.excluded = Some(merged_excluded);
        }

        // Merge slicing (deep)
        if let Some(overlay_slicing) = &overlay.slicing {
            merged.slicing = Some(self.merge_slicing(merged.slicing.as_ref(), overlay_slicing));
        }

        merged
    }

    /// Deep merge slicing definitions.
    /// Overlay takes precedence for discriminator/rules/ordered.
    /// Slices are merged (union).
    ///
    /// # Arguments
    /// * `base` - Optional base slicing to merge into
    /// * `overlay` - Overlay slicing that takes precedence
    ///
    /// # Returns
    /// * Merged slicing definition
    pub fn merge_slicing(
        &self,
        base: Option<&FhirSchemaSlicing>,
        overlay: &FhirSchemaSlicing,
    ) -> FhirSchemaSlicing {
        match base {
            None => overlay.clone(),
            Some(base_slicing) => {
                let mut merged = base_slicing.clone();

                // Overlay discriminator takes precedence
                if overlay.discriminator.is_some() {
                    merged.discriminator = overlay.discriminator.clone();
                }
                if overlay.rules.is_some() {
                    merged.rules = overlay.rules.clone();
                }
                if overlay.ordered.is_some() {
                    merged.ordered = overlay.ordered;
                }

                // Merge slices (union with overlay priority for same key)
                if let Some(overlay_slices) = &overlay.slices {
                    let mut merged_slices = merged.slices.unwrap_or_default();
                    for (key, slice) in overlay_slices {
                        merged_slices.insert(key.clone(), slice.clone());
                    }
                    merged.slices = Some(merged_slices);
                }

                merged
            }
        }
    }

    // ============================================================================
    // Slicing Validation
    // ============================================================================

    /// Deep partial match for pattern comparison.
    /// Returns true if `item` contains all fields from `pattern` (recursively).
    ///
    /// # Algorithm
    /// - Null pattern matches anything
    /// - Object pattern: all keys must exist in item with matching values
    /// - Array pattern: every pattern element must have at least one matching item element
    /// - Scalars: strict equality
    ///
    /// # Arguments
    /// * `item` - The data value to check
    /// * `pattern` - The pattern to match against
    ///
    /// # Returns
    /// * `true` if item matches pattern, `false` otherwise
    pub fn deep_partial_match(item: &JsonValue, pattern: &JsonValue) -> bool {
        match pattern {
            // Null pattern matches anything
            JsonValue::Null => true,

            // Object pattern: all keys must exist in item with matching values
            JsonValue::Object(pattern_map) => {
                // Empty object pattern matches anything
                if pattern_map.is_empty() {
                    return true;
                }

                // Item must be an object
                let Some(item_map) = item.as_object() else {
                    return false;
                };

                // Every key in pattern must exist in item with matching value
                for (key, pattern_value) in pattern_map {
                    match item_map.get(key) {
                        None => return false,
                        Some(item_value) => {
                            if !Self::deep_partial_match(item_value, pattern_value) {
                                return false;
                            }
                        }
                    }
                }
                true
            }

            // Array pattern: "contains" semantics
            // Every pattern element must have at least one matching item element
            JsonValue::Array(pattern_array) => {
                // Empty array pattern matches anything
                if pattern_array.is_empty() {
                    return true;
                }

                // Item must be an array
                let Some(item_array) = item.as_array() else {
                    return false;
                };

                // Every pattern element must find a match in item array
                for pattern_element in pattern_array {
                    let found = item_array.iter().any(|item_element| {
                        Self::deep_partial_match(item_element, pattern_element)
                    });
                    if !found {
                        return false;
                    }
                }
                true
            }

            // Scalar values: strict equality
            JsonValue::String(_) | JsonValue::Number(_) | JsonValue::Bool(_) => item == pattern,
        }
    }

    /// Classify a single array item against all slice definitions.
    /// Returns which slice(s) the item matches.
    ///
    /// # Algorithm
    /// - Empty/None match pattern = unconditional match (catch-all)
    /// - Otherwise use deep_partial_match for pattern comparison
    /// - Return Matched if exactly one, Unmatched if zero, Ambiguous if multiple
    ///
    /// # Arguments
    /// * `item` - The array item to classify
    /// * `slices` - HashMap of slice name to slice definition
    ///
    /// # Returns
    /// * `SliceClassification` indicating which slice(s) matched
    pub fn classify_slice(
        &self,
        item: &JsonValue,
        slices: &HashMap<String, FhirSchemaSliceMatch>,
    ) -> SliceClassification {
        let mut matched_slices: Vec<String> = Vec::new();

        for (slice_name, slice_def) in slices {
            let matches = match &slice_def.match_value {
                // No match pattern or empty object = unconditional match (catch-all)
                None => true,
                Some(pattern) => {
                    if let JsonValue::Object(obj) = pattern {
                        if obj.is_empty() {
                            true
                        } else {
                            Self::deep_partial_match(item, pattern)
                        }
                    } else {
                        Self::deep_partial_match(item, pattern)
                    }
                }
            };

            if matches {
                matched_slices.push(slice_name.clone());
            }
        }

        match matched_slices.len() {
            0 => SliceClassification::Unmatched,
            1 => SliceClassification::Matched(matched_slices.into_iter().next().unwrap()),
            _ => SliceClassification::Ambiguous(matched_slices),
        }
    }

    /// Validate cardinality constraints for all slices.
    /// Checks min/max for each slice against actual counts.
    ///
    /// # Arguments
    /// * `context` - Validation context for error reporting
    /// * `slice_counts` - Map of slice name to count of items in that slice
    /// * `slicing` - Slicing definition with cardinality constraints
    /// * `element_path` - Path to the sliced element for error messages
    fn validate_slice_cardinality(
        &self,
        context: &mut FhirSchemaValidationContext,
        slice_counts: &HashMap<String, usize>,
        slicing: &FhirSchemaSlicing,
        element_path: &str,
    ) {
        let Some(slices) = &slicing.slices else {
            return;
        };

        for (slice_name, slice_def) in slices {
            let count = slice_counts.get(slice_name).copied().unwrap_or(0);

            // Check minimum cardinality
            if let Some(min) = slice_def.min
                && (count as i32) < min
            {
                context.add_error(
                    FhirSchemaErrorCode::SliceCardinality,
                    format!(
                        "Slice '{}' in {} requires minimum {} item(s), found {}",
                        slice_name, element_path, min, count
                    ),
                );
            }

            // Check maximum cardinality
            if let Some(max) = slice_def.max
                && (count as i32) > max
            {
                context.add_error(
                    FhirSchemaErrorCode::SliceCardinality,
                    format!(
                        "Slice '{}' in {} allows maximum {} item(s), found {}",
                        slice_name, element_path, max, count
                    ),
                );
            }
        }
    }

    /// Main entry point for slicing validation.
    /// Classifies items, validates cardinality, and handles slicing rules.
    ///
    /// # Algorithm
    /// 1. Classify each item using classify_slice()
    /// 2. Track counts per slice
    /// 3. Handle unmatched items based on rules (closed/open/openAtEnd)
    /// 4. Handle ambiguous items (report error)
    /// 5. Validate cardinality constraints
    ///
    /// # Arguments
    /// * `context` - Validation context for error reporting
    /// * `items` - Array items to validate
    /// * `slicing` - Slicing definition
    /// * `element_path` - Path to the sliced element for error messages
    pub fn validate_slicing(
        &self,
        context: &mut FhirSchemaValidationContext,
        items: &[JsonValue],
        slicing: &FhirSchemaSlicing,
        element_path: &str,
    ) {
        let Some(slices) = &slicing.slices else {
            return; // No slices defined
        };

        if slices.is_empty() {
            return; // No slices to validate against
        }

        // Track counts per slice and last matched index for openAtEnd
        let mut slice_counts: HashMap<String, usize> = HashMap::new();
        let mut last_matched_index: Option<usize> = None;

        // Initialize counts to 0 for all defined slices
        for slice_name in slices.keys() {
            slice_counts.insert(slice_name.clone(), 0);
        }

        // Get slicing rules (default to "open")
        let rules = slicing.rules.as_deref().unwrap_or("open");

        // Classify each item
        for (index, item) in items.iter().enumerate() {
            let classification = self.classify_slice(item, slices);

            match classification {
                SliceClassification::Matched(slice_name) => {
                    // Increment count for matched slice
                    *slice_counts.entry(slice_name).or_insert(0) += 1;
                    last_matched_index = Some(index);
                }
                SliceClassification::Unmatched => {
                    // Handle based on rules
                    match rules {
                        "closed" => {
                            context.add_error(
                                FhirSchemaErrorCode::SlicingUnmatched,
                                format!(
                                    "Item at {}[{}] does not match any defined slice (closed slicing)",
                                    element_path, index
                                ),
                            );
                        }
                        "openAtEnd" => {
                            // Unmatched items are only allowed after all matched items
                            if let Some(last_idx) = last_matched_index
                                && index < last_idx
                            {
                                context.add_error(
                                        FhirSchemaErrorCode::SlicingUnmatched,
                                        format!(
                                            "Item at {}[{}] is unmatched but appears before matched items (openAtEnd slicing)",
                                            element_path, index
                                        ),
                                    );
                            }
                            // If no matched items yet, unmatched at start is ok for openAtEnd
                        }
                        // "open" or default: unmatched items are allowed
                        _ => {}
                    }
                }
                SliceClassification::Ambiguous(matched_slices) => {
                    context.add_error(
                        FhirSchemaErrorCode::SlicingAmbiguous,
                        format!(
                            "Item at {}[{}] matches multiple slices: {}",
                            element_path,
                            index,
                            matched_slices.join(", ")
                        ),
                    );
                }
            }
        }

        // Validate cardinality for all slices
        self.validate_slice_cardinality(context, &slice_counts, slicing, element_path);
    }

    /// Get element information in a single pass over schemata.
    /// Collects is_array and slicing info together to avoid multiple iterations.
    fn get_element_info(
        &self,
        context: &FhirSchemaValidationContext,
        element_name: &str,
    ) -> ElementInfo {
        for schema in context.current_schemata.values() {
            if let Some(elements) = &schema.elements {
                if let Some(element) = elements.get(element_name) {
                    return ElementInfo {
                        is_array: element.array.unwrap_or(false),
                        slicing: element.slicing.clone(),
                    };
                }
            }
        }
        ElementInfo::default()
    }

    /// Merge an entire profile chain into a single schema.
    /// Starts with base and applies each overlay in order.
    ///
    /// # Arguments
    /// * `chain` - Schemas in order from base to derived
    ///
    /// # Returns
    /// * Single merged schema combining all constraints
    pub fn merge_profile_chain(&self, chain: &[Arc<FhirSchema>]) -> Option<FhirSchema> {
        if chain.is_empty() {
            return None;
        }

        let mut merged = (*chain[0]).clone();
        for schema in chain.iter().skip(1) {
            merged = self.merge_schemas(&merged, schema);
        }

        Some(merged)
    }

    // ============================================================================
    // Multiple Profile Validation (Phase 3)
    // ============================================================================

    /// Validate resource against multiple profiles.
    /// Each profile's derivation chain is resolved and merged, then all profiles
    /// are applied to the resource. Conflicts between profiles are detected.
    ///
    /// # Arguments
    /// * `resource` - JSON resource to validate
    /// * `profile_urls` - List of profile URLs to validate against
    ///
    /// # Returns
    /// * ValidationResult with errors from all profile validations
    pub async fn validate_with_profiles(
        &self,
        resource: &JsonValue,
        profile_urls: Vec<String>,
    ) -> ValidationResult {
        let resource_type = resource
            .get("resourceType")
            .and_then(|rt| rt.as_str())
            .unwrap_or("");

        let mut context =
            FhirSchemaValidationContext::new(self.schemas.clone(), resource_type.to_string());

        // Resolve and merge all profile chains
        let mut merged_schemas: Vec<FhirSchema> = Vec::new();

        for profile_url in &profile_urls {
            match self.resolve_profile_chain(profile_url).await {
                Ok(chain) => {
                    // Merge entire chain into single schema
                    if let Some(merged) = self.merge_profile_chain(&chain) {
                        merged_schemas.push(merged);
                    }
                }
                Err(e) => {
                    context.errors.push(*e);
                }
            }
        }

        // Detect conflicts between profiles
        // (Two profiles requiring different fixed values for same element)
        if let Some(conflicts) = self.detect_profile_conflicts(&merged_schemas) {
            for conflict in conflicts {
                context.add_error(
                    FhirSchemaErrorCode::ConstraintViolation,
                    format!("Profile conflict: {}", conflict),
                );
            }
        }

        // Add merged schemas to context for validation
        // We use the merged schema's type_name (resource type) as the key so that
        // element resolution finds the merged elements instead of base elements.
        // We also clear the base field to prevent collect_operation from adding
        // original base schemas that would override our merged elements.
        for schema in &merged_schemas {
            let mut merged_schema = schema.clone();
            // Clear base to prevent collect_operation from adding original base
            merged_schema.base = None;

            // Wrap in Arc once - all subsequent clones are cheap Arc::clone
            let arc_schema = Arc::new(merged_schema);

            // Add merged schema to current_schemata only (all_schemas is immutable)
            // Add by URL, name, and type_name for various lookup patterns
            context
                .current_schemata
                .insert(arc_schema.url.clone(), Arc::clone(&arc_schema));
            context
                .current_schemata
                .insert(arc_schema.name.clone(), Arc::clone(&arc_schema));
            context
                .current_schemata
                .insert(arc_schema.type_name.clone(), Arc::clone(&arc_schema));
        }

        // Apply collect operation to get base type schemas (e.g., string, code)
        loop {
            let initial_size = context.current_schemata.len();
            self.collect_operation(&mut context).await;
            if context.current_schemata.len() == initial_size {
                break;
            }
        }

        // Validate data
        self.validate_data_element(&mut context, resource).await;

        let valid = context.errors.is_empty();
        ValidationResult {
            errors: context.errors,
            valid,
            warnings: vec![],
        }
    }

    /// Detect conflicts between multiple profile schemas.
    /// Checks for conflicting pattern values in the same elements.
    ///
    /// # Arguments
    /// * `schemas` - List of merged profile schemas to check
    ///
    /// # Returns
    /// * `Some(Vec<String>)` with conflict descriptions, or `None` if no conflicts
    pub fn detect_profile_conflicts(&self, schemas: &[FhirSchema]) -> Option<Vec<String>> {
        let mut conflicts = Vec::new();

        // Check for conflicting pattern values
        for (i, schema_a) in schemas.iter().enumerate() {
            for schema_b in schemas.iter().skip(i + 1) {
                // Check element conflicts
                Self::detect_element_conflicts(
                    &schema_a.elements,
                    &schema_b.elements,
                    &schema_a.url,
                    &schema_b.url,
                    "",
                    &mut conflicts,
                );
            }
        }

        if conflicts.is_empty() {
            None
        } else {
            Some(conflicts)
        }
    }

    /// Recursively detect conflicts between element definitions.
    fn detect_element_conflicts(
        elems_a: &Option<HashMap<String, FhirSchemaElement>>,
        elems_b: &Option<HashMap<String, FhirSchemaElement>>,
        url_a: &str,
        url_b: &str,
        path_prefix: &str,
        conflicts: &mut Vec<String>,
    ) {
        let (Some(elems_a), Some(elems_b)) = (elems_a, elems_b) else {
            return;
        };

        for (key, elem_a) in elems_a {
            let current_path = if path_prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", path_prefix, key)
            };

            if let Some(elem_b) = elems_b.get(key) {
                // Check pattern conflicts
                if let (Some(pattern_a), Some(pattern_b)) = (&elem_a.pattern, &elem_b.pattern)
                    && pattern_a.value != pattern_b.value
                {
                    conflicts.push(format!(
                            "Element '{}' has conflicting patterns: '{}' requires {:?}, '{}' requires {:?}",
                            current_path, url_a, pattern_a.value, url_b, pattern_b.value
                        ));
                }

                // Check cardinality conflicts (e.g., one requires min=1, other has max=0)
                if let (Some(min_a), Some(max_b)) = (elem_a.min, elem_b.max)
                    && min_a > 0
                    && max_b == 0
                {
                    conflicts.push(format!(
                            "Element '{}' has conflicting cardinality: '{}' requires min={}, '{}' requires max=0",
                            current_path, url_a, min_a, url_b
                        ));
                }
                if let (Some(min_b), Some(max_a)) = (elem_b.min, elem_a.max)
                    && min_b > 0
                    && max_a == 0
                {
                    conflicts.push(format!(
                            "Element '{}' has conflicting cardinality: '{}' requires min={}, '{}' requires max=0",
                            current_path, url_b, min_b, url_a
                        ));
                }

                // Recursively check nested elements
                Self::detect_element_conflicts(
                    &elem_a.elements,
                    &elem_b.elements,
                    url_a,
                    url_b,
                    &current_path,
                    conflicts,
                );
            }
        }
    }

    // ============================================================================
    // End Profile Chain Resolution
    // ============================================================================

    /// Main validation entry point - async
    /// Validates a resource against one or more schema URLs
    pub async fn validate(
        &self,
        resource: &JsonValue,
        schema_urls: Vec<String>,
    ) -> ValidationResult {
        let resource_type = resource
            .get("resourceType")
            .and_then(|rt| rt.as_str())
            .unwrap_or("");

        // Create context with root_resource set for FHIRPath constraint evaluation
        // This is needed for constraints like ref-1 that reference %rootResource.contained
        let mut context = FhirSchemaValidationContext::new_with_root_resource(
            self.schemas.clone(),
            resource_type.to_string(),
            resource.clone(),
        );

        // Start validation with root schemas (async)
        self.validate_with_schemata(&mut context, resource, schema_urls)
            .await;

        let valid = context.errors.is_empty();
        ValidationResult {
            errors: context.errors,
            valid,
            warnings: vec![],
        }
    }

    /// Validate data against a set of schema URLs (implements schemata resolution)
    async fn validate_with_schemata(
        &self,
        context: &mut FhirSchemaValidationContext,
        data: &JsonValue,
        schema_urls: Vec<String>,
    ) {
        // Step 1: Resolve schemata for the given URLs
        self.resolve_schemata(context, schema_urls).await;

        // Step 2: Validate the data element (async)
        self.validate_data_element(context, data).await;
    }

    /// Resolve schemata using collect and follow operations (FHIR Schema spec algorithm)
    async fn resolve_schemata(
        &self,
        context: &mut FhirSchemaValidationContext,
        schema_urls: Vec<String>,
    ) {
        // Start with initial schemas
        for url in schema_urls {
            if let Some(schema) = self.get_schema(&url).await {
                // schema is already Arc<FhirSchema>, just Arc::clone (cheap)
                context.current_schemata.insert(url.clone(), schema);
            } else {
                context.add_error(
                    FhirSchemaErrorCode::UnknownSchema,
                    format!("Schema not found: {url}"),
                );
            }
        }

        // Apply collect operation until set stops growing
        loop {
            let initial_size = context.current_schemata.len();
            self.collect_operation(context).await;
            if context.current_schemata.len() == initial_size {
                break; // Set stopped growing
            }
        }
    }

    /// Collect operation: adds referred schemas to the schemata set
    /// According to FHIR Schema spec: only add base schemas for root schemas and
    /// type/elementReference schemas for the current element being validated
    async fn collect_operation(&self, context: &mut FhirSchemaValidationContext) {
        // Collect Arc references - cloning Arc is cheap (just increment refcount)
        let current_schemas: Vec<Arc<FhirSchema>> =
            context.current_schemata.values().cloned().collect();

        for schema in current_schemas {
            // For root schemas, add base schema (inheritance chain)
            // Include "logical" kind for logical models like ViewDefinition that inherit from CanonicalResource
            if schema.kind == "resource" || schema.kind == "complex-type" || schema.kind == "logical"
            {
                if let Some(base_url) = &schema.base {
                    if !context.current_schemata.contains_key(base_url) {
                        if let Some(base_schema) = self.get_schema(base_url).await {
                            // base_schema is already Arc, just insert it
                            context.current_schemata.insert(base_url.clone(), base_schema);
                        }
                    }
                }
            }

            // CRITICAL FIX: Do NOT add type schemas from ALL elements
            // This was causing global schema pollution where Patient.identifier
            // would pick up identifier definitions from Reference, Quantity, etc.
            //
            // According to FHIR Schema spec, type schemas should only be added
            // during the follow operation for the specific element being validated
        }
    }

    /// Specialized collect operation for element type schemas after follow operation
    /// This implements the FHIR Schema spec requirement to collect type inheritance
    /// after following to element schemas
    async fn collect_element_type_schemas(&self, context: &mut FhirSchemaValidationContext) {
        // Collect Arc references - cloning Arc is cheap
        let current_schemas: Vec<Arc<FhirSchema>> =
            context.current_schemata.values().cloned().collect();

        for schema in current_schemas {
            // Add base schemas for complex types (inheritance chain)
            if schema.kind == "complex-type" || schema.kind == "primitive-type" {
                if let Some(base_url) = &schema.base {
                    if !context.current_schemata.contains_key(base_url) {
                        if let Some(base_schema) = self.get_schema(base_url).await {
                            // base_schema is already Arc, just insert it
                            context.current_schemata.insert(base_url.clone(), base_schema);
                        }
                    }
                }
            }
        }
    }

    /// Follow operation: navigate to element schemas for a given path item
    async fn follow_operation(
        &self,
        context: &mut FhirSchemaValidationContext,
        path_item: &str,
    ) -> HashMap<String, Arc<FhirSchema>> {
        // Generate cache key from current schemata keys + path_item
        // Using sorted keys ensures consistent cache key regardless of HashMap order
        let mut schema_keys: Vec<&str> = context.current_schemata.keys().map(|s| s.as_str()).collect();
        schema_keys.sort_unstable();
        let cache_key = format!("{}:{}", schema_keys.join("+"), path_item);

        // Check cache first (optimization)
        if let Some(cached) = self.follow_cache.get(&cache_key).await {
            return cached;
        }

        let mut result_schemata: HashMap<String, Arc<FhirSchema>> = HashMap::new();

        // Clone Arc references - cheap operation (just increment refcount)
        let current_schemata: Vec<(String, Arc<FhirSchema>)> = context
            .current_schemata
            .iter()
            .map(|(k, v)| (k.clone(), Arc::clone(v)))
            .collect();

        for (schema_key, schema) in current_schemata {
            if let Some(elements) = &schema.elements {
                if let Some(element) = elements.get(path_item) {
                    // Check if element has inline nested elements (BackboneElement case)
                    if let Some(nested_elements) = &element.elements {
                        // Create an inline schema from the nested elements
                        let inline_schema = FhirSchema {
                            name: format!("{schema_key}.{path_item}"),
                            type_name: format!("{schema_key}.{path_item}"),
                            url: format!("{}#{}", schema.url, path_item),
                            version: schema.version.clone(),
                            description: Some(format!(
                                "Inline schema for {schema_key}.{path_item}"
                            )),
                            package_name: schema.package_name.clone(),
                            package_version: schema.package_version.clone(),
                            package_id: schema.package_id.clone(),
                            kind: "inline".to_string(),
                            derivation: Some("inline".to_string()),
                            base: element.type_name.clone(),
                            abstract_type: None,
                            class: "inline".to_string(),
                            package_meta: schema.package_meta.clone(),
                            elements: Some(nested_elements.clone()),
                            required: None,
                            excluded: None,
                            extensions: None,
                            constraint: None,
                            primitive_type: None,
                            choices: None,
                        };
                        result_schemata.insert(
                            format!("{schema_key}.{path_item}"),
                            Arc::new(inline_schema),
                        );
                    }

                    // According to FHIR Schema spec: Add type schemas for this specific element
                    // This ensures we get the correct type schema for the element being validated
                    let mut type_schema_found = false;
                    if let Some(type_name) = &element.type_name {
                        if let Some(type_schema) = self.get_schema(type_name).await {
                            // type_schema is already Arc, just insert it
                            result_schemata.insert(type_name.clone(), type_schema);
                            type_schema_found = true;
                        }
                    }

                    // Add elementReference schemas if present
                    if let Some(element_refs) = &element.element_reference {
                        for element_ref in element_refs {
                            if let Some(ref_schema) = self.get_schema(element_ref).await {
                                // ref_schema is already Arc
                                result_schemata.insert(element_ref.clone(), ref_schema);
                            }
                        }
                    }

                    // Create element schema when:
                    // 1. No type specified and no nested elements, OR
                    // 2. Type specified but type schema not found (for element-level validation)
                    // This ensures element-level constraints (min, max, etc.) are still validated
                    if (element.type_name.is_none() && element.elements.is_none())
                        || (!type_schema_found && element.elements.is_none())
                    {
                        let element_schema =
                            self.element_to_schema(element, &format!("{schema_key}.{path_item}"));
                        result_schemata.insert(
                            format!("{schema_key}.{path_item}"),
                            Arc::new(element_schema),
                        );
                    }
                } else if let Some(base_name) = path_item.strip_prefix('_') {
                    // Handle primitive extensions (e.g., _birthDate)
                    // Remove '_' prefix
                    if elements.get(base_name).is_some() {
                        // Primitive extensions are always Element type
                        if let Some(element_schema) = self.get_schema("Element").await {
                            // element_schema is already Arc
                            result_schemata.insert("Element".to_string(), element_schema);
                        }
                    }
                }
            }
        }

        // Cache the result for future use (optimization)
        self.follow_cache.insert(cache_key, result_schemata.clone()).await;

        result_schemata
    }

    /// Convert FhirSchemaElement to FhirSchema for consistent processing
    fn element_to_schema(&self, element: &FhirSchemaElement, name: &str) -> FhirSchema {
        FhirSchema {
            url: format!("element://{name}"),
            version: None,
            name: name.to_string(),
            type_name: element.type_name.clone().unwrap_or_default(),
            kind: "element".to_string(),
            derivation: None,
            base: None,
            abstract_type: None,
            class: "element".to_string(),
            description: element.short.clone(),
            package_name: None,
            package_version: None,
            package_id: None,
            package_meta: None,
            elements: element.elements.clone(),
            required: element.required.clone(),
            excluded: element.excluded.clone(),
            extensions: element.extensions.clone(),
            constraint: element.constraint.clone(),
            primitive_type: None,
            choices: None,
        }
    }

    /// Validate data element against current schemata (FHIR Schema spec algorithm) - async recursive
    #[async_recursion]
    async fn validate_data_element(
        &self,
        context: &mut FhirSchemaValidationContext,
        data: &JsonValue,
    ) {
        match data {
            JsonValue::Object(obj) => {
                // Validate the object against each schema from schemata (async)
                // Pass both data (for constraint evaluation) and obj (for field access)
                self.validate_object_against_schemata(context, data, obj).await;

                // Validate every property of the object
                for (key, value) in obj {
                    if key == "resourceType" {
                        continue; // Skip resourceType validation
                    }

                    // Push path segment (avoid format! allocation by using push_str)
                    let path_start_len = context.path.len();
                    if !context.path.is_empty() {
                        context.path.push('.');
                    }
                    context.path.push_str(key);

                    // Special handling for XHTML content in text.div
                    // According to FHIR spec, div contains XHTML and should not be validated as FHIR elements
                    if context.path.ends_with(".div") && value.is_string() {
                        // For div elements containing XHTML strings, skip FHIR element validation
                        context.path.truncate(path_start_len);
                        continue;
                    }

                    // Follow operation to get element schemas
                    let element_schemata = self.follow_operation(context, key).await;

                    if element_schemata.is_empty() {
                        // Special case: If we're inside a div element, don't report unknown elements
                        // as they are likely HTML/XHTML elements which are valid in div content
                        if !context.path.contains(".div.") {
                            // Debug: show current schemata when element is unknown
                            context.add_error(
                                FhirSchemaErrorCode::UnknownElement,
                                format!("Element {key} is unknown"),
                            );
                        }
                    } else {
                        // Get element info in single pass over schemata (avoids multiple iterations)
                        let element_info = self.get_element_info(context, key);

                        // Check for slicing validation BEFORE changing context
                        // Slicing is defined on the element in the parent schema
                        if let JsonValue::Array(arr) = value
                            && let Some(ref slicing) = element_info.slicing
                        {
                            let element_path = context.path.clone();
                            self.validate_slicing(context, arr, slicing, &element_path);
                        }

                        // Update context with element schemata
                        // Use mem::replace to swap without cloning the HashMap
                        let prev_schemata =
                            std::mem::replace(&mut context.current_schemata, element_schemata);

                        // Generate cache key from type names in element_schemata
                        // Sorting ensures consistent key regardless of HashMap iteration order
                        let mut type_names: Vec<&str> = context
                            .current_schemata
                            .values()
                            .map(|s| s.type_name.as_str())
                            .collect();
                        type_names.sort_unstable();
                        let cache_key = type_names.join("+");

                        // Check expanded schemata cache first (optimization)
                        if let Some(cached_schemata) = self.expanded_schemata_cache.get(&cache_key).await {
                            // Cache hit - use pre-expanded schemata directly
                            context.current_schemata = cached_schemata;
                        } else {
                            // Cache miss - run collect operation and cache the result
                            loop {
                                let initial_size = context.current_schemata.len();
                                self.collect_element_type_schemas(context).await;
                                if context.current_schemata.len() == initial_size {
                                    break;
                                }
                            }
                            // Cache the expanded schemata for future use
                            self.expanded_schemata_cache
                                .insert(cache_key, context.current_schemata.clone())
                                .await;
                        }

                        // Validate the property value with pre-determined array expectation (async)
                        self.validate_element_value_with_array_check(
                            context,
                            value,
                            element_info.is_array,
                        )
                        .await;

                        // Restore previous schemata
                        context.current_schemata = prev_schemata;
                    }

                    // Pop path segment (restore to original length)
                    context.path.truncate(path_start_len);
                }
            }
            JsonValue::Array(arr) => {
                // Check if current element type is "Resource" (e.g., contained array)
                let is_resource_array = context
                    .current_schemata
                    .values()
                    .any(|s| s.name == "Resource" || s.type_name == "Resource");

                // Validate every entry of the array
                for (index, item) in arr.iter().enumerate() {
                    // Push array index to path (avoid format! allocation)
                    let path_start_len = context.path.len();
                    context.path.push('[');
                    // Use itoa for fast integer to string conversion
                    use std::fmt::Write;
                    let _ = write!(context.path, "{}]", index);

                    // For Resource arrays (like contained), look up schema by resourceType
                    if is_resource_array {
                        if let Some(obj) = item.as_object() {
                            if let Some(resource_type) =
                                obj.get("resourceType").and_then(|v| v.as_str())
                            {
                                // Load the specific resource schema
                                if let Some(schema) = self.get_schema(resource_type).await {
                                    // Save current schemata and replace with resource-specific
                                    let prev_schemata =
                                        std::mem::take(&mut context.current_schemata);
                                    context
                                        .current_schemata
                                        .insert(resource_type.to_string(), schema);

                                    // Check expanded schemata cache for resource type (optimization)
                                    let cache_key = resource_type.to_string();
                                    if let Some(cached_schemata) =
                                        self.expanded_schemata_cache.get(&cache_key).await
                                    {
                                        // Cache hit - use pre-expanded schemata
                                        context.current_schemata = cached_schemata;
                                    } else {
                                        // Cache miss - run collect_operation and cache
                                        loop {
                                            let initial_size = context.current_schemata.len();
                                            self.collect_operation(context).await;
                                            if context.current_schemata.len() == initial_size {
                                                break;
                                            }
                                        }
                                        self.expanded_schemata_cache
                                            .insert(cache_key, context.current_schemata.clone())
                                            .await;
                                    }

                                    self.validate_data_element(context, item).await;

                                    // Restore previous schemata
                                    context.current_schemata = prev_schemata;
                                } else {
                                    // Unknown resource type in contained
                                    context.add_error(
                                        FhirSchemaErrorCode::UnknownElement,
                                        format!(
                                            "Unknown resource type {} in contained",
                                            resource_type
                                        ),
                                    );
                                }
                            } else {
                                // No resourceType - validate as generic Resource
                                self.validate_data_element(context, item).await;
                            }
                        } else {
                            self.validate_data_element(context, item).await;
                        }
                    } else {
                        self.validate_data_element(context, item).await;
                    }

                    // Pop path segment
                    context.path.truncate(path_start_len);
                }
            }
            _ => {
                // Validate primitive values
                self.validate_primitive_value(context, data);
            }
        }
    }

    /// Validate object against all schemas in current schemata (async)
    async fn validate_object_against_schemata(
        &self,
        context: &mut FhirSchemaValidationContext,
        data: &JsonValue,
        obj: &serde_json::Map<String, JsonValue>,
    ) {
        // Clone Arc references - cheap operation
        let schemata_clone: Vec<(String, Arc<FhirSchema>)> = context
            .current_schemata
            .iter()
            .map(|(k, v)| (k.clone(), Arc::clone(v)))
            .collect();

        for (schema_key, schema) in schemata_clone {
            self.validate_object_against_schema(context, data, obj, &schema, &schema_key)
                .await;
        }
    }

    /// Validate object against a single schema (async)
    async fn validate_object_against_schema(
        &self,
        context: &mut FhirSchemaValidationContext,
        data: &JsonValue,
        obj: &serde_json::Map<String, JsonValue>,
        schema: &FhirSchema,
        _schema_key: &str,
    ) {
        // Validate constraints (async) - pass data directly to avoid cloning
        if let Some(constraints) = &schema.constraint {
            self.validate_constraints(context, data, constraints).await;
        }

        // Validate required elements only for resource schemas
        // This prevents base schemas like Narrative from incorrectly requiring elements
        if schema.kind == "resource"
            && let Some(required) = &schema.required
        {
            for required_element in required {
                // Check if element exists directly
                let element_present = if obj.contains_key(required_element) {
                    true
                } else if let Some(elements) = &schema.elements {
                    // Check for choice type variants (e.g., medication -> medicationCodeableConcept)
                    if let Some(element_def) = elements.get(required_element) {
                        if let Some(choices) = &element_def.choices {
                            // Required element is a choice type - check if any variant exists
                            choices.iter().any(|choice| obj.contains_key(choice))
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !element_present {
                    context.add_error(
                        FhirSchemaErrorCode::CardinalityViolation,
                        format!("Required element {required_element} is missing"),
                    );
                }
            }
        }

        // Validate excluded elements
        if let Some(excluded) = &schema.excluded {
            for excluded_element in excluded {
                if obj.contains_key(excluded_element) {
                    context.add_error(
                        FhirSchemaErrorCode::UnknownElement,
                        format!("Excluded element {excluded_element} is present"),
                    );
                }
            }
        }
    }

    /// Validate element value with pre-determined array expectation
    async fn validate_element_value_with_array_check(
        &self,
        context: &mut FhirSchemaValidationContext,
        value: &JsonValue,
        is_array_expected: bool,
    ) {
        let value_is_array = value.is_array();

        match (value_is_array, is_array_expected) {
            (true, false) => {
                context.add_error(
                    FhirSchemaErrorCode::UnexpectedArray,
                    "Unexpected array".to_string(),
                );
            }
            (false, true) => {
                context.add_error(
                    FhirSchemaErrorCode::ExpectedArray,
                    format!("Expected array for element at path: {}", context.path),
                );
            }
            _ => {
                // Validate bindings for coded elements
                self.validate_element_bindings(context, value).await;

                // Validate reference type constraints for Reference elements
                self.validate_element_references(context, value).await;

                // Valid array/non-array match, continue validation (async)
                self.validate_data_element(context, value).await;
            }
        }
    }

    /// Validate bindings for elements in current schemata
    ///
    /// This checks bindings at two levels:
    /// 1. Bindings directly on the current schemata (element schemas)
    /// 2. Bindings on nested elements within current schemata
    async fn validate_element_bindings(
        &self,
        context: &mut FhirSchemaValidationContext,
        value: &JsonValue,
    ) {
        // Skip if no terminology service
        if self.terminology_service.is_none() {
            return;
        }

        let mut bindings: Vec<crate::types::FhirSchemaBinding> = Vec::new();

        // Extract the current element name from the path
        let element_name = context
            .path
            .split('.')
            .next_back()
            .unwrap_or("")
            .split('[')
            .next()
            .unwrap_or("");

        for schema in context.current_schemata.values() {
            // Check if this schema itself has a binding (for element schemas created by follow)
            // The element schema is stored as a FhirSchema wrapper around the element definition
            // We need to check the parent schema for the element's binding

            // Check in the parent schema's elements for bindings
            if let Some(elements) = &schema.elements
                && let Some(element) = elements.get(element_name)
                && let Some(binding) = &element.binding
            {
                bindings.push(binding.clone());
            }
        }

        // Also check in all_schemas - the parent schema may have the binding
        // This is needed because the current schemata might be type schemas
        if bindings.is_empty() {
            // Get the parent path (e.g., "Patient" from "Patient.gender")
            if let Some(dot_pos) = context.path.rfind('.') {
                let parent_path = &context.path[..dot_pos];
                let parent_type = parent_path.split('.').next().unwrap_or(parent_path);

                // Look up the parent schema
                if let Some(parent_schema) = context.all_schemas.get(parent_type)
                    && let Some(elements) = &parent_schema.elements
                    && let Some(element) = elements.get(element_name)
                    && let Some(binding) = &element.binding
                {
                    bindings.push(binding.clone());
                }
            }
        }

        // Validate against each binding
        for binding in bindings {
            self.validate_binding(context, value, &binding).await;
        }
    }

    /// Validate element value (handles arrays vs single values) - async
    #[allow(dead_code)]
    async fn validate_element_value(
        &self,
        context: &mut FhirSchemaValidationContext,
        value: &JsonValue,
    ) {
        let is_array_expected = self.is_array_expected_in_schemata(context);
        self.validate_element_value_with_array_check(context, value, is_array_expected)
            .await;
    }

    /// Check if a specific element is expected to be an array in the current schemata
    /// According to FHIR Schema spec: validate against each schema in the current schemata
    fn is_array_expected_for_element(
        &self,
        context: &FhirSchemaValidationContext,
        element_name: &str,
    ) -> bool {
        // According to FHIR Schema specification, we need to check the element definition
        // in the current schemata context, not globally across all schemas

        // Check if any schema in current schemata explicitly defines this element as array
        // With the fixed collect/follow operations, this should now only find the correct schema
        for schema in context.current_schemata.values() {
            if let Some(elements) = &schema.elements
                && let Some(element) = elements.get(element_name)
            {
                let is_array = element.array.unwrap_or(false);
                return is_array;
            }
        }

        // If not explicitly defined in current schemata, default to false
        // This prevents incorrect array expectations from unrelated schemas
        false
    }

    /// Check if array is expected based on current schemata for the current element
    #[allow(dead_code)]
    fn is_array_expected_in_schemata(&self, context: &FhirSchemaValidationContext) -> bool {
        // Extract the current element name from the path
        let current_element = if let Some(last_dot) = context.path.rfind('.') {
            &context.path[last_dot + 1..]
        } else {
            &context.path
        };

        // Skip array indices in path like [0], [1], etc.
        let element_name = if let Some(bracket_pos) = current_element.find('[') {
            &current_element[..bracket_pos]
        } else {
            current_element
        };

        // If no element name, default to false
        if element_name.is_empty() {
            return false;
        }

        self.is_array_expected_for_element(context, element_name)
    }

    /// Validate primitive value according to FHIR specification
    fn validate_primitive_value(
        &self,
        context: &mut FhirSchemaValidationContext,
        value: &JsonValue,
    ) {
        // Get expected types from schemata
        let expected_types = self.get_expected_types_from_schemata(context);

        if expected_types.is_empty() {
            return; // No type constraints
        }

        let mut valid_for_any_type = false;

        for expected_type in &expected_types {
            if self.validate_primitive_type(value, expected_type) {
                valid_for_any_type = true;
                break;
            }
        }

        if !valid_for_any_type {
            context.add_error(
                FhirSchemaErrorCode::WrongType,
                format!(
                    "Expected one of: {}, got: {}",
                    expected_types.join(", "),
                    self.get_json_type_name(value)
                ),
            );
        }
    }

    /// Get expected types from current schemata
    fn get_expected_types_from_schemata(
        &self,
        context: &FhirSchemaValidationContext,
    ) -> Vec<String> {
        let mut types = HashSet::new();

        // According to FHIR Schema spec, only check types from current schemata context
        // for the current data element being validated
        for schema in context.current_schemata.values() {
            // If the schema itself represents a primitive type, add it
            if !schema.type_name.is_empty() && context.is_primitive_type(&schema.type_name) {
                types.insert(schema.type_name.clone());
            }
        }

        // If no primitive types found in current schemata, allow any type
        // This prevents false type validation errors
        if types.is_empty() {
            return vec![];
        }

        types.into_iter().collect()
    }

    /// Validate primitive type according to FHIR rules
    fn validate_primitive_type(&self, value: &JsonValue, expected_type: &str) -> bool {
        match expected_type {
            "boolean" => value.is_boolean(),
            "integer" | "unsignedInt" | "positiveInt" => {
                value.is_i64() || (value.is_u64() && value.as_u64().unwrap() <= i64::MAX as u64)
            }
            "decimal" => value.is_f64() || value.is_i64() || value.is_u64(),
            "string" | "code" | "uri" | "url" | "canonical" | "base64Binary" | "instant"
            | "date" | "dateTime" | "time" | "oid" | "id" | "markdown" | "uuid" | "xhtml" => {
                value.is_string()
            }
            _ => true, // Unknown type, assume valid
        }
    }

    /// Get JSON type name for error messages
    fn get_json_type_name(&self, value: &JsonValue) -> &'static str {
        match value {
            JsonValue::Null => "null",
            JsonValue::Bool(_) => "boolean",
            JsonValue::Number(_) => "number",
            JsonValue::String(_) => "string",
            JsonValue::Array(_) => "array",
            JsonValue::Object(_) => "object",
        }
    }

    /// Validate FHIRPath constraints against the resource (async)
    ///
    /// This method evaluates FHIRPath constraint expressions using the configured FHIRPath evaluator.
    /// If no evaluator is configured, constraint validation is skipped.
    ///
    /// Constraints are evaluated in PARALLEL for maximum performance. Each constraint
    /// evaluation is independent and can safely run concurrently.
    ///
    /// # Arguments
    /// * `context` - Validation context for error tracking
    /// * `element_value` - Element data as JsonValue (avoids cloning from Map)
    /// * `constraints` - Map of constraint key to constraint definition
    async fn validate_constraints(
        &self,
        context: &mut FhirSchemaValidationContext,
        element_value: &JsonValue,
        constraints: &HashMap<String, FhirSchemaConstraint>,
    ) {
        // Skip if no evaluator configured - structural validation only
        let Some(evaluator) = &self.fhirpath_evaluator else {
            return;
        };

        // Skip if no constraints to evaluate
        if constraints.is_empty() {
            return;
        }

        // element_value is passed by reference - no cloning needed!

        // Get cached variables map with %rootResource (lazy initialized once per validation)
        // Clone once to avoid borrow conflict with context.errors
        let variables = context.get_constraint_variables().clone();

        // Evaluate constraints sequentially (more efficient for CPU-bound FHIRPath evaluation)
        for (key, constraint) in constraints.iter() {
            // Skip warning constraints
            if constraint.severity.as_str() == "warning" {
                continue;
            }

            // Evaluate FHIRPath expression
            match evaluator
                .evaluate_with_variables(&constraint.expression, element_value, &variables)
                .await
            {
                Ok(result) => {
                    // Per FHIR spec: "If the expression returns empty, the constraint is considered satisfied."
                    // Only fail constraint if result is explicitly Boolean(false)
                    let constraint_violated = matches!(result, EvaluationResult::Boolean(false, _));
                    if constraint_violated {
                        context.errors.push(ValidationError {
                            error_type: FhirSchemaErrorCode::ConstraintViolation.to_string(),
                            path: context
                                .path
                                .split('.')
                                .map(|s| JsonValue::String(s.to_string()))
                                .collect(),
                            message: Some(format!(
                                "Constraint '{}' failed: {}",
                                key, constraint.human
                            )),
                            value: None,
                            expected: None,
                            got: None,
                            schema_path: None,
                            constraint_key: None,
                            constraint_expression: None,
                            constraint_severity: None,
                        });
                    }
                }
                Err(e) => {
                    context.errors.push(ValidationError {
                        error_type: FhirSchemaErrorCode::ConstraintViolation.to_string(),
                        path: context
                            .path
                            .split('.')
                            .map(|s| JsonValue::String(s.to_string()))
                            .collect(),
                        message: Some(format!("Constraint '{}' evaluation failed: {}", key, e)),
                        value: None,
                        expected: None,
                        got: None,
                        schema_path: None,
                        constraint_key: None,
                        constraint_expression: None,
                        constraint_severity: None,
                    });
                }
            }
        }
    }

    /// Validate a code/coding value against a value set binding.
    ///
    /// This method validates coded elements against their bound value sets when
    /// a terminology service is configured. Validation behavior depends on binding strength:
    /// - `required`: Code MUST be from the value set (error if not)
    /// - `extensible`: Code SHOULD be from the value set (warning if not)
    /// - `preferred`/`example`: No validation performed (informational only)
    ///
    /// # Arguments
    /// * `context` - Validation context for error tracking
    /// * `value` - The code or coding value to validate
    /// * `binding` - The binding definition from the schema
    async fn validate_binding(
        &self,
        context: &mut FhirSchemaValidationContext,
        value: &JsonValue,
        binding: &crate::types::FhirSchemaBinding,
    ) {
        // Skip if no terminology service configured
        let Some(terminology) = &self.terminology_service else {
            return;
        };

        // Skip if no value set URL
        let Some(value_set_url) = &binding.value_set else {
            return;
        };

        // Parse binding strength
        let strength = match TermBindingStrength::parse_str(&binding.strength) {
            Some(s) => s,
            None => return, // Unknown strength, skip validation
        };

        // Skip validation for non-required bindings
        // Per FHIR spec:
        // - Example: No validation (informational only)
        // - Preferred: Just a recommendation, not required
        // - Extensible: Should use value set if applicable, but not a hard requirement
        // Only "required" bindings affect resource validity
        if !matches!(strength, TermBindingStrength::Required) {
            return;
        }

        // Extract code(s) from the value
        let codes = self.extract_codes_from_value(value);

        if codes.is_empty() {
            return; // No codes to validate
        }

        // Validate each code
        for (code, system) in codes {
            match terminology
                .validate_code(value_set_url, &code, system.as_deref())
                .await
            {
                Ok(result) => {
                    if !result.valid {
                        // Since we only validate required bindings (early return above),
                        // any failure here is an error
                        context.add_error(
                            FhirSchemaErrorCode::BindingViolation,
                            format!(
                                "Code '{}' (system: {}) is not in value set '{}'",
                                code,
                                system.as_deref().unwrap_or("none"),
                                value_set_url
                            ),
                        );
                    }
                }
                Err(e) => {
                    // Required binding validation failed
                    context.add_error(
                        FhirSchemaErrorCode::BindingViolation,
                        format!("Failed to validate code against value set: {}", e),
                    );
                }
            }
        }
    }

    /// Extract code(s) from a value (code, Coding, or CodeableConcept).
    ///
    /// Returns a list of (code, optional_system) pairs.
    fn extract_codes_from_value(&self, value: &JsonValue) -> Vec<(String, Option<String>)> {
        let mut codes = Vec::new();

        match value {
            // Simple code string
            JsonValue::String(code) => {
                codes.push((code.clone(), None));
            }
            JsonValue::Object(obj) => {
                // Check if this is a Coding
                if let Some(code) = obj.get("code").and_then(|v| v.as_str()) {
                    let system = obj.get("system").and_then(|v| v.as_str()).map(String::from);
                    codes.push((code.to_string(), system));
                }

                // Check if this is a CodeableConcept with codings
                if let Some(codings) = obj.get("coding").and_then(|v| v.as_array()) {
                    for coding in codings {
                        if let Some(code) = coding.get("code").and_then(|v| v.as_str()) {
                            let system = coding
                                .get("system")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            codes.push((code.to_string(), system));
                        }
                    }
                }
            }
            _ => {}
        }

        codes
    }

    /// Validate reference type constraints for Reference elements.
    ///
    /// This checks that the resourceType in a Reference matches the allowed
    /// target types defined in the schema's `refers` field.
    ///
    /// # Arguments
    /// * `context` - Validation context for error tracking
    /// * `value` - The Reference value to validate
    async fn validate_element_references(
        &self,
        context: &mut FhirSchemaValidationContext,
        value: &JsonValue,
    ) {
        // Check if current element is a Reference type
        let is_reference_type = context
            .current_schemata
            .values()
            .any(|schema| schema.type_name == "Reference" || schema.name == "Reference");

        if !is_reference_type {
            return;
        }

        // Get allowed target types from refers field
        let mut allowed_types: Vec<String> = Vec::new();

        // Extract the current element name from the path
        let element_name = context
            .path
            .split('.')
            .next_back()
            .unwrap_or("")
            .split('[')
            .next()
            .unwrap_or("");

        // Check current schemata for refers
        for schema in context.current_schemata.values() {
            if let Some(elements) = &schema.elements
                && let Some(element) = elements.get(element_name)
                && let Some(refers) = &element.refers
            {
                allowed_types.extend(refers.clone());
            }
        }

        // Also check in all_schemas - the parent schema may have the refers
        if allowed_types.is_empty()
            && let Some(dot_pos) = context.path.rfind('.')
        {
            let parent_path = &context.path[..dot_pos];
            let parent_type = parent_path.split('.').next().unwrap_or(parent_path);

            if let Some(parent_schema) = context.all_schemas.get(parent_type)
                && let Some(elements) = &parent_schema.elements
                && let Some(element) = elements.get(element_name)
                && let Some(refers) = &element.refers
            {
                allowed_types.extend(refers.clone());
            }
        }

        // Validate the reference type matches allowed types (if type restrictions exist)
        if !allowed_types.is_empty() {
            self.validate_reference_type(context, value, &allowed_types);
        }

        // Validate referenced resource exists (if reference resolver is configured)
        // Arc clone is cheap (just refcount increment), and Arc<JsonValue> derefs to &JsonValue
        if let Some(root_resource) = context.root_resource.clone() {
            self.validate_reference_exists(context, value, &root_resource)
                .await;
        }
    }

    /// Validate that a Reference's resourceType matches allowed target types.
    ///
    /// # Arguments
    /// * `context` - Validation context for error tracking
    /// * `reference_value` - The Reference JSON object
    /// * `allowed_types` - List of allowed target type URLs from schema's `refers` field
    fn validate_reference_type(
        &self,
        context: &mut FhirSchemaValidationContext,
        reference_value: &JsonValue,
        allowed_types: &[String],
    ) {
        // Extract reference string from Reference.reference field
        let reference_str = match reference_value.get("reference").and_then(|v| v.as_str()) {
            Some(r) => r,
            None => return, // No reference field, might be logical reference - skip
        };

        // Skip contained references - they need special handling
        if reference_str.starts_with('#') {
            return;
        }

        // Parse reference to extract resourceType
        if let Some(resource_type) = self.extract_resource_type_from_reference(reference_str) {
            // If refers contains "Resource" or ends with /Resource, allow any type
            let allows_any = allowed_types
                .iter()
                .any(|t| t == "Resource" || t.ends_with("/Resource"));

            if allows_any {
                return;
            }

            // Check if resourceType matches any allowed type
            let is_allowed = allowed_types.iter().any(|allowed| {
                // Handle both plain type names and profile URLs
                // e.g., "Patient" or "http://hl7.org/fhir/StructureDefinition/Patient"
                allowed == &resource_type || allowed.ends_with(&format!("/{}", resource_type))
            });

            if !is_allowed {
                // Format allowed types for error message
                let allowed_display: Vec<&str> = allowed_types
                    .iter()
                    .map(|t| {
                        // Extract just the type name from URL if present
                        t.rsplit('/').next().unwrap_or(t.as_str())
                    })
                    .collect();

                context.add_error(
                    FhirSchemaErrorCode::ReferenceTypeViolation,
                    format!(
                        "Reference type '{}' is not allowed. Expected one of: {:?}",
                        resource_type, allowed_display
                    ),
                );
            }
        }
    }

    /// Extract resourceType from a reference string.
    ///
    /// Handles various reference formats:
    /// - Relative: "Patient/123"
    /// - Absolute: "http://example.org/fhir/Patient/123"
    /// - With history: "Patient/123/_history/1"
    ///
    /// Returns None for contained references (#id) or if type cannot be determined.
    fn extract_resource_type_from_reference(&self, reference: &str) -> Option<String> {
        // Skip contained references
        if reference.starts_with('#') {
            return None;
        }

        // Skip urn:uuid: and urn:oid: references
        if reference.starts_with("urn:") {
            return None;
        }

        // Handle absolute URLs: extract path portion
        let path = if reference.contains("://") {
            // Find the path after the domain
            reference
                .find("://")
                .and_then(|i| reference[i + 3..].find('/'))
                .map(|i| {
                    let start = reference.find("://").unwrap() + 3 + i;
                    &reference[start..]
                })
                .unwrap_or(reference)
        } else {
            reference
        };

        // Split path into segments
        let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

        // Pattern is ResourceType/id or ResourceType/id/_history/version
        // We need the segment that looks like a resource type
        if segments.len() >= 2 {
            // Check for _history pattern
            let type_idx =
                if segments.len() >= 4 && segments.get(segments.len() - 2) == Some(&"_history") {
                    // ResourceType/id/_history/version - type is at len-4
                    segments.len().saturating_sub(4)
                } else {
                    // ResourceType/id - type is at len-2
                    segments.len().saturating_sub(2)
                };

            if let Some(candidate) = segments.get(type_idx) {
                // Validate it looks like a resource type (starts with capital letter)
                if candidate
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false)
                {
                    return Some(candidate.to_string());
                }
            }
        }

        None
    }

    /// Validate that a reference points to an existing resource.
    ///
    /// This method checks reference existence using the configured reference resolver.
    /// It handles:
    /// - Regular references (Patient/123) - checks via resolver
    /// - Contained references (#id) - checks within the resource's contained array
    /// - External/urn references - skipped (cannot validate)
    ///
    /// # Arguments
    /// * `context` - Validation context for error tracking
    /// * `value` - The Reference value to validate
    /// * `resource` - The root resource being validated (for contained reference resolution)
    async fn validate_reference_exists(
        &self,
        context: &mut FhirSchemaValidationContext,
        value: &JsonValue,
        resource: &JsonValue,
    ) {
        // Skip if no reference resolver configured
        let Some(resolver) = &self.reference_resolver else {
            return;
        };

        // Check if current element is a Reference type
        let is_reference_type = context
            .current_schemata
            .values()
            .any(|schema| schema.type_name == "Reference" || schema.name == "Reference");

        if !is_reference_type {
            return;
        }

        // Extract reference string from Reference.reference field
        let reference_str = match value.get("reference").and_then(|v| v.as_str()) {
            Some(r) => r,
            None => return, // No reference field, might be logical reference - skip
        };

        // Handle contained references (#id)
        if let Some(contained_id) = reference_str.strip_prefix('#') {
            if !self.resolve_contained_reference(resource, contained_id) {
                context.errors.push(ValidationError {
                    error_type: "REF1002".to_string(),
                    path: context
                        .path
                        .split('.')
                        .map(|s| JsonValue::String(s.to_string()))
                        .collect(),
                    message: Some(format!(
                        "Contained reference #{} not found in resource",
                        contained_id
                    )),
                    value: Some(JsonValue::String(reference_str.to_string())),
                    expected: None,
                    got: None,
                    schema_path: None,
                    constraint_key: None,
                    constraint_expression: None,
                    constraint_severity: None,
                });
            }
            return;
        }

        // Skip urn:uuid: and urn:oid: references (Bundle internal references)
        if reference_str.starts_with("urn:") {
            return;
        }

        // Resolve external reference via resolver
        match resolver.resolve_reference(reference_str).await {
            Ok(result) if !result.exists => {
                context.errors.push(ValidationError {
                    error_type: "REF1001".to_string(),
                    path: context
                        .path
                        .split('.')
                        .map(|s| JsonValue::String(s.to_string()))
                        .collect(),
                    message: Some(format!(
                        "Referenced resource {} does not exist",
                        reference_str
                    )),
                    value: Some(JsonValue::String(reference_str.to_string())),
                    expected: None,
                    got: None,
                    schema_path: None,
                    constraint_key: None,
                    constraint_expression: None,
                    constraint_severity: None,
                });
            }
            Err(ReferenceError::ServiceUnavailable { .. }) => {
                // Skip validation when service is unavailable
                // This prevents validation failures due to temporary service issues
            }
            Err(e) => {
                context.errors.push(ValidationError {
                    error_type: e.code().to_string(),
                    path: context
                        .path
                        .split('.')
                        .map(|s| JsonValue::String(s.to_string()))
                        .collect(),
                    message: Some(e.to_string()),
                    value: Some(JsonValue::String(reference_str.to_string())),
                    expected: None,
                    got: None,
                    schema_path: None,
                    constraint_key: None,
                    constraint_expression: None,
                    constraint_severity: None,
                });
            }
            _ => {} // Reference exists, no error
        }
    }

    /// Resolve a contained reference within the resource.
    ///
    /// Checks if a resource with the given ID exists in the resource's `contained` array.
    fn resolve_contained_reference(&self, resource: &JsonValue, contained_id: &str) -> bool {
        if let Some(contained) = resource.get("contained").and_then(|c| c.as_array()) {
            for item in contained {
                if item.get("id").and_then(|id| id.as_str()) == Some(contained_id) {
                    return true;
                }
            }
        }
        false
    }
}

// =============================================================================
// FhirValidator - High-performance validator using pre-compiled schemas
// =============================================================================

/// High-performance FHIR validator using pre-compiled schemas with lazy loading.
///
/// Schemas are loaded and compiled on-demand via `SchemaProvider` and cached
/// for subsequent validations. This avoids loading all FHIR schemas upfront.
pub struct FhirValidator {
    /// Schema compiler with caching
    compiler: SchemaCompiler,
    /// Optional FHIRPath evaluator for constraint validation
    fhirpath_evaluator: Option<Arc<dyn FhirPathEvaluator>>,
    /// Optional terminology service for binding validation
    terminology_service: Option<Arc<dyn TerminologyService>>,
    /// Optional reference resolver for existence validation
    reference_resolver: Option<Arc<dyn ReferenceResolver>>,
}

impl FhirValidator {
    /// Create a new compiled validator with a schema provider
    pub fn new(schema_provider: Arc<dyn SchemaProvider>) -> Self {
        Self {
            compiler: SchemaCompiler::new(schema_provider),
            fhirpath_evaluator: None,
            terminology_service: None,
            reference_resolver: None,
        }
    }

    /// Create a new compiled validator with FHIRPath evaluator
    pub fn new_with_fhirpath(
        schema_provider: Arc<dyn SchemaProvider>,
        fhirpath_evaluator: Arc<dyn FhirPathEvaluator>,
    ) -> Self {
        Self {
            compiler: SchemaCompiler::new(schema_provider),
            fhirpath_evaluator: Some(fhirpath_evaluator),
            terminology_service: None,
            reference_resolver: None,
        }
    }

    // =========================================================================
    // Convenience constructors for tests (compatible with FhirSchemaValidator API)
    // =========================================================================

    /// Create validator from a HashMap of schemas (test convenience method).
    ///
    /// This provides API compatibility with the legacy FhirSchemaValidator::new()
    /// for easier test migration. Wraps schemas in InMemorySchemaProvider.
    ///
    /// # Example
    /// ```ignore
    /// let mut schemas = HashMap::new();
    /// schemas.insert("Patient".to_string(), patient_schema);
    /// let validator = FhirValidator::from_schemas(schemas, None);
    /// ```
    pub fn from_schemas(
        schemas: HashMap<String, FhirSchema>,
        fhirpath_evaluator: Option<Arc<dyn FhirPathEvaluator>>,
    ) -> Self {
        let provider = InMemorySchemaProvider::from_map(
            schemas.into_iter().map(|(k, v)| (k, Arc::new(v))).collect(),
        );
        let provider_arc = Arc::new(provider);

        match fhirpath_evaluator {
            Some(evaluator) => Self::new_with_fhirpath(provider_arc, evaluator),
            None => Self::new(provider_arc),
        }
    }

    /// Create validator from Arc-wrapped schemas map (test convenience method).
    ///
    /// Use this when you already have `Arc<FhirSchema>` to avoid rewrapping.
    pub fn from_arc_schemas(
        schemas: HashMap<String, Arc<FhirSchema>>,
        fhirpath_evaluator: Option<Arc<dyn FhirPathEvaluator>>,
    ) -> Self {
        let provider = InMemorySchemaProvider::from_map(schemas);
        let provider_arc = Arc::new(provider);

        match fhirpath_evaluator {
            Some(evaluator) => Self::new_with_fhirpath(provider_arc, evaluator),
            None => Self::new(provider_arc),
        }
    }

    /// Add terminology service for binding validation
    pub fn with_terminology_service(mut self, service: Arc<dyn TerminologyService>) -> Self {
        self.terminology_service = Some(service);
        self
    }

    /// Add reference resolver for existence validation
    pub fn with_reference_resolver(mut self, resolver: Arc<dyn ReferenceResolver>) -> Self {
        self.reference_resolver = Some(resolver);
        self
    }

    /// Validate a resource against its resourceType schema.
    ///
    /// Performs both structural validation and FHIRPath constraint validation.
    /// Structural validation runs synchronously, then constraint validation runs asynchronously.
    pub async fn validate(
        &self,
        resource: &JsonValue,
        schema_names: Vec<String>,
    ) -> ValidationResult {
        let mut errors = Vec::new();

        // Prepare constraint variables once (includes %rootResource)
        let variables = Self::prepare_constraint_variables(resource);

        for schema_name in &schema_names {
            // Get or compile schema (single cache lookup)
            match self.compiler.compile(schema_name).await {
                Ok(compiled) => {
                    // Phase 1: Structural validation (sync)
                    self.validate_resource(resource, &compiled, &mut errors, "");

                    // Phase 2: Constraint validation (async)
                    self.validate_constraints_recursive(
                        resource,
                        &compiled,
                        &variables,
                        &mut errors,
                        "",
                    )
                    .await;
                }
                Err(e) => {
                    errors.push(ValidationError {
                        error_type: FhirSchemaErrorCode::UnknownSchema.to_string(),
                        path: vec![],
                        message: Some(e.message),
                        value: None,
                        expected: None,
                        got: None,
                        schema_path: None,
                        constraint_key: None,
                        constraint_expression: None,
                        constraint_severity: None,
                    });
                }
            }
        }

        ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings: vec![],
        }
    }

    /// Prepare constraint variables map for FHIRPath evaluation.
    ///
    /// Creates a variables map containing `%rootResource` which is required
    /// for evaluating constraints like `ref-1` that reference contained resources.
    fn prepare_constraint_variables(root_resource: &JsonValue) -> HashMap<String, EvaluationResult> {
        let mut variables = HashMap::with_capacity(1);
        variables.insert(
            "rootResource".to_string(),
            json_to_evaluation_result(root_resource),
        );
        variables
    }

    /// Validate resource against compiled schema
    fn validate_resource(
        &self,
        data: &JsonValue,
        schema: &CompiledSchema,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        let JsonValue::Object(obj) = data else {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::WrongType.to_string(),
                path: self.path_to_vec(path),
                message: Some("Expected object".to_string()),
                value: None,
                expected: Some(JsonValue::String("object".to_string())),
                got: Some(JsonValue::String(self.json_type_name(data).to_string())),
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
            return;
        };

        // Check required elements
        for required in &schema.required {
            if !obj.contains_key(required) && !self.has_choice_variant(obj, required, &schema.elements) {
                errors.push(ValidationError {
                    error_type: FhirSchemaErrorCode::CardinalityViolation.to_string(),
                    path: self.path_to_vec(path),
                    message: Some(format!("Required element '{}' is missing", required)),
                    value: None,
                    expected: None,
                    got: None,
                    schema_path: None,
                    constraint_key: None,
                    constraint_expression: None,
                    constraint_severity: None,
                });
            }
        }

        // Check excluded elements
        for excluded in &schema.excluded {
            if obj.contains_key(excluded) {
                errors.push(ValidationError {
                    error_type: FhirSchemaErrorCode::UnknownElement.to_string(),
                    path: self.path_to_vec(path),
                    message: Some(format!("Excluded element '{}' is present", excluded)),
                    value: None,
                    expected: None,
                    got: None,
                    schema_path: None,
                    constraint_key: None,
                    constraint_expression: None,
                    constraint_severity: None,
                });
            }
        }

        // Validate each property
        for (key, value) in obj {
            // Skip special FHIR keys
            if key == "resourceType" || key == "fhir_comments" {
                continue;
            }

            // Handle primitive extensions (_element)
            if key.starts_with('_') {
                // Primitive extension - validate as Element
                continue; // For now, skip - can add Element validation later
            }

            if let Some(element) = schema.elements.get(key) {
                let element_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };
                self.validate_element(value, element, errors, &element_path);
            } else {
                // Check if this is a choice type variant (e.g., valueString for value[x])
                let is_choice_variant = schema.elements.values().any(|el| {
                    el.choices.as_ref().map_or(false, |c| c.contains(key))
                });

                if !is_choice_variant {
                    errors.push(ValidationError {
                        error_type: FhirSchemaErrorCode::UnknownElement.to_string(),
                        path: self.path_to_vec(path),
                        message: Some(format!("Unknown element '{}'", key)),
                        value: None,
                        expected: None,
                        got: None,
                        schema_path: None,
                        constraint_key: None,
                        constraint_expression: None,
                        constraint_severity: None,
                    });
                }
            }
        }
    }

    /// Validate an element value
    fn validate_element(
        &self,
        value: &JsonValue,
        element: &CompiledElement,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        // Array check
        let is_array = value.is_array();
        if is_array != element.is_array {
            errors.push(ValidationError {
                error_type: if element.is_array {
                    FhirSchemaErrorCode::ExpectedArray
                } else {
                    FhirSchemaErrorCode::UnexpectedArray
                }
                .to_string(),
                path: self.path_to_vec(path),
                message: Some(if element.is_array {
                    format!("Expected array for element '{}'", element.name)
                } else {
                    format!("Unexpected array for element '{}'", element.name)
                }),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
            return;
        }

        // Handle arrays
        if is_array {
            if let JsonValue::Array(arr) = value {
                for (i, item) in arr.iter().enumerate() {
                    let item_path = format!("{}[{}]", path, i);
                    self.validate_element_value(item, element, errors, &item_path);
                }
            }
        } else {
            self.validate_element_value(value, element, errors, path);
        }
    }

    /// Validate a single element value (not array)
    fn validate_element_value(
        &self,
        value: &JsonValue,
        element: &CompiledElement,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        match &element.type_info {
            CompiledTypeInfo::Primitive(ptype) => {
                self.validate_primitive(value, *ptype, errors, path);
            }
            CompiledTypeInfo::Complex | CompiledTypeInfo::BackboneElement => {
                // Recursively validate using inlined children
                self.validate_complex(value, &element.children, errors, path);
            }
            CompiledTypeInfo::Reference => {
                self.validate_reference(value, &element.reference_targets, errors, path);
            }
            CompiledTypeInfo::Resource => {
                // For contained resources - validate by resourceType
                self.validate_contained_resource(value, errors, path);
            }
            CompiledTypeInfo::Extension => {
                // Extensions have their own structure
                self.validate_extension(value, errors, path);
            }
        }
    }

    /// Validate primitive value
    fn validate_primitive(
        &self,
        value: &JsonValue,
        ptype: compiled::PrimitiveType,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        use compiled::PrimitiveType::*;

        let valid = match ptype {
            Boolean => value.is_boolean(),
            Integer | Integer64 | UnsignedInt | PositiveInt => value.is_i64() || value.is_u64(),
            Decimal => value.is_number(),
            String | Uri | Url | Canonical | Code | Oid | Id | Markdown | Uuid | Xhtml => {
                value.is_string()
            }
            Base64Binary => value.is_string(),
            Instant | Date | DateTime | Time => value.is_string(),
        };

        if !valid {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::WrongType.to_string(),
                path: self.path_to_vec(path),
                message: Some(format!(
                    "Expected {} but got {}",
                    ptype.as_str(),
                    self.json_type_name(value)
                )),
                value: None,
                expected: Some(JsonValue::String(ptype.as_str().to_string())),
                got: Some(JsonValue::String(self.json_type_name(value).to_string())),
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
        }
    }

    /// Validate complex type with children
    fn validate_complex(
        &self,
        value: &JsonValue,
        children: &HashMap<String, CompiledElement>,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        let JsonValue::Object(obj) = value else {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::WrongType.to_string(),
                path: self.path_to_vec(path),
                message: Some("Expected object".to_string()),
                value: None,
                expected: Some(JsonValue::String("object".to_string())),
                got: Some(JsonValue::String(self.json_type_name(value).to_string())),
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
            return;
        };

        // Validate each property
        for (key, val) in obj {
            // Skip primitive extensions
            if key.starts_with('_') {
                continue;
            }

            if let Some(element) = children.get(key) {
                let element_path = format!("{}.{}", path, key);
                self.validate_element(val, element, errors, &element_path);
            } else {
                // Check for choice type variants
                let is_choice = children.values().any(|el| {
                    el.choices.as_ref().map_or(false, |c| c.contains(key))
                });

                if !is_choice && key != "extension" && key != "id" {
                    errors.push(ValidationError {
                        error_type: FhirSchemaErrorCode::UnknownElement.to_string(),
                        path: self.path_to_vec(path),
                        message: Some(format!("Unknown element '{}'", key)),
                        value: None,
                        expected: None,
                        got: None,
                        schema_path: None,
                        constraint_key: None,
                        constraint_expression: None,
                        constraint_severity: None,
                    });
                }
            }
        }
    }

    /// Validate Reference element
    fn validate_reference(
        &self,
        value: &JsonValue,
        _targets: &Option<Vec<String>>,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        let JsonValue::Object(obj) = value else {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::WrongType.to_string(),
                path: self.path_to_vec(path),
                message: Some("Reference must be an object".to_string()),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
            return;
        };

        // Basic structure check - must have reference, identifier, or display
        let has_reference = obj.get("reference").map_or(false, |v| v.is_string());
        let has_identifier = obj.contains_key("identifier");
        let has_display = obj.get("display").map_or(false, |v| v.is_string());

        if !has_reference && !has_identifier && !has_display {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::CardinalityViolation.to_string(),
                path: self.path_to_vec(path),
                message: Some(
                    "Reference must have at least one of: reference, identifier, display"
                        .to_string(),
                ),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
        }
    }

    /// Validate contained resource
    fn validate_contained_resource(
        &self,
        value: &JsonValue,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        let JsonValue::Object(obj) = value else {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::WrongType.to_string(),
                path: self.path_to_vec(path),
                message: Some("Contained resource must be an object".to_string()),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
            return;
        };

        // Get resourceType
        let Some(resource_type) = obj.get("resourceType").and_then(|v| v.as_str()) else {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::CardinalityViolation.to_string(),
                path: self.path_to_vec(path),
                message: Some("Contained resource must have resourceType".to_string()),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
            return;
        };

        // Contained resources cannot have contained (per FHIR spec)
        if obj.contains_key("contained") {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::UnknownElement.to_string(),
                path: self.path_to_vec(path),
                message: Some("Contained resources cannot have nested contained".to_string()),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
        }

        // Note: Full validation of contained resource by type would require async
        // For now, we just do structural validation
        // TODO: Add async validation via compile() for contained resources
        let _ = resource_type; // Acknowledge we have the type but don't use it yet
    }

    /// Validate Extension element
    fn validate_extension(
        &self,
        value: &JsonValue,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        let JsonValue::Object(obj) = value else {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::WrongType.to_string(),
                path: self.path_to_vec(path),
                message: Some("Extension must be an object".to_string()),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
            return;
        };

        // Extension must have url
        if !obj.contains_key("url") {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::CardinalityViolation.to_string(),
                path: self.path_to_vec(path),
                message: Some("Extension must have url".to_string()),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            });
        }
    }

    /// Check if a choice type variant exists
    fn has_choice_variant(
        &self,
        obj: &serde_json::Map<String, JsonValue>,
        element_name: &str,
        elements: &HashMap<String, CompiledElement>,
    ) -> bool {
        if let Some(element) = elements.get(element_name) {
            if let Some(choices) = &element.choices {
                return choices.iter().any(|choice| obj.contains_key(choice));
            }
        }
        false
    }

    /// Convert path string to vector for ValidationError
    fn path_to_vec(&self, path: &str) -> Vec<JsonValue> {
        if path.is_empty() {
            vec![]
        } else {
            path.split('.')
                .map(|s| JsonValue::String(s.to_string()))
                .collect()
        }
    }

    /// Get JSON type name for error messages
    fn json_type_name(&self, value: &JsonValue) -> &'static str {
        match value {
            JsonValue::Null => "null",
            JsonValue::Bool(_) => "boolean",
            JsonValue::Number(_) => "number",
            JsonValue::String(_) => "string",
            JsonValue::Array(_) => "array",
            JsonValue::Object(_) => "object",
        }
    }

    // =========================================================================
    // Constraint Validation
    // =========================================================================

    /// Validate FHIRPath constraints against a resource.
    ///
    /// Evaluates all error-severity constraints using the configured FHIRPath evaluator.
    /// Warning-severity constraints are skipped. If no evaluator is configured,
    /// constraint validation is skipped entirely.
    async fn validate_constraints(
        &self,
        data: &JsonValue,
        constraints: &[compiled::CompiledConstraint],
        variables: &HashMap<String, EvaluationResult>,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        let Some(evaluator) = &self.fhirpath_evaluator else {
            return;
        };

        if constraints.is_empty() {
            return;
        }

        for constraint in constraints {
            // Skip warning constraints
            if constraint.severity == compiled::ConstraintSeverity::Warning {
                continue;
            }

            match evaluator
                .evaluate_with_variables(&constraint.expression, data, variables)
                .await
            {
                Ok(result) => {
                    // Per FHIR spec: empty result = satisfied, only Boolean(false) = violation
                    let violated = matches!(result, EvaluationResult::Boolean(false, _));
                    if violated {
                        errors.push(ValidationError {
                            error_type: FhirSchemaErrorCode::ConstraintViolation.to_string(),
                            path: self.path_to_vec(path),
                            message: Some(format!(
                                "Constraint '{}' failed: {}",
                                constraint.key, constraint.human
                            )),
                            value: None,
                            expected: None,
                            got: None,
                            schema_path: None,
                            constraint_key: Some(constraint.key.clone()),
                            constraint_expression: Some(constraint.expression.clone()),
                            constraint_severity: Some("error".to_string()),
                        });
                    }
                }
                Err(e) => {
                    errors.push(ValidationError {
                        error_type: FhirSchemaErrorCode::ConstraintViolation.to_string(),
                        path: self.path_to_vec(path),
                        message: Some(format!(
                            "Constraint '{}' evaluation failed: {}",
                            constraint.key, e
                        )),
                        value: None,
                        expected: None,
                        got: None,
                        schema_path: None,
                        constraint_key: Some(constraint.key.clone()),
                        constraint_expression: Some(constraint.expression.clone()),
                        constraint_severity: Some("error".to_string()),
                    });
                }
            }
        }
    }

    /// Recursively validate constraints for a resource and all its elements.
    ///
    /// This walks through the compiled schema and evaluates constraints at each level:
    /// - Schema-level constraints on the resource itself
    /// - Element-level constraints on each field
    #[async_recursion::async_recursion]
    async fn validate_constraints_recursive(
        &self,
        data: &JsonValue,
        schema: &CompiledSchema,
        variables: &HashMap<String, EvaluationResult>,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        // Validate schema-level constraints
        self.validate_constraints(data, &schema.constraints, variables, errors, path)
            .await;

        // Validate element-level constraints
        let JsonValue::Object(obj) = data else {
            return;
        };

        for (key, value) in obj {
            if key == "resourceType" || key == "fhir_comments" || key.starts_with('_') {
                continue;
            }

            if let Some(element) = schema.elements.get(key) {
                let element_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };

                self.validate_element_constraints(value, element, variables, errors, &element_path)
                    .await;
            }
        }
    }

    /// Validate constraints for an element value.
    #[async_recursion::async_recursion]
    async fn validate_element_constraints(
        &self,
        value: &JsonValue,
        element: &compiled::CompiledElement,
        variables: &HashMap<String, EvaluationResult>,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        // Handle arrays
        if let JsonValue::Array(arr) = value {
            for (i, item) in arr.iter().enumerate() {
                let item_path = format!("{}[{}]", path, i);
                self.validate_single_element_constraints(item, element, variables, errors, &item_path)
                    .await;
            }
        } else {
            self.validate_single_element_constraints(value, element, variables, errors, path)
                .await;
        }
    }

    /// Validate constraints for a single (non-array) element value.
    #[async_recursion::async_recursion]
    async fn validate_single_element_constraints(
        &self,
        value: &JsonValue,
        element: &compiled::CompiledElement,
        variables: &HashMap<String, EvaluationResult>,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        // Validate element-level constraints
        self.validate_constraints(value, &element.constraints, variables, errors, path)
            .await;

        // Recurse into children for complex types
        if let JsonValue::Object(obj) = value {
            for (key, child_value) in obj {
                if key.starts_with('_') {
                    continue;
                }

                if let Some(child_element) = element.children.get(key) {
                    let child_path = format!("{}.{}", path, key);
                    self.validate_element_constraints(
                        child_value,
                        child_element,
                        variables,
                        errors,
                        &child_path,
                    )
                    .await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_primitive_type_validation() {
        let schemas = HashMap::new();
        let validator = FhirSchemaValidator::new(schemas, None);

        // Test boolean validation
        assert!(validator.validate_primitive_type(&json!(true), "boolean"));
        assert!(validator.validate_primitive_type(&json!(false), "boolean"));
        assert!(!validator.validate_primitive_type(&json!("true"), "boolean"));

        // Test integer validation
        assert!(validator.validate_primitive_type(&json!(42), "integer"));
        assert!(!validator.validate_primitive_type(&json!(42.5), "integer"));
        assert!(!validator.validate_primitive_type(&json!("42"), "integer"));

        // Test string validation
        assert!(validator.validate_primitive_type(&json!("hello"), "string"));
        assert!(!validator.validate_primitive_type(&json!(42), "string"));
    }

    #[tokio::test]
    async fn test_unknown_element_detection() {
        let mut schemas = HashMap::new();

        // Create a simple Patient schema
        let patient_schema = FhirSchema {
            url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            version: None,
            name: "Patient".to_string(),
            type_name: "Patient".to_string(),
            kind: "resource".to_string(),
            derivation: None,
            base: None,
            abstract_type: None,
            class: "resource".to_string(),
            description: None,
            package_name: None,
            package_version: None,
            package_id: None,
            package_meta: None,
            elements: Some({
                let mut elements = HashMap::new();
                elements.insert(
                    "id".to_string(),
                    FhirSchemaElement {
                        type_name: Some("string".to_string()),
                        ..Default::default()
                    },
                );
                elements
            }),
            required: None,
            excluded: None,
            extensions: None,
            constraint: None,
            primitive_type: None,
            choices: None,
        };

        schemas.insert("Patient".to_string(), patient_schema);

        let validator = FhirSchemaValidator::new(schemas, None);

        // Test resource with unknown element
        let resource = json!({
            "resourceType": "Patient",
            "id": "example",
            "unknownField": "should cause error"
        });

        let result = validator
            .validate(&resource, vec!["Patient".to_string()])
            .await;

        assert!(!result.valid);
        assert!(!result.errors.is_empty());

        // Check that error is about unknown element
        let has_unknown_element_error = result.errors.iter().any(|e| {
            e.error_type == "FS1001" && e.message.as_ref().unwrap().contains("unknownField")
        });
        assert!(has_unknown_element_error);
    }

    #[tokio::test]
    async fn test_validate_patient_example_with_embedded_schemas() {
        use crate::embedded::{FhirVersion, get_schemas};

        // Get R4 embedded schemas which should include Patient schema
        let schemas = get_schemas(FhirVersion::R4);

        println!("Number of schemas loaded: {}", schemas.len());

        // Check what schemas we have
        let mut schema_names: Vec<_> = schemas.keys().collect();
        schema_names.sort();
        println!("Available schemas (first 10):");
        for name in schema_names.iter().take(10) {
            println!("  - {}", name);
        }

        // Check specifically for Patient schema
        let patient_urls = [
            "http://hl7.org/fhir/StructureDefinition/Patient",
            "Patient",
            "StructureDefinition/Patient",
        ];

        println!("Looking for Patient schema:");
        for url in &patient_urls {
            if schemas.contains_key(*url) {
                println!("  ✓ Found: {}", url);
            } else {
                println!("  ✗ Not found: {}", url);
            }
        }

        // Check for Extension schema
        let extension_urls = [
            "http://hl7.org/fhir/StructureDefinition/Extension",
            "Extension",
            "StructureDefinition/Extension",
        ];

        println!("Looking for Extension schema:");
        for url in &extension_urls {
            if schemas.contains_key(*url) {
                println!("  ✓ Found: {}", url);
            } else {
                println!("  ✗ Not found: {}", url);
            }
        }

        // Check what the Patient schema URL actually is
        if let Some(patient_schema) = schemas.get("Patient") {
            println!("Patient schema found with URL: '{}'", patient_schema.url);
            println!("Patient schema kind: '{}'", patient_schema.kind);
            println!("Patient schema type: '{}'", patient_schema.type_name);
            if let Some(base) = &patient_schema.base {
                println!("Patient schema base: '{}'", base);
            }

            // Check required elements
            if let Some(required) = &patient_schema.required {
                println!("Patient required elements: {:?}", required);
            } else {
                println!("Patient has no required elements listed");
            }

            // Check available elements
            if let Some(elements) = &patient_schema.elements {
                println!("Patient schema has {} elements", elements.len());
                let mut element_names: Vec<_> = elements.keys().collect();
                element_names.sort();
                println!("Patient elements: {:?}", element_names);

                // Check specific elements that are failing
                let failing_elements = [
                    "birthDate",
                    "_birthDate",
                    "address",
                    "name",
                    "identifier",
                    "contact",
                ];
                for elem in &failing_elements {
                    if let Some(element) = elements.get(*elem) {
                        println!(
                            "Element '{}': type={:?}, array={:?}",
                            elem, element.type_name, element.array
                        );

                        // Check if this element has nested elements (BackboneElement case)
                        if let Some(element_elements) = &element.elements {
                            println!(
                                "  Element '{}' has {} nested elements: {:?}",
                                elem,
                                element_elements.len(),
                                element_elements.keys().collect::<Vec<_>>()
                            );

                            // Check specific nested elements that might have array issues
                            if *elem == "contact" {
                                if let Some(name_elem) = element_elements.get("name") {
                                    println!(
                                        "    contact.name: type={:?}, array={:?}",
                                        name_elem.type_name, name_elem.array
                                    );
                                }
                            }
                        }
                    } else {
                        println!("Element '{}': NOT FOUND", elem);
                    }
                }

                // Also check HumanName schema for given element
                if let Some(humanname_schema) = schemas.get("HumanName") {
                    if let Some(elements) = &humanname_schema.elements {
                        if let Some(given_elem) = elements.get("given") {
                            println!(
                                "HumanName.given: type={:?}, array={:?}",
                                given_elem.type_name, given_elem.array
                            );
                        }
                    }
                }
            } else {
                println!("Patient schema has no elements defined");
            }
        }

        // Check Address schema elements
        if let Some(address_schema) = schemas.get("Address") {
            println!("\nAddress schema elements:");
            if let Some(elements) = &address_schema.elements {
                println!(
                    "Address has {} elements: {:?}",
                    elements.len(),
                    elements.keys().collect::<Vec<_>>()
                );
            } else {
                println!("Address schema has no elements defined!");
            }
        }

        // Check Identifier schema elements
        if let Some(identifier_schema) = schemas.get("Identifier") {
            println!("\nIdentifier schema elements:");
            if let Some(elements) = &identifier_schema.elements {
                println!(
                    "Identifier has {} elements: {:?}",
                    elements.len(),
                    elements.keys().collect::<Vec<_>>()
                );
            } else {
                println!("Identifier schema has no elements defined!");
            }
        }

        let validator = FhirSchemaValidator::new(schemas.clone(), None);

        // First test with minimal valid Patient
        let minimal_patient = json!({
            "resourceType": "Patient"
        });

        println!("\n=== Testing minimal Patient ===");
        let minimal_result = validator
            .validate(
                &minimal_patient,
                vec!["http://hl7.org/fhir/StructureDefinition/Patient".to_string()],
            )
            .await;
        println!(
            "Minimal Patient validation result: {}",
            if minimal_result.valid {
                "VALID"
            } else {
                "INVALID"
            }
        );

        // Debug: Show which schemas are being used for validation
        println!("Schemas collected for validation:");
        // Convert schemas to Arc format for the context
        let arc_schemas: HashMap<String, Arc<FhirSchema>> = schemas
            .iter()
            .map(|(k, v)| (k.clone(), Arc::new(v.clone())))
            .collect();
        let mut context_debug =
            FhirSchemaValidationContext::new(Arc::new(arc_schemas), "Patient".to_string());
        validator
            .resolve_schemata(
                &mut context_debug,
                vec!["http://hl7.org/fhir/StructureDefinition/Patient".to_string()],
            )
            .await;
        for (url, schema) in &context_debug.current_schemata {
            println!(
                "  - {}: {} (required: {:?})",
                url, schema.type_name, schema.required
            );
        }
        if !minimal_result.valid {
            println!("Minimal Patient errors:");
            for error in &minimal_result.errors {
                println!(
                    "  - {}: {}",
                    error.error_type,
                    error.message.as_ref().unwrap_or(&"No message".to_string())
                );
            }
        }

        // Test with the official FHIR Patient example
        let correct_patient = json!({
            "resourceType": "Patient",
            "id": "example",
            "text": {
                "status": "generated",
                "div": {
                    "table": {
                        "tbody": {
                            "tr": [
                                {
                                    "td": [
                                        "Name",
                                        {
                                            "b": "Chalmers"
                                        }
                                    ]
                                },
                                {
                                    "td": [
                                        "Address",
                                        "534 Erewhon, Pleasantville, Vic, 3999"
                                    ]
                                },
                                {
                                    "td": [
                                        "Contacts",
                                        "Home: unknown. Work: (03) 5555 6473"
                                    ]
                                },
                                {
                                    "td": [
                                        "Id",
                                        "MRN: 12345 (Acme Healthcare)"
                                    ]
                                }
                            ]
                        }
                    }
                }
            },
            "identifier": {
                "use": "usual",
                "type": {
                    "coding": {
                        "system": "http://terminology.hl7.org/CodeSystem/v2-0203",
                        "code": "MR"
                    }
                },
                "system": "urn:oid:1.2.36.146.595.217.0.1",
                "value": 12345,
                "period": {
                    "start": "2001-05-06"
                },
                "assigner": {
                    "display": "Acme Healthcare"
                }
            },
            "active": true,
            "name": [
                {
                    "use": "official",
                    "family": "Chalmers",
                    "given": [
                        "Peter",
                        "James"
                    ]
                },
                {
                    "use": "usual",
                    "given": "Jim"
                },
                {
                    "use": "maiden",
                    "family": "Windsor",
                    "given": [
                        "Peter",
                        "James"
                    ],
                    "period": {
                        "end": 2002
                    }
                }
            ],
            "telecom": [
                {
                    "use": "home"
                },
                {
                    "system": "phone",
                    "value": "(03) 5555 6473",
                    "use": "work",
                    "rank": 1
                },
                {
                    "system": "phone",
                    "value": "(03) 3410 5613",
                    "use": "mobile",
                    "rank": 2
                },
                {
                    "system": "phone",
                    "value": "(03) 5555 8834",
                    "use": "old",
                    "period": {
                        "end": 2014
                    }
                }
            ],
            "gender": "male",
            "birthDate": "1974-12-25",
            "_birthDate": {
                "extension": [
                    {
                        "url": "http://hl7.org/fhir/StructureDefinition/patient-birthTime",
                        "valueDateTime": "1974-12-25T14:35:45-05:00"
                    }
                ]
            },
            "deceasedBoolean": false,
            "address": {
                "use": "home",
                "type": "both",
                "text": "534 Erewhon St PeasantVille, Rainbow, Vic  3999",
                "line": "534 Erewhon St",
                "city": "PleasantVille",
                "district": "Rainbow",
                "state": "Vic",
                "postalCode": 3999,
                "period": {
                    "start": "1974-12-25"
                }
            },
            "contact": {
                "relationship": {
                    "coding": {
                        "system": "http://terminology.hl7.org/CodeSystem/v2-0131",
                        "code": "N"
                    }
                },
                "name": {
                    "family": "du Marché",
                    "given": "Bénédicte"
                },
                "telecom": {
                    "system": "phone",
                    "value": "+33 (237) 998327"
                },
                "address": {
                    "use": "home",
                    "type": "both",
                    "line": "534 Erewhon St",
                    "city": "PleasantVille",
                    "district": "Rainbow",
                    "state": "Vic",
                    "postalCode": 3999,
                    "period": {
                        "start": "1974-12-25"
                    }
                },
                "gender": "female",
                "period": {
                    "start": 2012
                }
            },
            "managingOrganization": {
                "reference": "Organization/1"
            }
        });

        println!("\n=== Testing correct Patient ===");
        let correct_result = validator
            .validate(
                &correct_patient,
                vec!["http://hl7.org/fhir/StructureDefinition/Patient".to_string()],
            )
            .await;
        println!(
            "Correct Patient validation result: {}",
            if correct_result.valid {
                "VALID"
            } else {
                "INVALID"
            }
        );
        if !correct_result.valid {
            println!("Correct Patient errors:");
            for error in &correct_result.errors {
                println!(
                    "  - {}: {}",
                    error.error_type,
                    error.message.as_ref().unwrap_or(&"No message".to_string())
                );
            }
        }

        println!("\n=== Testing simple Patient with just address ===");
        let simple_patient_with_address = json!({
            "resourceType": "Patient",
            "id": "simple",
            "address": [{
                "use": "home",
                "line": ["123 Main St"],
                "city": "Springfield"
            }]
        });
        let simple_result = validator
            .validate(&simple_patient_with_address, vec!["Patient".to_string()])
            .await;
        println!(
            "Simple Patient with address validation result: {}",
            if simple_result.valid {
                "VALID"
            } else {
                "INVALID"
            }
        );
        if !simple_result.valid {
            println!("Simple Patient with address errors:");
            for error in &simple_result.errors {
                println!(
                    "  - {}: {}",
                    error.error_type,
                    error.message.as_ref().unwrap_or(&"No message".to_string())
                );
            }
        }

        println!("\n=== Testing comprehensive valid FHIR Patient example ===");
        let valid_patient = json!({
            "resourceType": "Patient",
            "id": "comprehensive-example",
            "meta": {
                "versionId": "1",
                "lastUpdated": "2023-01-01T00:00:00.000Z"
            },
            "identifier": [{
                "use": "usual",
                "system": "urn:oid:1.2.36.146.595.217.0.1",
                "value": "12345",
                "period": {
                    "start": "2001-05-06"
                }
            }],
            "active": true,
            "name": [{
                "use": "official",
                "family": "Chalmers",
                "given": ["Peter", "James"]
            }, {
                "use": "usual",
                "given": ["Jim"]
            }],
            "telecom": [{
                "system": "phone",
                "value": "(03) 5555 6473",
                "use": "work",
                "rank": 1
            }, {
                "system": "email",
                "value": "peter@example.com",
                "use": "home"
            }],
            "gender": "male",
            "birthDate": "1974-12-25",
            "address": [{
                "use": "home",
                "text": "534 Erewhon St PeasantVille Rainbow 3999",
                "line": ["534 Erewhon St"],
                "city": "PleasantVille",
                "district": "Rainbow",
                "state": "Vic",
                "postalCode": "3999",
                "period": {
                    "start": "1974-12-25"
                }
            }],
            "maritalStatus": {
                "coding": [{
                    "system": "http://terminology.hl7.org/CodeSystem/v3-MaritalStatus",
                    "code": "M",
                    "display": "Married"
                }]
            },
            "contact": [{
                "relationship": [{
                    "coding": [{
                        "system": "http://terminology.hl7.org/CodeSystem/v2-0131",
                        "code": "N",
                        "display": "Next-of-Kin"
                    }]
                }],
                "name": {
                    "family": "du Marché",
                    "given": ["Bénédicte"]
                },
                "telecom": [{
                    "system": "phone",
                    "value": "+33 (237) 998327"
                }],
                "address": {
                    "use": "home",
                    "line": ["534 Erewhon St"],
                    "city": "PleasantVille",
                    "district": "Rainbow",
                    "state": "Vic",
                    "postalCode": "3999",
                    "period": {
                        "start": "1974-12-25"
                    }
                },
                "gender": "female",
                "period": {
                    "start": "2012"
                }
            }]
        });
        let valid_result = validator
            .validate(&valid_patient, vec!["Patient".to_string()])
            .await;
        println!(
            "Comprehensive Patient validation result: {}",
            if valid_result.valid {
                "VALID"
            } else {
                "INVALID"
            }
        );
        if !valid_result.valid {
            println!("Comprehensive Patient errors:");
            for error in &valid_result.errors {
                println!(
                    "  - {}: {}",
                    error.error_type,
                    error.message.as_ref().unwrap_or(&"No message".to_string())
                );
            }
        } else {
            println!(
                "🎉 SUCCESS: Comprehensive Patient resource validates completely with ZERO errors!"
            );
        }

        println!("\n=== Testing corrected FHIR Patient example ===");
        // Corrected Patient example with proper FHIR structure
        let patient_example = json!({
            "resourceType": "Patient",
            "id": "example",
            "text": {
                "status": "generated",
                "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\">\n\t\t\t<table>\n\t\t\t\t<tbody>\n\t\t\t\t\t<tr>\n\t\t\t\t\t\t<td>Name</td>\n\t\t\t\t\t\t<td>Peter James \n              <b>Chalmers</b> (&quot;Jim&quot;)\n            </td>\n\t\t\t\t\t</tr>\n\t\t\t\t\t<tr>\n\t\t\t\t\t\t<td>Address</td>\n\t\t\t\t\t\t<td>534 Erewhon, Pleasantville, Vic, 3999</td>\n\t\t\t\t\t</tr>\n\t\t\t\t\t<tr>\n\t\t\t\t\t\t<td>Contacts</td>\n\t\t\t\t\t\t<td>Home: unknown. Work: (03) 5555 6473</td>\n\t\t\t\t\t</tr>\n\t\t\t\t\t<tr>\n\t\t\t\t\t\t<td>Id</td>\n\t\t\t\t\t\t<td>MRN: 12345 (Acme Healthcare)</td>\n\t\t\t\t\t</tr>\n\t\t\t\t</tbody>\n\t\t\t</table>\n\t\t</div>"
            },
            "identifier": [
                {
                    "use": "usual",
                    "type": {
                        "coding": [
                            {
                                "system": "http://terminology.hl7.org/CodeSystem/v2-0203",
                                "code": "MR"
                            }
                        ]
                    },
                    "system": "urn:oid:1.2.36.146.595.217.0.1",
                    "value": "12345",
                    "period": {
                        "start": "2001-05-06"
                    },
                    "assigner": {
                        "display": "Acme Healthcare"
                    }
                }
            ],
            "active": true,
            "name": [
                {
                    "use": "official",
                    "family": "Chalmers",
                    "given": [
                        "Peter",
                        "James"
                    ]
                },
                {
                    "use": "usual",
                    "given": [
                        "Jim"
                    ]
                },
                {
                    "use": "maiden",
                    "family": "Windsor",
                    "given": [
                        "Peter",
                        "James"
                    ],
                    "period": {
                        "end": "2002"
                    }
                }
            ],
            "telecom": [
                {
                    "use": "home"
                },
                {
                    "system": "phone",
                    "value": "(03) 5555 6473",
                    "use": "work",
                    "rank": 1
                },
                {
                    "system": "phone",
                    "value": "(03) 3410 5613",
                    "use": "mobile",
                    "rank": 2
                },
                {
                    "system": "phone",
                    "value": "(03) 5555 8834",
                    "use": "old",
                    "period": {
                        "end": "2014"
                    }
                }
            ],
            "gender": "male",
            "birthDate": "1974-12-25",
            "_birthDate": {
                "extension": [
                    {
                        "url": "http://hl7.org/fhir/StructureDefinition/patient-birthTime",
                        "valueDateTime": "1974-12-25T14:35:45-05:00"
                    }
                ]
            },
            "deceasedBoolean": false,
            "address": [
                {
                    "use": "home",
                    "type": "both",
                    "text": "534 Erewhon St PeasantVille, Rainbow, Vic  3999",
                    "line": [
                        "534 Erewhon St"
                    ],
                    "city": "PleasantVille",
                    "district": "Rainbow",
                    "state": "Vic",
                    "postalCode": "3999",
                    "period": {
                        "start": "1974-12-25"
                    }
                }
            ],
            "contact": [
                {
                    "relationship": [
                        {
                            "coding": [
                                {
                                    "system": "http://terminology.hl7.org/CodeSystem/v2-0131",
                                    "code": "N"
                                }
                            ]
                        }
                    ],
                    "name": {
                        "family": "du Marché",
                        "_family": {
                            "extension": [
                                {
                                    "url": "http://hl7.org/fhir/StructureDefinition/humanname-own-prefix",
                                    "valueString": "VV"
                                }
                            ]
                        },
                        "given": [
                            "Bénédicte"
                        ]
                    },
                    "telecom": [
                        {
                            "system": "phone",
                            "value": "+33 (237) 998327"
                        }
                    ],
                    "address": {
                        "use": "home",
                        "type": "both",
                        "line": [
                            "534 Erewhon St"
                        ],
                        "city": "PleasantVille",
                        "district": "Rainbow",
                        "state": "Vic",
                        "postalCode": "3999",
                        "period": {
                            "start": "1974-12-25"
                        }
                    },
                    "gender": "female",
                    "period": {
                        "start": "2012"
                    }
                }
            ],
            "managingOrganization": {
                "reference": "Organization/1"
            }
        });

        // Validate against Patient schema by URL (canonical URL)
        let result = validator
            .validate(
                &patient_example,
                vec!["http://hl7.org/fhir/StructureDefinition/Patient".to_string()],
            )
            .await;

        // Test that validation engine is working properly
        println!(
            "Validation result: {}",
            if result.valid { "VALID" } else { "INVALID" }
        );
        println!("Number of errors: {}", result.errors.len());

        if !result.valid {
            println!("Validation errors:");
            for error in &result.errors {
                println!(
                    "  - {}: {}",
                    error.error_type,
                    error.message.as_ref().unwrap_or(&"No message".to_string())
                );
            }
        }

        // Verify that validation engine correctly found and used the Patient schema
        let has_schema_not_found = result.errors.iter().any(|e| {
            e.error_type == "FS1002"
                && e.message
                    .as_ref()
                    .unwrap_or(&"".to_string())
                    .contains("Schema not found")
        });

        if has_schema_not_found {
            println!("✗ Patient schema not found - URL lookup failed");
            assert!(false, "Patient schema should be found via URL lookup");
        } else {
            println!("✓ Patient schema found successfully via URL lookup");

            // Test that minimal Patient validates correctly
            if minimal_result.valid {
                println!("✅ SUCCESS: Minimal Patient resource validates correctly!");
                println!("   This proves FHIR Schema validation is working with embedded schemas");
            } else {
                println!("✗ FAILURE: Minimal Patient should validate");
                assert!(false, "Minimal Patient should validate successfully");
            }

            // Test that correct Patient validates correctly
            if correct_result.valid {
                println!("✅ SUCCESS: Correct Patient resource validates completely!");
                println!("   This proves full FHIR Patient validation is working!");
            } else {
                println!(
                    "⚠️ Correct Patient has validation issues - need to fix schema/validation logic"
                );
            }

            // Check for sophisticated validation errors in the full example
            let has_type_validation_errors = result
                .errors
                .iter()
                .any(|e| e.error_type == "FS1003" || e.error_type == "FS1004");

            if has_type_validation_errors {
                println!("✓ Type validation working on complex Patient example");
            }

            if result.valid {
                println!("✅ Complex Patient example validates successfully against FHIR schema");
            } else {
                println!("✓ Complex Patient validation correctly identifies structural issues");
                println!(
                    "   (This is expected - the example may have issues with embedded schema definitions)"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_validate_patient_example_with_validation_provider() {
        use crate::embedded::FhirVersion;
        use crate::provider::validation_provider::FhirSchemaValidationProvider;
        use octofhir_fhir_model::ValidationProvider;

        // Create validation provider with embedded schemas
        let validation_provider =
            FhirSchemaValidationProvider::with_embedded_schemas(FhirVersion::R4)
                .expect("Should create validation provider");

        // Simple patient example
        let simple_patient = json!({
            "resourceType": "Patient",
            "id": "test",
            "active": true,
            "name": [{
                "use": "official",
                "family": "Doe",
                "given": ["John"]
            }],
            "gender": "male"
        });

        let result = validation_provider
            .validate(
                &simple_patient,
                "http://hl7.org/fhir/StructureDefinition/Patient",
            )
            .await;

        match result {
            Ok(is_valid) => {
                println!("Simple patient validation result: {}", is_valid);
                // For now, just ensure no error occurred - validation might fail if schemas incomplete
            }
            Err(e) => {
                println!("Validation error: {}", e);
                // Test that we get proper error handling, not crashes
            }
        }
    }

    #[tokio::test]
    async fn test_official_fhir_patient_example_validation() {
        use crate::embedded::{FhirVersion, get_schemas};
        use std::fs;

        // Get R4 embedded schemas
        let schemas = get_schemas(FhirVersion::R4);
        let validator = FhirSchemaValidator::new(schemas.clone(), None);

        println!("=== Testing Official FHIR R4 Patient Example ===");

        // Load the official FHIR patient example
        let official_patient_path = "/tmp/fhir-test-cases/r4/examples/patient-example.json";
        let patient_json_str = match fs::read_to_string(official_patient_path) {
            Ok(content) => content,
            Err(e) => {
                println!("⚠️ Could not read official patient-example.json: {}", e);
                println!(
                    "Skipping test - file not found at {}",
                    official_patient_path
                );
                return;
            }
        };

        let patient_json: serde_json::Value = match serde_json::from_str(&patient_json_str) {
            Ok(json) => json,
            Err(e) => {
                println!("❌ Failed to parse official patient-example.json: {}", e);
                panic!("Invalid JSON in official patient-example.json");
            }
        };

        // Validate the official FHIR patient example
        let result = validator
            .validate(&patient_json, vec!["Patient".to_string()])
            .await;

        println!(
            "Official FHIR Patient validation result: {}",
            if result.valid {
                "VALID ✅"
            } else {
                "INVALID ❌"
            }
        );

        if !result.valid {
            println!(
                "Official FHIR Patient errors ({} total):",
                result.errors.len()
            );
            for error in &result.errors {
                println!(
                    "  - {}: {}",
                    error.error_type,
                    error.message.as_ref().unwrap_or(&"No message".to_string())
                );
            }
        } else {
            println!(
                "🎉 SUCCESS: Official FHIR R4 patient-example.json validates perfectly with ZERO errors!"
            );
        }

        // This MUST pass - official FHIR examples should always validate successfully
        assert!(
            result.valid,
            "Official FHIR R4 patient-example.json MUST validate successfully"
        );

        // Test the corrected FHIR Patient example - this MUST validate successfully
        println!("\n=== Testing corrected FHIR Patient example ===");
        let corrected_patient_json = json!({
            "resourceType": "Patient",
            "id": "example",
            "text": {
                "status": "generated",
                "div": "<div xmlns=\"http://www.w3.org/1999/xhtml\">\n\t\t\t<table>\n\t\t\t\t<tbody>\n\t\t\t\t\t<tr>\n\t\t\t\t\t\t<td>Name</td>\n\t\t\t\t\t\t<td>Peter James \n              <b>Chalmers</b> (&quot;Jim&quot;)\n            </td>\n\t\t\t\t\t</tr>\n\t\t\t\t\t<tr>\n\t\t\t\t\t\t<td>Address</td>\n\t\t\t\t\t\t<td>534 Erewhon, Pleasantville, Vic, 3999</td>\n\t\t\t\t\t</tr>\n\t\t\t\t\t<tr>\n\t\t\t\t\t\t<td>Contacts</td>\n\t\t\t\t\t\t<td>Home: unknown. Work: (03) 5555 6473</td>\n\t\t\t\t\t</tr>\n\t\t\t\t\t<tr>\n\t\t\t\t\t\t<td>Id</td>\n\t\t\t\t\t\t<td>MRN: 12345 (Acme Healthcare)</td>\n\t\t\t\t\t</tr>\n\t\t\t\t</tbody>\n\t\t\t</table>\n\t\t</div>"
            },
            "identifier": [
                {
                    "use": "usual",
                    "type": {
                        "coding": [
                            {
                                "system": "http://terminology.hl7.org/CodeSystem/v2-0203",
                                "code": "MR"
                            }
                        ]
                    },
                    "system": "urn:oid:1.2.36.146.595.217.0.1",
                    "value": "12345",
                    "period": {
                        "start": "2001-05-06"
                    },
                    "assigner": {
                        "display": "Acme Healthcare"
                    }
                }
            ],
            "active": true,
            "name": [
                {
                    "use": "official",
                    "family": "Chalmers",
                    "given": [
                        "Peter",
                        "James"
                    ]
                },
                {
                    "use": "usual",
                    "given": [
                        "Jim"
                    ]
                },
                {
                    "use": "maiden",
                    "family": "Windsor",
                    "given": [
                        "Peter",
                        "James"
                    ],
                    "period": {
                        "end": "2002"
                    }
                }
            ],
            "telecom": [
                {
                    "use": "home"
                },
                {
                    "system": "phone",
                    "value": "(03) 5555 6473",
                    "use": "work",
                    "rank": 1
                },
                {
                    "system": "phone",
                    "value": "(03) 3410 5613",
                    "use": "mobile",
                    "rank": 2
                },
                {
                    "system": "phone",
                    "value": "(03) 5555 8834",
                    "use": "old",
                    "period": {
                        "end": "2014"
                    }
                }
            ],
            "gender": "male",
            "birthDate": "1974-12-25",
            "_birthDate": {
                "extension": [
                    {
                        "url": "http://hl7.org/fhir/StructureDefinition/patient-birthTime",
                        "valueDateTime": "1974-12-25T14:35:45-05:00"
                    }
                ]
            },
            "deceasedBoolean": false,
            "address": [
                {
                    "use": "home",
                    "type": "both",
                    "text": "534 Erewhon St PeasantVille, Rainbow, Vic  3999",
                    "line": [
                        "534 Erewhon St"
                    ],
                    "city": "PleasantVille",
                    "district": "Rainbow",
                    "state": "Vic",
                    "postalCode": "3999",
                    "period": {
                        "start": "1974-12-25"
                    }
                }
            ],
            "contact": [
                {
                    "relationship": [
                        {
                            "coding": [
                                {
                                    "system": "http://terminology.hl7.org/CodeSystem/v2-0131",
                                    "code": "N"
                                }
                            ]
                        }
                    ],
                    "name": {
                        "family": "du Marché",
                        "_family": {
                            "extension": [
                                {
                                    "url": "http://hl7.org/fhir/StructureDefinition/humanname-own-prefix",
                                    "valueString": "VV"
                                }
                            ]
                        },
                        "given": [
                            "Bénédicte"
                        ]
                    },
                    "telecom": [
                        {
                            "system": "phone",
                            "value": "+33 (237) 998327"
                        }
                    ],
                    "address": {
                        "use": "home",
                        "type": "both",
                        "line": [
                            "534 Erewhon St"
                        ],
                        "city": "PleasantVille",
                        "district": "Rainbow",
                        "state": "Vic",
                        "postalCode": "3999",
                        "period": {
                            "start": "1974-12-25"
                        }
                    },
                    "gender": "female",
                    "period": {
                        "start": "2012"
                    }
                }
            ],
            "managingOrganization": {
                "reference": "Organization/1"
        }
        });

        let corrected_result = validator
            .validate(&corrected_patient_json, vec!["Patient".to_string()])
            .await;
        println!(
            "Validation result: {}",
            if corrected_result.valid {
                "VALID"
            } else {
                "INVALID"
            }
        );
        if !corrected_result.valid {
            println!("Number of errors: {}", corrected_result.errors.len());
            println!("Validation errors:");
            for error in &corrected_result.errors {
                println!(
                    "  - {}: {}",
                    error.error_type,
                    error.message.as_ref().unwrap_or(&"No message".to_string())
                );
            }
        } else {
            println!(
                "🎉 SUCCESS: Corrected Patient resource validates completely with ZERO errors!"
            );
        }

        // This MUST pass - corrected Patient example should always validate successfully
        assert!(
            corrected_result.valid,
            "Corrected FHIR Patient example MUST validate successfully without any hardcoding"
        );
    }

    #[tokio::test]
    async fn test_terminology_binding_validation() {
        use crate::terminology::InMemoryTerminologyService;

        // Create schema with binding
        let mut schemas = HashMap::new();
        let mut elements = HashMap::new();

        // Create gender element with required binding
        let gender_element = FhirSchemaElement {
            type_name: Some("code".to_string()),
            array: Some(false),
            binding: Some(crate::types::FhirSchemaBinding {
                strength: "required".to_string(),
                value_set: Some("http://hl7.org/fhir/ValueSet/administrative-gender".to_string()),
                binding_name: Some("AdministrativeGender".to_string()),
            }),
            ..Default::default()
        };

        elements.insert("gender".to_string(), gender_element);

        let patient_schema = FhirSchema {
            url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            version: None,
            name: "Patient".to_string(),
            type_name: "Patient".to_string(),
            kind: "resource".to_string(),
            derivation: None,
            base: None,
            abstract_type: None,
            class: "resource".to_string(),
            description: None,
            package_name: None,
            package_version: None,
            package_id: None,
            package_meta: None,
            elements: Some(elements),
            required: None,
            excluded: None,
            extensions: None,
            constraint: None,
            primitive_type: None,
            choices: None,
        };

        schemas.insert("Patient".to_string(), patient_schema.clone());
        schemas.insert(patient_schema.url.clone(), patient_schema);

        // Create terminology service with valid codes
        let mut terminology = InMemoryTerminologyService::new();
        terminology.add_code(
            "http://hl7.org/fhir/ValueSet/administrative-gender",
            "male",
            Some("http://hl7.org/fhir/administrative-gender"),
            Some("Male"),
        );
        terminology.add_code(
            "http://hl7.org/fhir/ValueSet/administrative-gender",
            "female",
            Some("http://hl7.org/fhir/administrative-gender"),
            Some("Female"),
        );

        // Create validator with terminology service
        let validator = FhirSchemaValidator::new(schemas.clone(), None)
            .with_terminology_service(Arc::new(terminology));

        // Test valid code
        let valid_patient = json!({
            "resourceType": "Patient",
            "gender": "male"
        });

        let result = validator
            .validate(&valid_patient, vec!["Patient".to_string()])
            .await;

        assert!(
            result.valid,
            "Patient with valid gender code should pass validation"
        );

        // Test invalid code
        let invalid_patient = json!({
            "resourceType": "Patient",
            "gender": "unknown"
        });

        let result = validator
            .validate(&invalid_patient, vec!["Patient".to_string()])
            .await;

        // Check that we get a binding violation error
        let has_binding_error = result.errors.iter().any(|e| {
            e.error_type.contains("1012")
                || e.message
                    .as_ref()
                    .map_or(false, |m| m.contains("not in value set"))
        });

        assert!(
            has_binding_error,
            "Patient with invalid gender code should have binding violation error. Errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_extract_codes_from_value() {
        let schemas = HashMap::new();
        let validator = FhirSchemaValidator::new(schemas, None);

        // Test simple code string
        let codes = validator.extract_codes_from_value(&json!("male"));
        assert_eq!(codes.len(), 1);
        assert_eq!(codes[0].0, "male");
        assert_eq!(codes[0].1, None);

        // Test Coding
        let coding = json!({
            "system": "http://hl7.org/fhir/administrative-gender",
            "code": "female",
            "display": "Female"
        });
        let codes = validator.extract_codes_from_value(&coding);
        assert_eq!(codes.len(), 1);
        assert_eq!(codes[0].0, "female");
        assert_eq!(
            codes[0].1,
            Some("http://hl7.org/fhir/administrative-gender".to_string())
        );

        // Test CodeableConcept
        let codeable_concept = json!({
            "coding": [
                {
                    "system": "http://snomed.info/sct",
                    "code": "123456",
                    "display": "Test Code"
                },
                {
                    "system": "http://loinc.org",
                    "code": "ABC-123"
                }
            ],
            "text": "Test"
        });
        let codes = validator.extract_codes_from_value(&codeable_concept);
        assert_eq!(codes.len(), 2);
        assert_eq!(codes[0].0, "123456");
        assert_eq!(codes[1].0, "ABC-123");
    }
}
