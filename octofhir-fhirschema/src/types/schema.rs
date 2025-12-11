//! Core FHIR Schema type definitions.
//!
//! This module contains the core types for representing FHIR Schemas:
//! - [`FhirSchema`] - Main schema definition
//! - [`FhirSchemaElement`] - Element definitions within schemas
//! - Supporting types for bindings, patterns, constraints, and slicing

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Value set binding information for an element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaBinding {
    /// Binding strength: required | extensible | preferred | example
    pub strength: String,
    /// Value set URL/canonical
    #[serde(rename = "valueSet", skip_serializing_if = "Option::is_none")]
    pub value_set: Option<String>,
    /// Human-readable binding name
    #[serde(rename = "bindingName", skip_serializing_if = "Option::is_none")]
    pub binding_name: Option<String>,
}

/// Pattern or fixed value definition for an element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaPattern {
    /// FHIR type of the pattern value
    #[serde(rename = "type")]
    pub type_name: String,
    /// The actual pattern/fixed value
    pub value: serde_json::Value,
    /// String representation (for string patterns)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string: Option<String>,
}

/// FHIRPath constraint definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaConstraint {
    /// FHIRPath expression to evaluate
    pub expression: String,
    /// Human-readable description
    pub human: String,
    /// Severity: error | warning
    pub severity: String,
}

/// Slicing discriminator definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaDiscriminator {
    /// Discriminator type: value | exists | pattern | type | profile
    #[serde(rename = "type")]
    pub type_name: String,
    /// Path to the discriminating element
    pub path: String,
}

/// Individual slice definition within a slicing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaSliceMatch {
    /// Pattern value to match against
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub match_value: Option<serde_json::Value>,
    /// Schema for items in this slice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<FhirSchemaElement>,
    /// Minimum cardinality for this slice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<i32>,
    /// Maximum cardinality for this slice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<i32>,
}

/// Slicing definition for array elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaSlicing {
    /// Discriminators defining how to distinguish slices
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Vec<FhirSchemaDiscriminator>>,
    /// Slicing rules: closed | open | openAtEnd
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<String>,
    /// Whether order matters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordered: Option<bool>,
    /// Named slice definitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slices: Option<HashMap<String, FhirSchemaSliceMatch>>,
}

/// Element definition within a FHIR Schema.
///
/// Represents a single data element with type, cardinality, constraints, and other metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FhirSchemaElement {
    // Type information
    /// FHIR type of this element
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// Default type for choice elements
    #[serde(rename = "defaultType", skip_serializing_if = "Option::is_none")]
    pub default_type: Option<String>,
    /// Whether this element is an array
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array: Option<bool>,

    // Cardinality
    /// Minimum cardinality
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<i32>,
    /// Maximum cardinality (None means unbounded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<i32>,

    // References
    /// Target profiles for Reference elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refers: Option<Vec<String>>,
    /// Element references (contentReference)
    #[serde(rename = "elementReference", skip_serializing_if = "Option::is_none")]
    pub element_reference: Option<Vec<String>>,

    // Documentation
    /// Short description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,

    // Binding
    /// Value set binding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<FhirSchemaBinding>,

    // Pattern/Fixed values
    /// Pattern or fixed value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<FhirSchemaPattern>,

    // Constraints
    /// FHIRPath constraints keyed by constraint ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<HashMap<String, FhirSchemaConstraint>>,

    // Nested elements
    /// Nested element definitions (for BackboneElement)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<HashMap<String, FhirSchemaElement>>,

    // Choice type handling
    /// Name of the choice group this element belongs to
    #[serde(rename = "choiceOf", skip_serializing_if = "Option::is_none")]
    pub choice_of: Option<String>,
    /// Allowed types for this choice element
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,

    // Extension URL
    /// URL for extension definitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    // Modifiers
    /// Whether this element must be supported
    #[serde(rename = "mustSupport", skip_serializing_if = "Option::is_none")]
    pub must_support: Option<bool>,
    /// Whether this element can modify meaning
    #[serde(rename = "isModifier", skip_serializing_if = "Option::is_none")]
    pub is_modifier: Option<bool>,
    /// Reason this element is a modifier
    #[serde(rename = "isModifierReason", skip_serializing_if = "Option::is_none")]
    pub is_modifier_reason: Option<String>,
    /// Whether this element is included in summary
    #[serde(rename = "isSummary", skip_serializing_if = "Option::is_none")]
    pub is_summary: Option<bool>,

    // Slicing
    /// Slicing definition for this element
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slicing: Option<FhirSchemaSlicing>,

    // Extensions - using Value to support both HashMap and "[Circular Reference]" string
    /// Extension definitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,

    // Required/excluded elements
    /// Required child elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    /// Excluded child elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded: Option<Vec<String>>,

    // Internal flags
    /// Internal required flag
    #[serde(rename = "_required", skip_serializing_if = "Option::is_none")]
    pub required_flag: Option<bool>,
    /// Element index for ordering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,

    // Additional properties for array ordering
    /// Meaning of ordering for arrays
    #[serde(rename = "orderMeaning", skip_serializing_if = "Option::is_none")]
    pub order_meaning: Option<String>,
}

/// Main FHIR Schema definition.
///
/// Represents a complete FHIR Schema which can be a resource, complex type,
/// primitive type, or profile. Contains all element definitions and metadata.
///
/// # Example
/// ```ignore
/// let schema: FhirSchema = serde_json::from_str(schema_json)?;
/// println!("Schema: {} ({})", schema.name, schema.kind);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchema {
    // Identification
    /// Canonical URL identifying this schema
    pub url: String,
    /// Version of this schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Human-readable name
    pub name: String,

    // Structure type
    /// FHIR type this schema defines
    #[serde(rename = "type")]
    pub type_name: String,
    /// Kind: resource | complex-type | primitive-type
    pub kind: String,

    // Derivation
    /// Derivation mode: specialization | constraint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derivation: Option<String>,
    /// Base schema URL for derived schemas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// Whether this schema is abstract
    #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub abstract_type: Option<bool>,
    /// Class of this schema
    pub class: String,

    // Documentation
    /// Description of this schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // Package information
    /// Source package name
    #[serde(rename = "package_name", skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
    /// Source package version
    #[serde(rename = "package_version", skip_serializing_if = "Option::is_none")]
    pub package_version: Option<String>,
    /// Source package ID
    #[serde(rename = "package_id", skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    /// Package metadata
    #[serde(rename = "package_meta", skip_serializing_if = "Option::is_none")]
    pub package_meta: Option<serde_json::Value>,

    // Content
    /// Element definitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<HashMap<String, FhirSchemaElement>>,
    /// Required elements at root level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    /// Excluded elements at root level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded: Option<Vec<String>>,
    /// Extension definitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
    /// FHIRPath constraints keyed by constraint ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<HashMap<String, FhirSchemaConstraint>>,

    // For primitive types
    /// Primitive type pattern
    #[serde(rename = "primitiveType", skip_serializing_if = "Option::is_none")]
    pub primitive_type: Option<String>,
    /// Choice type definitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<HashMap<String, Vec<String>>>,
}

// Constants

/// FHIR primitive types
pub const FHIR_PRIMITIVE_TYPES: &[&str] = &[
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

/// FHIR complex types
pub const FHIR_COMPLEX_TYPES: &[&str] = &[
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
];

// Type guards

/// Check if a JSON value represents a FHIR Schema
pub fn is_fhir_schema(obj: &serde_json::Value) -> bool {
    obj.is_object() && obj.get("url").is_some() && obj.get("type").is_some()
}

/// Check if a JSON value represents a FHIR Schema element
pub fn is_fhir_schema_element(obj: &serde_json::Value) -> bool {
    obj.is_object() && (obj.get("type").is_some() || obj.get("elements").is_some())
}
