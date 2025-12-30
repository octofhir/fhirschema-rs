//! Integration tests for octofhir-fhirschema with real FHIRPath evaluator
//!
//! This crate provides integration testing for FHIRPath constraint validation.
//! It depends on both octofhir-fhirschema and octofhir-fhirpath to test the
//! full validation pipeline without creating cycle dependencies.
//!
//! # Architecture
//!
//! ```text
//! octofhir-fhirschema (main lib)
//! ├─> octofhir-fhir-model (trait FhirPathEvaluator)
//! └─> NO octofhir-fhirpath ❌ (would create cycle)
//!
//! octofhir-fhirschema-tests (this crate)
//! ├─> octofhir-fhirschema ✅
//! └─> octofhir-fhirpath ✅ (safe here, no cycle)
//! ```

pub mod mock_evaluator;

pub use octofhir_fhir_model::FhirPathEvaluator;
/// Re-export for convenience
pub use octofhir_fhirschema::validation::FhirValidator;
