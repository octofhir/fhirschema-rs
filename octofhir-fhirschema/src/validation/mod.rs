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

use crate::reference::ReferenceResolver;
use crate::terminology::TerminologyService;
use crate::types::{FhirSchema, FhirSchemaSlicing, ValidationError, ValidationResult};
use async_trait::async_trait;
use octofhir_fhir_model::FhirPathEvaluator;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
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
    // Convenience constructors for tests
    // =========================================================================

    /// Create validator from a HashMap of schemas (test convenience method).
    ///
    /// Wraps schemas in InMemorySchemaProvider for easy test setup.
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
    fn prepare_constraint_variables(root_resource: &JsonValue) -> HashMap<String, Arc<JsonValue>> {
        let mut variables = HashMap::with_capacity(1);
        variables.insert("rootResource".to_string(), Arc::new(root_resource.clone()));
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
            if !obj.contains_key(required)
                && !self.has_choice_variant(obj, required, &schema.elements)
            {
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

            // Compute element path once for both known and unknown elements
            let element_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", path, key)
            };

            if let Some(element) = schema.elements.get(key) {
                self.validate_element(value, element, errors, &element_path);
            } else {
                // Check if this is a choice type variant (e.g., valueString for value[x])
                let is_choice_variant = schema
                    .elements
                    .values()
                    .any(|el| el.choices.as_ref().is_some_and(|c| c.contains(key)));

                if !is_choice_variant {
                    errors.push(ValidationError {
                        error_type: FhirSchemaErrorCode::UnknownElement.to_string(),
                        path: self.path_to_vec(&element_path),
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
                // Validate slicing if defined
                if let Some(slicing) = &element.slicing {
                    self.validate_slicing(arr, slicing, errors, path);
                }

                // Validate each item
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

            let element_path = format!("{}.{}", path, key);

            if let Some(element) = children.get(key) {
                self.validate_element(val, element, errors, &element_path);
            } else {
                // Check for choice type variants
                let is_choice = children
                    .values()
                    .any(|el| el.choices.as_ref().is_some_and(|c| c.contains(key)));

                if !is_choice && key != "extension" && key != "id" {
                    errors.push(ValidationError {
                        error_type: FhirSchemaErrorCode::UnknownElement.to_string(),
                        path: self.path_to_vec(&element_path),
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
        let has_reference = obj.get("reference").is_some_and(|v| v.is_string());
        let has_identifier = obj.contains_key("identifier");
        let has_display = obj.get("display").is_some_and(|v| v.is_string());

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
    fn validate_extension(&self, value: &JsonValue, errors: &mut Vec<ValidationError>, path: &str) {
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
        if let Some(element) = elements.get(element_name)
            && let Some(choices) = &element.choices
        {
            return choices.iter().any(|choice| obj.contains_key(choice));
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
        variables: &HashMap<String, Arc<JsonValue>>,
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
                .evaluate_constraint_with_variables(&constraint.expression, data, variables)
                .await
            {
                Ok(satisfied) => {
                    if !satisfied {
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
        variables: &HashMap<String, Arc<JsonValue>>,
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
        variables: &HashMap<String, Arc<JsonValue>>,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        // Handle arrays
        if let JsonValue::Array(arr) = value {
            for (i, item) in arr.iter().enumerate() {
                let item_path = format!("{}[{}]", path, i);
                self.validate_single_element_constraints(
                    item, element, variables, errors, &item_path,
                )
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
        variables: &HashMap<String, Arc<JsonValue>>,
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

    // =========================================================================
    // Slicing Validation
    // =========================================================================

    /// Deep partial match for pattern matching in slicing.
    ///
    /// Matches if all keys in pattern exist in item with matching values.
    /// For arrays, uses "contains" semantics - every pattern element must match at least one item.
    pub fn deep_partial_match(item: &JsonValue, pattern: &JsonValue) -> bool {
        match pattern {
            // Null pattern matches anything
            JsonValue::Null => true,

            // Object pattern: all keys must exist in item with matching values
            JsonValue::Object(pattern_map) => {
                if pattern_map.is_empty() {
                    return true;
                }

                let Some(item_map) = item.as_object() else {
                    return false;
                };

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

            // Array pattern: every pattern element must find a match
            JsonValue::Array(pattern_array) => {
                if pattern_array.is_empty() {
                    return true;
                }

                let Some(item_array) = item.as_array() else {
                    return false;
                };

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

    /// Classify an array item against slice definitions.
    ///
    /// Returns which slice(s) the item matches based on match patterns.
    pub fn classify_slice(
        &self,
        item: &JsonValue,
        slices: &HashMap<String, compiled::CompiledSlice>,
    ) -> compiled::SliceClassification {
        let mut matched_slices: Vec<String> = Vec::new();

        for (slice_name, slice_def) in slices {
            let matches = match &slice_def.match_value {
                None => true, // No pattern = unconditional match (catch-all)
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
            0 => compiled::SliceClassification::Unmatched,
            1 => compiled::SliceClassification::Matched(matched_slices.into_iter().next().unwrap()),
            _ => compiled::SliceClassification::Ambiguous(matched_slices),
        }
    }

    /// Validate slicing for an array element.
    ///
    /// Classifies items, validates cardinality, and enforces slicing rules.
    pub fn validate_slicing(
        &self,
        items: &[JsonValue],
        slicing: &compiled::CompiledSlicing,
        errors: &mut Vec<ValidationError>,
        element_path: &str,
    ) {
        if slicing.slices.is_empty() {
            return;
        }

        // Track counts per slice and last matched index for openAtEnd
        let mut slice_counts: HashMap<String, usize> = HashMap::new();
        let mut last_matched_index: Option<usize> = None;

        // Initialize counts
        for slice_name in slicing.slices.keys() {
            slice_counts.insert(slice_name.clone(), 0);
        }

        // Classify each item
        for (index, item) in items.iter().enumerate() {
            let classification = self.classify_slice(item, &slicing.slices);

            match classification {
                compiled::SliceClassification::Matched(slice_name) => {
                    *slice_counts.entry(slice_name).or_insert(0) += 1;
                    last_matched_index = Some(index);
                }
                compiled::SliceClassification::Unmatched => {
                    match slicing.rules {
                        compiled::SlicingRules::Closed => {
                            errors.push(ValidationError {
                                error_type: FhirSchemaErrorCode::SlicingUnmatched.to_string(),
                                path: self.path_to_vec(&format!("{}[{}]", element_path, index)),
                                message: Some(
                                    "Item does not match any defined slice (closed slicing)"
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
                        compiled::SlicingRules::OpenAtEnd => {
                            if let Some(last_idx) = last_matched_index
                                && index < last_idx
                            {
                                errors.push(ValidationError {
                                    error_type: FhirSchemaErrorCode::SlicingUnmatched.to_string(),
                                    path: self.path_to_vec(&format!("{}[{}]", element_path, index)),
                                    message: Some(
                                        "Unmatched item appears before matched items (openAtEnd)"
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
                        compiled::SlicingRules::Open => {} // Unmatched allowed
                    }
                }
                compiled::SliceClassification::Ambiguous(matched_slices) => {
                    errors.push(ValidationError {
                        error_type: FhirSchemaErrorCode::SlicingAmbiguous.to_string(),
                        path: self.path_to_vec(&format!("{}[{}]", element_path, index)),
                        message: Some(format!(
                            "Item matches multiple slices: {}",
                            matched_slices.join(", ")
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
        }

        // Validate cardinality
        self.validate_slice_cardinality(&slice_counts, slicing, errors, element_path);
    }

    /// Validate cardinality constraints for all slices.
    fn validate_slice_cardinality(
        &self,
        slice_counts: &HashMap<String, usize>,
        slicing: &compiled::CompiledSlicing,
        errors: &mut Vec<ValidationError>,
        element_path: &str,
    ) {
        for (slice_name, slice_def) in &slicing.slices {
            let count = slice_counts.get(slice_name).copied().unwrap_or(0);

            // Check minimum
            if let Some(min) = slice_def.min
                && (count as i32) < min
            {
                errors.push(ValidationError {
                    error_type: FhirSchemaErrorCode::SliceCardinality.to_string(),
                    path: self.path_to_vec(element_path),
                    message: Some(format!(
                        "Slice '{}' requires minimum {} items, found {}",
                        slice_name, min, count
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

            // Check maximum
            if let Some(max) = slice_def.max
                && (count as i32) > max
            {
                errors.push(ValidationError {
                    error_type: FhirSchemaErrorCode::SliceCardinality.to_string(),
                    path: self.path_to_vec(element_path),
                    message: Some(format!(
                        "Slice '{}' allows maximum {} items, found {}",
                        slice_name, max, count
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
    }
}
