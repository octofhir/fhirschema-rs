//! # FHIRSchema Converter
//!
//! Convert FHIR StructureDefinition to FHIRSchema format.
//!
//! This crate provides functionality to transform FHIR StructureDefinition resources
//! into the more developer-friendly FHIRSchema format.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod converter;
pub mod element_converter;
pub mod constraint_converter;
pub mod slicing_converter;
pub mod reference_resolver;
pub mod error;

pub use converter::StructureDefinitionConverter;
pub use error::{Error, Result};

/// The version of FHIR supported by this converter.
pub const SUPPORTED_FHIR_VERSION: &str = "4.0.1";

#[cfg(test)]
mod tests;

#[cfg(test)]
mod lib_tests {
    use super::*;

    #[test]
    fn test_supported_version() {
        assert!(!SUPPORTED_FHIR_VERSION.is_empty());
    }
}
