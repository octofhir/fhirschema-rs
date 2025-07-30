//! FHIRSchema Binding definition.

use serde::{Deserialize, Serialize};

/// A FHIRSchema Binding represents terminology binding information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    /// ValueSet reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_set: Option<String>,

    /// Binding strength (required, extensible, preferred, example)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strength: Option<String>,

    /// Description of the binding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Code systems that are part of this binding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_systems: Option<Vec<String>>,

    /// Additional bindings for extensible bindings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional: Option<Vec<AdditionalBinding>>,

    /// Extensions for binding-specific properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// Additional binding information for extensible bindings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdditionalBinding {
    /// Purpose of the additional binding
    pub purpose: String,

    /// ValueSet reference for the additional binding
    pub value_set: String,

    /// Documentation for the additional binding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

impl Binding {
    /// Create a new Binding.
    pub fn new() -> Self {
        Self {
            value_set: None,
            strength: None,
            description: None,
            code_systems: None,
            additional: None,
            extensions: None,
        }
    }

    /// Create a new Binding with a ValueSet reference.
    pub fn with_value_set(value_set: String) -> Self {
        Self {
            value_set: Some(value_set),
            strength: None,
            description: None,
            code_systems: None,
            additional: None,
            extensions: None,
        }
    }
}

impl AdditionalBinding {
    /// Create a new AdditionalBinding.
    pub fn new(purpose: String, value_set: String) -> Self {
        Self {
            purpose,
            value_set,
            documentation: None,
        }
    }
}

impl Default for Binding {
    fn default() -> Self {
        Self::new()
    }
}
