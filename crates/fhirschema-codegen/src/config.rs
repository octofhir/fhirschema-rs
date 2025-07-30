//! Configuration types for code generation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Target language for code generation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageTarget {
    /// TypeScript interfaces and classes
    TypeScript,
    /// Rust structs and enums
    Rust,
    /// JSON Schema
    JsonSchema,
}

impl std::fmt::Display for LanguageTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LanguageTarget::TypeScript => write!(f, "typescript"),
            LanguageTarget::Rust => write!(f, "rust"),
            LanguageTarget::JsonSchema => write!(f, "json-schema"),
        }
    }
}

impl std::str::FromStr for LanguageTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "typescript" | "ts" => Ok(LanguageTarget::TypeScript),
            "rust" | "rs" => Ok(LanguageTarget::Rust),
            "json-schema" | "jsonschema" => Ok(LanguageTarget::JsonSchema),
            _ => Err(format!("Unknown language target: {}", s)),
        }
    }
}

/// Output configuration for generated files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Base output directory
    pub output_dir: PathBuf,
    /// Whether to create subdirectories for different schemas
    pub create_subdirs: bool,
    /// File naming convention
    pub naming_convention: NamingConvention,
    /// Whether to overwrite existing files
    pub overwrite: bool,
    /// Whether to format generated code
    pub format_code: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("./generated"),
            create_subdirs: true,
            naming_convention: NamingConvention::KebabCase,
            overwrite: false,
            format_code: true,
        }
    }
}

/// Naming convention for generated files and identifiers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NamingConvention {
    /// kebab-case (default)
    KebabCase,
    /// snake_case
    SnakeCase,
    /// camelCase
    CamelCase,
    /// PascalCase
    PascalCase,
}

impl NamingConvention {
    /// Convert a string to the specified naming convention
    pub fn convert(&self, input: &str) -> String {
        match self {
            NamingConvention::KebabCase => to_kebab_case(input),
            NamingConvention::SnakeCase => to_snake_case(input),
            NamingConvention::CamelCase => to_camel_case(input),
            NamingConvention::PascalCase => to_pascal_case(input),
        }
    }
}

/// TypeScript-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeScriptConfig {
    /// Generate interfaces instead of classes
    pub interfaces_only: bool,
    /// Include JSDoc comments
    pub include_jsdoc: bool,
    /// Export style (named, default, namespace)
    pub export_style: ExportStyle,
    /// Whether to generate index files
    pub generate_index: bool,
    /// Custom type mappings
    pub type_mappings: HashMap<String, String>,
}

impl Default for TypeScriptConfig {
    fn default() -> Self {
        Self {
            interfaces_only: false,
            include_jsdoc: true,
            export_style: ExportStyle::Named,
            generate_index: true,
            type_mappings: HashMap::new(),
        }
    }
}

/// Export style for TypeScript generation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportStyle {
    /// Named exports (export { Type })
    Named,
    /// Default exports (export default Type)
    Default,
    /// Namespace exports (export namespace Types)
    Namespace,
}

/// Rust-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustConfig {
    /// Generate serde derives
    pub serde_derives: bool,
    /// Include documentation comments
    pub include_docs: bool,
    /// Custom derive macros
    pub custom_derives: Vec<String>,
    /// Module organization style
    pub module_style: ModuleStyle,
}

impl Default for RustConfig {
    fn default() -> Self {
        Self {
            serde_derives: true,
            include_docs: true,
            custom_derives: vec!["Debug".to_string(), "Clone".to_string()],
            module_style: ModuleStyle::Flat,
        }
    }
}

/// Module organization style for Rust generation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleStyle {
    /// All types in a single module
    Flat,
    /// Separate module per schema
    PerSchema,
    /// Hierarchical modules based on schema structure
    Hierarchical,
}

/// Complete configuration for code generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodegenConfig {
    /// Target language
    pub target: LanguageTarget,
    /// Output configuration
    pub output: OutputConfig,
    /// TypeScript-specific configuration
    pub typescript: TypeScriptConfig,
    /// Rust-specific configuration
    pub rust: RustConfig,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for CodegenConfig {
    fn default() -> Self {
        Self {
            target: LanguageTarget::TypeScript,
            output: OutputConfig::default(),
            typescript: TypeScriptConfig::default(),
            rust: RustConfig::default(),
            metadata: HashMap::new(),
        }
    }
}

impl CodegenConfig {
    /// Create a new configuration for TypeScript generation
    pub fn typescript() -> Self {
        Self {
            target: LanguageTarget::TypeScript,
            ..Default::default()
        }
    }

    /// Create a new configuration for Rust generation
    pub fn rust() -> Self {
        Self {
            target: LanguageTarget::Rust,
            ..Default::default()
        }
    }

    /// Set the output directory
    pub fn with_output_dir(mut self, dir: PathBuf) -> Self {
        self.output.output_dir = dir;
        self
    }

    /// Set whether to overwrite existing files
    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.output.overwrite = overwrite;
        self
    }
}

// Helper functions for naming conventions
fn to_kebab_case(input: &str) -> String {
    input
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() && i > 0 {
                format!("-{}", c.to_lowercase())
            } else {
                c.to_lowercase().to_string()
            }
        })
        .collect()
}

fn to_snake_case(input: &str) -> String {
    input
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() && i > 0 {
                format!("_{}", c.to_lowercase())
            } else {
                c.to_lowercase().to_string()
            }
        })
        .collect()
}

fn to_camel_case(input: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for (i, c) in input.chars().enumerate() {
        if c == '-' || c == '_' || c == ' ' {
            capitalize_next = true;
        } else if i == 0 {
            result.push(c.to_lowercase().next().unwrap());
        } else if capitalize_next {
            result.push(c.to_uppercase().next().unwrap());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }

    result
}

fn to_pascal_case(input: &str) -> String {
    let camel = to_camel_case(input);
    if let Some(first_char) = camel.chars().next() {
        format!("{}{}", first_char.to_uppercase(), &camel[1..])
    } else {
        camel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_target_from_str() {
        assert_eq!("typescript".parse::<LanguageTarget>().unwrap(), LanguageTarget::TypeScript);
        assert_eq!("ts".parse::<LanguageTarget>().unwrap(), LanguageTarget::TypeScript);
        assert_eq!("rust".parse::<LanguageTarget>().unwrap(), LanguageTarget::Rust);
        assert!("invalid".parse::<LanguageTarget>().is_err());
    }

    #[test]
    fn test_naming_conventions() {
        assert_eq!(NamingConvention::KebabCase.convert("TestCase"), "test-case");
        assert_eq!(NamingConvention::SnakeCase.convert("TestCase"), "test_case");
        assert_eq!(NamingConvention::CamelCase.convert("test-case"), "testCase");
        assert_eq!(NamingConvention::PascalCase.convert("test-case"), "TestCase");
    }

    #[test]
    fn test_config_builders() {
        let ts_config = CodegenConfig::typescript();
        assert_eq!(ts_config.target, LanguageTarget::TypeScript);

        let rust_config = CodegenConfig::rust();
        assert_eq!(rust_config.target, LanguageTarget::Rust);
    }
}
