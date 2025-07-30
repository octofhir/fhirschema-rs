//! Main StructureDefinition converter.

use fhirschema_core::Schema;
use crate::{Result, Error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Simplified FHIR StructureDefinition representation for parsing
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureDefinition {
    pub url: String,
    pub name: String,
    pub title: Option<String>,
    pub status: String,
    pub kind: String,
    pub abstract_: Option<bool>,
    pub r#type: String,
    pub base_definition: Option<String>,
    pub derivation: Option<String>,
    pub differential: Option<ElementDefinitionContainer>,
    pub snapshot: Option<ElementDefinitionContainer>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ElementDefinitionContainer {
    pub element: Vec<ElementDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementDefinition {
    pub id: String,
    pub path: String,
    pub short: Option<String>,
    pub definition: Option<String>,
    pub comment: Option<String>,
    pub requirements: Option<String>,
    pub alias: Option<Vec<String>>,
    pub min: Option<u32>,
    pub max: Option<String>,
    pub r#type: Option<Vec<ElementType>>,
    pub fixed: Option<serde_json::Value>,
    pub pattern: Option<serde_json::Value>,
    pub example: Option<Vec<serde_json::Value>>,
    pub constraint: Option<Vec<ElementConstraint>>,
    pub binding: Option<ElementBinding>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ElementType {
    pub code: String,
    pub profile: Option<Vec<String>>,
    #[serde(rename = "targetProfile")]
    pub target_profile: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ElementConstraint {
    pub key: String,
    pub severity: String,
    pub human: String,
    pub expression: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementBinding {
    pub strength: String,
    pub description: Option<String>,
    pub value_set: Option<String>,
}

/// Main converter for transforming FHIR StructureDefinition to FHIRSchema.
pub struct StructureDefinitionConverter {
    // Converter state can be added here later
}

impl StructureDefinitionConverter {
    /// Create a new converter instance.
    pub fn new() -> Self {
        Self {}
    }

    /// Convert a StructureDefinition JSON string to FHIRSchema.
    pub fn convert(&self, structure_definition_json: &str) -> Result<Schema> {
        // Parse the StructureDefinition JSON
        let structure_def: StructureDefinition = serde_json::from_str(structure_definition_json)
            .map_err(|e| Error::InvalidStructureDefinition(format!("Failed to parse JSON: {}", e)))?;

        // Convert to FHIRSchema
        self.convert_structure_definition(&structure_def)
    }

    /// Convert a parsed StructureDefinition to FHIRSchema.
    fn convert_structure_definition(&self, structure_def: &StructureDefinition) -> Result<Schema> {
        // Determine derivation type
        let derivation = match structure_def.derivation.as_deref() {
            Some("specialization") => "specialization".to_string(),
            Some("constraint") => "constraint".to_string(),
            _ => "specialization".to_string(), // Default
        };

        // Create the base schema
        let mut schema = Schema::new(
            structure_def.url.clone(),
            structure_def.r#type.clone(),
            structure_def.name.clone(),
            derivation,
        );

        // Set base if available
        schema.base = structure_def.base_definition.clone();

        // Convert elements from differential or snapshot
        let elements_to_convert = structure_def.differential.as_ref()
            .or(structure_def.snapshot.as_ref());

        if let Some(element_container) = elements_to_convert {
            let mut converted_elements = HashMap::new();
            let mut converted_constraints = HashMap::new();

            for element_def in &element_container.element {
                // Skip the root element (same as resource type)
                if element_def.path == structure_def.r#type {
                    continue;
                }

                // Convert element
                let element = self.convert_element_definition(element_def)?;
                converted_elements.insert(element_def.path.clone(), element);

                // Convert constraints if present
                if let Some(constraints) = &element_def.constraint {
                    for constraint in constraints {
                        let fhir_constraint = fhirschema_core::Constraint {
                            key: constraint.key.clone(),
                            expression: constraint.expression.clone(),
                            human: Some(constraint.human.clone()),
                            severity: Some(constraint.severity.clone()),
                        };
                        converted_constraints.insert(constraint.key.clone(), fhir_constraint);
                    }
                }
            }

            if !converted_elements.is_empty() {
                schema.elements = Some(converted_elements);
            }

            if !converted_constraints.is_empty() {
                schema.constraints = Some(converted_constraints);
            }
        }

        Ok(schema)
    }

    /// Convert an ElementDefinition to a FHIRSchema Element.
    fn convert_element_definition(&self, element_def: &ElementDefinition) -> Result<fhirschema_core::Element> {
        let mut element = fhirschema_core::Element::new();

        // Set cardinality
        element.min = element_def.min;
        element.max = element_def.max.clone();

        // Set type information
        if let Some(types) = &element_def.r#type {
            if types.len() == 1 {
                element.element_type = Some(types[0].code.clone());

                // Handle reference targets
                if types[0].code == "Reference" {
                    if let Some(target_profiles) = &types[0].target_profile {
                        element.refers = Some(target_profiles.clone());
                    }
                }
            } else if types.len() > 1 {
                // Handle choice types
                element.choice_of = Some("type".to_string());
                let mut choices = HashMap::new();
                for (i, type_def) in types.iter().enumerate() {
                    let mut choice_element = fhirschema_core::Element::new();
                    choice_element.element_type = Some(type_def.code.clone());
                    choices.insert(format!("choice_{}", i), choice_element);
                }
                element.choices = Some(choices);
            }
        }

        // Set informational properties
        element.short = element_def.short.clone();
        element.definition = element_def.definition.clone();
        element.comment = element_def.comment.clone();
        element.requirements = element_def.requirements.clone();
        element.alias = element_def.alias.clone();

        // Set fixed/pattern values
        element.fixed = element_def.fixed.clone();
        element.pattern = element_def.pattern.clone();

        // Set examples
        element.example = element_def.example.clone();

        // Convert binding if present
        if let Some(binding) = &element_def.binding {
            let fhir_binding = fhirschema_core::Binding {
                value_set: binding.value_set.clone(),
                strength: Some(binding.strength.clone()),
                description: binding.description.clone(),
                code_systems: None,
                additional: None,
                extensions: None,
            };
            element.binding = Some(fhir_binding);
        }

        Ok(element)
    }

    /// Convert and store a StructureDefinition directly to a repository
    #[cfg(feature = "repository")]
    pub async fn convert_and_store<R>(
        &self,
        structure_definition_json: &str,
        repository: &R,
        version: Option<&fhirschema_repository::SchemaVersion>,
    ) -> Result<()>
    where
        R: fhirschema_repository::SchemaRepository + Send + Sync,
    {
        // Convert to FHIRSchema
        let schema = self.convert(structure_definition_json)?;

        // Convert to FhirSchema format for repository storage
        let fhir_schema = self.convert_schema_to_fhir_schema(schema)?;

        // Store in repository
        repository
            .store_schema(&fhir_schema.url.clone().unwrap_or_default(), &fhir_schema, version)
            .await
            .map_err(|e| Error::ConversionFailed(format!("Failed to store schema in repository: {}", e)))?;

        Ok(())
    }

    /// Convert multiple StructureDefinitions and store them in a repository
    #[cfg(feature = "repository")]
    pub async fn convert_batch_and_store<R>(
        &self,
        structure_definitions: Vec<&str>,
        repository: &R,
        version: Option<&fhirschema_repository::SchemaVersion>,
    ) -> Result<Vec<String>>
    where
        R: fhirschema_repository::SchemaRepository + Send + Sync,
    {
        let mut stored_urls = Vec::new();

        for structure_def_json in structure_definitions {
            match self.convert_and_store(structure_def_json, repository, version).await {
                Ok(()) => {
                    // Extract URL from the JSON to track successful storage
                    if let Ok(structure_def) = serde_json::from_str::<StructureDefinition>(structure_def_json) {
                        stored_urls.push(structure_def.url);
                    }
                }
                Err(e) => {
                    // Log error but continue with other schemas
                    eprintln!("Failed to convert and store schema: {}", e);
                }
            }
        }

        Ok(stored_urls)
    }

    /// Resolve base schemas from repository during conversion
    #[cfg(feature = "repository")]
    pub async fn convert_with_repository_resolution<R>(
        &self,
        structure_definition_json: &str,
        repository: &R,
    ) -> Result<Schema>
    where
        R: fhirschema_repository::SchemaRepository + Send + Sync,
    {
        // Parse the StructureDefinition JSON
        let structure_def: StructureDefinition = serde_json::from_str(structure_definition_json)
            .map_err(|e| Error::InvalidStructureDefinition(format!("Failed to parse JSON: {}", e)))?;

        // Convert to FHIRSchema
        let mut schema = self.convert_structure_definition(&structure_def)?;

        // If this schema has a base, try to resolve it from the repository
        if let Some(base_url) = &schema.base {
            match repository.get_schema(base_url, None).await {
                Ok(Some(base_fhir_schema)) => {
                    // Base schema found in repository - could be used for validation or inheritance
                    // For now, we just verify it exists
                    println!("Base schema found in repository: {}", base_url);
                }
                Ok(None) => {
                    println!("Base schema not found in repository: {}", base_url);
                }
                Err(e) => {
                    println!("Error accessing repository for base schema {}: {}", base_url, e);
                }
            }
        }

        Ok(schema)
    }

    /// Convert internal Schema to FhirSchema format for repository storage
    #[cfg(feature = "repository")]
    fn convert_schema_to_fhir_schema(&self, schema: Schema) -> Result<fhirschema_core::FhirSchema> {
        let mut fhir_schema = fhirschema_core::FhirSchema {
            url: Some(schema.url.clone()),
            name: Some(schema.name.clone()),
            description: None, // Could be extracted from schema if available
            derivation: Some(schema.derivation.clone()),
            base_definition: schema.base.clone(),
            elements: Vec::new(),
            ..Default::default()
        };

        // Convert elements
        if let Some(elements) = schema.elements {
            for (path, element) in elements {
                let element_def = fhirschema_core::ElementDefinition {
                    path: path.clone(),
                    element_type: element.element_type,
                    min: element.min,
                    max: element.max,
                    short: element.short,
                    definition: element.definition,
                    comment: element.comment,
                    requirements: element.requirements,
                    alias: element.alias,
                    fixed: element.fixed,
                    pattern: element.pattern,
                    example: element.example,
                    ..Default::default()
                };
                fhir_schema.elements.push(element_def);
            }
        }

        Ok(fhir_schema)
    }
}

impl Default for StructureDefinitionConverter {
    fn default() -> Self {
        Self::new()
    }
}
