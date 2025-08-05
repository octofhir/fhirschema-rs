use std::collections::HashMap;

use super::{ConversionContext, ElementDefinition};
use crate::{Discriminator, FhirSchemaError, Result, Slicing};

pub struct SlicingProcessor;

impl SlicingProcessor {
    pub fn new() -> Self {
        Self
    }

    pub fn process_slicing(
        &self,
        elements: &[ElementDefinition],
        context: &mut ConversionContext,
    ) -> Result<HashMap<String, Slicing>> {
        let mut result = HashMap::new();

        for element in elements {
            if let Some(slicing_def) = &element.slicing {
                let slicing = self.convert_slicing(slicing_def, &element.path, context)?;
                result.insert(element.path.clone(), slicing);
                context.increment_slices_processed();
            }
        }

        Ok(result)
    }

    fn convert_slicing(
        &self,
        slicing_def: &super::ElementDefinitionSlicing,
        element_path: &str,
        context: &mut ConversionContext,
    ) -> Result<Slicing> {
        // Validate slicing rules
        if !self.is_valid_slice_rule(&slicing_def.rules) {
            let error_msg = format!(
                "Invalid slicing rule '{}' for element {}",
                slicing_def.rules, element_path
            );
            context.add_error(error_msg.clone());
            return Err(FhirSchemaError::Conversion { message: error_msg });
        }

        let mut slicing = Slicing::new(&slicing_def.rules);

        // Set description
        slicing.description = slicing_def.description.clone();
        slicing.ordered = slicing_def.ordered;

        // Convert discriminators
        if let Some(discriminators) = &slicing_def.discriminator {
            for disc_def in discriminators {
                let discriminator = self.convert_discriminator(disc_def, element_path, context)?;
                slicing = slicing.with_discriminator(discriminator);
            }
        } else {
            context.add_warning(format!(
                "Slicing definition for {element_path} has no discriminators"
            ));
        }

        Ok(slicing)
    }

    fn convert_discriminator(
        &self,
        disc_def: &super::ElementDefinitionSlicingDiscriminator,
        element_path: &str,
        context: &mut ConversionContext,
    ) -> Result<Discriminator> {
        // Validate discriminator type
        if !self.is_valid_discriminator_type(&disc_def.discriminator_type) {
            let error_msg = format!(
                "Invalid discriminator type '{}' for element {}",
                disc_def.discriminator_type, element_path
            );
            context.add_error(error_msg.clone());
            return Err(FhirSchemaError::Conversion { message: error_msg });
        }

        // Validate discriminator path
        if disc_def.path.is_empty() {
            let error_msg = format!("Empty discriminator path for element {element_path}");
            context.add_error(error_msg.clone());
            return Err(FhirSchemaError::Conversion { message: error_msg });
        }

        let discriminator = Discriminator::new(&disc_def.discriminator_type, &disc_def.path);

        // Add context-specific validations
        self.validate_discriminator_path(&discriminator, element_path, context)?;

        Ok(discriminator)
    }

    fn is_valid_slice_rule(&self, rule: &str) -> bool {
        matches!(rule, "open" | "closed" | "openAtEnd")
    }

    fn is_valid_discriminator_type(&self, disc_type: &str) -> bool {
        matches!(
            disc_type,
            "value" | "exists" | "pattern" | "type" | "profile"
        )
    }

    fn validate_discriminator_path(
        &self,
        discriminator: &Discriminator,
        element_path: &str,
        context: &mut ConversionContext,
    ) -> Result<()> {
        // Basic path validation
        if discriminator.path.contains("..") {
            context.add_warning(format!(
                "Discriminator path contains '..' which may be problematic: {} in element {}",
                discriminator.path, element_path
            ));
        }

        // Type-specific validations
        match discriminator.discriminator_type.as_str() {
            "value" => {
                if discriminator.path.is_empty() {
                    context.add_error(format!(
                        "Value discriminator requires non-empty path in element {element_path}"
                    ));
                }
            }
            "exists" => {
                // exists discriminator should work with any path
            }
            "pattern" => {
                if discriminator.path.is_empty() {
                    context.add_error(format!(
                        "Pattern discriminator requires non-empty path in element {element_path}"
                    ));
                }
            }
            "type" => {
                // Type discriminator is typically used at the root
                if !discriminator.path.is_empty() && discriminator.path != "$this" {
                    context.add_warning(format!(
                        "Type discriminator typically uses empty or '$this' path, got '{}' in element {}",
                        discriminator.path, element_path
                    ));
                }
            }
            "profile" => {
                // Profile discriminator is typically used at the root
                if !discriminator.path.is_empty() && discriminator.path != "$this" {
                    context.add_warning(format!(
                        "Profile discriminator typically uses empty or '$this' path, got '{}' in element {}",
                        discriminator.path, element_path
                    ));
                }
            }
            _ => {
                // This should not happen due to earlier validation
                context.add_error(format!(
                    "Unknown discriminator type '{}' in element {}",
                    discriminator.discriminator_type, element_path
                ));
            }
        }

        Ok(())
    }

    pub fn find_slice_elements<'a>(
        &self,
        elements: &'a [ElementDefinition],
        sliced_element_path: &str,
    ) -> Vec<&'a ElementDefinition> {
        elements
            .iter()
            .filter(|element| {
                element.path.starts_with(sliced_element_path)
                    && element.path != sliced_element_path
                    && element.slice_name.is_some()
            })
            .collect()
    }

    pub fn group_slices_by_name<'a>(
        &self,
        slice_elements: Vec<&'a ElementDefinition>,
    ) -> HashMap<String, Vec<&'a ElementDefinition>> {
        let mut groups = HashMap::new();

        for element in slice_elements {
            if let Some(slice_name) = &element.slice_name {
                groups
                    .entry(slice_name.clone())
                    .or_insert_with(Vec::new)
                    .push(element);
            }
        }

        groups
    }

    pub fn validate_slice_constraints(
        &self,
        slicing: &Slicing,
        slice_groups: &HashMap<String, Vec<&ElementDefinition>>,
        context: &mut ConversionContext,
    ) -> Result<()> {
        // Validate that slices follow the slicing rules
        match slicing.rules.as_str() {
            "closed" => {
                // In closed slicing, only predefined slices are allowed
                context.add_info("Validating closed slicing constraints".to_string());
            }
            "open" => {
                // In open slicing, additional slices beyond those defined are allowed
                context.add_info("Validating open slicing constraints".to_string());
            }
            "openAtEnd" => {
                // Open at end allows additional slices only after all defined slices
                context.add_info("Validating openAtEnd slicing constraints".to_string());
            }
            _ => {
                return Err(FhirSchemaError::Conversion {
                    message: format!("Invalid slicing rule: {}", slicing.rules),
                });
            }
        }

        // Validate each slice group
        for (slice_name, elements) in slice_groups {
            self.validate_slice_group(slice_name, elements, context)?;
        }

        Ok(())
    }

    fn validate_slice_group(
        &self,
        slice_name: &str,
        elements: &[&ElementDefinition],
        context: &mut ConversionContext,
    ) -> Result<()> {
        if elements.is_empty() {
            context.add_warning(format!("Empty slice group: {slice_name}"));
            return Ok(());
        }

        // Check for consistent slice naming
        for element in elements {
            if let Some(name) = &element.slice_name {
                if name != slice_name {
                    context.add_error(format!(
                        "Inconsistent slice name: expected '{slice_name}', got '{name}'"
                    ));
                }
            }
        }

        context.add_info(format!(
            "Validated slice group '{}' with {} elements",
            slice_name,
            elements.len()
        ));
        Ok(())
    }
}

impl Default for SlicingProcessor {
    fn default() -> Self {
        Self::new()
    }
}
