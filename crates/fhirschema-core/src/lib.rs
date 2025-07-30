//! # FHIRSchema Core
//!
//! Core data structures and types for FHIRSchema.
//!
//! This crate provides the fundamental data structures for representing FHIRSchema
//! definitions, including Schema, Element, Constraint, and related types.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod schema;
pub mod element;
pub mod constraint;
pub mod slicing;
pub mod binding;
pub mod error;

pub use schema::Schema;
pub use element::{Element, ElementType};
pub use constraint::Constraint;
pub use slicing::{Slicing, Slice, Discriminator};
pub use binding::Binding;
pub use error::{Error, Result};

/// The current version of the FHIRSchema specification supported by this crate.
pub const FHIRSCHEMA_VERSION: &str = "1.0.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!FHIRSCHEMA_VERSION.is_empty());
    }
}
