#![allow(clippy::uninlined_format_args)]
#![allow(clippy::only_used_in_recursion)]

pub mod fhirpath_validation_engine;
pub mod field_validator;

use crate::{FhirSchema, Result};
use serde_json::Value;
use std::collections::HashMap;

pub use fhirpath_validation_engine::{ConstraintValidationStats, FhirPathValidationEngine};
pub use field_validator::{
    FhirSchemaFieldValidator, FieldInfo, FieldValidationContext, FieldValidationResult,
};

pub trait SchemaValidator {
    fn validate_schema(&self, schema: &FhirSchema) -> Result<Vec<ValidationIssue>>;
    fn validate_element_paths(&self, schema: &FhirSchema) -> Result<Vec<ValidationIssue>>;
    fn validate_constraints(&self, schema: &FhirSchema) -> Result<Vec<ValidationIssue>>;
}

/// Core validation engine trait for validating FHIR resources against FHIRSchema definitions
pub trait ValidationEngine {
    /// Validate a FHIR resource against a schema
    fn validate_resource(&self, resource: &Value, schema: &FhirSchema) -> Result<ValidationResult>;

    /// Validate a FHIR resource against multiple schemas
    fn validate_resource_with_schemas(
        &self,
        resource: &Value,
        schemas: &[&FhirSchema],
    ) -> Result<ValidationResult>;
}

/// Validation context that maintains state during validation
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// Current path in the resource being validated
    pub current_path: String,

    /// Stack of paths for nested validation
    pub path_stack: Vec<String>,

    /// Collection of validation issues found
    pub issues: Vec<ValidationIssue>,

    /// Context variables for FHIRPath evaluation
    pub variables: HashMap<String, Value>,

    /// Current resource being validated
    pub resource: Value,

    /// Schemas available for validation
    pub schemas: HashMap<String, FhirSchema>,
}

/// Result of validation containing all issues found
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// All validation issues found
    pub issues: Vec<ValidationIssue>,

    /// Whether validation passed (no errors)
    pub is_valid: bool,

    /// Summary statistics
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
}

impl ValidationContext {
    /// Create a new validation context for a resource
    pub fn new(resource: Value) -> Self {
        Self {
            current_path: String::new(),
            path_stack: Vec::new(),
            issues: Vec::new(),
            variables: HashMap::new(),
            resource,
            schemas: HashMap::new(),
        }
    }

    /// Push a path segment onto the path stack
    pub fn push_path(&mut self, segment: &str) {
        self.path_stack.push(self.current_path.clone());
        if self.current_path.is_empty() {
            self.current_path = segment.to_string();
        } else {
            self.current_path = format!("{}.{}", self.current_path, segment);
        }
    }

    /// Pop the last path segment from the path stack
    pub fn pop_path(&mut self) {
        if let Some(previous_path) = self.path_stack.pop() {
            self.current_path = previous_path;
        }
    }

    /// Add a validation issue to the context
    pub fn add_issue(&mut self, mut issue: ValidationIssue) {
        if issue.path.is_none() && !self.current_path.is_empty() {
            issue.path = Some(self.current_path.clone());
        }
        self.issues.push(issue);
    }

    /// Add an error to the validation context
    pub fn add_error(&mut self, code: impl Into<String>, message: impl Into<String>) {
        self.add_issue(ValidationIssue::error(code, message));
    }

    /// Add a warning to the validation context
    pub fn add_warning(&mut self, code: impl Into<String>, message: impl Into<String>) {
        self.add_issue(ValidationIssue::warning(code, message));
    }

    /// Set a context variable for FHIRPath evaluation
    pub fn set_variable(&mut self, name: impl Into<String>, value: Value) {
        self.variables.insert(name.into(), value);
    }

    /// Get a context variable
    pub fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }

    /// Add a schema to the context
    pub fn add_schema(&mut self, name: impl Into<String>, schema: FhirSchema) {
        self.schemas.insert(name.into(), schema);
    }

    /// Get a schema from the context
    pub fn get_schema(&self, name: &str) -> Option<&FhirSchema> {
        self.schemas.get(name)
    }

    /// Convert the context into a validation result
    pub fn into_result(self) -> ValidationResult {
        ValidationResult::from_issues(self.issues)
    }
}

impl ValidationResult {
    /// Create a new validation result from a list of issues
    pub fn from_issues(issues: Vec<ValidationIssue>) -> Self {
        let error_count = issues
            .iter()
            .filter(|i| matches!(i.severity, ValidationSeverity::Error))
            .count();
        let warning_count = issues
            .iter()
            .filter(|i| matches!(i.severity, ValidationSeverity::Warning))
            .count();
        let info_count = issues
            .iter()
            .filter(|i| matches!(i.severity, ValidationSeverity::Information))
            .count();

        Self {
            is_valid: error_count == 0,
            issues,
            error_count,
            warning_count,
            info_count,
        }
    }

    /// Create a successful validation result with no issues
    pub fn success() -> Self {
        Self {
            issues: Vec::new(),
            is_valid: true,
            error_count: 0,
            warning_count: 0,
            info_count: 0,
        }
    }

    /// Create a failed validation result with a single error
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        let issue = ValidationIssue::error(code, message);
        Self::from_issues(vec![issue])
    }

    /// Merge another validation result into this one
    pub fn merge(&mut self, other: ValidationResult) {
        self.issues.extend(other.issues);
        self.error_count += other.error_count;
        self.warning_count += other.warning_count;
        self.info_count += other.info_count;
        self.is_valid = self.is_valid && other.is_valid;
    }
}

/// Concrete implementation of the ValidationEngine for FHIRSchema validation
#[derive(Debug)]
pub struct FhirSchemaValidationEngine {
    /// Whether to perform strict validation
    pub strict_mode: bool,
}

impl FhirSchemaValidationEngine {
    /// Create a new validation engine
    pub fn new() -> Self {
        Self { strict_mode: false }
    }

    /// Create a new validation engine with strict mode enabled
    pub fn new_strict() -> Self {
        Self { strict_mode: true }
    }
}

impl Default for FhirSchemaValidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationEngine for FhirSchemaValidationEngine {
    fn validate_resource(&self, resource: &Value, schema: &FhirSchema) -> Result<ValidationResult> {
        let mut context = ValidationContext::new(resource.clone());
        context.add_schema(&schema.schema_type, schema.clone());

        // Start validation with the root schema
        self.validate_resource_with_context(resource, schema, &mut context)?;

        Ok(context.into_result())
    }

    fn validate_resource_with_schemas(
        &self,
        resource: &Value,
        schemas: &[&FhirSchema],
    ) -> Result<ValidationResult> {
        let mut context = ValidationContext::new(resource.clone());

        // Add all schemas to the context
        for schema in schemas {
            context.add_schema(&schema.schema_type, (*schema).clone());
        }

        let mut final_result = ValidationResult::success();

        // Validate against each schema
        for schema in schemas {
            let mut schema_context = context.clone();
            self.validate_resource_with_context(resource, schema, &mut schema_context)?;
            final_result.merge(schema_context.into_result());
        }

        Ok(final_result)
    }
}

impl FhirSchemaValidationEngine {
    /// Internal method to validate a resource with a given context
    fn validate_resource_with_context(
        &self,
        resource: &Value,
        schema: &FhirSchema,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Validate resourceType if present
        if let Some(resource_type) = resource.get("resourceType") {
            context.push_path("resourceType");
            if let Some(resource_type_str) = resource_type.as_str() {
                if resource_type_str != schema.schema_type {
                    context.add_error(
                        "resource-type-mismatch",
                        format!(
                            "Resource type '{}' does not match expected schema type '{}'. Check that you're validating against the correct schema.",
                            resource_type_str, schema.schema_type
                        ),
                    );
                }
            } else {
                context.add_error(
                    "invalid-resource-type",
                    format!(
                        "resourceType field must be a string, found: {}",
                        match resource_type {
                            Value::Null => "null",
                            Value::Bool(_) => "boolean",
                            Value::Number(_) => "number",
                            Value::String(_) => "string",
                            Value::Array(_) => "array",
                            Value::Object(_) => "object",
                        }
                    ),
                );
            }
            context.pop_path();
        } else if schema.schema_type != "Element" && schema.schema_type != "BackboneElement" {
            // Most FHIR resources require a resourceType field
            context.push_path("resourceType");
            context.add_error(
                "missing-resource-type",
                format!(
                    "Required field 'resourceType' is missing. Expected value: '{}'",
                    schema.schema_type
                ),
            );
            context.pop_path();
        }

        // Validate elements according to schema definition
        self.validate_elements(resource, schema, context)?;

        // Validate type rules
        self.validate_type_rules(resource, schema, context)?;

        // Handle schema composition if needed
        self.validate_schema_composition(resource, schema, context)?;

        Ok(())
    }

    /// Validate elements according to schema definition
    fn validate_elements(
        &self,
        resource: &Value,
        schema: &FhirSchema,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Validate each element defined in the schema
        for (element_path, element) in &schema.elements {
            context.push_path(element_path);

            // Get the value at this path from the resource
            let value = self.get_value_at_path(resource, element_path);

            // Validate element presence and cardinality
            self.validate_element_cardinality(value, element, context)?;

            // Check for excluded elements
            self.validate_excluded_elements(value, element, context)?;

            // Validate element type if value is present
            if let Some(val) = value {
                self.validate_element_type(val, element, context)?;
                self.validate_element_constraints(val, element, context)?;

                // Validate Reference types specifically
                if let Some(ref element_types) = element.element_type {
                    for element_type in element_types {
                        if element_type.code == "Reference" {
                            self.validate_reference(val, element_path, context)?;
                        }
                    }
                }

                // Validate primitive extensions if this is a primitive element
                self.validate_primitive_extensions(resource, element_path, element, context)?;
            }

            context.pop_path();
        }

        Ok(())
    }

    /// Validate type rules from the schema
    fn validate_type_rules(
        &self,
        resource: &Value,
        schema: &FhirSchema,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Check if resource conforms to the expected type structure
        if let Some(base_definition) = &schema.base_definition {
            // If this schema extends another, validate base type conformance
            context.add_warning(
                "base-definition-validation",
                format!(
                    "Base definition validation not yet implemented for: {}",
                    base_definition
                ),
            );
        }

        // Validate derivation rules if present
        if let Some(derivation) = &schema.derivation {
            match derivation.as_str() {
                "specialization" => {
                    // This is a profile - validate it conforms to base type
                    self.validate_specialization_rules(resource, schema, context)?;
                }
                "constraint" => {
                    // This is a constraint - validate additional restrictions
                    self.validate_constraint_rules(resource, schema, context)?;
                }
                _ => {
                    context.add_warning(
                        "unknown-derivation",
                        format!("Unknown derivation type: {}", derivation),
                    );
                }
            }
        }

        Ok(())
    }

    /// Handle schema composition and multiple schema validation
    fn validate_schema_composition(
        &self,
        resource: &Value,
        schema: &FhirSchema,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Handle schema extensions and additional properties
        for (key, value) in &schema.extensions {
            if key.starts_with("$") {
                // This is a schema directive, handle accordingly
                self.validate_schema_directive(resource, key, value, context)?;
            }
        }

        // Validate slicing rules if present
        for (slice_path, slicing) in &schema.slicing {
            context.push_path(slice_path);
            self.validate_slicing_rules(resource, slicing, context)?;
            context.pop_path();
        }

        Ok(())
    }

    /// Get value at a specific path in the resource
    fn get_value_at_path<'a>(&self, resource: &'a Value, path: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = resource;

        for part in parts {
            match current {
                Value::Object(obj) => {
                    current = obj.get(part)?;
                }
                Value::Array(arr) => {
                    // Handle array indices if the part is numeric
                    if let Ok(index) = part.parse::<usize>() {
                        current = arr.get(index)?;
                    } else {
                        // For arrays, we might need to check all elements
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(current)
    }

    /// Validate element cardinality (min/max occurrences)
    fn validate_element_cardinality(
        &self,
        value: Option<&Value>,
        element: &crate::Element,
        context: &mut ValidationContext,
    ) -> Result<()> {
        let is_present = value.is_some();
        let array_length = if let Some(Value::Array(arr)) = value {
            arr.len()
        } else if is_present {
            1
        } else {
            0
        };

        // Check minimum cardinality
        if let Some(min) = element.min {
            let min_usize = min as usize;
            if array_length < min_usize {
                context.add_error(
                    "cardinality-min-violation",
                    format!(
                        "Element '{}' requires at least {} occurrence(s), found {}",
                        element.path, min, array_length
                    ),
                );
            }
        }

        // Check maximum cardinality
        if let Some(ref max_str) = element.max {
            if max_str != "*" {
                if let Ok(max_num) = max_str.parse::<usize>() {
                    if array_length > max_num {
                        context.add_error(
                            "cardinality-max-violation",
                            format!(
                                "Element '{}' allows at most {} occurrence(s), found {}",
                                element.path, max_str, array_length
                            ),
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate element type constraints
    fn validate_element_type(
        &self,
        value: &Value,
        element: &crate::Element,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Validate against element types if specified
        if let Some(ref element_types) = element.element_type {
            if !element_types.is_empty() {
                let mut type_matched = false;

                for element_type in element_types {
                    if self.value_matches_type(value, &element_type.code) {
                        type_matched = true;
                        break;
                    }
                }

                if !type_matched {
                    let expected_types: Vec<String> =
                        element_types.iter().map(|t| t.code.clone()).collect();
                    context.add_error(
                        "type-mismatch",
                        format!(
                            "Element '{}' value does not match expected types: {}",
                            element.path,
                            expected_types.join(", ")
                        ),
                    );
                }
            }
        }

        Ok(())
    }

    /// Validate excluded elements (elements that should not be present)
    fn validate_excluded_elements(
        &self,
        value: Option<&Value>,
        element: &crate::Element,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Check if this element is excluded (max cardinality of 0)
        if let Some(ref max_str) = element.max {
            if max_str == "0" {
                // This element should not be present
                if value.is_some() {
                    context.add_error(
                        "excluded-element-present",
                        format!(
                            "Element '{}' is excluded (max cardinality 0) but is present in the resource",
                            element.path
                        ),
                    );
                }
            }
        }

        Ok(())
    }

    /// Check if a value has valid FHIR Reference structure
    fn is_valid_reference_structure(&self, value: &Value) -> bool {
        if let Value::Object(ref_obj) = value {
            // A Reference must have either 'reference' or 'identifier' or both
            let has_reference = ref_obj.contains_key("reference");
            let has_identifier = ref_obj.contains_key("identifier");

            // At least one of reference or identifier must be present
            has_reference || has_identifier
        } else {
            false
        }
    }

    /// Check if a value matches a FHIR type
    fn value_matches_type(&self, value: &Value, fhir_type: &str) -> bool {
        match fhir_type {
            "string" | "uri" | "url" | "canonical" | "code" | "oid" | "id" | "markdown" => {
                value.is_string()
            }
            "boolean" => value.is_boolean(),
            "integer" | "positiveInt" | "unsignedInt" => {
                value.is_number() && value.as_f64().is_some_and(|n| n.fract() == 0.0)
            }
            "decimal" => value.is_number(),
            "date" | "dateTime" | "instant" | "time" => {
                // Basic string check - more sophisticated date validation would be needed
                value.is_string()
            }
            "base64Binary" => value.is_string(),
            "Reference" => {
                // Reference must be an object with specific structure
                value.is_object() && self.is_valid_reference_structure(value)
            }
            _ => {
                // For complex types, assume object structure
                value.is_object()
            }
        }
    }

    /// Validate element constraints (patterns, fixed values, etc.)
    fn validate_element_constraints(
        &self,
        value: &Value,
        element: &crate::Element,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Validate fixed values
        if let Some(ref fixed_value) = element.fixed {
            if value != fixed_value {
                context.add_error(
                    "fixed-value-violation",
                    format!(
                        "Element '{}' has fixed value constraint violation",
                        element.path
                    ),
                );
            }
        }

        // Validate pattern constraints
        if let Some(ref pattern_value) = element.pattern {
            if !self.value_matches_pattern(value, pattern_value) {
                context.add_error(
                    "pattern-violation",
                    format!("Element '{}' does not match required pattern", element.path),
                );
            }
        }

        Ok(())
    }

    /// Validate primitive extensions for FHIR primitive elements
    fn validate_primitive_extensions(
        &self,
        resource: &Value,
        element_path: &str,
        element: &crate::Element,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Check if this is a primitive element that might have extensions
        if let Some(ref element_types) = element.element_type {
            for element_type in element_types {
                if self.is_primitive_type(&element_type.code) {
                    // For primitive elements, check for the corresponding extension element
                    let extension_path = format!(
                        "_{}",
                        element_path.split('.').next_back().unwrap_or(element_path)
                    );

                    // Get the parent object to check for extension
                    if let Some(parent_path) = element_path.rsplit_once('.') {
                        let parent_value = self.get_value_at_path(resource, parent_path.0);
                        if let Some(Value::Object(parent_obj)) = parent_value {
                            if let Some(extension_value) = parent_obj.get(&extension_path) {
                                // Validate extension structure
                                self.validate_extension_structure(
                                    extension_value,
                                    &extension_path,
                                    context,
                                )?;
                            }
                        }
                    } else {
                        // Root level primitive - check for extension in root object
                        if let Value::Object(root_obj) = resource {
                            if let Some(extension_value) = root_obj.get(&extension_path) {
                                self.validate_extension_structure(
                                    extension_value,
                                    &extension_path,
                                    context,
                                )?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a FHIR type is a primitive type
    fn is_primitive_type(&self, fhir_type: &str) -> bool {
        matches!(
            fhir_type,
            "boolean"
                | "integer"
                | "string"
                | "decimal"
                | "uri"
                | "url"
                | "canonical"
                | "base64Binary"
                | "instant"
                | "date"
                | "dateTime"
                | "time"
                | "code"
                | "oid"
                | "id"
                | "markdown"
                | "unsignedInt"
                | "positiveInt"
                | "uuid"
        )
    }

    /// Validate extension structure
    fn validate_extension_structure(
        &self,
        extension_value: &Value,
        extension_path: &str,
        context: &mut ValidationContext,
    ) -> Result<()> {
        match extension_value {
            Value::Object(ext_obj) => {
                // Extension object should have either 'url' and 'value[x]' or 'url' and 'extension'
                if !ext_obj.contains_key("url") {
                    context.add_error(
                        "extension-missing-url",
                        format!(
                            "Extension at '{}' must have a 'url' property",
                            extension_path
                        ),
                    );
                }

                // Check for value[x] or nested extensions
                let has_value = ext_obj.keys().any(|k| k.starts_with("value"));
                let has_extension = ext_obj.contains_key("extension");

                if !has_value && !has_extension {
                    context.add_error(
                        "extension-missing-content",
                        format!(
                            "Extension at '{}' must have either a value[x] or nested extensions",
                            extension_path
                        ),
                    );
                }

                if has_value && has_extension {
                    context.add_error(
                        "extension-conflicting-content",
                        format!(
                            "Extension at '{}' cannot have both value[x] and nested extensions",
                            extension_path
                        ),
                    );
                }
            }
            Value::Array(ext_array) => {
                // Array of extensions - validate each one
                for (index, ext_item) in ext_array.iter().enumerate() {
                    let item_path = format!("{}[{}]", extension_path, index);
                    self.validate_extension_structure(ext_item, &item_path, context)?;
                }
            }
            _ => {
                context.add_error(
                    "extension-invalid-type",
                    format!(
                        "Extension at '{}' must be an object or array of objects",
                        extension_path
                    ),
                );
            }
        }

        Ok(())
    }

    /// Validate FHIR Reference structure and content
    fn validate_reference(
        &self,
        value: &Value,
        element_path: &str,
        context: &mut ValidationContext,
    ) -> Result<()> {
        if let Value::Object(ref_obj) = value {
            let has_reference = ref_obj.contains_key("reference");
            let has_identifier = ref_obj.contains_key("identifier");

            // At least one of reference or identifier must be present
            if !has_reference && !has_identifier {
                context.add_error(
                    "reference-missing-content",
                    format!(
                        "Reference at '{}' must have either 'reference' or 'identifier'",
                        element_path
                    ),
                );
                return Ok(());
            }

            // Validate reference field if present
            if let Some(reference_value) = ref_obj.get("reference") {
                self.validate_reference_field(reference_value, element_path, context)?;
            }

            // Validate identifier field if present
            if let Some(identifier_value) = ref_obj.get("identifier") {
                self.validate_identifier_field(identifier_value, element_path, context)?;
            }

            // Validate display field if present
            if let Some(display_value) = ref_obj.get("display") {
                if !display_value.is_string() {
                    context.add_error(
                        "reference-invalid-display",
                        format!("Reference display at '{}' must be a string", element_path),
                    );
                }
            }

            // Validate type field if present
            if let Some(type_value) = ref_obj.get("type") {
                if !type_value.is_string() {
                    context.add_error(
                        "reference-invalid-type",
                        format!("Reference type at '{}' must be a string", element_path),
                    );
                }
            }
        } else {
            context.add_error(
                "reference-invalid-structure",
                format!("Reference at '{}' must be an object", element_path),
            );
        }

        Ok(())
    }

    /// Validate reference field content
    fn validate_reference_field(
        &self,
        reference_value: &Value,
        element_path: &str,
        context: &mut ValidationContext,
    ) -> Result<()> {
        if let Some(reference_str) = reference_value.as_str() {
            if reference_str.is_empty() {
                context.add_error(
                    "reference-empty",
                    format!("Reference value at '{}' cannot be empty", element_path),
                );
                return Ok(());
            }

            // Basic reference format validation
            if reference_str.starts_with('#') {
                // Fragment reference - should reference an element in the same resource
                if reference_str.len() == 1 {
                    context.add_error(
                        "reference-invalid-fragment",
                        format!(
                            "Fragment reference at '{}' cannot be just '#'",
                            element_path
                        ),
                    );
                }
            } else if reference_str.contains('/') {
                // Relative or absolute reference
                let parts: Vec<&str> = reference_str.split('/').collect();
                if parts.len() >= 2 {
                    let resource_type = parts[parts.len() - 2];
                    let resource_id = parts[parts.len() - 1];

                    if resource_type.is_empty() || resource_id.is_empty() {
                        context.add_error(
                            "reference-invalid-format",
                            format!(
                                "Reference at '{}' has invalid format: '{}'",
                                element_path, reference_str
                            ),
                        );
                    }
                }
            } else {
                // Could be a logical reference or other format
                // For now, just ensure it's not empty (already checked above)
            }
        } else {
            context.add_error(
                "reference-invalid-type",
                format!("Reference value at '{}' must be a string", element_path),
            );
        }

        Ok(())
    }

    /// Validate identifier field content
    fn validate_identifier_field(
        &self,
        identifier_value: &Value,
        element_path: &str,
        context: &mut ValidationContext,
    ) -> Result<()> {
        if let Value::Object(identifier_obj) = identifier_value {
            // Identifier should have either system+value or just value
            let _has_system = identifier_obj.contains_key("system");
            let has_value = identifier_obj.contains_key("value");

            if !has_value {
                context.add_error(
                    "identifier-missing-value",
                    format!("Identifier at '{}' must have a 'value' field", element_path),
                );
            }

            // Validate system if present
            if let Some(system_value) = identifier_obj.get("system") {
                if !system_value.is_string() {
                    context.add_error(
                        "identifier-invalid-system",
                        format!("Identifier system at '{}' must be a string", element_path),
                    );
                }
            }

            // Validate value if present
            if let Some(value_field) = identifier_obj.get("value") {
                if !value_field.is_string() {
                    context.add_error(
                        "identifier-invalid-value",
                        format!("Identifier value at '{}' must be a string", element_path),
                    );
                }
            }
        } else {
            context.add_error(
                "identifier-invalid-structure",
                format!("Identifier at '{}' must be an object", element_path),
            );
        }

        Ok(())
    }

    /// Check if value matches a pattern
    fn value_matches_pattern(&self, value: &Value, pattern: &Value) -> bool {
        // Simplified pattern matching - in a full implementation,
        // this would need to handle partial object matching
        match (value, pattern) {
            (Value::Object(val_obj), Value::Object(pat_obj)) => {
                // All pattern properties must match
                for (key, pat_val) in pat_obj {
                    if let Some(val_val) = val_obj.get(key) {
                        if !self.value_matches_pattern(val_val, pat_val) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            }
            _ => value == pattern,
        }
    }

    /// Validate specialization rules for profiles
    fn validate_specialization_rules(
        &self,
        resource: &Value,
        schema: &FhirSchema,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // For specialization (profiles), we need to validate that the resource
        // conforms to the base definition and any additional constraints

        if let Some(base_definition) = &schema.base_definition {
            // Check if we have the base schema available in context
            let base_url = base_definition.to_string();
            if let Some(base_schema) = context.get_schema(&base_url) {
                // Validate against base schema first
                let mut base_context = context.clone();
                self.validate_resource_with_context(resource, base_schema, &mut base_context)?;

                // Merge base validation results
                context.issues.extend(base_context.issues);
            } else {
                context.add_warning(
                    "missing-base-schema",
                    format!("Base schema not available for validation: {}", base_url),
                );
            }
        }

        // Validate profile-specific constraints
        for constraint in &schema.constraints {
            self.validate_constraint(resource, constraint, context)?;
        }

        Ok(())
    }

    /// Validate a single constraint
    fn validate_constraint(
        &self,
        resource: &Value,
        constraint: &crate::Constraint,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Basic FHIRPath constraint evaluation
        let constraint_result = self.evaluate_fhirpath_constraint(resource, constraint, context)?;

        // If constraint evaluation fails, add appropriate issue
        if !constraint_result {
            match constraint.severity.as_str() {
                "error" => {
                    context.add_error(&constraint.key, &constraint.human);
                }
                "warning" => {
                    context.add_warning(&constraint.key, &constraint.human);
                }
                "information" => {
                    context.add_warning(&constraint.key, &constraint.human);
                }
                _ => {
                    // Default to warning for unknown severity
                    context.add_warning(
                        &constraint.key,
                        format!(
                            "Constraint failed (unknown severity '{}'): {}",
                            constraint.severity, constraint.human
                        ),
                    );
                }
            }
        }

        Ok(())
    }

    /// Basic FHIRPath constraint evaluation
    fn evaluate_fhirpath_constraint(
        &self,
        resource: &Value,
        constraint: &crate::Constraint,
        context: &mut ValidationContext,
    ) -> Result<bool> {
        // For now, implement basic constraint patterns that are commonly used
        let expression = &constraint.expression;

        // Handle simple existence checks
        if expression.starts_with("exists()") || expression.contains(".exists()") {
            return self.evaluate_exists_constraint(resource, expression, context);
        }

        // Handle simple count constraints
        if expression.contains(".count()") {
            return self.evaluate_count_constraint(resource, expression, context);
        }

        // Handle simple value constraints
        if expression.contains(" = ") || expression.contains(" != ") {
            return self.evaluate_value_constraint(resource, expression, context);
        }

        // Handle empty/not empty constraints
        if expression.contains(".empty()") || expression.contains("empty()") {
            return self.evaluate_empty_constraint(resource, expression, context);
        }

        // For complex FHIRPath expressions, add a warning and assume they pass for now
        context.add_warning(
            "complex-fhirpath-constraint",
            format!(
                "Complex FHIRPath constraint not yet fully supported: {} - {}",
                constraint.key, expression
            ),
        );

        // Assume constraint passes for unsupported expressions to avoid false positives
        Ok(true)
    }

    /// Evaluate existence constraints
    fn evaluate_exists_constraint(
        &self,
        resource: &Value,
        expression: &str,
        _context: &mut ValidationContext,
    ) -> Result<bool> {
        // Simple exists() check - extract path and check if value exists
        if let Some(path) = self.extract_path_from_exists(expression) {
            let value = self.get_value_at_path(resource, &path);
            Ok(value.is_some())
        } else {
            // If we can't parse the expression, assume it passes
            Ok(true)
        }
    }

    /// Evaluate count constraints
    fn evaluate_count_constraint(
        &self,
        resource: &Value,
        expression: &str,
        _context: &mut ValidationContext,
    ) -> Result<bool> {
        // Basic count constraint evaluation
        if let Some((path, operator, expected_count)) = self.parse_count_constraint(expression) {
            if let Some(value) = self.get_value_at_path(resource, &path) {
                let actual_count = match value {
                    Value::Array(arr) => arr.len(),
                    _ => 1, // Single value counts as 1
                };

                match operator.as_str() {
                    "=" | "==" => Ok(actual_count == expected_count),
                    "!=" => Ok(actual_count != expected_count),
                    ">" => Ok(actual_count > expected_count),
                    ">=" => Ok(actual_count >= expected_count),
                    "<" => Ok(actual_count < expected_count),
                    "<=" => Ok(actual_count <= expected_count),
                    _ => Ok(true), // Unknown operator, assume passes
                }
            } else {
                // Path doesn't exist, count is 0
                match operator.as_str() {
                    "=" | "==" => Ok(expected_count == 0),
                    "!=" => Ok(expected_count != 0),
                    ">" => Ok(false), // 0 is never > any positive number
                    ">=" => Ok(expected_count == 0), // 0 >= expected_count only if expected_count is 0
                    "<" => Ok(expected_count > 0),   // 0 < expected_count if expected_count > 0
                    "<=" => Ok(true),                // 0 <= any number is always true
                    _ => Ok(true),
                }
            }
        } else {
            // Can't parse, assume passes
            Ok(true)
        }
    }

    /// Evaluate value constraints
    fn evaluate_value_constraint(
        &self,
        resource: &Value,
        expression: &str,
        _context: &mut ValidationContext,
    ) -> Result<bool> {
        // Basic value constraint evaluation
        if let Some((path, operator, expected_value)) = self.parse_value_constraint(expression) {
            if let Some(actual_value) = self.get_value_at_path(resource, &path) {
                match operator.as_str() {
                    "=" | "==" => Ok(*actual_value == expected_value),
                    "!=" => Ok(*actual_value != expected_value),
                    _ => Ok(true), // Unknown operator, assume passes
                }
            } else {
                // Path doesn't exist
                match operator.as_str() {
                    "=" | "==" => Ok(expected_value.is_empty()),
                    "!=" => Ok(!expected_value.is_empty()),
                    _ => Ok(true),
                }
            }
        } else {
            // Can't parse, assume passes
            Ok(true)
        }
    }

    /// Evaluate empty constraints
    fn evaluate_empty_constraint(
        &self,
        resource: &Value,
        expression: &str,
        _context: &mut ValidationContext,
    ) -> Result<bool> {
        // Basic empty constraint evaluation
        if let Some(path) = self.extract_path_from_empty(expression) {
            let value = self.get_value_at_path(resource, &path);
            let is_empty = match value {
                None => true,
                Some(Value::Array(arr)) => arr.is_empty(),
                Some(Value::String(s)) => s.is_empty(),
                Some(Value::Null) => true,
                _ => false,
            };

            // Check if expression expects empty or not empty
            if expression.contains("not ") || expression.starts_with("!") {
                Ok(!is_empty)
            } else {
                Ok(is_empty)
            }
        } else {
            // Can't parse, assume passes
            Ok(true)
        }
    }

    /// Extract path from exists() expression
    fn extract_path_from_exists(&self, expression: &str) -> Option<String> {
        // Simple regex-like parsing for path.exists() or exists(path)
        if let Some(start) = expression.find("exists(") {
            if let Some(end) = expression[start..].find(')') {
                let path_part = &expression[start + 7..start + end];
                return Some(path_part.trim().to_string());
            }
        }

        // Handle path.exists() pattern
        if expression.ends_with(".exists()") {
            let path = expression.trim_end_matches(".exists()");
            return Some(path.to_string());
        }

        None
    }

    /// Parse count constraint expression
    fn parse_count_constraint(&self, expression: &str) -> Option<(String, String, usize)> {
        // Look for patterns like "path.count() = 1" or "path.count() > 0"
        if let Some(count_pos) = expression.find(".count()") {
            let path = expression[..count_pos].trim().to_string();
            let remainder = &expression[count_pos + 8..].trim();

            // Find operator and value
            for op in &[">=", "<=", "!=", "==", "=", ">", "<"] {
                if let Some(op_pos) = remainder.find(op) {
                    let operator = op.to_string();
                    let value_str = remainder[op_pos + op.len()..].trim();
                    if let Ok(value) = value_str.parse::<usize>() {
                        return Some((path, operator, value));
                    }
                }
            }
        }
        None
    }

    /// Parse value constraint expression
    fn parse_value_constraint(&self, expression: &str) -> Option<(String, String, String)> {
        // Look for patterns like "path = 'value'" or "path != 'value'"
        for op in &["!=", "==", "="] {
            if let Some(op_pos) = expression.find(op) {
                let path = expression[..op_pos].trim().to_string();
                let operator = op.to_string();
                let value = expression[op_pos + op.len()..]
                    .trim()
                    .trim_matches('\'')
                    .trim_matches('"')
                    .to_string();
                return Some((path, operator, value));
            }
        }
        None
    }

    /// Extract path from empty() expression
    fn extract_path_from_empty(&self, expression: &str) -> Option<String> {
        // Handle path.empty() pattern
        if expression.ends_with(".empty()") {
            let path = expression.trim_end_matches(".empty()");
            return Some(path.to_string());
        }

        // Handle empty(path) pattern
        if let Some(start) = expression.find("empty(") {
            if let Some(end) = expression[start..].find(')') {
                let path_part = &expression[start + 6..start + end];
                return Some(path_part.trim().to_string());
            }
        }

        None
    }

    /// Validate constraint rules
    fn validate_constraint_rules(
        &self,
        resource: &Value,
        schema: &FhirSchema,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Validate all constraints defined in the schema
        for constraint in &schema.constraints {
            self.validate_constraint(resource, constraint, context)?;
        }

        // Validate element-level constraints
        for (element_path, element) in &schema.elements {
            context.push_path(element_path);

            // Get the value at this path
            if let Some(value) = self.get_value_at_path(resource, element_path) {
                // Validate element-specific constraints
                for constraint in &element.constraints {
                    self.validate_constraint(resource, constraint, context)?;
                }

                // Validate required/excluded rules
                self.validate_element_requirements(value, element, context)?;
            } else if let Some(min) = element.min {
                // Check if missing element violates minimum cardinality
                if min > 0 {
                    context.add_error(
                        "required-element-missing",
                        format!("Required element '{}' is missing", element_path),
                    );
                }
            }

            context.pop_path();
        }

        Ok(())
    }

    /// Validate schema directives
    fn validate_schema_directive(
        &self,
        _resource: &Value,
        directive: &str,
        _value: &Value,
        context: &mut ValidationContext,
    ) -> Result<()> {
        context.add_warning(
            "schema-directive",
            format!(
                "Schema directive '{}' validation not yet implemented",
                directive
            ),
        );
        Ok(())
    }

    /// Validate element requirements (required/excluded rules)
    fn validate_element_requirements(
        &self,
        value: &Value,
        element: &crate::Element,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Check if element has fixed value constraint
        if let Some(fixed_value) = &element.fixed {
            if value != fixed_value {
                context.add_error(
                    "fixed-value-mismatch",
                    "Value does not match fixed value constraint".to_string(),
                );
            }
        }

        // Check if element matches pattern constraint
        if let Some(pattern_value) = &element.pattern {
            if !self.value_matches_pattern(value, pattern_value) {
                context.add_error(
                    "pattern-mismatch",
                    "Value does not match pattern constraint".to_string(),
                );
            }
        }

        // Check binding constraints for coded elements
        if let Some(binding) = &element.binding {
            self.validate_binding(value, binding, context)?;
        }

        Ok(())
    }

    /// Validate binding constraints
    fn validate_binding(
        &self,
        _value: &Value,
        binding: &crate::Binding,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // For now, just validate that coded values are present when required
        match binding.strength.as_str() {
            "required" => {
                // Required binding - value must be from the value set
                if let Some(value_set) = &binding.value_set {
                    context.add_warning(
                        "binding-validation",
                        format!(
                            "Required binding validation not yet implemented for value set: {}",
                            value_set
                        ),
                    );
                }
            }
            "extensible" => {
                // Extensible binding - value should be from value set if possible
                context.add_warning(
                    "binding-validation",
                    "Extensible binding validation not yet implemented",
                );
            }
            "preferred" | "example" => {
                // Preferred/example bindings are informational only
            }
            _ => {
                context.add_warning(
                    "unknown-binding-strength",
                    format!("Unknown binding strength: {}", binding.strength),
                );
            }
        }
        Ok(())
    }

    /// Validate slicing rules
    fn validate_slicing_rules(
        &self,
        resource: &Value,
        slicing: &crate::Slicing,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Validate slicing discriminators
        for discriminator in &slicing.discriminator {
            self.validate_discriminator(resource, discriminator, context)?;
        }

        // Check slicing rules
        match slicing.rules.as_str() {
            "open" => {
                // Open slicing allows additional slices - no additional validation needed
            }
            "closed" => {
                // Closed slicing - only defined slices allowed
                // This would require checking that all elements match defined slices
                // For now, we validate that discriminators are properly defined
                if slicing.discriminator.is_empty() {
                    context.add_error(
                        "closed-slicing-no-discriminators",
                        "Closed slicing requires at least one discriminator to identify valid slices",
                    );
                }
            }
            "openAtEnd" => {
                // Open at end - additional slices only at the end
                // This would require checking slice ordering
                // For now, we validate that discriminators are properly defined
                if slicing.discriminator.is_empty() {
                    context.add_error(
                        "open-at-end-slicing-no-discriminators",
                        "OpenAtEnd slicing requires at least one discriminator to identify slice boundaries",
                    );
                }

                // Check if slicing is ordered (required for openAtEnd)
                if slicing.ordered != Some(true) {
                    context.add_warning(
                        "open-at-end-slicing-unordered",
                        "OpenAtEnd slicing should typically be ordered for proper validation",
                    );
                }
            }
            _ => {
                context.add_error(
                    "invalid-slicing-rules",
                    format!("Invalid slicing rules: {}", slicing.rules),
                );
            }
        }

        Ok(())
    }

    /// Validate slicing discriminator
    fn validate_discriminator(
        &self,
        resource: &Value,
        discriminator: &crate::Discriminator,
        context: &mut ValidationContext,
    ) -> Result<()> {
        let discriminator_value = self.get_value_at_path(resource, &discriminator.path);

        match discriminator.discriminator_type.as_str() {
            "value" => {
                // Value discriminator - check specific value exists
                if discriminator_value.is_none() {
                    context.add_error(
                        "discriminator-value-missing",
                        format!(
                            "Discriminator value missing at path: {}",
                            discriminator.path
                        ),
                    );
                }
                // Note: Actual value comparison would require the expected value from slice definition
            }
            "exists" => {
                // Exists discriminator - element must be present for this slice
                if discriminator_value.is_none() {
                    context.add_error(
                        "discriminator-exists-failed",
                        format!(
                            "Required discriminator element missing at path: {}",
                            discriminator.path
                        ),
                    );
                }
            }
            "pattern" => {
                // Pattern discriminator - check pattern match
                if let Some(_value) = discriminator_value {
                    // For now, just validate that the value exists
                    // Full pattern matching would require the pattern from slice definition
                    context.add_warning(
                        "pattern-discriminator-partial",
                        format!("Pattern discriminator at path '{}' found value but pattern matching not fully implemented", discriminator.path),
                    );
                } else {
                    context.add_error(
                        "discriminator-pattern-missing",
                        format!(
                            "Pattern discriminator value missing at path: {}",
                            discriminator.path
                        ),
                    );
                }
            }
            "type" => {
                // Type discriminator - check element type
                if let Some(value) = discriminator_value {
                    // Basic type validation - check if value has expected structure
                    if value.is_object() {
                        // For complex types, check if resourceType exists
                        if let Some(resource_type) = value.get("resourceType") {
                            if !resource_type.is_string() {
                                context.add_error(
                                    "discriminator-type-invalid",
                                    format!(
                                        "Type discriminator at path '{}' has invalid resourceType",
                                        discriminator.path
                                    ),
                                );
                            }
                        }
                    }
                    // Note: Full type validation would require the expected type from slice definition
                } else {
                    context.add_error(
                        "discriminator-type-missing",
                        format!(
                            "Type discriminator value missing at path: {}",
                            discriminator.path
                        ),
                    );
                }
            }
            "profile" => {
                // Profile discriminator - check profile conformance
                if let Some(value) = discriminator_value {
                    // Basic profile validation - check if value is an object with meta.profile
                    if let Some(meta) = value.get("meta") {
                        if let Some(profile) = meta.get("profile") {
                            if !profile.is_array() {
                                context.add_error(
                                    "discriminator-profile-invalid",
                                    format!("Profile discriminator at path '{}' has invalid profile structure", discriminator.path),
                                );
                            }
                        } else {
                            context.add_warning(
                                "discriminator-profile-missing",
                                format!(
                                    "Profile discriminator at path '{}' missing meta.profile",
                                    discriminator.path
                                ),
                            );
                        }
                    } else {
                        context.add_warning(
                            "discriminator-profile-no-meta",
                            format!(
                                "Profile discriminator at path '{}' missing meta element",
                                discriminator.path
                            ),
                        );
                    }
                } else {
                    context.add_error(
                        "discriminator-profile-missing",
                        format!(
                            "Profile discriminator value missing at path: {}",
                            discriminator.path
                        ),
                    );
                }
            }
            _ => {
                context.add_error(
                    "invalid-discriminator-type",
                    format!(
                        "Invalid discriminator type: {}",
                        discriminator.discriminator_type
                    ),
                );
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub location: Option<ValidationLocation>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Information,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValidationLocation {
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub span: Option<(u32, u32)>,
}

impl ValidationIssue {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            code: code.into(),
            message: message.into(),
            path: None,
            location: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Warning,
            code: code.into(),
            message: message.into(),
            path: None,
            location: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug)]
pub struct BasicSchemaValidator;

impl SchemaValidator for BasicSchemaValidator {
    fn validate_schema(&self, schema: &FhirSchema) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        issues.extend(self.validate_element_paths(schema)?);
        issues.extend(self.validate_constraints(schema)?);

        Ok(issues)
    }

    fn validate_element_paths(&self, schema: &FhirSchema) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        for (path, element) in &schema.elements {
            if path != &element.path {
                issues.push(
                    ValidationIssue::error(
                        "element-path-mismatch",
                        format!(
                            "Element path '{}' does not match key '{}'",
                            element.path, path
                        ),
                    )
                    .with_path(path),
                );
            }

            if let Err(e) = element.validate() {
                issues.push(
                    ValidationIssue::error(
                        "element-validation-failed",
                        format!("Element validation failed: {e}"),
                    )
                    .with_path(path),
                );
            }
        }

        Ok(issues)
    }

    fn validate_constraints(&self, schema: &FhirSchema) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        for constraint in &schema.constraints {
            if let Err(e) = constraint.validate() {
                issues.push(ValidationIssue::error(
                    "constraint-validation-failed",
                    format!("Constraint validation failed: {e}"),
                ));
            }
        }

        Ok(issues)
    }
}
