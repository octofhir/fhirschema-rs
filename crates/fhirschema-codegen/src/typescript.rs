//! TypeScript code generation from FHIRSchema

use crate::{
    CodeGenerator, CodegenError, CodegenResult, GeneratedFile, GenerationContext,
    LanguageTarget,
};
use crate::config::ExportStyle;
use fhirschema_core::{Schema, Element};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// TypeScript code generator
pub struct TypeScriptGenerator {
    name: String,
}

impl TypeScriptGenerator {
    /// Create a new TypeScript generator
    pub fn new() -> Self {
        Self {
            name: "typescript".to_string(),
        }
    }

    /// Generate TypeScript interface from FHIRSchema
    fn generate_interface(&self, schema: &Schema, context: &GenerationContext) -> CodegenResult<String> {
        let mut output = String::new();
        let mut dependencies = HashSet::new();

        // Add file header
        output.push_str("// Generated from FHIRSchema - do not edit manually\n\n");

        // Collect dependencies from elements
        if let Some(elements) = &schema.elements {
            for (_path, element) in elements {
                self.collect_dependencies(element, &mut dependencies);
            }
        }

        // Generate imports
        if !dependencies.is_empty() {
            self.generate_imports(&dependencies, &mut output);
            output.push('\n');
        }

        // Generate interface
        let interface_name = &schema.name;

        // Add JSDoc if enabled (Schema doesn't have description field)
        if context.config.typescript.include_jsdoc {
            output.push_str(&format!("/**\n * {}\n */\n", interface_name));
        }

        // Interface declaration
        output.push_str(&format!("export interface {} {{\n", interface_name));

        // Generate properties from elements
        if let Some(elements) = &schema.elements {
            for (path, element) in elements {
                let property = self.generate_property(path, element, context)?;
                output.push_str(&format!("  {}\n", property));
            }
        }

        output.push_str("}\n");

        Ok(output)
    }

    /// Generate TypeScript class from FHIRSchema
    fn generate_class(&self, schema: &Schema, context: &GenerationContext) -> CodegenResult<String> {
        let mut output = String::new();
        let mut dependencies = HashSet::new();

        // Add file header
        output.push_str("// Generated from FHIRSchema - do not edit manually\n\n");

        // Collect dependencies from elements
        if let Some(elements) = &schema.elements {
            for (_path, element) in elements {
                self.collect_dependencies(element, &mut dependencies);
            }
        }

        // Generate imports
        if !dependencies.is_empty() {
            self.generate_imports(&dependencies, &mut output);
            output.push('\n');
        }

        let class_name = &schema.name;

        // Class declaration
        output.push_str(&format!("export class {} {{\n", class_name));

        // Generate properties from elements
        if let Some(elements) = &schema.elements {
            for (path, element) in elements {
                let property = self.generate_class_property(path, element, context)?;
                output.push_str(&format!("  {}\n", property));
            }
        }

        // Generate constructor
        output.push_str("\n  constructor(data?: Partial<");
        output.push_str(class_name);
        output.push_str(">) {\n");
        output.push_str("    if (data) {\n");
        output.push_str("      Object.assign(this, data);\n");
        output.push_str("    }\n");
        output.push_str("  }\n");

        output.push_str("}\n");

        Ok(output)
    }

    /// Generate a TypeScript property from an element definition
    fn generate_property(&self, path: &str, element: &Element, _context: &GenerationContext) -> CodegenResult<String> {
        let property_name = self.get_property_name(path);
        let type_annotation = self.get_type_annotation(element)?;
        let optional = self.is_optional(element);

        let optional_marker = if optional { "?" } else { "" };

        Ok(format!("{}{}: {};", property_name, optional_marker, type_annotation))
    }

    /// Generate a TypeScript class property from an element definition
    fn generate_class_property(&self, path: &str, element: &Element, context: &GenerationContext) -> CodegenResult<String> {
        let property_name = self.get_property_name(path);
        let type_annotation = self.get_type_annotation(element)?;
        let optional = self.is_optional(element);

        let optional_marker = if optional { "?" } else { "" };

        // Add JSDoc for property if enabled
        let mut output = String::new();
        if context.config.typescript.include_jsdoc {
            if let Some(short) = &element.short {
                output.push_str(&format!("  /**\n   * {}\n   */\n", short));
            }
        }

        output.push_str(&format!("  {}{}: {};", property_name, optional_marker, type_annotation));

        Ok(output)
    }

    /// Get property name from element path
    fn get_property_name(&self, path: &str) -> String {
        // Extract the last part of the path as property name
        path.split('.').last().unwrap_or(path).to_string()
    }

    /// Get TypeScript type annotation for an element
    fn get_type_annotation(&self, element: &Element) -> CodegenResult<String> {
        // Map FHIR types to TypeScript types
        let base_type = match element.element_type.as_deref() {
            Some("string") => "string",
            Some("boolean") => "boolean",
            Some("integer") => "number",
            Some("decimal") => "number",
            Some("date") => "string", // ISO date string
            Some("dateTime") => "string", // ISO datetime string
            Some("time") => "string", // ISO time string
            Some("code") => "string",
            Some("uri") => "string",
            Some("url") => "string",
            Some("canonical") => "string",
            Some("oid") => "string",
            Some("uuid") => "string",
            Some("id") => "string",
            Some("markdown") => "string",
            Some("base64Binary") => "string",
            Some("instant") => "string",
            Some("positiveInt") => "number",
            Some("unsignedInt") => "number",
            Some(custom_type) => custom_type, // Custom FHIR types
            None => "any", // Fallback
        };

        // Handle arrays
        let type_str = if self.is_array(element) {
            format!("{}[]", base_type)
        } else {
            base_type.to_string()
        };

        Ok(type_str)
    }

    /// Check if element is optional
    fn is_optional(&self, element: &Element) -> bool {
        element.min.unwrap_or(0) == 0
    }

    /// Check if element is an array
    fn is_array(&self, element: &Element) -> bool {
        element.max.as_ref()
            .map(|max| max != "1")
            .unwrap_or(false)
    }

    /// Collect type dependencies from an element
    fn collect_dependencies(&self, element: &Element, dependencies: &mut HashSet<String>) {
        if let Some(element_type) = &element.element_type {
            // Check if this is a custom FHIR type (starts with uppercase)
            if element_type.chars().next().map_or(false, |c| c.is_uppercase()) {
                // Skip primitive types that don't need imports
                match element_type.as_str() {
                    "string" | "boolean" | "integer" | "decimal" | "date" | "dateTime" |
                    "time" | "code" | "uri" | "url" | "canonical" | "oid" | "uuid" |
                    "id" | "markdown" | "base64Binary" | "instant" | "positiveInt" |
                    "unsignedInt" => {
                        // These are primitive types, don't add to dependencies
                    }
                    _ => {
                        // This is a custom type that needs to be imported
                        dependencies.insert(element_type.clone());
                    }
                }
            }
        }
    }

    /// Generate import statements for dependencies
    fn generate_imports(&self, dependencies: &HashSet<String>, output: &mut String) {
        if dependencies.is_empty() {
            return;
        }

        // Sort dependencies for consistent output
        let mut sorted_deps: Vec<_> = dependencies.iter().collect();
        sorted_deps.sort();

        for dep in sorted_deps {
            let filename = dep.to_lowercase().replace(' ', "-");
            output.push_str(&format!("import {{ {} }} from './{}';\n", dep, filename));
        }
    }

    /// Generate index file for multiple schemas
    fn generate_index(&self, schemas: &[Schema], _context: &GenerationContext) -> CodegenResult<String> {
        let mut output = String::new();

        output.push_str("// Generated index file - do not edit manually\n\n");

        for schema in schemas {
            let name = &schema.name;
            let filename = name.to_lowercase().replace(' ', "-");
            output.push_str(&format!("export * from './{}';\n", filename));
        }

        Ok(output)
    }
}

impl Default for TypeScriptGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeGenerator for TypeScriptGenerator {
    fn generate(&self, context: &GenerationContext) -> CodegenResult<Vec<GeneratedFile>> {
        // Validate that we're generating TypeScript
        if context.config.target != LanguageTarget::TypeScript {
            return Err(CodegenError::config_error("TypeScript generator requires TypeScript target"));
        }

        let mut files = Vec::new();

        // Generate files for each schema
        for schema in &context.schemas {
            let schema_name = &schema.name;

            let filename = format!("{}.ts", schema_name.to_lowercase().replace(' ', "-"));
            let file_path = if context.config.output.create_subdirs {
                context.config.output.output_dir.join("types").join(&filename)
            } else {
                context.config.output.output_dir.join(&filename)
            };

            let content = if context.config.typescript.interfaces_only {
                self.generate_interface(schema, context)?
            } else {
                self.generate_class(schema, context)?
            };

            files.push(GeneratedFile::new(file_path, content));
        }

        // Generate index file if requested
        if context.config.typescript.generate_index && !context.schemas.is_empty() {
            let index_path = if context.config.output.create_subdirs {
                context.config.output.output_dir.join("types").join("index.ts")
            } else {
                context.config.output.output_dir.join("index.ts")
            };

            let index_content = self.generate_index(&context.schemas, context)?;
            files.push(GeneratedFile::new(index_path, index_content));
        }

        Ok(files)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn file_extensions(&self) -> Vec<&str> {
        vec!["ts"]
    }

    fn validate_schemas(&self, schemas: &[Schema]) -> CodegenResult<()> {
        if schemas.is_empty() {
            return Err(CodegenError::invalid_input("No schemas provided"));
        }

        // Validate that all schemas have names (names are required fields, so this is always satisfied)
        for schema in schemas {
            if schema.name.is_empty() {
                return Err(CodegenError::schema_error("All schemas must have non-empty names for TypeScript generation"));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CodegenConfig;

    fn create_test_schema() -> Schema {
        use std::collections::HashMap;

        let mut elements = HashMap::new();
        elements.insert("Patient.id".to_string(), Element {
            element_type: Some("id".to_string()),
            min: Some(0),
            max: Some("1".to_string()),
            short: Some("Logical id of this artifact".to_string()),
            ..Default::default()
        });
        elements.insert("Patient.name".to_string(), Element {
            element_type: Some("HumanName".to_string()),
            min: Some(0),
            max: Some("*".to_string()),
            short: Some("A name associated with the patient".to_string()),
            ..Default::default()
        });

        Schema {
            url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            schema_type: "resource".to_string(),
            name: "Patient".to_string(),
            derivation: "specialization".to_string(),
            base: Some("http://hl7.org/fhir/StructureDefinition/DomainResource".to_string()),
            elements: Some(elements),
            constraints: None,
            extensions: None,
            additional_properties: None,
            any: None,
        }
    }

    #[test]
    fn test_typescript_generator_creation() {
        let generator = TypeScriptGenerator::new();
        assert_eq!(generator.name(), "typescript");
        assert_eq!(generator.file_extensions(), vec!["ts"]);
    }

    #[test]
    fn test_interface_generation() {
        let generator = TypeScriptGenerator::new();
        let schema = create_test_schema();
        let mut config = CodegenConfig::typescript();
        config.typescript.interfaces_only = true; // Ensure we generate interfaces
        let context = GenerationContext::new(vec![schema], config);

        let result = generator.generate(&context);
        assert!(result.is_ok());

        let files = result.unwrap();
        assert!(!files.is_empty());
        assert!(files[0].content.contains("export interface Patient"));
        // Should also contain import for HumanName type
        assert!(files[0].content.contains("import { HumanName } from './humanname';"));
    }

    #[test]
    fn test_class_generation() {
        let generator = TypeScriptGenerator::new();
        let schema = create_test_schema();
        let mut config = CodegenConfig::typescript();
        config.typescript.interfaces_only = false;
        let context = GenerationContext::new(vec![schema], config);

        let result = generator.generate(&context);
        assert!(result.is_ok());

        let files = result.unwrap();
        assert!(!files.is_empty());
        assert!(files[0].content.contains("export class Patient"));
        assert!(files[0].content.contains("constructor"));
    }
}
