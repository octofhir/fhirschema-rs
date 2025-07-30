//! Slicing converter for transforming FHIR slicing definitions to FHIRSchema format.

use fhirschema_core::{Slicing, Discriminator, Slice, Element};
use crate::{Result, Error};
use serde_json::Value;
use std::collections::HashMap;

/// Converter for slicing transformation.
pub struct SlicingConverter {
    // Converter state can be added here if needed
}

impl SlicingConverter {
    /// Create a new slicing converter.
    pub fn new() -> Self {
        Self {}
    }

    /// Convert FHIR slicing definition to FHIRSchema Slicing.
    pub fn convert(&self, slicing_json: &Value) -> Result<Slicing> {
        let mut slicing = Slicing::new();

        // Convert discriminators
        if let Some(discriminators) = slicing_json.get("discriminator").and_then(|d| d.as_array()) {
            let mut converted_discriminators = Vec::new();
            for disc in discriminators {
                if let Some(discriminator) = self.convert_discriminator(disc)? {
                    converted_discriminators.push(discriminator);
                }
            }
            if !converted_discriminators.is_empty() {
                slicing.discriminator = Some(converted_discriminators);
            }
        }

        // Convert ordered flag
        if let Some(ordered) = slicing_json.get("ordered").and_then(|o| o.as_bool()) {
            slicing.ordered = Some(ordered);
        }

        // Convert rules
        if let Some(rules) = slicing_json.get("rules").and_then(|r| r.as_str()) {
            slicing.rules = Some(rules.to_string());
        }

        // Convert description
        if let Some(description) = slicing_json.get("description").and_then(|d| d.as_str()) {
            slicing.description = Some(description.to_string());
        }

        Ok(slicing)
    }

    /// Convert FHIR discriminator to FHIRSchema Discriminator.
    fn convert_discriminator(&self, discriminator_json: &Value) -> Result<Option<Discriminator>> {
        let disc_type = discriminator_json.get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| Error::Conversion("Discriminator missing 'type' field".to_string()))?;

        let path = discriminator_json.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| Error::Conversion("Discriminator missing 'path' field".to_string()))?;

        Ok(Some(Discriminator::new(disc_type.to_string(), path.to_string())))
    }

    /// Convert element definitions with slicing to include slice information.
    pub fn convert_sliced_elements(&self, elements: &[Value]) -> Result<HashMap<String, Slice>> {
        let mut slices = HashMap::new();

        for element in elements {
            if let Some(slice_name) = self.extract_slice_name(element)? {
                let slice = self.convert_element_to_slice(element, &slice_name)?;
                slices.insert(slice_name, slice);
            }
        }

        Ok(slices)
    }

    /// Extract slice name from element definition.
    fn extract_slice_name(&self, element: &Value) -> Result<Option<String>> {
        if let Some(slice_name) = element.get("sliceName").and_then(|s| s.as_str()) {
            return Ok(Some(slice_name.to_string()));
        }

        // Try to extract from path if it contains slice notation
        if let Some(path) = element.get("path").and_then(|p| p.as_str()) {
            if let Some(colon_pos) = path.rfind(':') {
                let slice_name = &path[colon_pos + 1..];
                return Ok(Some(slice_name.to_string()));
            }
        }

        Ok(None)
    }

    /// Convert element definition to slice.
    fn convert_element_to_slice(&self, element: &Value, slice_name: &str) -> Result<Slice> {
        let mut slice = Slice::new(slice_name.to_string());

        // Convert cardinality
        if let Some(min) = element.get("min").and_then(|m| m.as_u64()) {
            slice.min = Some(min as u32);
        }

        if let Some(max) = element.get("max").and_then(|m| m.as_str()) {
            slice.max = Some(max.to_string());
        }

        // Convert short description
        if let Some(short) = element.get("short").and_then(|s| s.as_str()) {
            slice.short = Some(short.to_string());
        }

        // Convert definition
        if let Some(definition) = element.get("definition").and_then(|d| d.as_str()) {
            slice.definition = Some(definition.to_string());
        }

        // Convert match criteria from fixed values or patterns
        if let Some(fixed_value) = element.get("fixedValue") {
            slice.match_criteria = Some(format!("fixedValue: {}", fixed_value));
        } else if let Some(pattern) = element.get("pattern") {
            slice.match_criteria = Some(format!("pattern: {}", pattern));
        }

        Ok(slice)
    }

    /// Validate slicing definition for consistency.
    pub fn validate_slicing(&self, slicing: &Slicing) -> Result<()> {
        // Check that discriminators are valid
        if let Some(discriminators) = &slicing.discriminator {
            for disc in discriminators {
                if disc.discriminator_type.is_empty() {
                    return Err(Error::Conversion("Discriminator type cannot be empty".to_string()));
                }
                if disc.path.is_empty() {
                    return Err(Error::Conversion("Discriminator path cannot be empty".to_string()));
                }

                // Validate discriminator type
                match disc.discriminator_type.as_str() {
                    "value" | "exists" | "pattern" | "type" | "profile" => {},
                    _ => return Err(Error::Conversion(
                        format!("Invalid discriminator type: {}", disc.discriminator_type)
                    )),
                }
            }
        }

        // Check that rules are valid
        if let Some(rules) = &slicing.rules {
            match rules.as_str() {
                "closed" | "open" | "openAtEnd" => {},
                _ => return Err(Error::Conversion(
                    format!("Invalid slicing rules: {}", rules)
                )),
            }
        }

        Ok(())
    }
}

impl Default for SlicingConverter {
    fn default() -> Self {
        Self::new()
    }
}
