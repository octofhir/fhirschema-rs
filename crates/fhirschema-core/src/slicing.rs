//! FHIRSchema Slicing definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::Element;

/// A FHIRSchema Slicing definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slicing {
    /// Discriminator definitions for slicing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Vec<Discriminator>>,

    /// Whether slices are ordered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordered: Option<bool>,

    /// Slicing rules (closed, open, openAtEnd)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<String>,

    /// Description of the slicing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Individual slices
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slices: Option<HashMap<String, Slice>>,
}

/// A discriminator for slicing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Discriminator {
    /// Type of discriminator (value, exists, pattern, type, profile)
    #[serde(rename = "type")]
    pub discriminator_type: String,

    /// Path for the discriminator
    pub path: String,
}

/// A FHIRSchema Slice definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slice {
    /// Slice name
    pub name: String,

    /// Slice matching criteria
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_criteria: Option<String>,

    /// Element definition for this slice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<Element>,

    /// Minimum cardinality for this slice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<u32>,

    /// Maximum cardinality for this slice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,

    /// Short description of the slice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,

    /// Definition of the slice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
}

impl Slicing {
    /// Create a new Slicing.
    pub fn new() -> Self {
        Self {
            discriminator: None,
            ordered: None,
            rules: None,
            description: None,
            slices: None,
        }
    }
}

impl Default for Slicing {
    fn default() -> Self {
        Self::new()
    }
}

impl Discriminator {
    /// Create a new Discriminator.
    pub fn new(discriminator_type: String, path: String) -> Self {
        Self {
            discriminator_type,
            path,
        }
    }
}

impl Slice {
    /// Create a new Slice.
    pub fn new(name: String) -> Self {
        Self {
            name,
            match_criteria: None,
            element: None,
            min: None,
            max: None,
            short: None,
            definition: None,
        }
    }
}
