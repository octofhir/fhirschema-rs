//! StructureDefinition types for conversion.
//!
//! This module contains types for parsing and processing FHIR StructureDefinition
//! resources, which are then converted to FhirSchema format by the converter.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type information within a StructureDefinition element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionType {
    /// FHIR type code
    pub code: String,
    /// Profile URLs for this type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<Vec<String>>,
    /// Target profile URLs for Reference types
    #[serde(rename = "targetProfile", skip_serializing_if = "Option::is_none")]
    pub target_profile: Option<Vec<String>>,
    /// Extensions on the type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<Vec<StructureDefinitionExtension>>,
}

/// Constraint definition in StructureDefinition format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionConstraint {
    /// Constraint key (e.g., "dom-1")
    pub key: String,
    /// Requirements text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<String>,
    /// Severity: error | warning
    pub severity: String,
    /// Human-readable description
    pub human: String,
    /// FHIRPath expression
    pub expression: String,
    /// XPath expression (deprecated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xpath: Option<String>,
}

/// Value set binding in StructureDefinition format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionBinding {
    /// Binding strength: required | extensible | preferred | example
    pub strength: String,
    /// Binding description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Value set URL
    #[serde(rename = "valueSet", skip_serializing_if = "Option::is_none")]
    pub value_set: Option<String>,
    /// Extensions on the binding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<Vec<StructureDefinitionExtension>>,
}

/// Slicing definition in StructureDefinition format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionSlicing {
    /// Discriminators for slicing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Vec<StructureDefinitionDiscriminator>>,
    /// Slicing rules: closed | open | openAtEnd
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<String>,
    /// Whether order matters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordered: Option<bool>,
}

/// Slicing discriminator in StructureDefinition format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionDiscriminator {
    /// Discriminator type: value | exists | pattern | type | profile
    #[serde(rename = "type")]
    pub type_name: String,
    /// Path to discriminating element
    pub path: String,
}

/// Element definition in StructureDefinition format.
///
/// This is the raw format from FHIR StructureDefinition resources.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StructureDefinitionElement {
    /// Element ID (path with slice names)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Element path (e.g., "Patient.name")
    pub path: String,
    /// Slice name (for sliced elements)
    #[serde(rename = "sliceName", skip_serializing_if = "Option::is_none")]
    pub slice_name: Option<String>,
    /// Slicing definition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slicing: Option<StructureDefinitionSlicing>,
    /// Short description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    /// Full definition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    /// Usage comments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Requirements text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<String>,
    /// Element aliases
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<Vec<String>>,
    /// Minimum cardinality
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<i32>,
    /// Maximum cardinality (as string, e.g., "1" or "*")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
    /// Base element definition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<StructureDefinitionBase>,
    /// Content reference (for recursive elements)
    #[serde(rename = "contentReference", skip_serializing_if = "Option::is_none")]
    pub content_reference: Option<String>,
    /// Type information
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_info: Option<Vec<StructureDefinitionType>>,
    /// Constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<Vec<StructureDefinitionConstraint>>,
    /// Must support flag
    #[serde(rename = "mustSupport", skip_serializing_if = "Option::is_none")]
    pub must_support: Option<bool>,
    /// Is modifier flag
    #[serde(rename = "isModifier", skip_serializing_if = "Option::is_none")]
    pub is_modifier: Option<bool>,
    /// Modifier reason
    #[serde(rename = "isModifierReason", skip_serializing_if = "Option::is_none")]
    pub is_modifier_reason: Option<String>,
    /// Is summary flag
    #[serde(rename = "isSummary", skip_serializing_if = "Option::is_none")]
    pub is_summary: Option<bool>,
    /// Value set binding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<StructureDefinitionBinding>,
    /// Mappings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping: Option<Vec<serde_json::Value>>,
    /// Examples
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<Vec<serde_json::Value>>,
    /// Extensions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<Vec<StructureDefinitionExtension>>,

    /// Pattern\[x\] and Fixed\[x\] fields - handled dynamically
    #[serde(flatten)]
    pub pattern_fields: HashMap<String, serde_json::Value>,

    // Internal fields for processing
    /// Target profiles for Reference types
    #[serde(skip)]
    pub refers: Option<Vec<String>>,
    /// Choice group name
    #[serde(skip)]
    pub choice_of: Option<String>,
    /// Allowed choices
    #[serde(skip)]
    pub choices: Option<Vec<String>>,
}

/// Base element definition reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionBase {
    /// Base element path
    pub path: String,
    /// Base minimum cardinality
    pub min: i32,
    /// Base maximum cardinality
    pub max: String,
}

/// Extension within a StructureDefinition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionExtension {
    /// Extension URL
    pub url: String,
    /// String value
    #[serde(rename = "valueString", skip_serializing_if = "Option::is_none")]
    pub value_string: Option<String>,
    /// Canonical URL value
    #[serde(rename = "valueCanonical", skip_serializing_if = "Option::is_none")]
    pub value_canonical: Option<String>,
    /// URL value
    #[serde(rename = "valueUrl", skip_serializing_if = "Option::is_none")]
    pub value_url: Option<String>,
}

/// Main StructureDefinition resource.
///
/// Represents a complete StructureDefinition resource from FHIR.
/// Used as input to the converter to generate FhirSchema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinition {
    /// Resource type (always "StructureDefinition")
    #[serde(rename = "resourceType")]
    pub resource_type: String,
    /// Resource ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Canonical URL
    pub url: String,
    /// Version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Name
    pub name: String,
    /// Title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Publication status
    pub status: String,
    /// Publication date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Kind: resource | complex-type | primitive-type | logical
    pub kind: String,
    /// Whether abstract
    #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub abstract_type: Option<bool>,
    /// Type being defined
    #[serde(rename = "type")]
    pub type_name: String,
    /// Base definition URL
    #[serde(rename = "baseDefinition", skip_serializing_if = "Option::is_none")]
    pub base_definition: Option<String>,
    /// Derivation mode: specialization | constraint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derivation: Option<String>,
    /// Package name
    #[serde(rename = "package_name", skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
    /// Package version
    #[serde(rename = "package_version", skip_serializing_if = "Option::is_none")]
    pub package_version: Option<String>,
    /// Package ID
    #[serde(rename = "package_id", skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    /// Snapshot view
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<StructureDefinitionSnapshot>,
    /// Differential view
    #[serde(skip_serializing_if = "Option::is_none")]
    pub differential: Option<StructureDefinitionDifferential>,
}

/// Snapshot view of a StructureDefinition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionSnapshot {
    /// Complete element list
    pub element: Vec<StructureDefinitionElement>,
}

/// Differential view of a StructureDefinition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureDefinitionDifferential {
    /// Changed elements only
    pub element: Vec<StructureDefinitionElement>,
}

// Type guard

/// Check if a JSON value represents a StructureDefinition
pub fn is_structure_definition(obj: &serde_json::Value) -> bool {
    obj.get("resourceType")
        .and_then(|rt| rt.as_str())
        .map(|rt| rt == "StructureDefinition")
        .unwrap_or(false)
}

// Path parsing types for converter

/// Parsed path component for stack processing.
#[derive(Debug, Clone, PartialEq)]
pub struct PathComponent {
    /// Element name
    pub el: String,
    /// Slicing definition
    pub slicing: Option<serde_json::Value>,
    /// Slice name
    pub slice_name: Option<String>,
    /// Slice definition
    pub slice: Option<serde_json::Value>,
}

/// Action types for stack processing during conversion.
#[derive(Debug, Clone)]
pub enum Action {
    /// Enter an element
    Enter {
        /// Element name
        el: String,
    },
    /// Exit an element
    Exit {
        /// Element name
        el: String,
    },
    /// Enter a slice
    EnterSlice {
        /// Slice name
        slice_name: String,
    },
    /// Exit a slice
    ExitSlice {
        /// Slice name
        slice_name: String,
        /// Slicing definition
        slicing: Option<serde_json::Value>,
        /// Slice definition
        slice: Option<serde_json::Value>,
    },
}

/// Context for conversion operations.
#[derive(Debug, Clone)]
pub struct ConversionContext {
    /// Package metadata
    pub package_meta: Option<serde_json::Value>,
}
