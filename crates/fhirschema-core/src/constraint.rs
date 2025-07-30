//! FHIRSchema Constraint definition.

use serde::{Deserialize, Serialize};

/// A FHIRSchema Constraint represents a FHIRPath constraint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    /// Constraint key/identifier
    pub key: String,

    /// FHIRPath expression
    pub expression: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub human: Option<String>,

    /// Constraint severity (error, warning, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
}

impl Constraint {
    /// Create a new Constraint.
    pub fn new(key: String, expression: String) -> Self {
        Self {
            key,
            expression,
            human: None,
            severity: None,
        }
    }
}
