use std::collections::HashMap;

use super::{ConversionContext, ConverterConfig, ElementDefinition};
use crate::{
    ChoiceInconsistency, ChoiceTypePattern, ChoiceValidationResult, Element, ElementType,
    FhirSchemaError, ResolvedChoiceType, Result,
};

pub struct ChoiceTypeExpander {
    config: ConverterConfig,
}

impl ChoiceTypeExpander {
    pub fn new(config: &ConverterConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub fn expand_choice_type(
        &self,
        element_def: &ElementDefinition,
        context: &mut ConversionContext,
    ) -> Result<HashMap<String, Element>> {
        if !element_def.path.ends_with("[x]") {
            return Err(FhirSchemaError::Conversion {
                message: format!("Element {} is not a choice type", element_def.path),
            });
        }

        let base_path = element_def.path.trim_end_matches("[x]");
        let mut result = HashMap::new();
        let mut expanded_paths = Vec::new();

        if let Some(types) = &element_def.element_type {
            for element_type in types {
                let type_specific_path =
                    format!("{}{}", base_path, self.capitalize_first(&element_type.code));

                let mut element = Element::new(&type_specific_path);

                // Copy base properties
                element.definition = element_def.definition.clone();
                element.short = element_def.short.clone();
                element.comment = element_def.comment.clone();
                element.min = element_def.min;
                element.max = element_def.max.clone();
                element.is_modifier = element_def.is_modifier.unwrap_or(false);
                element.is_summary = element_def.is_summary.unwrap_or(false);

                // Set the specific type
                let mut converted_type = ElementType::new(&element_type.code);
                converted_type.profile = element_type.profile.clone();
                converted_type.target_profile = element_type.target_profile.clone();
                converted_type.aggregation = element_type.aggregation.clone();
                converted_type.versioning = element_type.versioning.clone();

                element.element_type = Some(vec![converted_type]);

                // Handle type-specific constraints
                if let Some(constraints) = &element_def.constraint {
                    for constraint_def in constraints {
                        // Adapt constraint expression for the specific type
                        let adapted_expression = self.adapt_constraint_for_type(
                            constraint_def.expression.as_deref().unwrap_or("true"),
                            &element_type.code,
                            context,
                        )?;

                        let constraint = crate::Constraint::new(
                            &constraint_def.key,
                            &constraint_def.severity,
                            &constraint_def.human,
                            &adapted_expression,
                        );
                        element.constraints.push(constraint);
                    }
                }

                // Handle type-specific fixed/pattern values
                if let Some(fixed_value) = &element_def.fixed_value {
                    element.fixed =
                        Some(self.adapt_value_for_type(fixed_value, &element_type.code)?);
                }

                if let Some(pattern_value) = &element_def.pattern_value {
                    element.pattern =
                        Some(self.adapt_value_for_type(pattern_value, &element_type.code)?);
                }

                // Copy binding if applicable
                if let Some(binding_def) = &element_def.binding {
                    if self.is_binding_applicable_to_type(&element_type.code) {
                        let binding = crate::Binding::new(&binding_def.strength);
                        element.binding = Some(binding);
                    }
                }

                expanded_paths.push(type_specific_path.clone());
                result.insert(type_specific_path, element);
                context.mark_element_processed(&element_def.path);
            }
        } else {
            // No types specified - this is an error for choice types
            context.add_error(format!(
                "Choice type element {} has no type definitions",
                element_def.path
            ));

            return Err(FhirSchemaError::Conversion {
                message: format!("Choice type {} missing type definitions", element_def.path),
            });
        }

        context.add_choice_type_expansion(base_path.to_string(), expanded_paths);
        Ok(result)
    }

    pub async fn expand_choice_type_async(
        &self,
        element_def: &ElementDefinition,
        context: &mut ConversionContext,
    ) -> Result<HashMap<String, Element>> {
        if !element_def.path.ends_with("[x]") {
            return Err(FhirSchemaError::Conversion {
                message: format!("Element {} is not a choice type", element_def.path),
            });
        }

        let base_path = element_def.path.trim_end_matches("[x]");
        let mut result = HashMap::new();
        let mut expanded_paths = Vec::new();

        if let Some(types) = &element_def.element_type {
            for element_type in types {
                let type_specific_path =
                    format!("{}{}", base_path, self.capitalize_first(&element_type.code));

                let mut element = Element::new(&type_specific_path);

                // Copy base properties
                element.definition = element_def.definition.clone();
                element.short = element_def.short.clone();
                element.comment = element_def.comment.clone();
                element.min = element_def.min;
                element.max = element_def.max.clone();
                element.is_modifier = element_def.is_modifier.unwrap_or(false);
                element.is_summary = element_def.is_summary.unwrap_or(false);

                // Set the specific type (with async profile resolution if needed)
                let mut converted_type = ElementType::new(&element_type.code);
                converted_type.profile = element_type.profile.clone();
                converted_type.target_profile = element_type.target_profile.clone();
                converted_type.aggregation = element_type.aggregation.clone();
                converted_type.versioning = element_type.versioning.clone();

                // Async profile resolution for choice type elements
                if let Some(profiles) = &element_type.profile {
                    if self.config.resolve_profiles {
                        for profile_url in profiles {
                            if let Ok(Some(_profile)) =
                                context.resolve_profile_async(profile_url).await
                            {
                                context.add_info(format!(
                                    "Resolved profile for choice type {type_specific_path}: {profile_url}"
                                ));
                            }
                        }
                    }
                }

                element.element_type = Some(vec![converted_type]);

                // Handle type-specific constraints
                if let Some(constraints) = &element_def.constraint {
                    for constraint_def in constraints {
                        // Adapt constraint expression for the specific type
                        let adapted_expression = self.adapt_constraint_for_type(
                            constraint_def.expression.as_deref().unwrap_or("true"),
                            &element_type.code,
                            context,
                        )?;

                        let constraint = crate::Constraint::new(
                            &constraint_def.key,
                            &constraint_def.severity,
                            &constraint_def.human,
                            &adapted_expression,
                        );
                        element.constraints.push(constraint);
                    }
                }

                // Handle type-specific fixed/pattern values
                if let Some(fixed_value) = &element_def.fixed_value {
                    element.fixed =
                        Some(self.adapt_value_for_type(fixed_value, &element_type.code)?);
                }

                if let Some(pattern_value) = &element_def.pattern_value {
                    element.pattern =
                        Some(self.adapt_value_for_type(pattern_value, &element_type.code)?);
                }

                // Copy binding if applicable
                if let Some(binding_def) = &element_def.binding {
                    if self.is_binding_applicable_to_type(&element_type.code) {
                        let binding = crate::Binding::new(&binding_def.strength);
                        element.binding = Some(binding);
                    }
                }

                expanded_paths.push(type_specific_path.clone());
                result.insert(type_specific_path, element);
                context.mark_element_processed(&element_def.path);
            }
        } else {
            // No types specified - this is an error for choice types
            context.add_error(format!(
                "Choice type element {} has no type definitions",
                element_def.path
            ));

            return Err(FhirSchemaError::Conversion {
                message: format!("Choice type {} missing type definitions", element_def.path),
            });
        }

        context.add_choice_type_expansion(base_path.to_string(), expanded_paths);
        Ok(result)
    }

    pub fn capitalize_first(&self, s: &str) -> String {
        if s.is_empty() {
            return s.to_string();
        }

        let mut chars: Vec<char> = s.chars().collect();
        chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
        chars.into_iter().collect()
    }

    fn adapt_constraint_for_type(
        &self,
        expression: &str,
        type_code: &str,
        context: &mut ConversionContext,
    ) -> Result<String> {
        // For now, we'll return the expression as-is
        // In a full implementation, this would parse FHIRPath expressions
        // and adapt them for the specific type

        if expression.contains("[x]") {
            context.add_warning(format!(
                "Constraint expression contains [x] placeholder that may need adaptation for type {type_code}: {expression}"
            ));
        }

        Ok(expression.to_string())
    }

    fn adapt_value_for_type(
        &self,
        value: &serde_json::Value,
        _type_code: &str,
    ) -> Result<serde_json::Value> {
        // For now, return the value as-is
        // In a full implementation, this would transform the value
        // to match the specific type requirements
        Ok(value.clone())
    }

    fn is_binding_applicable_to_type(&self, type_code: &str) -> bool {
        // Bindings typically apply to coded types
        matches!(
            type_code,
            "code" | "Coding" | "CodeableConcept" | "string" | "uri" | "url" | "canonical"
        )
    }

    pub fn get_common_choice_types() -> Vec<&'static str> {
        vec![
            "boolean",
            "integer",
            "string",
            "decimal",
            "uri",
            "url",
            "canonical",
            "base64Binary",
            "instant",
            "date",
            "dateTime",
            "time",
            "code",
            "oid",
            "id",
            "markdown",
            "unsignedInt",
            "positiveInt",
            "uuid",
            "Address",
            "Age",
            "Annotation",
            "Attachment",
            "CodeableConcept",
            "Coding",
            "ContactPoint",
            "Count",
            "Distance",
            "Duration",
            "HumanName",
            "Identifier",
            "Money",
            "Period",
            "Quantity",
            "Range",
            "Ratio",
            "Reference",
            "SampledData",
            "Signature",
            "Timing",
        ]
    }

    pub fn is_primitive_type(type_code: &str) -> bool {
        matches!(
            type_code,
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

    /// NEW: Resolve choice type from actual data value
    pub fn resolve_choice_from_value(
        &self,
        value: &serde_json::Value,
        base_path: &str,
    ) -> Option<ResolvedChoiceType> {
        match value {
            serde_json::Value::String(_) => Some(ResolvedChoiceType::new(base_path, "string")),
            serde_json::Value::Number(n) => {
                if n.is_i64() {
                    Some(ResolvedChoiceType::new(base_path, "integer"))
                } else {
                    Some(ResolvedChoiceType::new(base_path, "decimal"))
                }
            }
            serde_json::Value::Bool(_) => Some(ResolvedChoiceType::new(base_path, "boolean")),
            serde_json::Value::Object(obj) => {
                // Detect complex type by looking at object structure
                self.detect_complex_type_from_object(obj, base_path)
            }
            _ => None,
        }
    }

    /// NEW: Get all expanded paths for a choice type base
    pub fn get_all_choice_expansions(
        &self,
        choice_path: &str,
    ) -> Result<HashMap<String, ElementType>> {
        if !choice_path.ends_with("[x]") {
            return Err(FhirSchemaError::Conversion {
                message: format!("{choice_path} is not a choice type"),
            });
        }

        let base_path = choice_path.trim_end_matches("[x]");
        let mut expansions = HashMap::new();

        // Generate expansions for all possible types
        for type_code in Self::get_common_choice_types() {
            let expanded_path = format!("{}{}", base_path, self.capitalize_first(type_code));
            let element_type = ElementType::new(type_code);
            expansions.insert(expanded_path, element_type);
        }

        Ok(expansions)
    }

    /// NEW: Pattern recognition for choice types in schemas
    pub fn identify_choice_patterns(
        &self,
        elements: &HashMap<String, Element>,
    ) -> Vec<ChoiceTypePattern> {
        let mut patterns = Vec::new();
        let mut processed_bases = std::collections::HashSet::new();

        for path in elements.keys() {
            if let Some(base_path) = self.extract_choice_base_path(path) {
                if processed_bases.contains(&base_path) {
                    continue;
                }

                let pattern = self.analyze_choice_pattern(&base_path, elements);
                patterns.push(pattern);
                processed_bases.insert(base_path);
            }
        }

        patterns
    }

    /// NEW: Validate choice type consistency
    pub fn validate_choice_type_consistency(
        &self,
        base_path: &str,
        elements: &HashMap<String, Element>,
    ) -> Result<ChoiceValidationResult> {
        let choice_elements: Vec<_> = elements
            .iter()
            .filter(|(path, _)| {
                path.starts_with(base_path) && path.as_str() != format!("{base_path}[x]")
            })
            .collect();

        if choice_elements.is_empty() {
            return Ok(ChoiceValidationResult::valid());
        }

        let mut validation = ChoiceValidationResult::new();

        // Check cardinality consistency
        self.validate_choice_cardinality(&choice_elements, &mut validation);

        // Check constraint consistency
        self.validate_choice_constraints(&choice_elements, &mut validation);

        // Check binding consistency
        self.validate_choice_bindings(&choice_elements, &mut validation);

        Ok(validation)
    }

    // === HELPER METHODS ===

    fn detect_complex_type_from_object(
        &self,
        obj: &serde_json::Map<String, serde_json::Value>,
        base_path: &str,
    ) -> Option<ResolvedChoiceType> {
        // Look for type indicators in the object
        if obj.contains_key("system") && obj.contains_key("code") {
            return Some(ResolvedChoiceType::new(base_path, "Coding"));
        }

        if obj.contains_key("coding") || obj.contains_key("text") {
            return Some(ResolvedChoiceType::new(base_path, "CodeableConcept"));
        }

        if obj.contains_key("value") && obj.contains_key("unit") {
            return Some(ResolvedChoiceType::new(base_path, "Quantity"));
        }

        if obj.contains_key("start") || obj.contains_key("end") {
            return Some(ResolvedChoiceType::new(base_path, "Period"));
        }

        if obj.contains_key("reference") {
            return Some(ResolvedChoiceType::new(base_path, "Reference"));
        }

        None
    }

    pub fn extract_choice_base_path(&self, path: &str) -> Option<String> {
        // Pattern: somePathSomeType -> somePath[x]
        // Look for paths that match choice type naming pattern
        for type_name in Self::get_common_choice_types() {
            let capitalized = self.capitalize_first(type_name);
            if path.ends_with(&capitalized) {
                let base = path.trim_end_matches(&capitalized);
                return Some(format!("{base}[x]"));
            }
        }
        None
    }

    fn analyze_choice_pattern(
        &self,
        base_path: &str,
        elements: &HashMap<String, Element>,
    ) -> ChoiceTypePattern {
        let base_without_choice = base_path.trim_end_matches("[x]");

        let expansions: Vec<_> = elements
            .keys()
            .filter(|path| path.starts_with(base_without_choice) && path.as_str() != base_path)
            .cloned()
            .collect();

        ChoiceTypePattern {
            base_path: base_path.to_string(),
            expansions: expansions.clone(),
            detected_types: self.extract_types_from_paths(&expansions),
        }
    }

    fn extract_types_from_paths(&self, paths: &[String]) -> Vec<String> {
        let mut types = Vec::new();

        for path in paths {
            for type_name in Self::get_common_choice_types() {
                let capitalized = self.capitalize_first(type_name);
                if path.ends_with(&capitalized) {
                    types.push(type_name.to_string());
                    break;
                }
            }
        }

        types
    }

    fn validate_choice_cardinality(
        &self,
        choice_elements: &[(&String, &Element)],
        validation: &mut ChoiceValidationResult,
    ) {
        if let Some((_, first_element)) = choice_elements.first() {
            let expected_min = first_element.min;
            let expected_max = &first_element.max;

            for (path, element) in choice_elements.iter().skip(1) {
                if element.min != expected_min || element.max != *expected_max {
                    validation.add_inconsistency(ChoiceInconsistency {
                        issue_type: "warning".to_string(),
                        paths: vec![path.to_string()],
                        description: format!(
                            "Cardinality mismatch: expected {:?}..{:?}, found {:?}..{:?}",
                            expected_min, expected_max, element.min, element.max
                        ),
                    });
                }
            }
        }
    }

    fn validate_choice_constraints(
        &self,
        choice_elements: &[(&String, &Element)],
        validation: &mut ChoiceValidationResult,
    ) {
        for (path, element) in choice_elements {
            if element.constraints.is_empty() {
                validation.add_warning(format!("Choice element {path} has no constraints"));
            }
        }
    }

    fn validate_choice_bindings(
        &self,
        choice_elements: &[(&String, &Element)],
        validation: &mut ChoiceValidationResult,
    ) {
        for (path, element) in choice_elements {
            if let Some(element_types) = &element.element_type {
                for element_type in element_types {
                    if self.is_binding_applicable_to_type(&element_type.code)
                        && element.binding.is_none()
                    {
                        validation.add_warning(format!(
                            "Choice element {path} with type {} could have a binding",
                            element_type.code
                        ));
                    }
                }
            }
        }
    }
}
