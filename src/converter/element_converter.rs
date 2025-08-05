use std::collections::HashMap;

use super::{ChoiceTypeExpander, ConversionContext, ConverterConfig, ElementDefinition};
use crate::{Binding, Element, ElementType, FhirSchemaError, Result};

pub struct ElementConverter {
    config: ConverterConfig,
    choice_type_expander: ChoiceTypeExpander,
}

impl ElementConverter {
    pub fn new(config: &ConverterConfig) -> Self {
        Self {
            config: config.clone(),
            choice_type_expander: ChoiceTypeExpander::new(config),
        }
    }

    pub fn convert_element(
        &self,
        element_def: &ElementDefinition,
        context: &mut ConversionContext,
    ) -> Result<HashMap<String, Element>> {
        context.validate_state()?;

        let mut result = HashMap::new();

        // Check if this element should be expanded as a choice type
        if self.config.expand_choice_types && self.is_choice_type(&element_def.path) {
            let expanded_elements = self
                .choice_type_expander
                .expand_choice_type(element_def, context)?;
            result.extend(expanded_elements);
        } else {
            let element = self.convert_single_element(element_def, context)?;
            result.insert(element_def.path.clone(), element);
        }

        Ok(result)
    }

    pub async fn convert_element_async(
        &self,
        element_def: &ElementDefinition,
        context: &mut ConversionContext,
    ) -> Result<HashMap<String, Element>> {
        context.validate_state()?;

        let mut result = HashMap::new();

        // Check if this element should be expanded as a choice type
        if self.config.expand_choice_types && self.is_choice_type(&element_def.path) {
            let expanded_elements = self
                .choice_type_expander
                .expand_choice_type_async(element_def, context)
                .await?;
            result.extend(expanded_elements);
        } else {
            let element = self
                .convert_single_element_async(element_def, context)
                .await?;
            result.insert(element_def.path.clone(), element);
        }

        Ok(result)
    }

    fn convert_single_element(
        &self,
        element_def: &ElementDefinition,
        context: &mut ConversionContext,
    ) -> Result<Element> {
        let mut element = Element::new(&element_def.path);

        // Basic properties
        element.definition = element_def.definition.clone();
        element.short = element_def.short.clone();
        element.comment = element_def.comment.clone();

        // Cardinality
        element.min = element_def.min;
        element.max = element_def.max.clone();

        // Validate cardinality
        self.validate_cardinality(&element, context)?;

        // Element types
        if let Some(types) = &element_def.element_type {
            element.element_type = Some(self.convert_element_types(types, context)?);
        }

        // Fixed and pattern values
        element.fixed = element_def.fixed_value.clone();
        element.pattern = element_def.pattern_value.clone();

        // Constraints
        if let Some(constraints) = &element_def.constraint {
            for constraint_def in constraints {
                let constraint = crate::Constraint::new(
                    &constraint_def.key,
                    &constraint_def.severity,
                    &constraint_def.human,
                    constraint_def.expression.as_deref().unwrap_or("true"),
                );
                element.constraints.push(constraint);
            }
        }

        // Binding
        if let Some(binding_def) = &element_def.binding {
            element.binding = Some(self.convert_binding(binding_def, context)?);
        }

        // Mapping
        if let Some(mappings) = &element_def.mapping {
            for mapping_def in mappings {
                let mapping = crate::Mapping {
                    identity: mapping_def.identity.clone(),
                    language: mapping_def.language.clone(),
                    map: mapping_def.map.clone(),
                    comment: mapping_def.comment.clone(),
                };
                element.mapping.push(mapping);
            }
        }

        // Modifier flags
        element.is_modifier = element_def.is_modifier.unwrap_or(false);
        element.is_summary = element_def.is_summary.unwrap_or(false);

        context.mark_element_processed(&element_def.path);
        Ok(element)
    }

    async fn convert_single_element_async(
        &self,
        element_def: &ElementDefinition,
        context: &mut ConversionContext,
    ) -> Result<Element> {
        let mut element = Element::new(&element_def.path);

        // Basic properties
        element.definition = element_def.definition.clone();
        element.short = element_def.short.clone();
        element.comment = element_def.comment.clone();

        // Cardinality
        element.min = element_def.min;
        element.max = element_def.max.clone();

        // Validate cardinality
        self.validate_cardinality(&element, context)?;

        // Element types (with async profile resolution)
        if let Some(types) = &element_def.element_type {
            element.element_type = Some(self.convert_element_types_async(types, context).await?);
        }

        // Fixed and pattern values
        element.fixed = element_def.fixed_value.clone();
        element.pattern = element_def.pattern_value.clone();

        // Constraints
        if let Some(constraints) = &element_def.constraint {
            for constraint_def in constraints {
                let constraint = crate::Constraint::new(
                    &constraint_def.key,
                    &constraint_def.severity,
                    &constraint_def.human,
                    constraint_def.expression.as_deref().unwrap_or("true"),
                );
                element.constraints.push(constraint);
            }
        }

        // Binding
        if let Some(binding_def) = &element_def.binding {
            element.binding = Some(self.convert_binding(binding_def, context)?);
        }

        // Mapping
        if let Some(mappings) = &element_def.mapping {
            for mapping_def in mappings {
                let mapping = crate::Mapping {
                    identity: mapping_def.identity.clone(),
                    language: mapping_def.language.clone(),
                    map: mapping_def.map.clone(),
                    comment: mapping_def.comment.clone(),
                };
                element.mapping.push(mapping);
            }
        }

        // Modifier flags
        element.is_modifier = element_def.is_modifier.unwrap_or(false);
        element.is_summary = element_def.is_summary.unwrap_or(false);

        context.mark_element_processed(&element_def.path);
        Ok(element)
    }

    fn convert_element_types(
        &self,
        types: &[super::ElementDefinitionType],
        context: &mut ConversionContext,
    ) -> Result<Vec<ElementType>> {
        let mut result = Vec::new();

        for type_def in types {
            let mut element_type = ElementType::new(&type_def.code);

            // Profiles
            if let Some(profiles) = &type_def.profile {
                element_type.profile = Some(profiles.clone());

                // Try to resolve profiles if configured
                if self.config.resolve_profiles {
                    for profile_url in profiles {
                        if let Ok(Some(_profile)) = context.resolve_profile(profile_url) {
                            // Profile resolved successfully
                            context.add_info(format!("Resolved profile: {profile_url}"));
                        }
                    }
                }
            }

            // Target profiles (for references)
            if let Some(target_profiles) = &type_def.target_profile {
                element_type.target_profile = Some(target_profiles.clone());
            }

            // Aggregation rules
            element_type.aggregation = type_def.aggregation.clone();
            element_type.versioning = type_def.versioning.clone();

            result.push(element_type);
        }

        Ok(result)
    }

    async fn convert_element_types_async(
        &self,
        types: &[super::ElementDefinitionType],
        context: &mut ConversionContext,
    ) -> Result<Vec<ElementType>> {
        let mut result = Vec::new();

        for type_def in types {
            let mut element_type = ElementType::new(&type_def.code);

            // Profiles (with async resolution)
            if let Some(profiles) = &type_def.profile {
                element_type.profile = Some(profiles.clone());

                // Try to resolve profiles if configured
                if self.config.resolve_profiles {
                    for profile_url in profiles {
                        if let Ok(Some(_profile)) = context.resolve_profile_async(profile_url).await
                        {
                            // Profile resolved successfully
                            context.add_info(format!("Resolved profile: {profile_url}"));
                        }
                    }
                }
            }

            // Target profiles (for references)
            if let Some(target_profiles) = &type_def.target_profile {
                element_type.target_profile = Some(target_profiles.clone());
            }

            // Aggregation rules
            element_type.aggregation = type_def.aggregation.clone();
            element_type.versioning = type_def.versioning.clone();

            result.push(element_type);
        }

        Ok(result)
    }

    fn convert_binding(
        &self,
        binding_def: &super::ElementDefinitionBinding,
        _context: &mut ConversionContext,
    ) -> Result<Binding> {
        let mut binding = Binding::new(&binding_def.strength);

        binding.description = binding_def.description.clone();

        if let Some(value_set) = &binding_def.value_set {
            binding = binding.with_value_set(value_set.clone());
        }

        Ok(binding)
    }

    fn validate_cardinality(
        &self,
        element: &Element,
        context: &mut ConversionContext,
    ) -> Result<()> {
        if let (Some(min), Some(max_str)) = (&element.min, &element.max) {
            if max_str != "*" {
                if let Ok(max_num) = max_str.parse::<u32>() {
                    if *min > max_num {
                        let error_msg = format!(
                            "Invalid cardinality for element {}: min ({}) > max ({})",
                            element.path, min, max_num
                        );
                        context.add_error(error_msg.clone());
                        return Err(FhirSchemaError::Conversion { message: error_msg });
                    }
                } else {
                    context.add_warning(format!(
                        "Unparseable max cardinality for element {}: {}",
                        element.path, max_str
                    ));
                }
            }
        }
        Ok(())
    }

    fn is_choice_type(&self, path: &str) -> bool {
        path.ends_with("[x]")
    }
}
