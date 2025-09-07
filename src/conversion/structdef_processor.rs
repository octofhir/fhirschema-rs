// StructureDefinition processing with comprehensive metadata, elements, and constraints handling

use crate::conversion::{ConstraintMapper, ElementConverter};
use crate::error::Result;
use crate::types::{ConstraintSeverity, FhirConstraint, FhirSchemaProperty};
use serde_json::Value;
use std::collections::HashMap;

pub struct StructDefProcessor {
    element_converter: ElementConverter,
    constraint_mapper: ConstraintMapper,
}

impl StructDefProcessor {
    pub fn new() -> Self {
        Self {
            element_converter: ElementConverter::new(),
            constraint_mapper: ConstraintMapper::new(),
        }
    }

    /// Process metadata from StructureDefinition into schema metadata
    pub async fn process_metadata(&self, structure_def: &Value) -> Result<HashMap<String, Value>> {
        let mut metadata = HashMap::new();

        // Basic StructureDefinition metadata
        if let Some(url) = structure_def.get("url") {
            metadata.insert("fhir_url".to_string(), url.clone());
        }

        if let Some(version) = structure_def.get("version") {
            metadata.insert("fhir_version".to_string(), version.clone());
        }

        if let Some(name) = structure_def.get("name") {
            metadata.insert("fhir_name".to_string(), name.clone());
        }

        if let Some(status) = structure_def.get("status") {
            metadata.insert("fhir_status".to_string(), status.clone());
        }

        if let Some(kind) = structure_def.get("kind") {
            metadata.insert("fhir_kind".to_string(), kind.clone());
        }

        if let Some(abstract_flag) = structure_def.get("abstract") {
            metadata.insert("fhir_abstract".to_string(), abstract_flag.clone());
        }

        if let Some(base_definition) = structure_def.get("baseDefinition") {
            metadata.insert("fhir_base_definition".to_string(), base_definition.clone());
        }

        if let Some(derivation) = structure_def.get("derivation") {
            metadata.insert("fhir_derivation".to_string(), derivation.clone());
        }

        // Context information
        if let Some(context) = structure_def.get("context") {
            metadata.insert("fhir_context".to_string(), context.clone());
        }

        // Publisher and contact information
        if let Some(publisher) = structure_def.get("publisher") {
            metadata.insert("fhir_publisher".to_string(), publisher.clone());
        }

        if let Some(contact) = structure_def.get("contact") {
            metadata.insert("fhir_contact".to_string(), contact.clone());
        }

        // Jurisdiction and copyright
        if let Some(jurisdiction) = structure_def.get("jurisdiction") {
            metadata.insert("fhir_jurisdiction".to_string(), jurisdiction.clone());
        }

        if let Some(copyright) = structure_def.get("copyright") {
            metadata.insert("fhir_copyright".to_string(), copyright.clone());
        }

        // Keywords and experimental flag
        if let Some(keyword) = structure_def.get("keyword") {
            metadata.insert("fhir_keyword".to_string(), keyword.clone());
        }

        if let Some(experimental) = structure_def.get("experimental") {
            metadata.insert("fhir_experimental".to_string(), experimental.clone());
        }

        // Date information
        if let Some(date) = structure_def.get("date") {
            metadata.insert("fhir_date".to_string(), date.clone());
        }

        // Add processing timestamp
        metadata.insert(
            "processed_at".to_string(),
            Value::String(chrono::Utc::now().to_rfc3339()),
        );

        Ok(metadata)
    }

    /// Process differential and snapshot elements into schema properties
    pub async fn process_elements(
        &self,
        structure_def: &Value,
    ) -> Result<HashMap<String, FhirSchemaProperty>> {
        let mut properties = HashMap::new();

        // Process differential elements first
        if let Some(differential) = structure_def.get("differential") {
            if let Some(elements) = differential.get("element").and_then(|e| e.as_array()) {
                for element in elements {
                    if let Some(path) = element.get("path").and_then(|p| p.as_str()) {
                        match self.element_converter.convert_element(element, path).await {
                            Ok(property) => {
                                properties.insert(self.sanitize_property_name(path), property);
                            }
                            Err(e) => {
                                // Log error but continue processing
                                eprintln!("Failed to convert element at path {path}: {e}");
                            }
                        }
                    }
                }
            }
        }

        // Process snapshot elements if differential is empty
        if properties.is_empty() {
            if let Some(snapshot) = structure_def.get("snapshot") {
                if let Some(elements) = snapshot.get("element").and_then(|e| e.as_array()) {
                    for element in elements {
                        if let Some(path) = element.get("path").and_then(|p| p.as_str()) {
                            match self.element_converter.convert_element(element, path).await {
                                Ok(property) => {
                                    properties.insert(self.sanitize_property_name(path), property);
                                }
                                Err(e) => {
                                    // Log error but continue processing
                                    eprintln!("Failed to convert element at path {path}: {e}");
                                }
                            }
                        }
                    }
                }
            }
        }

        // If we still have no properties, create a basic structure
        if properties.is_empty() {
            // Add basic resourceType property for FHIR resources
            if let Some(kind) = structure_def.get("kind").and_then(|k| k.as_str()) {
                if kind == "resource" {
                    properties.insert(
                        "resourceType".to_string(),
                        FhirSchemaProperty::string().with_description("The type of the resource"),
                    );
                }
            }

            // Add id property
            properties.insert(
                "id".to_string(),
                FhirSchemaProperty::string()
                    .with_description("Logical id of this artifact")
                    .with_pattern("^[A-Za-z0-9\\-\\.]{1,64}$"),
            );

            // Add meta property
            properties.insert(
                "meta".to_string(),
                FhirSchemaProperty::reference("#/definitions/Meta")
                    .with_description("Metadata about the resource"),
            );
        }

        Ok(properties)
    }

    /// Process constraints from the StructureDefinition
    pub async fn process_constraints(&self, structure_def: &Value) -> Result<Vec<FhirConstraint>> {
        let mut constraints = Vec::new();

        // Process differential constraints
        if let Some(differential) = structure_def.get("differential") {
            if let Some(elements) = differential.get("element").and_then(|e| e.as_array()) {
                for element in elements {
                    if let Some(element_constraints) =
                        element.get("constraint").and_then(|c| c.as_array())
                    {
                        for constraint in element_constraints {
                            match self.constraint_mapper.convert_constraint(constraint).await {
                                Ok(fhir_constraint) => constraints.push(fhir_constraint),
                                Err(e) => {
                                    eprintln!("Failed to convert constraint: {e}");
                                }
                            }
                        }
                    }
                }
            }
        }

        // Process snapshot constraints if differential has none
        if constraints.is_empty() {
            if let Some(snapshot) = structure_def.get("snapshot") {
                if let Some(elements) = snapshot.get("element").and_then(|e| e.as_array()) {
                    for element in elements {
                        if let Some(element_constraints) =
                            element.get("constraint").and_then(|c| c.as_array())
                        {
                            for constraint in element_constraints {
                                match self.constraint_mapper.convert_constraint(constraint).await {
                                    Ok(fhir_constraint) => constraints.push(fhir_constraint),
                                    Err(e) => {
                                        eprintln!("Failed to convert constraint: {e}");
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Add global constraints if this is a resource
        if let Some(kind) = structure_def.get("kind").and_then(|k| k.as_str()) {
            if kind == "resource" {
                // Add resourceType constraint
                constraints.push(
                    FhirConstraint::new(
                        "global-1",
                        ConstraintSeverity::Error,
                        "All FHIR elements must have a @value or children",
                    )
                    .with_expression("hasValue() or (children().count() > id.count())"),
                );

                // Add id constraint if not already present
                if !constraints.iter().any(|c| c.key == "id-1") {
                    constraints.push(
                        FhirConstraint::new("id-1", ConstraintSeverity::Error, "id must be valid")
                            .with_expression("matches('^[A-Za-z0-9\\\\-\\\\.]{1,64}$')"),
                    );
                }
            }
        }

        Ok(constraints)
    }

    /// Sanitize property names for JSON Schema compatibility
    fn sanitize_property_name(&self, path: &str) -> String {
        // Remove resource type prefix and convert to camelCase
        let parts: Vec<&str> = path.split('.').collect();

        if parts.len() > 1 {
            // Skip the first part (resource type) and join the rest
            parts[1..].join(".")
        } else {
            path.to_string()
        }
        .replace("[x]", "")
        .replace("-", "_")
    }
}

impl Default for StructDefProcessor {
    fn default() -> Self {
        Self::new()
    }
}
