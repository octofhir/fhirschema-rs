//! Validation result types.
//!
//! This module contains types for representing validation results:
//! - [`ValidationContext`] - Context for validation with available schemas
//! - [`ValidationError`] - Individual validation error
//! - [`ValidationResult`] - Overall validation result with errors and warnings

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::schema::FhirSchema;

/// Context for validation containing available schemas.
///
/// The validation context holds all schemas that can be used during validation,
/// indexed by their name or URL for quick lookup.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationContext {
    /// Available schemas indexed by name or URL
    pub schemas: HashMap<String, FhirSchema>,
}

/// A single validation error or warning.
///
/// Contains detailed information about what went wrong during validation,
/// including the location (path), expected vs actual values, and constraint information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Error type code (e.g., "FS1001" for unknown element)
    #[serde(rename = "type", default)]
    pub error_type: String,
    /// Path to the element that failed validation (can contain strings and numbers)
    #[serde(default)]
    pub path: Vec<serde_json::Value>,
    /// Human-readable error message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The actual value that caused the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// The expected value or type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<serde_json::Value>,
    /// The actual value that was found
    #[serde(skip_serializing_if = "Option::is_none")]
    pub got: Option<serde_json::Value>,
    /// Path in the schema that was violated
    #[serde(rename = "schema-path", skip_serializing_if = "Option::is_none")]
    pub schema_path: Option<Vec<serde_json::Value>>,

    // FHIRPath constraint-specific fields
    /// Constraint key (e.g., "dom-1")
    #[serde(rename = "constraint-key", skip_serializing_if = "Option::is_none")]
    pub constraint_key: Option<String>,
    /// FHIRPath expression that failed
    #[serde(
        rename = "constraint-expression",
        skip_serializing_if = "Option::is_none"
    )]
    pub constraint_expression: Option<String>,
    /// Constraint severity (error | warning)
    #[serde(
        rename = "constraint-severity",
        skip_serializing_if = "Option::is_none"
    )]
    pub constraint_severity: Option<String>,
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

/// Result of validating a resource.
///
/// Contains all errors and warnings found during validation,
/// along with a boolean indicating overall validity.
///
/// # Example
/// ```ignore
/// let result = validator.validate(&resource, vec!["Patient".to_string()]).await;
/// if !result.valid {
///     for error in &result.errors {
///         eprintln!("Error: {}", error);
///     }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationResult {
    /// List of validation errors (severity: error)
    #[serde(default)]
    pub errors: Vec<ValidationError>,
    /// Whether the resource is valid (no errors)
    #[serde(default)]
    pub valid: bool,
    /// List of validation warnings (severity: warning)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<ValidationError>,
}

/// Validation error type constants
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
