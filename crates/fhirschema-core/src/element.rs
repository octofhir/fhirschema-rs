//! FHIRSchema Element definition.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{Constraint, Slicing, Binding};

/// Element type classification for validation purposes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ElementType {
    /// Simple type with a single type name
    Simple(String),
    /// Choice type with multiple possible types
    Choice(HashMap<String, Element>),
    /// Complex type with nested elements
    Complex(HashMap<String, Element>),
    /// Reference type
    Reference(Vec<String>),
}

/// A FHIRSchema Element represents a data element within a schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    // Shape properties
    /// Whether this element is an array
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array: Option<bool>,

    /// Whether this element is scalar
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scalar: Option<bool>,

    // Cardinality properties
    /// Minimum cardinality
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<u32>,

    /// Maximum cardinality
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,

    // Choice type properties
    /// Indicates this element is a choice of types
    #[serde(rename = "choiceOf", skip_serializing_if = "Option::is_none")]
    pub choice_of: Option<String>,

    /// Available choices for choice types
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<HashMap<String, Element>>,

    // Type reference properties
    /// Element reference to another element
    #[serde(rename = "elementReference", skip_serializing_if = "Option::is_none")]
    pub element_reference: Option<String>,

    /// Type reference
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub element_type: Option<String>,

    // Nested elements
    /// Child elements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elements: Option<HashMap<String, Element>>,

    // Constraints
    /// Constraints that apply to this element
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<HashMap<String, Constraint>>,

    // Slicing
    /// Slicing definition for arrays
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slicing: Option<Slicing>,

    // Terminology binding
    /// Terminology binding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<Binding>,

    // Pattern matching
    /// Fixed value pattern
    #[serde(rename = "fixed", skip_serializing_if = "Option::is_none")]
    pub fixed: Option<serde_json::Value>,

    /// Pattern value
    #[serde(rename = "pattern", skip_serializing_if = "Option::is_none")]
    pub pattern: Option<serde_json::Value>,

    // Reference targets
    /// Target profiles for references
    #[serde(rename = "refers", skip_serializing_if = "Option::is_none")]
    pub refers: Option<Vec<String>>,

    // Informational properties
    /// Short description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,

    /// Definition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,

    /// Comments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    /// Requirements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<String>,

    /// Aliases
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<Vec<String>>,

    /// Examples
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<Vec<serde_json::Value>>,

    // Extensions
    /// Extensions for additional properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<HashMap<String, serde_json::Value>>,
}

impl Element {
    /// Create a new Element.
    pub fn new() -> Self {
        Self {
            array: None,
            scalar: None,
            min: None,
            max: None,
            choice_of: None,
            choices: None,
            element_reference: None,
            element_type: None,
            elements: None,
            constraints: None,
            slicing: None,
            binding: None,
            fixed: None,
            pattern: None,
            refers: None,
            short: None,
            definition: None,
            comment: None,
            requirements: None,
            alias: None,
            example: None,
            extensions: None,
        }
    }

    /// Get the ElementType classification for this element
    pub fn get_element_type(&self) -> Option<ElementType> {
        // Check for choice types first
        if let Some(ref choices) = self.choices {
            return Some(ElementType::Choice(choices.clone()));
        }

        // Check for reference types
        if let Some(ref refers) = self.refers {
            return Some(ElementType::Reference(refers.clone()));
        }

        // Check for complex types with nested elements
        if let Some(ref elements) = self.elements {
            return Some(ElementType::Complex(elements.clone()));
        }

        // Check for simple types
        if let Some(ref type_name) = self.element_type {
            return Some(ElementType::Simple(type_name.clone()));
        }

        None
    }
}

impl Default for Element {
    fn default() -> Self {
        Self::new()
    }
}
