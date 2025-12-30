//! Compiled FHIR Schema types for high-performance validation.
//!
//! This module provides pre-compiled schema representations where all nested
//! types are inlined recursively. This eliminates the need for follow/collect
//! operations during validation, resulting in significant performance improvements.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// A fully-compiled schema with all nested types inlined.
/// No external references - ready for direct validation.
#[derive(Debug, Clone)]
pub struct CompiledSchema {
    /// Original schema URL/name for identification
    pub url: String,
    /// Schema name (e.g., "Patient", "HumanName")
    pub name: String,
    /// Root element definitions with all types expanded inline
    pub elements: HashMap<String, CompiledElement>,
    /// All FHIRPath constraints collected from the type hierarchy
    pub constraints: Vec<CompiledConstraint>,
    /// Required elements at root level
    pub required: HashSet<String>,
    /// Excluded elements (for profiles)
    pub excluded: HashSet<String>,
    /// Whether this is a resource (has resourceType, id, meta)
    pub is_resource: bool,
    /// Schema kind: "resource", "complex-type", "primitive-type"
    pub kind: SchemaKind,
}

/// Schema kind classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    /// FHIR Resource (Patient, Observation, etc.)
    Resource,
    /// Complex data type (HumanName, Address, CodeableConcept)
    ComplexType,
    /// Primitive type (string, boolean, integer, etc.)
    PrimitiveType,
    /// Logical model
    Logical,
}

impl SchemaKind {
    pub fn parse(s: &str) -> Self {
        match s {
            "resource" => SchemaKind::Resource,
            "complex-type" => SchemaKind::ComplexType,
            "primitive-type" => SchemaKind::PrimitiveType,
            "logical" => SchemaKind::Logical,
            _ => SchemaKind::ComplexType, // Default
        }
    }
}

/// Compiled element with all type information inlined
#[derive(Debug, Clone)]
pub struct CompiledElement {
    /// Element name (e.g., "name", "birthDate")
    pub name: String,
    /// Fully resolved type info
    pub type_info: CompiledTypeInfo,
    /// Is this an array element
    pub is_array: bool,
    /// Minimum cardinality
    pub min: i32,
    /// Maximum cardinality (None = unbounded)
    pub max: Option<i32>,
    /// Nested elements (for complex types, inlined from type schema)
    pub children: HashMap<String, CompiledElement>,
    /// Binding info for coded elements
    pub binding: Option<CompiledBinding>,
    /// Reference target types (for Reference elements)
    pub reference_targets: Option<Vec<String>>,
    /// Element-level FHIRPath constraints
    pub constraints: Vec<CompiledConstraint>,
    /// Pattern/fixed value constraints
    pub pattern: Option<serde_json::Value>,
    /// Choice type variants
    pub choices: Option<Vec<String>>,
    /// Slicing definition (for array elements with slices)
    pub slicing: Option<CompiledSlicing>,
    /// Short description
    pub short: Option<String>,
    /// Must support flag
    pub must_support: bool,
    /// Is modifier flag
    pub is_modifier: bool,
}

impl Default for CompiledElement {
    fn default() -> Self {
        Self {
            name: String::new(),
            type_info: CompiledTypeInfo::Complex,
            is_array: false,
            min: 0,
            max: None,
            children: HashMap::new(),
            binding: None,
            reference_targets: None,
            constraints: Vec::new(),
            pattern: None,
            choices: None,
            slicing: None,
            short: None,
            must_support: false,
            is_modifier: false,
        }
    }
}

/// Type classification for compiled elements
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledTypeInfo {
    /// Primitive FHIR type
    Primitive(PrimitiveType),
    /// Complex type with nested children
    Complex,
    /// Reference to another resource
    Reference,
    /// Resource type (for contained resources)
    Resource,
    /// Extension element
    Extension,
    /// BackboneElement (inline complex type)
    BackboneElement,
}

/// FHIR primitive types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Boolean,
    Integer,
    Integer64,
    String,
    Decimal,
    Uri,
    Url,
    Canonical,
    Base64Binary,
    Instant,
    Date,
    DateTime,
    Time,
    Code,
    Oid,
    Id,
    Markdown,
    UnsignedInt,
    PositiveInt,
    Uuid,
    Xhtml,
}

impl PrimitiveType {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "boolean" => Some(PrimitiveType::Boolean),
            "integer" => Some(PrimitiveType::Integer),
            "integer64" => Some(PrimitiveType::Integer64),
            "string" => Some(PrimitiveType::String),
            "decimal" => Some(PrimitiveType::Decimal),
            "uri" => Some(PrimitiveType::Uri),
            "url" => Some(PrimitiveType::Url),
            "canonical" => Some(PrimitiveType::Canonical),
            "base64Binary" => Some(PrimitiveType::Base64Binary),
            "instant" => Some(PrimitiveType::Instant),
            "date" => Some(PrimitiveType::Date),
            "dateTime" => Some(PrimitiveType::DateTime),
            "time" => Some(PrimitiveType::Time),
            "code" => Some(PrimitiveType::Code),
            "oid" => Some(PrimitiveType::Oid),
            "id" => Some(PrimitiveType::Id),
            "markdown" => Some(PrimitiveType::Markdown),
            "unsignedInt" => Some(PrimitiveType::UnsignedInt),
            "positiveInt" => Some(PrimitiveType::PositiveInt),
            "uuid" => Some(PrimitiveType::Uuid),
            "xhtml" => Some(PrimitiveType::Xhtml),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PrimitiveType::Boolean => "boolean",
            PrimitiveType::Integer => "integer",
            PrimitiveType::Integer64 => "integer64",
            PrimitiveType::String => "string",
            PrimitiveType::Decimal => "decimal",
            PrimitiveType::Uri => "uri",
            PrimitiveType::Url => "url",
            PrimitiveType::Canonical => "canonical",
            PrimitiveType::Base64Binary => "base64Binary",
            PrimitiveType::Instant => "instant",
            PrimitiveType::Date => "date",
            PrimitiveType::DateTime => "dateTime",
            PrimitiveType::Time => "time",
            PrimitiveType::Code => "code",
            PrimitiveType::Oid => "oid",
            PrimitiveType::Id => "id",
            PrimitiveType::Markdown => "markdown",
            PrimitiveType::UnsignedInt => "unsignedInt",
            PrimitiveType::PositiveInt => "positiveInt",
            PrimitiveType::Uuid => "uuid",
            PrimitiveType::Xhtml => "xhtml",
        }
    }
}

/// Check if a type name is a primitive type
pub fn is_primitive_type(type_name: &str) -> bool {
    PrimitiveType::parse(type_name).is_some()
}

/// Compiled FHIRPath constraint
#[derive(Debug, Clone)]
pub struct CompiledConstraint {
    /// Constraint key (e.g., "ele-1", "pat-1")
    pub key: String,
    /// FHIRPath expression
    pub expression: String,
    /// Human-readable description
    pub human: String,
    /// Severity: error or warning
    pub severity: ConstraintSeverity,
}

/// Constraint severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSeverity {
    Error,
    Warning,
}

impl ConstraintSeverity {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "warning" => ConstraintSeverity::Warning,
            _ => ConstraintSeverity::Error,
        }
    }
}

/// Compiled binding information for coded elements
#[derive(Debug, Clone)]
pub struct CompiledBinding {
    /// Value set URL
    pub value_set: String,
    /// Binding strength
    pub strength: BindingStrength,
    /// Description
    pub description: Option<String>,
}

/// Binding strength levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingStrength {
    Required,
    Extensible,
    Preferred,
    Example,
}

impl BindingStrength {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "required" => BindingStrength::Required,
            "extensible" => BindingStrength::Extensible,
            "preferred" => BindingStrength::Preferred,
            _ => BindingStrength::Example,
        }
    }
}

/// Type alias for shared compiled schema
pub type SharedCompiledSchema = Arc<CompiledSchema>;

// =============================================================================
// Slicing Types
// =============================================================================

/// Compiled slicing definition for array elements
#[derive(Debug, Clone)]
pub struct CompiledSlicing {
    /// Slicing rules: "open", "closed", or "openAtEnd"
    pub rules: SlicingRules,
    /// Whether order matters
    pub ordered: bool,
    /// Discriminator definitions
    pub discriminators: Vec<CompiledDiscriminator>,
    /// Individual slice definitions
    pub slices: HashMap<String, CompiledSlice>,
}

/// Slicing rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlicingRules {
    /// Additional content allowed anywhere
    #[default]
    Open,
    /// No additional content allowed
    Closed,
    /// Additional content allowed only at the end
    OpenAtEnd,
}

impl SlicingRules {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "closed" => SlicingRules::Closed,
            "openatend" => SlicingRules::OpenAtEnd,
            _ => SlicingRules::Open,
        }
    }
}

/// Compiled discriminator
#[derive(Debug, Clone)]
pub struct CompiledDiscriminator {
    /// Discriminator type
    pub discriminator_type: DiscriminatorType,
    /// Path to discriminating element
    pub path: String,
}

/// Discriminator type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscriminatorType {
    /// Match by value
    Value,
    /// Match by existence
    Exists,
    /// Match by pattern
    Pattern,
    /// Match by type
    Type,
    /// Match by profile
    Profile,
}

impl DiscriminatorType {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "exists" => DiscriminatorType::Exists,
            "pattern" => DiscriminatorType::Pattern,
            "type" => DiscriminatorType::Type,
            "profile" => DiscriminatorType::Profile,
            _ => DiscriminatorType::Value,
        }
    }
}

/// Compiled slice definition
#[derive(Debug, Clone)]
pub struct CompiledSlice {
    /// Slice name
    pub name: String,
    /// Match pattern (for discriminator matching)
    pub match_value: Option<serde_json::Value>,
    /// Minimum cardinality for this slice
    pub min: Option<i32>,
    /// Maximum cardinality for this slice
    pub max: Option<i32>,
    /// Schema for items in this slice (nested element definition)
    pub schema: Option<Box<CompiledElement>>,
}

/// Result of classifying an array item against slices
#[derive(Debug, Clone, PartialEq)]
pub enum SliceClassification {
    /// Item matched exactly one slice
    Matched(String),
    /// Item didn't match any slice
    Unmatched,
    /// Item matched multiple slices (ambiguous)
    Ambiguous(Vec<String>),
}
