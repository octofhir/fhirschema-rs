//! # FHIRSchema Code Generation
//!
//! This crate provides code generation capabilities from FHIRSchema definitions,
//! supporting multiple target languages including TypeScript and Rust.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod error;
pub mod generator;
pub mod typescript;
pub mod template;
pub mod config;

pub use error::{CodegenError, CodegenResult};
pub use generator::{CodeGenerator, GenerationContext, GeneratedFile};
pub use config::{CodegenConfig, LanguageTarget, OutputConfig};

/// The version of this code generator
pub const CODEGEN_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        assert!(!CODEGEN_VERSION.is_empty());
    }
}
