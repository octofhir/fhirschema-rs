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
pub mod questionnaire;

pub use compiled::*;
pub use compiler::*;
pub use questionnaire::{QrStrictness, QuestionnaireProvider};

use crate::reference::ReferenceResolver;
use crate::terminology::TerminologyService;
use crate::types::{FhirSchema, FhirSchemaSlicing, ValidationError, ValidationResult};
use async_trait::async_trait;
use octofhir_fhir_model::FhirPathEvaluator;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

// FHIR R4 primitive type regexes (anchored full-match)
// Source: https://www.hl7.org/fhir/R4/datatypes.html
const INT32_MIN: i64 = -2_147_483_648;
const INT32_MAX: i64 = 2_147_483_647;

static RE_DATE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([0-9]([0-9]([0-9][1-9]|[1-9]0)|[1-9]00)|[1-9]000)(-(0[1-9]|1[0-2])(-(0[1-9]|[1-2][0-9]|3[0-1]))?)?$").unwrap()
});
static RE_DATETIME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([0-9]([0-9]([0-9][1-9]|[1-9]0)|[1-9]00)|[1-9]000)(-(0[1-9]|1[0-2])(-(0[1-9]|[1-2][0-9]|3[0-1])(T([01][0-9]|2[0-3]):[0-5][0-9]:([0-5][0-9]|60)(\.[0-9]{1,9})?(Z|[+\-]((0[0-9]|1[0-3]):[0-5][0-9]|14:00)))?)?)?$").unwrap()
});
static RE_INSTANT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([0-9]([0-9]([0-9][1-9]|[1-9]0)|[1-9]00)|[1-9]000)-(0[1-9]|1[0-2])-(0[1-9]|[1-2][0-9]|3[0-1])T([01][0-9]|2[0-3]):[0-5][0-9]:([0-5][0-9]|60)(\.[0-9]{1,9})?(Z|[+\-]((0[0-9]|1[0-3]):[0-5][0-9]|14:00))$").unwrap()
});
static RE_TIME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([01][0-9]|2[0-3]):[0-5][0-9]:([0-5][0-9]|60)(\.[0-9]{1,9})?$").unwrap()
});
static RE_CODE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[^\s]+(\s[^\s]+)*$").unwrap());
static RE_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9\-\.]{1,64}$").unwrap());
static RE_OID: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^urn:oid:[0-2](\.(0|[1-9][0-9]*))+$").unwrap());
static RE_UUID: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^urn:uuid:[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$",
    )
    .unwrap()
});
static RE_BASE64: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\s*[0-9a-zA-Z+/=]\s*){4,}$").unwrap());

/// Calendar validity for a FHIR date/dateTime/instant date portion. Accepts
/// partial dates (`YYYY`, `YYYY-MM`) — only `YYYY-MM-DD` triggers a day-level
/// check (e.g. rejects `2024-02-31`, `2023-02-29`).
fn is_valid_calendar_date(s: &str) -> bool {
    if s.len() < 10 {
        return true;
    }
    let year: i32 = match s[0..4].parse() {
        Ok(y) => y,
        Err(_) => return false,
    };
    let month: u32 = match s[5..7].parse() {
        Ok(m) => m,
        Err(_) => return false,
    };
    let day: u32 = match s[8..10].parse() {
        Ok(d) => d,
        Err(_) => return false,
    };
    chrono::NaiveDate::from_ymd_opt(year, month, day).is_some()
}

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
    InvalidValue = 1014,
    ReferenceNotFound = 1015,
    QuestionnaireViolation = 1016,
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
            FhirSchemaErrorCode::InvalidValue => write!(f, "FS1014"),
            FhirSchemaErrorCode::ReferenceNotFound => write!(f, "FS1015"),
            FhirSchemaErrorCode::QuestionnaireViolation => write!(f, "FS1016"),
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
    /// Optional provider that resolves `Questionnaire` canonicals so a
    /// `QuestionnaireResponse` can be validated against its form definition.
    questionnaire_provider: Option<Arc<dyn questionnaire::QuestionnaireProvider>>,
    /// Which QuestionnaireResponse convention checks to enforce.
    questionnaire_strictness: questionnaire::QrStrictness,
}

impl FhirValidator {
    /// Create a new compiled validator with a schema provider
    pub fn new(schema_provider: Arc<dyn SchemaProvider>) -> Self {
        Self {
            compiler: SchemaCompiler::new(schema_provider),
            fhirpath_evaluator: None,
            terminology_service: None,
            reference_resolver: None,
            questionnaire_provider: None,
            questionnaire_strictness: questionnaire::QrStrictness::default(),
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
            questionnaire_provider: None,
            questionnaire_strictness: questionnaire::QrStrictness::default(),
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

    /// Add a Questionnaire provider so a `QuestionnaireResponse` is validated
    /// against its referenced `Questionnaire`.
    pub fn with_questionnaire_provider(
        mut self,
        provider: Arc<dyn questionnaire::QuestionnaireProvider>,
    ) -> Self {
        self.questionnaire_provider = Some(provider);
        self
    }

    /// Set which QuestionnaireResponse convention checks to enforce (unknown
    /// linkId, required-missing, disabled-answered). Defaults to normative-only.
    pub fn with_questionnaire_strictness(
        mut self,
        strictness: questionnaire::QrStrictness,
    ) -> Self {
        self.questionnaire_strictness = strictness;
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
        self.validate_with_known_references(resource, schema_names, None)
            .await
    }

    /// Validate a resource, treating a set of references as already existing.
    ///
    /// `known_references` is a set of literal `Type/id` reference strings that
    /// should be considered to exist even if the storage-backed resolver cannot
    /// find them yet. This is required for FHIR `transaction` Bundles: a resource
    /// may reference a sibling that is created in the same Bundle and is therefore
    /// not yet committed to storage when this resource is validated. Without it,
    /// reference existence validation (Phase 4) would reject every intra-Bundle
    /// reference. Passing `None` is equivalent to `validate`.
    pub async fn validate_with_known_references(
        &self,
        resource: &JsonValue,
        schema_names: Vec<String>,
        known_references: Option<&std::collections::HashSet<String>>,
    ) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Prepare constraint variables once (includes %rootResource)
        let variables = Self::prepare_constraint_variables(resource);

        // Memo of FHIRPath constraint results for this resource, shared across
        // every schema in `schema_names`. Overlapping profiles (base type +
        // meta.profile snapshot) repeat the same invariants at the same paths;
        // this evaluates each `(path, expression)` once. Errors are still
        // emitted per schema, so output is unchanged.
        let mut constraint_cache: HashMap<String, bool> = HashMap::new();

        // Start FHIRPath expressions at the resource's resourceType (e.g. "Patient",
        // "Parameters") so issue.expression matches the FHIRPath spec.
        let root_path: std::string::String = resource
            .get("resourceType")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let mut any_schema_compiled = false;
        for schema_name in &schema_names {
            // Get or compile schema (single cache lookup)
            match self.compiler.compile(schema_name).await {
                Ok(compiled) => {
                    any_schema_compiled = true;
                    // Phase 1: Structural validation (sync)
                    self.validate_resource(resource, &compiled, &mut errors, &root_path);

                    // Phase 2: Constraint validation (async)
                    self.validate_constraints_recursive(
                        resource,
                        &compiled,
                        &variables,
                        &mut errors,
                        &root_path,
                        &mut constraint_cache,
                    )
                    .await;
                }
                Err(e) => {
                    // An unresolvable profile canonical (e.g. a `meta.profile`
                    // pointing at a StructureDefinition from a package that is
                    // not loaded) is non-fatal per the FHIR spec: the resource is
                    // still validated against every schema that did resolve, and
                    // the unresolved profile is reported as a warning rather than
                    // failing validity. Only an unresolvable base type (a plain
                    // resourceType name, never a URL) is a hard error.
                    let is_profile_canonical = schema_name.contains("://");
                    let issue = ValidationError {
                        error_type: FhirSchemaErrorCode::UnknownSchema.to_string(),
                        path: vec![],
                        message: Some(e.message),
                        value: None,
                        expected: None,
                        got: None,
                        schema_path: None,
                        constraint_key: None,
                        constraint_expression: None,
                        constraint_severity: Some(if is_profile_canonical {
                            "warning".to_string()
                        } else {
                            "error".to_string()
                        }),
                    };
                    if is_profile_canonical {
                        warnings.push(issue);
                    } else {
                        errors.push(issue);
                    }
                }
            }
        }

        // Phase 3: Walk the JSON tree and validate every Extension against the
        // StructureDefinition referenced by its `url`. Covers nested extensions
        // inside `_field` primitive extensions too, which the constraint walker
        // skips. Extension validation is schema-independent (it resolves each
        // extension's own profile by URL), so run it once regardless of how many
        // schemas were validated — but only when at least one schema compiled,
        // matching the previous behavior of running inside the schema loop.
        if any_schema_compiled {
            self.validate_extensions_recursive(resource, &mut errors, &root_path)
                .await;
        }

        // Phase 3b: QuestionnaireResponse-against-Questionnaire validation.
        // When the resource is a QuestionnaireResponse and its Questionnaire can
        // be resolved (contained `#id` or via the configured provider), the
        // answers are checked against the form definition (answer types,
        // group/display/repeats, answerOption membership).
        if resource.get("resourceType").and_then(|v| v.as_str()) == Some("QuestionnaireResponse")
            && let Some(questionnaire) = self.resolve_questionnaire(resource).await
        {
            questionnaire::validate_questionnaire_response(
                resource,
                &questionnaire,
                self.questionnaire_strictness,
                &mut errors,
            );
        }

        // Phase 4: Reference existence validation (async, optional).
        // Runs only when a reference resolver is configured. Every Reference that
        // carries a literal `reference` string is checked for target existence;
        // contained (`#id`), `urn:`, and external references resolve as skipped
        // (treated as existing) by the resolver, so only genuinely-missing local
        // references are reported. Referential integrity is required by the FHIR
        // spec for servers that enforce it.
        if let Some(resolver) = &self.reference_resolver {
            let mut references: Vec<(String, String)> = Vec::new();
            Self::collect_references(resource, &root_path, &mut references);
            // Drop references that point to resources created/updated elsewhere in
            // the same transaction Bundle. They are not in storage yet but will be
            // after commit, so treat them as existing instead of false-rejecting.
            if let Some(known) = known_references {
                references.retain(|(_, reference)| !known.contains(reference));
            }
            // Resolve all references concurrently. Each resolution is an
            // independent backend round-trip (e.g. a storage existence check);
            // running them sequentially serialized N round-trips (and N pool
            // checkouts) per resource, which dominated write latency for
            // reference-heavy resources like ExplanationOfBenefit. join_all
            // overlaps them so the cost is ~one round-trip instead of N.
            let resolutions = futures::future::join_all(
                references
                    .iter()
                    .map(|(_, reference)| resolver.resolve_reference(reference)),
            )
            .await;
            for ((ref_path, reference), result) in references.into_iter().zip(resolutions) {
                match result {
                    Ok(result) if !result.exists => {
                        errors.push(ValidationError {
                            error_type: FhirSchemaErrorCode::ReferenceNotFound.to_string(),
                            path: self.path_to_vec(&ref_path),
                            message: Some(format!(
                                "Referenced resource '{reference}' does not exist"
                            )),
                            value: Some(JsonValue::String(reference.clone())),
                            expected: None,
                            got: Some(JsonValue::String(reference)),
                            schema_path: None,
                            constraint_key: None,
                            constraint_expression: None,
                            constraint_severity: Some("error".to_string()),
                        });
                    }
                    // Found, skipped (external/contained), or a transient resolver
                    // error: do not hard-fail on lookup failures to avoid false
                    // negatives when the backend is unavailable.
                    _ => {}
                }
            }
        }

        ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// Resolve the `Questionnaire` a `QuestionnaireResponse` answers, either
    /// from a contained resource (`questionnaire: "#id"`) or via the configured
    /// `QuestionnaireProvider` (a canonical URL, optionally `|version`). Returns
    /// `None` when it cannot be resolved, so form-based checks are skipped
    /// rather than reported as failures.
    async fn resolve_questionnaire(&self, qr: &JsonValue) -> Option<Arc<JsonValue>> {
        let canonical = qr.get("questionnaire").and_then(|v| v.as_str())?;

        if let Some(id) = canonical.strip_prefix('#') {
            let contained = qr.get("contained").and_then(|v| v.as_array())?;
            return contained
                .iter()
                .find(|c| {
                    c.get("resourceType").and_then(|v| v.as_str()) == Some("Questionnaire")
                        && c.get("id").and_then(|v| v.as_str()) == Some(id)
                })
                .map(|c| Arc::new(c.clone()));
        }

        self.questionnaire_provider
            .as_ref()?
            .resolve(canonical)
            .await
    }

    /// Recursively collect every literal Reference (`{ "reference": "Type/id" }`)
    /// in a resource as `(json_path, reference_string)` pairs. Logical references
    /// (identifier-only) are skipped because they cannot be existence-checked.
    fn collect_references(
        value: &JsonValue,
        path: &str,
        out: &mut Vec<(std::string::String, std::string::String)>,
    ) {
        match value {
            JsonValue::Object(obj) => {
                if let Some(JsonValue::String(reference)) = obj.get("reference") {
                    out.push((format!("{path}.reference"), reference.clone()));
                }
                for (key, child) in obj {
                    if key == "reference" {
                        continue;
                    }
                    Self::collect_references(child, &format!("{path}.{key}"), out);
                }
            }
            JsonValue::Array(arr) => {
                for (idx, child) in arr.iter().enumerate() {
                    Self::collect_references(child, &format!("{path}[{idx}]"), out);
                }
            }
            _ => {}
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
            if let Some(sibling) = key.strip_prefix('_') {
                self.validate_primitive_extension(
                    sibling,
                    value,
                    &schema.elements,
                    obj,
                    errors,
                    path,
                );
                continue;
            }

            // Translate choice variants (e.g. valueBoolean → value.ofType(boolean)) for
            // FHIRPath-style location strings. Lookup uses raw key; path uses display.
            let display_key = self.choice_display_key(key, &schema.elements);
            let element_path = if path.is_empty() {
                display_key.clone()
            } else {
                format!("{}.{}", path, display_key)
            };

            // Parallel primitive-extension array (`_key`) — used to allow `null`
            // entries in the value array that are filled by an Element extension.
            let underscore_arr = obj
                .get(&format!("_{}", key))
                .and_then(|v| v.as_array())
                .map(|v| v.as_slice());

            if let Some(element) = schema.elements.get(key) {
                self.validate_element_with_underscore(
                    value,
                    element,
                    underscore_arr,
                    errors,
                    &element_path,
                    &schema.elements,
                );
            } else {
                // Check if this is a choice type variant (e.g., valueString for value[x])
                let is_choice_variant = schema
                    .elements
                    .values()
                    .any(|el| el.choices.as_ref().is_some_and(|c| c.contains(key)));

                if is_choice_variant {
                    // Validate against the choice variant's element (locate by stem).
                    if let Some(stem_element) = schema
                        .elements
                        .values()
                        .find(|el| el.choices.as_ref().is_some_and(|c| c.contains(key)))
                    {
                        self.validate_element_with_underscore(
                            value,
                            stem_element,
                            underscore_arr,
                            errors,
                            &element_path,
                            &schema.elements,
                        );
                    }
                } else {
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

    /// Validate an element value, with optional access to the parallel
    /// primitive-extension array (`_field`). `null` entries inside a primitive
    /// array are allowed only at indices where the parallel `_field[i]` is a
    /// non-null Element supplying extension content.
    fn validate_element_with_underscore(
        &self,
        value: &JsonValue,
        element: &CompiledElement,
        underscore_array: Option<&[JsonValue]>,
        errors: &mut Vec<ValidationError>,
        path: &str,
        // Root schema elements, used to resolve `contentReference` targets when
        // descending into elements that reuse another element's definition.
        root: &HashMap<String, CompiledElement>,
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
                // FHIR JSON: empty arrays are invalid. An absent element is encoded
                // by omitting the key; `[]` is not allowed.
                if arr.is_empty() {
                    errors.push(ValidationError {
                        error_type: FhirSchemaErrorCode::CardinalityViolation.to_string(),
                        path: self.path_to_vec(path),
                        message: Some(format!(
                            "Array element '{}' must not be empty",
                            element.name
                        )),
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

                // Validate slicing if defined
                if let Some(slicing) = &element.slicing {
                    self.validate_slicing(arr, slicing, errors, path);
                }

                // Validate each item. `null` is only valid in parallel primitive-extension
                // arrays (`_field`); inside a regular value array it is invalid unless
                // the parallel `_field` array supplies a non-null Element at the same
                // index (extension-fill pattern).
                for (i, item) in arr.iter().enumerate() {
                    let item_path = format!("{}[{}]", path, i);
                    if item.is_null() {
                        // null is allowed only when the parallel `_field[i]` is an
                        // Element that actually provides content (extension or any
                        // key beyond `id`). `{id: "x"}` alone violates ele-1 and
                        // does not "fill" the null value position.
                        let ext_fill = underscore_array
                            .and_then(|arr| arr.get(i))
                            .and_then(|v| v.as_object())
                            .is_some_and(|obj| obj.keys().any(|k| k != "id"));
                        if ext_fill {
                            continue;
                        }
                        errors.push(ValidationError {
                            error_type: FhirSchemaErrorCode::WrongType.to_string(),
                            path: self.path_to_vec(&item_path),
                            message: Some(format!(
                                "null entries are not allowed in '{}' array",
                                element.name
                            )),
                            value: None,
                            expected: None,
                            got: Some(JsonValue::String("null".to_string())),
                            schema_path: None,
                            constraint_key: None,
                            constraint_expression: None,
                            constraint_severity: None,
                        });
                        continue;
                    }
                    self.validate_element_value(item, element, errors, &item_path, root);
                }
            }
        } else {
            // `null` for a non-array element is invalid.
            if value.is_null() {
                errors.push(ValidationError {
                    error_type: FhirSchemaErrorCode::WrongType.to_string(),
                    path: self.path_to_vec(path),
                    message: Some(format!("Element '{}' must not be null", element.name)),
                    value: None,
                    expected: None,
                    got: Some(JsonValue::String("null".to_string())),
                    schema_path: None,
                    constraint_key: None,
                    constraint_expression: None,
                    constraint_severity: None,
                });
                return;
            }
            self.validate_element_value(value, element, errors, path, root);
        }
    }

    /// Validate a single element value (not array)
    fn validate_element_value(
        &self,
        value: &JsonValue,
        element: &CompiledElement,
        errors: &mut Vec<ValidationError>,
        path: &str,
        root: &HashMap<String, CompiledElement>,
    ) {
        match &element.type_info {
            CompiledTypeInfo::Primitive(ptype) => {
                self.validate_primitive(value, *ptype, errors, path);
            }
            CompiledTypeInfo::Complex | CompiledTypeInfo::BackboneElement => {
                // Recursively validate using inlined children. When the element
                // reuses another element's definition via `contentReference`
                // (its own children are empty), resolve the target element from
                // the root schema and validate against its children instead.
                let children = if element.children.is_empty()
                    && let Some(target) =
                        Self::resolve_element_reference(root, element.element_reference.as_deref())
                {
                    &target.children
                } else {
                    &element.children
                };
                self.validate_complex(value, children, errors, path, root);
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

        // 1. JSON-level type check
        let type_ok = match ptype {
            Boolean => value.is_boolean(),
            Integer | Integer64 | UnsignedInt | PositiveInt => {
                // JSON numbers; reject decimal/floats here (only allowed via is_i64/is_u64)
                value.is_i64() || value.is_u64()
            }
            Decimal => value.is_number(),
            String | Uri | Url | Canonical | Code | Oid | Id | Markdown | Uuid | Xhtml => {
                value.is_string()
            }
            Base64Binary => value.is_string(),
            Instant | Date | DateTime | Time => value.is_string(),
        };

        if !type_ok {
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
            return;
        }

        // 2. FHIR-specific format / range validation
        let format_err: Option<std::string::String> = match ptype {
            Boolean => None,
            Integer => {
                let n = value.as_i64().or_else(|| value.as_u64().map(|u| u as i64));
                match n {
                    Some(n) if (INT32_MIN..=INT32_MAX).contains(&n) => None,
                    _ => Some(format!("integer out of 32-bit range: {}", value)),
                }
            }
            UnsignedInt => match value.as_i64().or_else(|| value.as_u64().map(|u| u as i64)) {
                Some(n) if (0..=INT32_MAX).contains(&n) => None,
                _ => Some(format!("unsignedInt out of range [0, 2^31-1]: {}", value)),
            },
            PositiveInt => match value.as_i64().or_else(|| value.as_u64().map(|u| u as i64)) {
                Some(n) if (1..=INT32_MAX).contains(&n) => None,
                _ => Some(format!("positiveInt out of range [1, 2^31-1]: {}", value)),
            },
            Integer64 => None,
            Decimal => {
                // serde_json::Number always parses as valid number; spec regex enforces no leading
                // zeros etc but we lean on JSON parser. Skip extra regex here.
                None
            }
            String | Markdown | Xhtml => {
                let s = value.as_str().unwrap_or("");
                if s.is_empty() {
                    Some(format!("{} must not be empty", ptype.as_str()))
                } else {
                    None
                }
            }
            Uri | Url | Canonical => {
                let s = value.as_str().unwrap_or("");
                if s.is_empty() {
                    Some(format!("{} must not be empty", ptype.as_str()))
                } else {
                    None
                }
            }
            Code => {
                let s = value.as_str().unwrap_or("");
                if !RE_CODE.is_match(s) {
                    Some(format!("code does not match FHIR regex: {:?}", s))
                } else {
                    None
                }
            }
            Id => {
                let s = value.as_str().unwrap_or("");
                if !RE_ID.is_match(s) {
                    Some(format!("id does not match FHIR regex: {:?}", s))
                } else {
                    None
                }
            }
            Oid => {
                let s = value.as_str().unwrap_or("");
                if !RE_OID.is_match(s) {
                    Some(format!("oid does not match FHIR regex: {:?}", s))
                } else {
                    None
                }
            }
            Uuid => {
                let s = value.as_str().unwrap_or("");
                if !RE_UUID.is_match(s) {
                    Some(format!("uuid does not match FHIR regex: {:?}", s))
                } else {
                    None
                }
            }
            Base64Binary => {
                let s = value.as_str().unwrap_or("");
                if !RE_BASE64.is_match(s) {
                    Some(format!("base64Binary does not match FHIR regex: {:?}", s))
                } else {
                    None
                }
            }
            Date => {
                let s = value.as_str().unwrap_or("");
                if !RE_DATE.is_match(s) {
                    Some(format!("date does not match FHIR regex: {:?}", s))
                } else if !is_valid_calendar_date(s) {
                    Some(format!("date is not a valid calendar date: {:?}", s))
                } else {
                    None
                }
            }
            DateTime => {
                let s = value.as_str().unwrap_or("");
                if !RE_DATETIME.is_match(s) {
                    Some(format!("dateTime does not match FHIR regex: {:?}", s))
                } else if !is_valid_calendar_date(&s[..s.len().min(10)]) {
                    Some(format!("dateTime is not a valid calendar date: {:?}", s))
                } else {
                    None
                }
            }
            Instant => {
                let s = value.as_str().unwrap_or("");
                if !RE_INSTANT.is_match(s) {
                    Some(format!("instant does not match FHIR regex: {:?}", s))
                } else if !is_valid_calendar_date(&s[..10]) {
                    Some(format!("instant is not a valid calendar date: {:?}", s))
                } else {
                    None
                }
            }
            Time => {
                let s = value.as_str().unwrap_or("");
                if !RE_TIME.is_match(s) {
                    Some(format!("time does not match FHIR regex: {:?}", s))
                } else {
                    None
                }
            }
        };

        if let Some(msg) = format_err {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::InvalidValue.to_string(),
                path: self.path_to_vec(path),
                message: Some(msg),
                value: Some(value.clone()),
                expected: Some(JsonValue::String(ptype.as_str().to_string())),
                got: None,
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
        root: &HashMap<String, CompiledElement>,
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

        // FHIR ele-1: complex element must have meaningful content. An object with
        // no entries (or only `id`) violates the constraint and is rejected here so
        // we produce a stable issue location even without FHIRPath constraint eval.
        let meaningful = obj.keys().any(|k| k != "id");
        if !meaningful {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::ConstraintViolation.to_string(),
                path: self.path_to_vec(path),
                message: Some("Element must have content (constraint ele-1)".to_string()),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: Some("ele-1".to_string()),
                constraint_expression: Some(
                    "hasValue() or (children().count() > id.count())".to_string(),
                ),
                constraint_severity: Some("error".to_string()),
            });
            return;
        }

        // Validate each property
        for (key, val) in obj {
            // Primitive extensions (`_field`): validate shape against the matching
            // sibling primitive element.
            if let Some(sibling) = key.strip_prefix('_') {
                self.validate_primitive_extension(sibling, val, children, obj, errors, path);
                continue;
            }

            let display_key = self.choice_display_key(key, children);
            let element_path = format!("{}.{}", path, display_key);

            let underscore_arr = obj
                .get(&format!("_{}", key))
                .and_then(|v| v.as_array())
                .map(|v| v.as_slice());

            if let Some(element) = children.get(key) {
                self.validate_element_with_underscore(
                    val,
                    element,
                    underscore_arr,
                    errors,
                    &element_path,
                    root,
                );
            } else {
                // Check for choice type variants
                let is_choice = children
                    .values()
                    .any(|el| el.choices.as_ref().is_some_and(|c| c.contains(key)));

                if is_choice {
                    if let Some(stem_element) = children
                        .values()
                        .find(|el| el.choices.as_ref().is_some_and(|c| c.contains(key)))
                    {
                        self.validate_element_with_underscore(
                            val,
                            stem_element,
                            underscore_arr,
                            errors,
                            &element_path,
                            root,
                        );
                    }
                    continue;
                }
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

    /// Resolve a `contentReference` target to the element it reuses.
    ///
    /// `reference` is the transformer's segment path, `[url, "elements", name,
    /// "elements", name, ...]`. The names after each `"elements"` marker form a
    /// path from the root schema elements, descending through `children`. Used
    /// for self-referential structures such as `QuestionnaireResponse.item.item`.
    fn resolve_element_reference<'a>(
        root: &'a HashMap<String, CompiledElement>,
        reference: Option<&[String]>,
    ) -> Option<&'a CompiledElement> {
        let reference = reference?;
        let mut names = Vec::new();
        let mut it = reference.iter();
        it.next(); // skip the leading resource/type url
        while let Some(seg) = it.next() {
            if seg == "elements"
                && let Some(name) = it.next()
            {
                names.push(name.as_str());
            }
        }
        let (first, rest) = names.split_first()?;
        let mut current = root.get(*first)?;
        for name in rest {
            current = current.children.get(*name)?;
        }
        Some(current)
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

    /// Validate a primitive extension property `_field`. `sibling` is the
    /// stripped key (e.g. `"active"` for `_active`). The matching schema
    /// element must exist, be primitive, and the value must be Element-shaped
    /// (object for scalars, array of object|null for repeating primitives).
    fn validate_primitive_extension(
        &self,
        sibling: &str,
        value: &JsonValue,
        elements: &HashMap<std::string::String, CompiledElement>,
        _parent_obj: &serde_json::Map<std::string::String, JsonValue>,
        errors: &mut Vec<ValidationError>,
        parent_path: &str,
    ) {
        let underscore_path = if parent_path.is_empty() {
            format!("_{}", sibling)
        } else {
            format!("{}._{}", parent_path, sibling)
        };
        let display_path = if parent_path.is_empty() {
            self.choice_display_key(sibling, elements)
        } else {
            format!(
                "{}.{}",
                parent_path,
                self.choice_display_key(sibling, elements)
            )
        };

        // Find the sibling element: direct lookup, then choice variant.
        let element_opt: Option<&CompiledElement> = elements.get(sibling).or_else(|| {
            elements.values().find(|el| {
                el.choices
                    .as_ref()
                    .is_some_and(|c| c.iter().any(|k| k == sibling))
            })
        });

        let Some(element) = element_opt else {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::UnknownElement.to_string(),
                path: self.path_to_vec(&underscore_path),
                message: Some(format!(
                    "Primitive extension '_{}' has no matching sibling element",
                    sibling
                )),
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

        // _field is only valid on primitive elements.
        if !matches!(element.type_info, CompiledTypeInfo::Primitive(_)) {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::WrongType.to_string(),
                path: self.path_to_vec(&display_path),
                message: Some(format!(
                    "Primitive extension '_{}' only valid on primitive elements",
                    sibling
                )),
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

        // Shape check: array primitive expects array of Element|null;
        // scalar primitive expects a single Element object.
        if element.is_array {
            let JsonValue::Array(arr) = value else {
                errors.push(ValidationError {
                    error_type: FhirSchemaErrorCode::ExpectedArray.to_string(),
                    path: self.path_to_vec(&display_path),
                    message: Some(format!(
                        "_{} must be an array (sibling primitive is repeating)",
                        sibling
                    )),
                    value: None,
                    expected: Some(JsonValue::String("array".to_string())),
                    got: Some(JsonValue::String(self.json_type_name(value).to_string())),
                    schema_path: None,
                    constraint_key: None,
                    constraint_expression: None,
                    constraint_severity: None,
                });
                return;
            };
            for (i, item) in arr.iter().enumerate() {
                let item_path = format!("{}[{}]", display_path, i);
                if item.is_null() {
                    continue;
                }
                self.validate_element_object(item, &item_path, errors);
            }
        } else {
            if value.is_array() {
                errors.push(ValidationError {
                    error_type: FhirSchemaErrorCode::UnexpectedArray.to_string(),
                    path: self.path_to_vec(&display_path),
                    message: Some(format!(
                        "_{} must be an Element object, not an array (sibling primitive is scalar)",
                        sibling
                    )),
                    value: None,
                    expected: Some(JsonValue::String("object".to_string())),
                    got: Some(JsonValue::String("array".to_string())),
                    schema_path: None,
                    constraint_key: None,
                    constraint_expression: None,
                    constraint_severity: None,
                });
                return;
            }
            self.validate_element_object(value, &display_path, errors);
        }
    }

    /// Validate a JSON value as a FHIR Element (object with optional id /
    /// extension). Rejects non-objects and unknown keys.
    fn validate_element_object(
        &self,
        value: &JsonValue,
        path: &str,
        errors: &mut Vec<ValidationError>,
    ) {
        let JsonValue::Object(obj) = value else {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::WrongType.to_string(),
                path: self.path_to_vec(path),
                message: Some("Element subpart must be an object with id/extension".to_string()),
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
        if obj.is_empty() {
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::ConstraintViolation.to_string(),
                path: self.path_to_vec(path),
                message: Some("Element subpart must have content (id or extension)".to_string()),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: Some("ele-1".to_string()),
                constraint_expression: Some(
                    "hasValue() or (children().count() > id.count())".to_string(),
                ),
                constraint_severity: Some("error".to_string()),
            });
            return;
        }
        for k in obj.keys() {
            if k != "id" && k != "extension" {
                errors.push(ValidationError {
                    error_type: FhirSchemaErrorCode::UnknownElement.to_string(),
                    path: self.path_to_vec(path),
                    message: Some(format!(
                        "Unknown key '{}' in Element (allowed: id, extension)",
                        k
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

    /// FHIRPath display for a choice variant element: `valueBoolean` →
    /// `value.ofType(boolean)`. Returns the input key unchanged if it isn't a
    /// choice variant of any element in `elements`.
    fn choice_display_key(
        &self,
        key: &str,
        elements: &HashMap<std::string::String, CompiledElement>,
    ) -> std::string::String {
        for el in elements.values() {
            if let Some(choices) = el.choices.as_ref()
                && choices.iter().any(|c| c == key)
                && let Some(suffix) = key.strip_prefix(el.name.as_str())
                && !suffix.is_empty()
            {
                let mut chars = suffix.chars();
                if let Some(first) = chars.next() {
                    let lower_first = first.to_ascii_lowercase();
                    let mut lower_type: std::string::String =
                        std::string::String::with_capacity(suffix.len());
                    lower_type.push(lower_first);
                    lower_type.push_str(chars.as_str());
                    return format!("{}.ofType({})", el.name, lower_type);
                }
            }
        }
        key.to_string()
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
    #[allow(clippy::too_many_arguments)]
    async fn validate_constraints(
        &self,
        data: &JsonValue,
        constraints: &[compiled::CompiledConstraint],
        variables: &HashMap<String, Arc<JsonValue>>,
        errors: &mut Vec<ValidationError>,
        path: &str,
        // When the caller already holds an `Arc<JsonValue>` for `data` (the
        // resource root reuses the `%rootResource` Arc), pass it to avoid a
        // redundant deep clone of the whole resource. `None` => clone.
        data_arc_hint: Option<Arc<JsonValue>>,
        // Per-validate memo of `(path, expression) -> satisfied`. The same
        // FHIRPath invariant is evaluated once per resource walk and reused
        // across overlapping schemas (e.g. base `Patient` and a
        // `us-core-patient` profile whose snapshot repeats the base
        // constraints). Same path == same data node and variables are constant
        // within a `validate` call, so the result is deterministic. Error
        // output is unchanged — every schema still emits its own error on a
        // cached failure; only the recompute is skipped.
        cache: &mut HashMap<String, bool>,
    ) {
        let Some(evaluator) = &self.fhirpath_evaluator else {
            return;
        };

        if constraints.is_empty() {
            return;
        }

        let make_key = |expr: &str| {
            let mut key = String::with_capacity(path.len() + 1 + expr.len());
            key.push_str(path);
            key.push('\u{1f}');
            key.push_str(expr);
            key
        };

        // Pass 1: gather the distinct, not-yet-cached constraint expressions at
        // this level. Warnings are skipped (never evaluated or reported).
        // Deduplicating by `(path, expression)` collapses the identical
        // invariants that overlapping schema snapshots repeat, and lets the
        // whole level be evaluated against a single shared FHIRPath context.
        let mut data_arc: Option<Arc<JsonValue>> = data_arc_hint;
        let mut pending_keys: HashMap<String, ()> = HashMap::new();
        let mut pending: Vec<(String, &str)> = Vec::new();
        for constraint in constraints {
            if constraint.severity == compiled::ConstraintSeverity::Warning {
                continue;
            }
            let key = make_key(&constraint.expression);
            if cache.contains_key(&key) {
                continue;
            }
            if pending_keys.insert(key.clone(), ()).is_none() {
                pending.push((key, constraint.expression.as_str()));
            }
        }

        // Evaluate the pending expressions once against a shared context: the
        // FHIRPath data model for `data` and the `%rootResource` variable are
        // built a single time and reused for every expression at this level,
        // instead of rebuilt per constraint. Per-expression semantics are
        // unchanged (empty / non-boolean / true => satisfied). Evaluation
        // errors stay isolated to the offending expression.
        let mut eval_errors: HashMap<String, String> = HashMap::new();
        if !pending.is_empty() {
            let arc = data_arc
                .get_or_insert_with(|| Arc::new(data.clone()))
                .clone();
            let exprs: Vec<&str> = pending.iter().map(|(_, e)| *e).collect();
            match evaluator
                .evaluate_constraints_shared_context(arc, variables, &exprs)
                .await
            {
                Ok(results) => {
                    for ((key, _), res) in pending.iter().zip(results) {
                        match res {
                            Ok(satisfied) => {
                                cache.insert(key.clone(), satisfied);
                            }
                            Err(e) => {
                                eval_errors.insert(key.clone(), e.to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    // The shared context could not be built. Mark every pending
                    // expression as an evaluation failure, matching the previous
                    // per-constraint behavior where the same build ran (and
                    // would have failed) for each constraint.
                    let msg = e.to_string();
                    for (key, _) in &pending {
                        eval_errors.insert(key.clone(), msg.clone());
                    }
                }
            }
        }

        // Pass 2: emit errors in original constraint order, so output is
        // identical to per-constraint evaluation. Each constraint reports with
        // its own key/human text even when it shares an expression with another.
        for constraint in constraints {
            if constraint.severity == compiled::ConstraintSeverity::Warning {
                continue;
            }
            let key = make_key(&constraint.expression);
            if let Some(&satisfied) = cache.get(&key) {
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
            } else if let Some(err_msg) = eval_errors.get(&key) {
                errors.push(ValidationError {
                    error_type: FhirSchemaErrorCode::ConstraintViolation.to_string(),
                    path: self.path_to_vec(path),
                    message: Some(format!(
                        "Constraint '{}' evaluation failed: {}",
                        constraint.key, err_msg
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
        cache: &mut HashMap<String, bool>,
    ) {
        // Validate schema-level constraints. `data` is the resource root, which
        // is also stored as the `%rootResource` variable — reuse that Arc to
        // skip a full deep clone of the resource.
        let root_arc = variables.get("rootResource").cloned();
        self.validate_constraints(
            data,
            &schema.constraints,
            variables,
            errors,
            path,
            root_arc,
            cache,
        )
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

                self.validate_element_constraints(
                    value,
                    element,
                    variables,
                    errors,
                    &element_path,
                    cache,
                )
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
        cache: &mut HashMap<String, bool>,
    ) {
        // Handle arrays
        if let JsonValue::Array(arr) = value {
            for (i, item) in arr.iter().enumerate() {
                let item_path = format!("{}[{}]", path, i);
                self.validate_single_element_constraints(
                    item, element, variables, errors, &item_path, cache,
                )
                .await;
            }
        } else {
            self.validate_single_element_constraints(
                value, element, variables, errors, path, cache,
            )
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
        cache: &mut HashMap<String, bool>,
    ) {
        // Validate element-level constraints
        self.validate_constraints(
            value,
            &element.constraints,
            variables,
            errors,
            path,
            None,
            cache,
        )
        .await;

        // Validate required ValueSet bindings via the terminology service.
        self.validate_binding(value, element, errors, path).await;

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
                        cache,
                    )
                    .await;
                }
            }
        }
    }

    /// Walk the resource JSON and validate every Extension against the
    /// StructureDefinition referenced by `extension.url`. Each Extension's
    /// `value[x]` choice is checked against the profile's allowed choice
    /// variants; mismatches emit `WrongType` errors. Missing/unresolvable
    /// profiles are silently ignored to avoid noise when packages are partial.
    #[async_recursion::async_recursion]
    async fn validate_extensions_recursive(
        &self,
        value: &JsonValue,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        match value {
            JsonValue::Object(obj) => {
                if let Some(JsonValue::Array(exts)) = obj.get("extension") {
                    for (i, ext) in exts.iter().enumerate() {
                        let ext_path = format!("{}.extension[{}]", path, i);
                        self.validate_one_extension(ext, errors, &ext_path).await;
                    }
                }
                for (k, v) in obj {
                    let child_path = if path.is_empty() {
                        k.clone()
                    } else if k.starts_with('_') {
                        // Underscore-prefixed fields live alongside their
                        // primitive sibling — keep them in the path verbatim
                        // so nested extension expressions are unambiguous.
                        format!("{}.{}", path, k)
                    } else {
                        format!("{}.{}", path, k)
                    };
                    self.validate_extensions_recursive(v, errors, &child_path)
                        .await;
                }
            }
            JsonValue::Array(arr) => {
                for (i, item) in arr.iter().enumerate() {
                    let item_path = format!("{}[{}]", path, i);
                    self.validate_extensions_recursive(item, errors, &item_path)
                        .await;
                }
            }
            _ => {}
        }
    }

    /// Validate a single Extension object against its profile's `value[x]`
    /// choice constraint. Pulls the profile via the configured SchemaProvider.
    async fn validate_one_extension(
        &self,
        ext: &JsonValue,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        let JsonValue::Object(obj) = ext else { return };
        let Some(url) = obj.get("url").and_then(|v| v.as_str()) else {
            return;
        };

        // Profile not loadable (unknown URL, registry incomplete, transport
        // failure) — bail silently rather than emit noise. Catalog coverage is
        // owned by the SchemaProvider implementation.
        let Ok(compiled) = self.compiler.compile(url).await else {
            return;
        };

        // Find the value[x] element in the profile. The element is keyed as
        // `"value"` (FHIR choice stem) and carries `choices: Some([...])` with
        // the allowed `valueXxx` variants.
        let Some(value_element) = compiled.elements.get("value") else {
            return;
        };
        if value_element.choices.is_none() {
            return;
        }
        let allowed: &[std::string::String] = value_element.choices.as_deref().unwrap_or(&[]);

        // Identify which valueXxx key the extension uses, if any.
        let mut used: Option<&str> = None;
        for k in obj.keys() {
            if k.starts_with("value") && k.len() > "value".len() {
                used = Some(k.as_str());
                break;
            }
        }
        let Some(used_key) = used else { return };

        if !allowed.iter().any(|a| a == used_key) {
            let allowed_list = allowed.join(", ");
            errors.push(ValidationError {
                error_type: FhirSchemaErrorCode::WrongType.to_string(),
                path: self.path_to_vec(&format!("{}.{}", path, used_key)),
                message: Some(format!(
                    "Extension {} does not allow {}; allowed value[x]: [{}]",
                    url, used_key, allowed_list
                )),
                value: None,
                expected: Some(JsonValue::String(allowed_list)),
                got: Some(JsonValue::String(used_key.to_string())),
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: Some("error".to_string()),
            });
        }
    }

    /// Validate a code value against its bound ValueSet via the configured
    /// `TerminologyService`. Only `required` bindings trigger a hard error
    /// here; weaker strengths (extensible/preferred/example) are advisory and
    /// left to other checks. If no terminology service is configured, this
    /// silently no-ops — callers wire one via `with_terminology_service`.
    async fn validate_binding(
        &self,
        value: &JsonValue,
        element: &compiled::CompiledElement,
        errors: &mut Vec<ValidationError>,
        path: &str,
    ) {
        let Some(binding) = &element.binding else {
            return;
        };
        if !matches!(binding.strength, compiled::BindingStrength::Required) {
            return;
        }
        let Some(terminology) = self.terminology_service.as_ref() else {
            return;
        };

        // Resolve (code, system) pairs from the element's actual shape.
        // - primitive `code`: value is a JSON string, no system
        // - `Coding`: { system?, code? }
        // - `CodeableConcept`: { coding: [{ system?, code? }, ...] }
        let mut codes: Vec<(
            std::string::String,
            Option<std::string::String>,
            std::string::String,
        )> = Vec::new();
        match value {
            JsonValue::String(s) => codes.push((s.clone(), None, path.to_string())),
            JsonValue::Object(obj) => {
                if let Some(JsonValue::Array(arr)) = obj.get("coding") {
                    for (i, c) in arr.iter().enumerate() {
                        if let JsonValue::Object(cobj) = c {
                            let code = cobj
                                .get("code")
                                .and_then(|v| v.as_str())
                                .map(str::to_string);
                            let system = cobj
                                .get("system")
                                .and_then(|v| v.as_str())
                                .map(str::to_string);
                            if let Some(code) = code {
                                let p = format!("{}.coding[{}]", path, i);
                                codes.push((code, system, p));
                            }
                        }
                    }
                } else if let Some(code) = obj.get("code").and_then(|v| v.as_str()) {
                    let system = obj
                        .get("system")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    codes.push((code.to_string(), system, path.to_string()));
                }
            }
            _ => return,
        }

        for (code, system, code_path) in codes {
            match terminology
                .validate_code(&binding.value_set, &code, system.as_deref())
                .await
            {
                Ok(result) if !result.valid => {
                    let msg = format!(
                        "Code '{}' is not valid in required ValueSet {}",
                        code, binding.value_set
                    );
                    errors.push(ValidationError {
                        error_type: FhirSchemaErrorCode::BindingViolation.to_string(),
                        path: self.path_to_vec(&code_path),
                        message: Some(msg),
                        value: Some(JsonValue::String(code.clone())),
                        expected: Some(JsonValue::String(binding.value_set.clone())),
                        got: Some(JsonValue::String(code.clone())),
                        schema_path: None,
                        constraint_key: None,
                        constraint_expression: None,
                        constraint_severity: Some("error".to_string()),
                    });
                }
                Ok(_) => {}
                Err(_) => {
                    // Lookup failure (unknown ValueSet, transport error, etc.): leave
                    // as advisory rather than hard error to avoid false negatives when
                    // the terminology backend is incomplete.
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
