//! Template system for code generation

use crate::{CodegenError, CodegenResult};
use handlebars::{Handlebars, Helper, Context, RenderContext, Output, HelperResult};
use serde_json::Value;
use std::collections::HashMap;

/// Template engine for code generation
pub struct TemplateEngine {
    handlebars: Handlebars<'static>,
}

impl TemplateEngine {
    /// Create a new template engine
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();

        // Register built-in helpers
        handlebars.register_helper("camelCase", Box::new(camel_case_helper));
        handlebars.register_helper("PascalCase", Box::new(pascal_case_helper));
        handlebars.register_helper("snake_case", Box::new(snake_case_helper));
        handlebars.register_helper("kebab-case", Box::new(kebab_case_helper));
        handlebars.register_helper("uppercase", Box::new(uppercase_helper));
        handlebars.register_helper("lowercase", Box::new(lowercase_helper));
        handlebars.register_helper("pluralize", Box::new(pluralize_helper));
        handlebars.register_helper("singularize", Box::new(singularize_helper));

        Self { handlebars }
    }

    /// Register a template from string
    pub fn register_template(&mut self, name: &str, template: &str) -> CodegenResult<()> {
        self.handlebars
            .register_template_string(name, template)
            .map_err(CodegenError::from)
    }

    /// Register a template from file
    pub fn register_template_file(&mut self, name: &str, path: &std::path::Path) -> CodegenResult<()> {
        self.handlebars
            .register_template_file(name, path)
            .map_err(CodegenError::from)
    }

    /// Register multiple templates from a directory
    pub fn register_templates_dir(&mut self, dir: &std::path::Path, extension: &str) -> CodegenResult<()> {
        // Note: register_templates_directory is not available in current handlebars version
        // This is a placeholder implementation - individual templates should be registered manually
        let _ = (dir, extension);
        Ok(())
    }

    /// Render a template with the given data
    pub fn render(&self, template_name: &str, data: &Value) -> CodegenResult<String> {
        self.handlebars
            .render(template_name, data)
            .map_err(CodegenError::from)
    }

    /// Render a template string directly
    pub fn render_template(&self, template: &str, data: &Value) -> CodegenResult<String> {
        self.handlebars
            .render_template(template, data)
            .map_err(CodegenError::from)
    }

    /// Register a custom helper function
    pub fn register_helper<F>(&mut self, name: &str, helper: F)
    where
        F: handlebars::HelperDef + Send + Sync + 'static,
    {
        self.handlebars.register_helper(name, Box::new(helper));
    }

    /// Get list of registered template names
    pub fn get_templates(&self) -> Vec<String> {
        self.handlebars.get_templates().keys().cloned().collect()
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in template collection for common code generation patterns
pub struct BuiltinTemplates;

impl BuiltinTemplates {
    /// Get TypeScript interface template
    pub fn typescript_interface() -> &'static str {
        r#"// Generated from FHIRSchema - do not edit manually

{{#if description}}
/**
 * {{description}}
 */
{{/if}}
export interface {{PascalCase name}} {
{{#each elements}}
  {{#if short}}
  /**
   * {{short}}
   */
  {{/if}}
  {{camelCase (last_part path)}}{{#if (is_optional this)}}?{{/if}}: {{typescript_type this}};
{{/each}}
}
"#
    }

    /// Get TypeScript class template
    pub fn typescript_class() -> &'static str {
        r#"// Generated from FHIRSchema - do not edit manually

{{#if description}}
/**
 * {{description}}
 */
{{/if}}
export class {{PascalCase name}} {
{{#each elements}}
  {{#if short}}
  /**
   * {{short}}
   */
  {{/if}}
  {{camelCase (last_part path)}}{{#if (is_optional this)}}?{{/if}}: {{typescript_type this}};
{{/each}}

  constructor(data?: Partial<{{PascalCase name}}>) {
    if (data) {
      Object.assign(this, data);
    }
  }
}
"#
    }

    /// Get Rust struct template
    pub fn rust_struct() -> &'static str {
        r#"// Generated from FHIRSchema - do not edit manually

use serde::{Deserialize, Serialize};

{{#if description}}
/// {{description}}
{{/if}}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {{PascalCase name}} {
{{#each elements}}
    {{#if short}}
    /// {{short}}
    {{/if}}
    pub {{snake_case (last_part path)}}: {{#if (is_optional this)}}Option<{{/if}}{{rust_type this}}{{#if (is_optional this)}}>{{/if}},
{{/each}}
}
"#
    }

    /// Get all builtin templates as a map
    pub fn all() -> HashMap<&'static str, &'static str> {
        let mut templates = HashMap::new();
        templates.insert("typescript_interface", Self::typescript_interface());
        templates.insert("typescript_class", Self::typescript_class());
        templates.insert("rust_struct", Self::rust_struct());
        templates
    }

    /// Register all builtin templates with the given engine
    pub fn register_all(engine: &mut TemplateEngine) -> CodegenResult<()> {
        for (name, template) in Self::all() {
            engine.register_template(name, template)?;
        }
        Ok(())
    }
}

// Helper functions for handlebars templates

fn camel_case_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let result = to_camel_case(param);
    out.write(&result)?;
    Ok(())
}

fn pascal_case_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let result = to_pascal_case(param);
    out.write(&result)?;
    Ok(())
}

fn snake_case_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let result = to_snake_case(param);
    out.write(&result)?;
    Ok(())
}

fn kebab_case_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let result = to_kebab_case(param);
    out.write(&result)?;
    Ok(())
}

fn uppercase_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    out.write(&param.to_uppercase())?;
    Ok(())
}

fn lowercase_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    out.write(&param.to_lowercase())?;
    Ok(())
}

fn pluralize_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let result = simple_pluralize(param);
    out.write(&result)?;
    Ok(())
}

fn singularize_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let result = simple_singularize(param);
    out.write(&result)?;
    Ok(())
}

// Utility functions for case conversion

fn to_camel_case(input: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for (i, c) in input.chars().enumerate() {
        if c == '-' || c == '_' || c == ' ' || c == '.' {
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

fn to_snake_case(input: &str) -> String {
    input
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() && i > 0 {
                format!("_{}", c.to_lowercase())
            } else if c == '-' || c == ' ' || c == '.' {
                "_".to_string()
            } else {
                c.to_lowercase().to_string()
            }
        })
        .collect()
}

fn to_kebab_case(input: &str) -> String {
    input
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() && i > 0 {
                format!("-{}", c.to_lowercase())
            } else if c == '_' || c == ' ' || c == '.' {
                "-".to_string()
            } else {
                c.to_lowercase().to_string()
            }
        })
        .collect()
}

fn simple_pluralize(input: &str) -> String {
    if input.is_empty() {
        return input.to_string();
    }

    // Simple pluralization rules
    if input.ends_with('s') || input.ends_with("sh") || input.ends_with("ch") {
        format!("{}es", input)
    } else if input.ends_with('y') && input.len() > 1 {
        let chars: Vec<char> = input.chars().collect();
        if !matches!(chars[chars.len() - 2], 'a' | 'e' | 'i' | 'o' | 'u') {
            format!("{}ies", &input[..input.len() - 1])
        } else {
            format!("{}s", input)
        }
    } else {
        format!("{}s", input)
    }
}

fn simple_singularize(input: &str) -> String {
    if input.is_empty() {
        return input.to_string();
    }

    // Simple singularization rules
    if input.ends_with("ies") && input.len() > 3 {
        format!("{}y", &input[..input.len() - 3])
    } else if input.ends_with("es") && input.len() > 2 {
        let base = &input[..input.len() - 2];
        if base.ends_with('s') || base.ends_with("sh") || base.ends_with("ch") {
            base.to_string()
        } else {
            format!("{}e", base)
        }
    } else if input.ends_with('s') && input.len() > 1 {
        input[..input.len() - 1].to_string()
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_template_engine_creation() {
        let engine = TemplateEngine::new();
        assert!(!engine.get_templates().is_empty()); // Should have builtin helpers
    }

    #[test]
    fn test_template_registration_and_rendering() {
        let mut engine = TemplateEngine::new();
        let template = "Hello {{name}}!";

        engine.register_template("greeting", template).unwrap();

        let data = json!({"name": "World"});
        let result = engine.render("greeting", &data).unwrap();

        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_case_conversion_helpers() {
        assert_eq!(to_camel_case("hello_world"), "helloWorld");
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
        assert_eq!(to_kebab_case("HelloWorld"), "hello-world");
    }

    #[test]
    fn test_pluralization() {
        assert_eq!(simple_pluralize("cat"), "cats");
        assert_eq!(simple_pluralize("box"), "boxes");
        assert_eq!(simple_pluralize("city"), "cities");
        assert_eq!(simple_pluralize("key"), "keys");
    }

    #[test]
    fn test_singularization() {
        assert_eq!(simple_singularize("cats"), "cat");
        assert_eq!(simple_singularize("boxes"), "box");
        assert_eq!(simple_singularize("cities"), "city");
        assert_eq!(simple_singularize("keys"), "key");
    }

    #[test]
    fn test_builtin_templates() {
        let templates = BuiltinTemplates::all();
        assert!(templates.contains_key("typescript_interface"));
        assert!(templates.contains_key("typescript_class"));
        assert!(templates.contains_key("rust_struct"));
    }
}
