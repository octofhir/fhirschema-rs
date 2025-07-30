//! FHIRSchema Schema definition.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{Element, Constraint};

/// A FHIRSchema Schema represents a FHIR resource or data type definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    /// The canonical URL of the schema
    pub url: String,

    /// The type of resource or data type this schema defines
    #[serde(rename = "type")]
    pub schema_type: String,

    /// Human-readable name for the schema
    pub name: String,

    /// How this schema was derived (specialization, constraint, etc.)
    pub derivation: String,

    /// Base schema this schema is derived from (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,

    /// Elements defined in this schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<HashMap<String, Element>>,

    /// Constraints that apply to this schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<HashMap<String, Constraint>>,

    /// Extensions (for FHIRSchema-specific features)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<HashMap<String, serde_json::Value>>,

    // FHIRSchema Extensions
    /// Allow any additional properties (FHIRSchema extension)
    #[serde(rename = "additionalProperties", skip_serializing_if = "Option::is_none")]
    pub additional_properties: Option<bool>,

    /// Allow any unvalidated content (FHIRSchema extension)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub any: Option<bool>,
}

impl Schema {
    /// Create a new Schema with required fields.
    pub fn new(url: String, schema_type: String, name: String, derivation: String) -> Self {
        Self {
            url,
            schema_type,
            name,
            derivation,
            base: None,
            elements: None,
            constraints: None,
            extensions: None,
            additional_properties: None,
            any: None,
        }
    }
}
