//! Core code generation traits and types

use crate::{CodegenError, CodegenResult, CodegenConfig};
use fhirschema_core::Schema;
use std::collections::HashMap;
use std::path::PathBuf;

/// A generated file with its path and content
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    /// The relative path where the file should be written
    pub path: PathBuf,
    /// The generated content of the file
    pub content: String,
    /// Whether the file should be formatted after generation
    pub format: bool,
}

impl GeneratedFile {
    /// Create a new generated file
    pub fn new(path: PathBuf, content: String) -> Self {
        Self {
            path,
            content,
            format: true,
        }
    }

    /// Create a new generated file without formatting
    pub fn new_unformatted(path: PathBuf, content: String) -> Self {
        Self {
            path,
            content,
            format: false,
        }
    }
}

/// Context information for code generation
#[derive(Debug, Clone)]
pub struct GenerationContext {
    /// The schemas being processed
    pub schemas: Vec<Schema>,
    /// Configuration for code generation
    pub config: CodegenConfig,
    /// Additional metadata for generation
    pub metadata: HashMap<String, serde_json::Value>,
}

impl GenerationContext {
    /// Create a new generation context
    pub fn new(schemas: Vec<Schema>, config: CodegenConfig) -> Self {
        Self {
            schemas,
            config,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the context
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Get metadata from the context
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }
}

/// Trait for code generators that can produce code from FHIRSchema
pub trait CodeGenerator {
    /// Generate code from the given schemas and context
    fn generate(&self, context: &GenerationContext) -> CodegenResult<Vec<GeneratedFile>>;

    /// Get the name of this generator
    fn name(&self) -> &str;

    /// Get the file extensions this generator produces
    fn file_extensions(&self) -> Vec<&str>;

    /// Validate that the generator can handle the given schemas
    fn validate_schemas(&self, schemas: &[Schema]) -> CodegenResult<()> {
        // Default implementation - generators can override for specific validation
        if schemas.is_empty() {
            return Err(CodegenError::invalid_input("No schemas provided"));
        }
        Ok(())
    }

    /// Pre-process schemas before generation (optional hook)
    fn preprocess_schemas(&self, _schemas: &mut [Schema]) -> CodegenResult<()> {
        // Default implementation does nothing
        Ok(())
    }

    /// Post-process generated files (optional hook)
    fn postprocess_files(&self, _files: &mut [GeneratedFile]) -> CodegenResult<()> {
        // Default implementation does nothing
        Ok(())
    }
}

/// Registry for managing multiple code generators
#[derive(Default)]
pub struct GeneratorRegistry {
    generators: HashMap<String, Box<dyn CodeGenerator>>,
}

impl GeneratorRegistry {
    /// Create a new generator registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a code generator
    pub fn register(&mut self, generator: Box<dyn CodeGenerator>) {
        let name = generator.name().to_string();
        self.generators.insert(name, generator);
    }

    /// Get a generator by name
    pub fn get(&self, name: &str) -> Option<&dyn CodeGenerator> {
        self.generators.get(name).map(|g| g.as_ref())
    }

    /// List all registered generator names
    pub fn list_generators(&self) -> Vec<&str> {
        self.generators.keys().map(|s| s.as_str()).collect()
    }

    /// Generate code using a specific generator
    pub fn generate(&self, generator_name: &str, context: &GenerationContext) -> CodegenResult<Vec<GeneratedFile>> {
        let generator = self.get(generator_name)
            .ok_or_else(|| CodegenError::invalid_input(format!("Unknown generator: {}", generator_name)))?;

        generator.generate(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    struct TestGenerator;

    impl CodeGenerator for TestGenerator {
        fn generate(&self, _context: &GenerationContext) -> CodegenResult<Vec<GeneratedFile>> {
            Ok(vec![GeneratedFile::new(
                PathBuf::from("test.txt"),
                "test content".to_string(),
            )])
        }

        fn name(&self) -> &str {
            "test"
        }

        fn file_extensions(&self) -> Vec<&str> {
            vec!["txt"]
        }
    }

    #[test]
    fn test_generated_file_creation() {
        let file = GeneratedFile::new(PathBuf::from("test.txt"), "content".to_string());
        assert_eq!(file.path, Path::new("test.txt"));
        assert_eq!(file.content, "content");
        assert!(file.format);
    }

    #[test]
    fn test_generator_registry() {
        let mut registry = GeneratorRegistry::new();
        registry.register(Box::new(TestGenerator));

        assert!(registry.get("test").is_some());
        assert!(registry.get("nonexistent").is_none());
        assert_eq!(registry.list_generators(), vec!["test"]);
    }
}
