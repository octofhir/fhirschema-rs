// Element conversion logic with comprehensive type handling

use crate::core::ResolutionContext;
use crate::error::{FhirSchemaError, Result};
use crate::types::{ConstraintSeverity, FhirConstraint, FhirSchemaProperty, TypeResolver};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ElementConverter {
    // Type mapping for FHIR primitive types
    primitive_type_map: HashMap<String, String>,
    // Optional type resolver for advanced type operations
    type_resolver: Option<Arc<TypeResolver>>,
}

impl ElementConverter {
    pub fn new() -> Self {
        let mut primitive_type_map = HashMap::new();

        // FHIR primitive types to JSON Schema types
        primitive_type_map.insert("boolean".to_string(), "boolean".to_string());
        primitive_type_map.insert("integer".to_string(), "integer".to_string());
        primitive_type_map.insert("integer64".to_string(), "integer".to_string());
        primitive_type_map.insert("decimal".to_string(), "number".to_string());
        primitive_type_map.insert("string".to_string(), "string".to_string());
        primitive_type_map.insert("uri".to_string(), "string".to_string());
        primitive_type_map.insert("url".to_string(), "string".to_string());
        primitive_type_map.insert("canonical".to_string(), "string".to_string());
        primitive_type_map.insert("base64Binary".to_string(), "string".to_string());
        primitive_type_map.insert("instant".to_string(), "string".to_string());
        primitive_type_map.insert("date".to_string(), "string".to_string());
        primitive_type_map.insert("dateTime".to_string(), "string".to_string());
        primitive_type_map.insert("time".to_string(), "string".to_string());
        primitive_type_map.insert("code".to_string(), "string".to_string());
        primitive_type_map.insert("oid".to_string(), "string".to_string());
        primitive_type_map.insert("id".to_string(), "string".to_string());
        primitive_type_map.insert("markdown".to_string(), "string".to_string());
        primitive_type_map.insert("unsignedInt".to_string(), "integer".to_string());
        primitive_type_map.insert("positiveInt".to_string(), "integer".to_string());
        primitive_type_map.insert("uuid".to_string(), "string".to_string());

        Self {
            primitive_type_map,
            type_resolver: None,
        }
    }

    /// Create element converter with advanced type resolver
    pub fn with_type_resolver(type_resolver: Arc<TypeResolver>) -> Self {
        let mut converter = Self::new();
        converter.type_resolver = Some(type_resolver);
        converter
    }

    /// Convert a FHIR ElementDefinition to a FhirSchemaProperty
    pub async fn convert_element(&self, element: &Value, path: &str) -> Result<FhirSchemaProperty> {
        let mut property = FhirSchemaProperty::object();

        // Extract basic information
        if let Some(definition) = element.get("definition").and_then(|d| d.as_str()) {
            property = property.with_description(definition);
        } else if let Some(short) = element.get("short").and_then(|s| s.as_str()) {
            property = property.with_description(short);
        }

        // Handle cardinality
        let min = element.get("min").and_then(|m| m.as_u64()).unwrap_or(0);
        let max = element.get("max").and_then(|m| m.as_str()).unwrap_or("1");

        // Handle type information
        if let Some(types) = element.get("type").and_then(|t| t.as_array()) {
            property = self
                .process_element_types(property, types, min, max)
                .await?;
        } else {
            // If no type specified, it's likely a BackboneElement or complex type
            property = self.handle_complex_element(property, element, path).await?;
        }

        // Handle fixed values
        if let Some(fixed_value) = self.extract_fixed_value(element) {
            property = self.apply_fixed_value(property, fixed_value)?;
        }

        // Handle pattern values
        if let Some(pattern_value) = self.extract_pattern_value(element) {
            property = self.apply_pattern_value(property, pattern_value)?;
        }

        // Handle binding information for coded elements
        if let Some(binding) = element.get("binding") {
            property = self.apply_binding(property, binding)?;
        }

        // Handle constraints
        if let Some(constraints) = element.get("constraint").and_then(|c| c.as_array()) {
            for constraint in constraints {
                let fhir_constraint = self.convert_element_constraint(constraint)?;
                property.add_constraint(fhir_constraint);
            }
        }

        // Handle slicing information
        if let Some(_slicing) = element.get("slicing") {
            property = self.handle_slicing(property, element).await?;
        }

        Ok(property)
    }

    /// Process element types and determine the appropriate JSON Schema type
    async fn process_element_types(
        &self,
        mut property: FhirSchemaProperty,
        types: &[Value],
        min: u64,
        max: &str,
    ) -> Result<FhirSchemaProperty> {
        if types.is_empty() {
            return Ok(property);
        }

        // Handle single type
        if types.len() == 1 {
            let type_info = &types[0];
            property = self.convert_single_type(property, type_info).await?;
        } else {
            // Handle choice types (polymorphic elements)
            property = self.convert_choice_type(property, types).await?;
        }

        // Apply cardinality constraints
        property = self.apply_cardinality(property, min, max)?;

        Ok(property)
    }

    /// Convert a single FHIR type to JSON Schema property
    async fn convert_single_type(
        &self,
        mut property: FhirSchemaProperty,
        type_info: &Value,
    ) -> Result<FhirSchemaProperty> {
        let code = type_info
            .get("code")
            .and_then(|c| c.as_str())
            .unwrap_or("string");

        if let Some(json_type) = self.primitive_type_map.get(code) {
            // Handle primitive types
            property.property_type = Some(json_type.clone());
            property = self.apply_primitive_constraints(property, code)?;
        } else {
            // Handle complex types or references
            match code {
                "Reference" => {
                    property = self.handle_reference_type(property, type_info)?;
                }
                "Extension" => {
                    property = FhirSchemaProperty::reference("#/definitions/Extension")
                        .with_description("May be used to represent additional information that is not part of the basic definition");
                }
                "Resource" => {
                    property = FhirSchemaProperty::reference("#/definitions/Resource")
                        .with_description("Base Resource");
                }
                "BackboneElement" => {
                    property = FhirSchemaProperty::object().with_description(
                        "Base definition for all elements that are defined inside a resource",
                    );
                }
                _ => {
                    // Complex type - create reference to definition
                    property = FhirSchemaProperty::reference(&format!("#/definitions/{code}"))
                        .with_description(&format!("{code} complex type"));
                }
            }
        }

        Ok(property)
    }

    /// Convert choice types (polymorphic elements) with advanced type resolution
    async fn convert_choice_type(
        &self,
        _property: FhirSchemaProperty,
        types: &[Value],
    ) -> Result<FhirSchemaProperty> {
        // For choice types, we create a oneOf schema
        let mut one_of_types = Vec::new();
        let mut choice_type_names = Vec::new();

        for type_info in types {
            let code = type_info
                .get("code")
                .and_then(|c| c.as_str())
                .unwrap_or("string");
            choice_type_names.push(code.to_string());

            // Use advanced type resolver if available
            if let Some(type_resolver) = &self.type_resolver {
                let context = ResolutionContext::new("choice_conversion");
                match type_resolver.resolve_type(code, &context).await {
                    Ok(resolved_type) => {
                        let type_property = if resolved_type.is_primitive {
                            let mut prop = FhirSchemaProperty::string();
                            if let Some(json_type) = self.primitive_type_map.get(code) {
                                prop.property_type = Some(json_type.clone());
                                prop = self.apply_primitive_constraints(prop, code)?;
                            }
                            prop
                        } else {
                            FhirSchemaProperty::reference(&format!("#/definitions/{code}"))
                                .with_description(&format!("Reference to {code} type"))
                        };
                        one_of_types.push(type_property);
                    }
                    Err(_) => {
                        // Fallback to basic conversion
                        let type_property = self.convert_type_fallback(code)?;
                        one_of_types.push(type_property);
                    }
                }
            } else {
                // Fallback to basic conversion when no type resolver
                let type_property = self.convert_type_fallback(code)?;
                one_of_types.push(type_property);
            }
        }

        // Create enhanced choice type representation with metadata
        let mut choice_property = FhirSchemaProperty::object();
        choice_property.metadata.insert(
            "choice_types".to_string(),
            serde_json::Value::Array(
                choice_type_names
                    .iter()
                    .map(|code| serde_json::Value::String(code.to_string()))
                    .collect(),
            ),
        );

        // Add choice type pattern information
        choice_property
            .metadata
            .insert("is_choice_type".to_string(), serde_json::Value::Bool(true));

        // Add oneOf schema for validation
        choice_property.metadata.insert(
            "oneOf".to_string(),
            serde_json::to_value(one_of_types)
                .map_err(|e| FhirSchemaError::conversion_failed("choice_type", &e.to_string()))?,
        );

        Ok(choice_property)
    }

    /// Fallback type conversion when advanced type resolver is not available
    fn convert_type_fallback(&self, code: &str) -> Result<FhirSchemaProperty> {
        if let Some(json_type) = self.primitive_type_map.get(code) {
            let mut type_property = FhirSchemaProperty::string();
            type_property.property_type = Some(json_type.clone());
            self.apply_primitive_constraints(type_property, code)
        } else {
            Ok(FhirSchemaProperty::reference(&format!(
                "#/definitions/{code}"
            )))
        }
    }

    /// Handle complex elements without explicit types
    async fn handle_complex_element(
        &self,
        _property: FhirSchemaProperty,
        element: &Value,
        _path: &str,
    ) -> Result<FhirSchemaProperty> {
        // This is likely a BackboneElement or container element
        Ok(FhirSchemaProperty::object().with_description(
            element
                .get("definition")
                .or_else(|| element.get("short"))
                .and_then(|d| d.as_str())
                .unwrap_or("Complex element"),
        ))
    }

    /// Apply cardinality constraints
    fn apply_cardinality(
        &self,
        mut property: FhirSchemaProperty,
        min: u64,
        max: &str,
    ) -> Result<FhirSchemaProperty> {
        if min > 0 {
            // Element is required
            if let Some(ref mut required) = property.required {
                required.push("value".to_string());
            } else {
                property.required = Some(vec!["value".to_string()]);
            }
        }

        if max != "*" && max != "1" {
            // Handle arrays
            if let Ok(max_count) = max.parse::<u64>() {
                if max_count > 1 {
                    property = FhirSchemaProperty::array(property);

                    if min > 0 {
                        property.min_length = Some(min as usize);
                    }
                    if max_count < u64::MAX {
                        property.max_length = Some(max_count as usize);
                    }
                }
            }
        } else if max == "*" {
            // Unbounded array
            property = FhirSchemaProperty::array(property);
            if min > 0 {
                property.min_length = Some(min as usize);
            }
        }

        Ok(property)
    }

    /// Apply primitive type specific constraints
    fn apply_primitive_constraints(
        &self,
        mut property: FhirSchemaProperty,
        fhir_type: &str,
    ) -> Result<FhirSchemaProperty> {
        match fhir_type {
            "id" => {
                property = property.with_pattern("^[A-Za-z0-9\\-\\.]{1,64}$");
            }
            "code" => {
                property = property.with_pattern("^[^\\s]+( [^\\s]+)*$");
            }
            "uri" | "url" | "canonical" => {
                property = property.with_format("uri");
            }
            "date" => {
                property = property.with_pattern("^([0-9]([0-9]([0-9][1-9]|[1-9]0)|[1-9]00)|[1-9]000)(-(0[1-9]|1[0-2])(-(0[1-9]|[1-2][0-9]|3[0-1]))?)?$");
            }
            "dateTime" => {
                property = property.with_pattern("^([0-9]([0-9]([0-9][1-9]|[1-9]0)|[1-9]00)|[1-9]000)(-(0[1-9]|1[0-2])(-(0[1-9]|[1-2][0-9]|3[0-1])(T([01][0-9]|2[0-3]):[0-5][0-9]:([0-5][0-9]|60)(\\.[0-9]+)?(Z|(\\+|-)((0[0-9]|1[0-3]):[0-5][0-9]|14:00)))?)?)?$");
            }
            "time" => {
                property = property
                    .with_pattern("^([01][0-9]|2[0-3]):[0-5][0-9]:([0-5][0-9]|60)(\\.[0-9]+)?$");
            }
            "instant" => {
                property = property.with_pattern("^([0-9]([0-9]([0-9][1-9]|[1-9]0)|[1-9]00)|[1-9]000)-(0[1-9]|1[0-2])-(0[1-9]|[1-2][0-9]|3[0-1])T([01][0-9]|2[0-3]):[0-5][0-9]:([0-5][0-9]|60)(\\.[0-9]+)?(Z|(\\+|-)((0[0-9]|1[0-3]):[0-5][0-9]|14:00))$");
            }
            "base64Binary" => {
                property = property.with_format("base64");
            }
            "oid" => {
                property = property.with_pattern("^urn:oid:[0-2](\\.(0|[1-9][0-9]*))+$");
            }
            "uuid" => {
                property = property.with_pattern(
                    "^urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
                );
            }
            "positiveInt" => {
                property.minimum = Some(1.0);
            }
            "unsignedInt" => {
                property.minimum = Some(0.0);
            }
            _ => {}
        }

        Ok(property)
    }

    /// Handle Reference type with target profiles
    fn handle_reference_type(
        &self,
        _property: FhirSchemaProperty,
        type_info: &Value,
    ) -> Result<FhirSchemaProperty> {
        let mut property = FhirSchemaProperty::reference("#/definitions/Reference")
            .with_description("A reference to another resource");

        // Add target profile information if available
        if let Some(target_profiles) = type_info.get("targetProfile").and_then(|tp| tp.as_array()) {
            let profiles: Vec<String> = target_profiles
                .iter()
                .filter_map(|p| p.as_str())
                .map(|s| s.to_string())
                .collect();

            if !profiles.is_empty() {
                property.metadata.insert(
                    "fhir_target_profiles".to_string(),
                    serde_json::to_value(profiles).map_err(FhirSchemaError::Serialization)?,
                );
            }
        }

        Ok(property)
    }

    /// Extract fixed value from element
    fn extract_fixed_value(&self, element: &Value) -> Option<Value> {
        // Check for fixed[x] fields
        for (key, value) in element.as_object()? {
            if key.starts_with("fixed") {
                return Some(value.clone());
            }
        }
        None
    }

    /// Extract pattern value from element
    fn extract_pattern_value(&self, element: &Value) -> Option<Value> {
        // Check for pattern[x] fields
        for (key, value) in element.as_object()? {
            if key.starts_with("pattern") {
                return Some(value.clone());
            }
        }
        None
    }

    /// Apply fixed value constraint
    fn apply_fixed_value(
        &self,
        mut property: FhirSchemaProperty,
        fixed_value: Value,
    ) -> Result<FhirSchemaProperty> {
        property.enum_values = Some(vec![fixed_value]);
        Ok(property)
    }

    /// Apply pattern value constraint
    fn apply_pattern_value(
        &self,
        mut property: FhirSchemaProperty,
        _pattern_value: Value,
    ) -> Result<FhirSchemaProperty> {
        // Pattern values are typically used for validation, not schema structure
        // We could add this as a custom constraint
        property
            .metadata
            .insert("fhir_pattern".to_string(), _pattern_value);
        Ok(property)
    }

    /// Apply binding information for coded elements
    fn apply_binding(
        &self,
        mut property: FhirSchemaProperty,
        binding: &Value,
    ) -> Result<FhirSchemaProperty> {
        if let Some(strength) = binding.get("strength").and_then(|s| s.as_str()) {
            property.metadata.insert(
                "fhir_binding_strength".to_string(),
                Value::String(strength.to_string()),
            );
        }

        if let Some(value_set) = binding.get("valueSet").and_then(|vs| vs.as_str()) {
            property.metadata.insert(
                "fhir_binding_valueset".to_string(),
                Value::String(value_set.to_string()),
            );
        }

        if let Some(description) = binding.get("description").and_then(|d| d.as_str()) {
            property.metadata.insert(
                "fhir_binding_description".to_string(),
                Value::String(description.to_string()),
            );
        }

        Ok(property)
    }

    /// Convert element constraint
    fn convert_element_constraint(&self, constraint: &Value) -> Result<FhirConstraint> {
        let key = constraint
            .get("key")
            .and_then(|k| k.as_str())
            .unwrap_or("unknown");
        let human = constraint
            .get("human")
            .and_then(|h| h.as_str())
            .unwrap_or("Constraint");

        let severity = constraint
            .get("severity")
            .and_then(|s| s.as_str())
            .map(|s| match s {
                "error" => ConstraintSeverity::Error,
                "warning" => ConstraintSeverity::Warning,
                "information" => ConstraintSeverity::Information,
                _ => ConstraintSeverity::Error,
            })
            .unwrap_or(ConstraintSeverity::Error);

        let mut fhir_constraint = FhirConstraint::new(key, severity, human);

        if let Some(expression) = constraint.get("expression").and_then(|e| e.as_str()) {
            fhir_constraint = fhir_constraint.with_expression(expression);
        }

        if let Some(xpath) = constraint.get("xpath").and_then(|x| x.as_str()) {
            fhir_constraint = fhir_constraint.with_xpath(xpath);
        }

        if let Some(source) = constraint.get("source").and_then(|s| s.as_str()) {
            fhir_constraint = fhir_constraint.with_source(source);
        }

        Ok(fhir_constraint)
    }

    /// Handle slicing information
    async fn handle_slicing(
        &self,
        property: FhirSchemaProperty,
        _element: &Value,
    ) -> Result<FhirSchemaProperty> {
        // For now, just return the property as-is
        // Full slicing support would be implemented in a later phase
        Ok(property)
    }
}

impl Default for ElementConverter {
    fn default() -> Self {
        Self::new()
    }
}
