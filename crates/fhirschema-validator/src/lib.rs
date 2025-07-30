//! FHIRSchema Validation Engine
//!
//! This crate provides comprehensive validation capabilities for FHIR resources
//! against FHIRSchema definitions, including schemata resolution, constraint
//! evaluation using FHIRPath, and primitive datatype validation.

pub mod error;
pub mod validator;
pub mod schemata;
pub mod element;
pub mod primitive;
pub mod constraint;
pub mod slicing;
pub mod context;

// Re-export main types for convenience
pub use validator::Validator;
pub use schemata::SchemataResolver;
pub use element::ElementValidator;
pub use primitive::PrimitiveValidator;
pub use constraint::ConstraintEvaluator;
pub use slicing::SlicingValidator;
pub use context::FHIRPathContext;
pub use error::{ValidationError, ValidationResult};

/// Validation configuration options
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Enable constraint evaluation (default: true)
    pub enable_constraints: bool,
    /// Enable slicing validation (default: true)
    pub enable_slicing: bool,
    /// Enable primitive validation (default: true)
    pub enable_primitives: bool,
    /// Maximum recursion depth for schema resolution (default: 100)
    pub max_recursion_depth: usize,
    /// Enable performance optimizations (default: true)
    pub enable_optimizations: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            enable_constraints: true,
            enable_slicing: true,
            enable_primitives: true,
            max_recursion_depth: 100,
            enable_optimizations: true,
        }
    }
}

/// Validation severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational messages
    Information,
    /// Warning messages
    Warning,
    /// Error messages (validation failures)
    Error,
}

/// Validation outcome with detailed results
#[derive(Debug, Clone)]
pub struct ValidationOutcome {
    /// Overall validation success
    pub success: bool,
    /// List of validation issues
    pub issues: Vec<ValidationIssue>,
    /// Validation statistics
    pub stats: ValidationStats,
}

/// Individual validation issue
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Issue severity
    pub severity: Severity,
    /// Issue code/identifier
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Location in the resource (FHIRPath expression)
    pub location: String,
    /// Additional context information
    pub context: Option<String>,
}

/// Validation statistics
#[derive(Debug, Clone, Default)]
pub struct ValidationStats {
    /// Number of elements validated
    pub elements_validated: usize,
    /// Number of constraints evaluated
    pub constraints_evaluated: usize,
    /// Number of primitives validated
    pub primitives_validated: usize,
    /// Validation duration in milliseconds
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_config_default() {
        let config = ValidationConfig::default();
        assert!(config.enable_constraints);
        assert!(config.enable_slicing);
        assert!(config.enable_primitives);
        assert_eq!(config.max_recursion_depth, 100);
        assert!(config.enable_optimizations);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Information < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }
}
