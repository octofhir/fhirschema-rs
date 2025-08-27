//! Field validation for FHIRPath expressions using FHIRSchema
//!
//! This module provides functionality to validate that field navigation in FHIRPath
//! expressions uses fields that actually exist in FHIR resource types according to
//! the FHIRSchema definitions.

use crate::error::{FhirSchemaError, Result};
use crate::package::ModelProvider;
use crate::types::FhirSchema;
use std::sync::Arc;

/// Field validation result
#[derive(Debug, Clone, PartialEq)]
pub struct FieldValidationResult {
    pub field_name: String,
    pub exists: bool,
    pub element_info: Option<FieldInfo>,
    pub suggestions: Vec<String>,
}

/// Information about a validated field
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInfo {
    pub path: String,
    pub element_types: Vec<String>,
    pub cardinality: String,
    pub is_required: bool,
    pub is_deprecated: bool,
    pub description: Option<String>,
}

/// Context for field validation
#[derive(Debug, Clone)]
pub struct FieldValidationContext {
    pub resource_type: String,
    pub current_path: Vec<String>,
    pub available_profiles: Vec<String>,
}

/// Field validator that uses FHIRSchema to validate field existence
pub struct FhirSchemaFieldValidator {
    model_provider: Arc<dyn ModelProvider>,
}

impl FhirSchemaFieldValidator {
    /// Create a new field validator
    pub fn new(model_provider: Arc<dyn ModelProvider>) -> Self {
        Self { model_provider }
    }

    /// Validate that a field exists in the given resource type
    pub async fn validate_field(
        &self,
        resource_type: &str,
        field_name: &str,
    ) -> Result<FieldValidationResult> {
        // Get the schema for the resource type
        let schema = self.get_schema_for_resource(resource_type).await?;

        // Check if the field exists in the schema
        let field_exists = self.check_field_exists(&schema, field_name).await;

        if field_exists {
            // Get detailed field information
            let element_info = self.get_field_info(&schema, field_name).await;

            Ok(FieldValidationResult {
                field_name: field_name.to_string(),
                exists: true,
                element_info,
                suggestions: vec![],
            })
        } else {
            // Generate suggestions for similar field names
            let suggestions = self.generate_field_suggestions(&schema, field_name).await;

            Ok(FieldValidationResult {
                field_name: field_name.to_string(),
                exists: false,
                element_info: None,
                suggestions,
            })
        }
    }

    /// Validate a field path (e.g., "name.given")
    pub async fn validate_field_path(
        &self,
        resource_type: &str,
        field_path: &str,
    ) -> Result<Vec<FieldValidationResult>> {
        let mut results = Vec::new();
        let path_parts: Vec<&str> = field_path.split('.').collect();
        let mut current_type = resource_type.to_string();
        let mut current_path = String::new();

        for (i, field_name) in path_parts.iter().enumerate() {
            if i > 0 {
                current_path.push('.');
            }
            current_path.push_str(field_name);

            // Validate this field in the current context
            let result = self.validate_field(&current_type, field_name).await?;

            if !result.exists {
                results.push(result);
                break; // Can't continue validation if field doesn't exist
            }

            // Update current type for next iteration
            if let Some(field_info) = &result.element_info {
                if let Some(next_type) = field_info.element_types.first() {
                    // Handle primitive types vs complex types
                    current_type = self.resolve_next_type(next_type).await;
                }
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Check if a resource type is valid
    pub async fn validate_resource_type(&self, resource_type: &str) -> Result<bool> {
        Ok(self.model_provider.has_resource_type(resource_type).await)
    }

    /// Get all available resource types
    pub async fn get_available_resource_types(&self) -> Result<Vec<String>> {
        Ok(self.model_provider.get_resource_types().await)
    }

    /// Get all fields available for a resource type
    pub async fn get_available_fields(&self, resource_type: &str) -> Result<Vec<String>> {
        let schema = self.get_schema_for_resource(resource_type).await?;
        let mut fields = Vec::new();

        for (field_path, _element) in &schema.elements {
            // Extract just the field name from the full path (e.g., "Patient.name" -> "name")
            if let Some(field_name) = field_path.split('.').last() {
                if !fields.contains(&field_name.to_string()) {
                    fields.push(field_name.to_string());
                }
            }
        }

        fields.sort();
        Ok(fields)
    }

    /// Get schema for a resource type (tries multiple approaches)
    async fn get_schema_for_resource(&self, resource_type: &str) -> Result<Arc<FhirSchema>> {
        // Try different canonical URL formats
        let possible_urls = vec![
            format!("http://hl7.org/fhir/StructureDefinition/{}", resource_type),
            format!("https://hl7.org/fhir/StructureDefinition/{}", resource_type),
            resource_type.to_string(),
        ];

        for url in possible_urls {
            if let Some(schema) = self.model_provider.get_schema(&url).await {
                return Ok(schema);
            }
        }

        // Try by resource type
        let schemas = self.model_provider.get_schemas_by_type(resource_type).await;
        if let Some(schema) = schemas.first() {
            return Ok(schema.clone());
        }

        Err(FhirSchemaError::Validation {
            message: format!("Schema not found for resource type: {}", resource_type),
        })
    }

    /// Check if a field exists in the schema
    async fn check_field_exists(&self, schema: &FhirSchema, field_name: &str) -> bool {
        // Check direct field references
        for (element_path, _element) in &schema.elements {
            if let Some(path_field_name) = element_path.split('.').last() {
                if path_field_name == field_name {
                    return true;
                }
            }
        }

        false
    }

    /// Get detailed information about a field
    async fn get_field_info(&self, schema: &FhirSchema, field_name: &str) -> Option<FieldInfo> {
        for (element_path, element) in &schema.elements {
            if let Some(path_field_name) = element_path.split('.').last() {
                if path_field_name == field_name {
                    let element_types = element
                        .element_type
                        .as_ref()
                        .map(|types| types.iter().map(|t| t.code.clone()).collect())
                        .unwrap_or_default();

                    let cardinality = format!(
                        "{}..{}",
                        element.min.unwrap_or(0),
                        element.max.as_deref().unwrap_or("*")
                    );

                    return Some(FieldInfo {
                        path: element_path.clone(),
                        element_types,
                        cardinality,
                        is_required: element.min.unwrap_or(0) > 0,
                        is_deprecated: false, // Would need to check constraints/extensions for this
                        description: element.definition.clone(),
                    });
                }
            }
        }

        None
    }

    /// Generate suggestions for similar field names
    async fn generate_field_suggestions(
        &self,
        schema: &FhirSchema,
        target_field: &str,
    ) -> Vec<String> {
        let mut scored_fields = Vec::new();
        let target_lower = target_field.to_lowercase();

        for (element_path, _element) in &schema.elements {
            if let Some(field_name) = element_path.split('.').last() {
                let field_lower = field_name.to_lowercase();
                let mut score = 0i32;

                // Exact match (shouldn't happen in error cases)
                if field_lower == target_lower {
                    score += 1000;
                }

                // Starts with target
                if field_lower.starts_with(&target_lower) {
                    score += 100;
                }

                // Target starts with field
                if target_lower.starts_with(&field_lower) {
                    score += 80;
                }

                // Contains target as substring
                if field_lower.contains(&target_lower) {
                    score += 50;
                }

                // Target contains field as substring
                if target_lower.contains(&field_lower) {
                    score += 40;
                }

                // Similar first few characters
                let prefix_len = 3.min(target_lower.len()).min(field_lower.len());
                if prefix_len > 0 {
                    let target_prefix = &target_lower[..prefix_len];
                    let field_prefix = &field_lower[..prefix_len];
                    if target_prefix == field_prefix {
                        score += 30;
                    }
                }

                // Length similarity bonus
                let len_diff = (target_field.len() as i32 - field_name.len() as i32).abs();
                if len_diff <= 2 {
                    score += 10;
                }

                if score > 0 {
                    scored_fields.push((field_name.to_string(), score));
                }
            }
        }

        // Sort by score descending and return top 3
        scored_fields.sort_by(|a, b| b.1.cmp(&a.1));
        scored_fields
            .into_iter()
            .take(3)
            .map(|(name, _)| name)
            .collect()
    }

    /// Resolve the next type in a field path traversal
    async fn resolve_next_type(&self, type_name: &str) -> String {
        // Handle primitive types
        match type_name {
            "string" | "code" | "id" | "markdown" | "uri" | "url" | "canonical" | "oid"
            | "uuid" => "string".to_string(),
            "integer" | "positiveInt" | "unsignedInt" => "integer".to_string(),
            "decimal" => "decimal".to_string(),
            "boolean" => "boolean".to_string(),
            "date" | "dateTime" | "instant" | "time" => "dateTime".to_string(),
            "base64Binary" => "base64Binary".to_string(),
            // For complex types, return as-is for further schema lookup
            _ => type_name.to_string(),
        }
    }

    /// Generate suggestions for resource types
    pub async fn generate_resource_type_suggestions(&self, target: &str) -> Result<Vec<String>> {
        let available_types = self.get_available_resource_types().await?;
        let target_lower = target.to_lowercase();
        let mut scored = Vec::new();

        for resource_type in available_types {
            let type_lower = resource_type.to_lowercase();
            let mut score = 0i32;

            if type_lower == target_lower {
                score += 1000;
            }

            if type_lower.starts_with(&target_lower) {
                score += 100;
            }

            if target_lower.starts_with(&type_lower) {
                score += 80;
            }

            if type_lower.contains(&target_lower) {
                score += 50;
            }

            if target_lower.contains(&type_lower) {
                score += 40;
            }

            let prefix_len = 3.min(target_lower.len()).min(type_lower.len());
            if prefix_len > 0 {
                let target_prefix = &target_lower[..prefix_len];
                let type_prefix = &type_lower[..prefix_len];
                if target_prefix == type_prefix {
                    score += 30;
                }
            }

            let len_diff = (target.len() as i32 - resource_type.len() as i32).abs();
            if len_diff <= 3 {
                score += 10;
            }

            if score > 0 {
                scored.push((resource_type, score));
            }
        }

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(scored.into_iter().take(5).map(|(name, _)| name).collect())
    }
}
