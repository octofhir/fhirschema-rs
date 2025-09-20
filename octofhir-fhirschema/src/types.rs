use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Core FHIRSchema Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaBinding {
    pub strength: String,
    #[serde(rename = "valueSet", skip_serializing_if = "Option::is_none")]
    pub value_set: Option<String>,
    #[serde(rename = "bindingName", skip_serializing_if = "Option::is_none")]
    pub binding_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaPattern {
    #[serde(rename = "type")]
    pub type_name: String,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaConstraint {
    pub expression: String,
    pub human: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaDiscriminator {
    #[serde(rename = "type")]
    pub type_name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaSliceMatch {
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub match_value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<FhirSchemaElement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaSlicing {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Vec<FhirSchemaDiscriminator>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordered: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slices: Option<HashMap<String, FhirSchemaSliceMatch>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FhirSchemaElement {
    // Type information
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(rename = "defaultType", skip_serializing_if = "Option::is_none")]
    pub default_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array: Option<bool>,

    // Cardinality
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<i32>,

    // References
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refers: Option<Vec<String>>,
    #[serde(rename = "elementReference", skip_serializing_if = "Option::is_none")]
    pub element_reference: Option<Vec<String>>,

    // Documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,

    // Binding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<FhirSchemaBinding>,

    // Pattern/Fixed values
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<FhirSchemaPattern>,

    // Constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<HashMap<String, FhirSchemaConstraint>>,

    // Nested elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<HashMap<String, FhirSchemaElement>>,

    // Choice type handling
    #[serde(rename = "choiceOf", skip_serializing_if = "Option::is_none")]
    pub choice_of: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,

    // Extension URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    // Modifiers
    #[serde(rename = "mustSupport", skip_serializing_if = "Option::is_none")]
    pub must_support: Option<bool>,
    #[serde(rename = "isModifier", skip_serializing_if = "Option::is_none")]
    pub is_modifier: Option<bool>,
    #[serde(rename = "isModifierReason", skip_serializing_if = "Option::is_none")]
    pub is_modifier_reason: Option<String>,
    #[serde(rename = "isSummary", skip_serializing_if = "Option::is_none")]
    pub is_summary: Option<bool>,

    // Slicing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slicing: Option<FhirSchemaSlicing>,

    // Extensions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<HashMap<String, FhirSchemaElement>>,

    // Required/excluded elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded: Option<Vec<String>>,

    // Internal flags
    #[serde(rename = "_required", skip_serializing_if = "Option::is_none")]
    pub required_flag: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,

    // Additional properties for array ordering
    #[serde(rename = "orderMeaning", skip_serializing_if = "Option::is_none")]
    pub order_meaning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchema {
    // Identification
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub name: String,

    // Structure type
    #[serde(rename = "type")]
    pub type_name: String,
    pub kind: String,

    // Derivation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derivation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub abstract_type: Option<bool>,
    pub class: String,

    // Documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // Package information
    #[serde(rename = "package_name", skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
    #[serde(rename = "package_version", skip_serializing_if = "Option::is_none")]
    pub package_version: Option<String>,
    #[serde(rename = "package_id", skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    #[serde(rename = "package_meta", skip_serializing_if = "Option::is_none")]
    pub package_meta: Option<serde_json::Value>,

    // Content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<HashMap<String, FhirSchemaElement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<HashMap<String, FhirSchemaElement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<HashMap<String, FhirSchemaConstraint>>,

    // For primitive types
    #[serde(rename = "primitiveType", skip_serializing_if = "Option::is_none")]
    pub primitive_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<HashMap<String, Vec<String>>>,
}

// Validation Types

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationContext {
    pub schemas: HashMap<String, FhirSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub path: Vec<serde_json::Value>, // Can be string or number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub got: Option<serde_json::Value>,
    #[serde(rename = "schema-path", skip_serializing_if = "Option::is_none")]
    pub schema_path: Option<Vec<serde_json::Value>>, // Can be string or number
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(message) = &self.message {
            write!(f, "{message}")
        } else {
            write!(f, "Validation error: {}", self.error_type)
        }
    }
}

impl std::error::Error for ValidationError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub valid: bool,
}

// Converter Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionType {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<Vec<String>>,
    #[serde(rename = "targetProfile", skip_serializing_if = "Option::is_none")]
    pub target_profile: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<Vec<StructureDefinitionExtension>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionConstraint {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<String>,
    pub severity: String,
    pub human: String,
    pub expression: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xpath: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionBinding {
    pub strength: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "valueSet", skip_serializing_if = "Option::is_none")]
    pub value_set: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<Vec<StructureDefinitionExtension>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionSlicing {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Vec<StructureDefinitionDiscriminator>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordered: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionDiscriminator {
    #[serde(rename = "type")]
    pub type_name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StructureDefinitionElement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub path: String,
    #[serde(rename = "sliceName", skip_serializing_if = "Option::is_none")]
    pub slice_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slicing: Option<StructureDefinitionSlicing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<StructureDefinitionBase>,
    #[serde(rename = "contentReference", skip_serializing_if = "Option::is_none")]
    pub content_reference: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_info: Option<Vec<StructureDefinitionType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<Vec<StructureDefinitionConstraint>>,
    #[serde(rename = "mustSupport", skip_serializing_if = "Option::is_none")]
    pub must_support: Option<bool>,
    #[serde(rename = "isModifier", skip_serializing_if = "Option::is_none")]
    pub is_modifier: Option<bool>,
    #[serde(rename = "isModifierReason", skip_serializing_if = "Option::is_none")]
    pub is_modifier_reason: Option<String>,
    #[serde(rename = "isSummary", skip_serializing_if = "Option::is_none")]
    pub is_summary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<StructureDefinitionBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<Vec<StructureDefinitionExtension>>,

    // Pattern[x] and Fixed[x] fields - we'll handle these dynamically
    #[serde(flatten)]
    pub pattern_fields: HashMap<String, serde_json::Value>,

    // Internal fields for processing
    #[serde(skip)]
    pub refers: Option<Vec<String>>,
    #[serde(skip)]
    pub choice_of: Option<String>,
    #[serde(skip)]
    pub choices: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionBase {
    pub path: String,
    pub min: i32,
    pub max: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionExtension {
    pub url: String,
    #[serde(rename = "valueString", skip_serializing_if = "Option::is_none")]
    pub value_string: Option<String>,
    #[serde(rename = "valueCanonical", skip_serializing_if = "Option::is_none")]
    pub value_canonical: Option<String>,
    #[serde(rename = "valueUrl", skip_serializing_if = "Option::is_none")]
    pub value_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinition {
    #[serde(rename = "resourceType")]
    pub resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub kind: String,
    #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub abstract_type: Option<bool>,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(rename = "baseDefinition", skip_serializing_if = "Option::is_none")]
    pub base_definition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derivation: Option<String>,
    #[serde(rename = "package_name", skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
    #[serde(rename = "package_version", skip_serializing_if = "Option::is_none")]
    pub package_version: Option<String>,
    #[serde(rename = "package_id", skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<StructureDefinitionSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub differential: Option<StructureDefinitionDifferential>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionSnapshot {
    pub element: Vec<StructureDefinitionElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionDifferential {
    pub element: Vec<StructureDefinitionElement>,
}

// Path parsing types

#[derive(Debug, Clone, PartialEq)]
pub struct PathComponent {
    pub el: String,
    pub slicing: Option<serde_json::Value>,
    pub slice_name: Option<String>,
    pub slice: Option<serde_json::Value>,
}

// Action types for stack processing

#[derive(Debug, Clone)]
pub enum Action {
    Enter {
        el: String,
    },
    Exit {
        el: String,
    },
    EnterSlice {
        slice_name: String,
    },
    ExitSlice {
        slice_name: String,
        slicing: Option<serde_json::Value>,
        slice: Option<serde_json::Value>,
    },
}

// Context types

#[derive(Debug, Clone)]
pub struct ConversionContext {
    pub package_meta: Option<serde_json::Value>,
}

// Constants

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

pub const VALIDATION_ERROR_TYPES: &[&str] = &[
    "required",
    "type",
    "cardinality",
    "pattern",
    "constraint",
    "reference",
    "unknown-element",
    "invalid-choice",
    "slice-cardinality",
    "discriminator",
];

// Type guards

pub fn is_fhir_schema(obj: &serde_json::Value) -> bool {
    obj.is_object() && obj.get("url").is_some() && obj.get("type").is_some()
}

pub fn is_fhir_schema_element(obj: &serde_json::Value) -> bool {
    obj.is_object() && (obj.get("type").is_some() || obj.get("elements").is_some())
}

pub fn is_structure_definition(obj: &serde_json::Value) -> bool {
    obj.get("resourceType")
        .and_then(|rt| rt.as_str())
        .map(|rt| rt == "StructureDefinition")
        .unwrap_or(false)
}
