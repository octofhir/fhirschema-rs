//! Schemata resolution for FHIRSchema validation
//!
//! This module implements the core "collect and follow" algorithm for FHIRSchema
//! validation, handling schema inheritance, reference resolution, and circular
//! reference detection.

use crate::{
    error::{ValidationError, ValidationResult},
    ValidationConfig,
};
use fhirschema_core::Schema;
use std::collections::{HashMap, HashSet};

#[cfg(feature = "repository")]
use fhirschema_repository::{SchemaRepository, SchemaVersion};

/// Schemata resolver for schema collection and following operations
pub struct SchemataResolver {
    /// Configuration for resolution behavior
    config: ValidationConfig,
    /// Cache for resolved schemata to improve performance
    cache: HashMap<String, Vec<String>>,
}

impl SchemataResolver {
    /// Create a new schemata resolver
    pub fn new(config: ValidationConfig) -> Self {
        Self {
            config,
            cache: HashMap::new(),
        }
    }

    /// Resolve schemata for a given schema (collect and follow operations)
    ///
    /// This implements the core FHIRSchema schemata resolution algorithm:
    /// 1. Collect: Gather all schemas in the inheritance chain
    /// 2. Follow: Resolve element references and type references
    pub fn resolve_schemata<'a>(
        &self,
        target_schema: &'a Schema,
        schema_repository: &'a HashMap<String, Schema>,
    ) -> ValidationResult<Vec<&'a Schema>> {
        let mut resolved_schemas = Vec::new();
        let mut visited = HashSet::new();
        let mut resolution_chain = Vec::new();

        // Start with the target schema
        self.collect_schemas(
            target_schema,
            schema_repository,
            &mut resolved_schemas,
            &mut visited,
            &mut resolution_chain,
        )?;

        // Follow element references and type references
        self.follow_references(&mut resolved_schemas, schema_repository)?;

        Ok(resolved_schemas)
    }

    /// Collect schemas in the inheritance chain (base schemas)
    fn collect_schemas<'a>(
        &self,
        schema: &'a Schema,
        repository: &'a HashMap<String, Schema>,
        resolved: &mut Vec<&'a Schema>,
        visited: &mut HashSet<String>,
        chain: &mut Vec<String>,
    ) -> ValidationResult<()> {
        // Check for circular references
        if visited.contains(&schema.url) {
            let chain_str = chain.join(" -> ");
            return Err(ValidationError::circular_reference(format!(
                "{} -> {}",
                chain_str, schema.url
            )));
        }

        // Check recursion depth
        if chain.len() >= self.config.max_recursion_depth {
            return Err(ValidationError::schema_resolution(format!(
                "Maximum recursion depth ({}) exceeded in schema chain: {}",
                self.config.max_recursion_depth,
                chain.join(" -> ")
            )));
        }

        // Mark as visited and add to chain
        visited.insert(schema.url.clone());
        chain.push(schema.url.clone());

        // Add current schema to resolved list
        resolved.push(schema);

        // Recursively collect base schemas
        if let Some(base_url) = &schema.base {
            if let Some(base_schema) = repository.get(base_url) {
                self.collect_schemas(base_schema, repository, resolved, visited, chain)?;
            } else {
                return Err(ValidationError::schema_not_found(base_url));
            }
        }

        // Remove from chain (backtrack)
        chain.pop();

        Ok(())
    }

    /// Follow element references and type references
    fn follow_references<'a>(
        &self,
        schemas: &mut Vec<&'a Schema>,
        repository: &'a HashMap<String, Schema>,
    ) -> ValidationResult<()> {
        let mut additional_schemas = Vec::new();

        for schema in schemas.iter() {
            if let Some(elements) = &schema.elements {
                for element in elements.values() {
                    // Follow element references
                    if let Some(element_ref) = &element.element_reference {
                        if let Some(referenced_schema) = repository.get(element_ref) {
                            // Check if we already have this schema
                            if !schemas.iter().any(|s| s.url == referenced_schema.url) {
                                additional_schemas.push(referenced_schema);
                            }
                        } else {
                            return Err(ValidationError::schema_not_found(element_ref));
                        }
                    }

                    // Follow type references
                    if let Some(element_type) = element.get_element_type() {
                        match element_type {
                            fhirschema_core::ElementType::Simple(type_name) => {
                                // Look for a schema that defines this type
                                if let Some(type_schema) = self.find_schema_by_type(&type_name, repository) {
                                    if !schemas.iter().any(|s| s.url == type_schema.url) {
                                        additional_schemas.push(type_schema);
                                    }
                                }
                            }
                            fhirschema_core::ElementType::Choice(choices) => {
                                // Follow each choice type
                                for (choice_name, _choice_element) in choices {
                                    if let Some(type_schema) = self.find_schema_by_type(&choice_name, repository) {
                                        if !schemas.iter().any(|s| s.url == type_schema.url) {
                                            additional_schemas.push(type_schema);
                                        }
                                    }
                                }
                            }
                            fhirschema_core::ElementType::Complex(_) => {
                                // Complex types don't need additional schema resolution
                            }
                            fhirschema_core::ElementType::Reference(_) => {
                                // Reference types don't need additional schema resolution
                            }
                        }
                    }
                }
            }
        }

        // Add additional schemas found through references
        schemas.extend(additional_schemas);

        Ok(())
    }

    /// Find a schema by its type name
    fn find_schema_by_type<'a>(
        &self,
        type_name: &str,
        repository: &'a HashMap<String, Schema>,
    ) -> Option<&'a Schema> {
        // Look for schemas where the type matches
        repository.values().find(|schema| schema.schema_type == type_name)
    }

    /// Resolve element path within schemata
    pub fn resolve_element_path<'a>(
        &self,
        path: &str,
        schemata: &[&'a Schema],
    ) -> ValidationResult<Option<&'a fhirschema_core::Element>> {
        // Try to find the element in each schema
        for schema in schemata {
            if let Some(elements) = &schema.elements {
                if let Some(element) = elements.get(path) {
                    return Ok(Some(element));
                }
            }
        }

        Ok(None)
    }

    /// Get all element paths from schemata
    pub fn get_all_element_paths(&self, schemata: &[&Schema]) -> Vec<String> {
        let mut paths = HashSet::new();

        for schema in schemata {
            if let Some(elements) = &schema.elements {
                for path in elements.keys() {
                    paths.insert(path.clone());
                }
            }
        }

        paths.into_iter().collect()
    }

    /// Check if a schema is a constraint (derivation = constraint)
    pub fn is_constraint_schema(&self, schema: &Schema) -> bool {
        schema.derivation == "constraint"
    }

    /// Check if a schema is a specialization (derivation = specialization)
    pub fn is_specialization_schema(&self, schema: &Schema) -> bool {
        schema.derivation == "specialization"
    }

    /// Get the base schema chain for a given schema
    pub fn get_base_chain<'a>(
        &self,
        schema: &'a Schema,
        repository: &'a HashMap<String, Schema>,
    ) -> ValidationResult<Vec<&'a Schema>> {
        let mut chain = Vec::new();
        let mut current = schema;
        let mut visited = HashSet::new();

        while let Some(base_url) = &current.base {
            if visited.contains(base_url) {
                return Err(ValidationError::circular_reference(format!(
                    "Circular base reference detected: {}",
                    base_url
                )));
            }

            visited.insert(base_url.clone());

            if let Some(base_schema) = repository.get(base_url) {
                chain.push(base_schema);
                current = base_schema;
            } else {
                return Err(ValidationError::schema_not_found(base_url));
            }

            // Prevent infinite loops
            if chain.len() > self.config.max_recursion_depth {
                return Err(ValidationError::schema_resolution(format!(
                    "Maximum recursion depth exceeded in base chain for schema: {}",
                    schema.url
                )));
            }
        }

        Ok(chain)
    }

    /// Clear the resolution cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.cache.len(), self.cache.capacity())
    }

    /// Resolve schemata using a repository (async version)
    #[cfg(feature = "repository")]
    pub async fn resolve_schemata_from_repository<R>(
        &self,
        target_schema_url: &str,
        repository: &R,
        version: Option<&SchemaVersion>,
    ) -> ValidationResult<Vec<Schema>>
    where
        R: SchemaRepository + Send + Sync,
    {
        let mut resolved_schemas = Vec::new();
        let mut visited = HashSet::new();
        let mut resolution_chain = Vec::new();

        // Load the target schema from repository
        let target_schema = repository
            .get_schema(target_schema_url, version)
            .await
            .map_err(|e| ValidationError::schema_resolution(format!("Failed to load schema '{}': {}", target_schema_url, e)))?
            .ok_or_else(|| ValidationError::schema_not_found(target_schema_url))?;

        // Convert to fhirschema_core::Schema if needed
        let target_core_schema = self.convert_to_core_schema(target_schema)?;

        // Start collection process
        self.collect_schemas_from_repository(
            &target_core_schema,
            repository,
            &mut resolved_schemas,
            &mut visited,
            &mut resolution_chain,
        ).await?;

        // Follow references
        self.follow_references_from_repository(&mut resolved_schemas, repository).await?;

        Ok(resolved_schemas)
    }

    /// Collect schemas from repository (async version)
    #[cfg(feature = "repository")]
    async fn collect_schemas_from_repository<R>(
        &self,
        schema: &Schema,
        repository: &R,
        resolved: &mut Vec<Schema>,
        visited: &mut HashSet<String>,
        chain: &mut Vec<String>,
    ) -> ValidationResult<()>
    where
        R: SchemaRepository + Send + Sync,
    {
        // Check for circular references
        if visited.contains(&schema.url) {
            let chain_str = chain.join(" -> ");
            return Err(ValidationError::circular_reference(format!(
                "{} -> {}",
                chain_str, schema.url
            )));
        }

        // Check recursion depth
        if chain.len() >= self.config.max_recursion_depth {
            return Err(ValidationError::schema_resolution(format!(
                "Maximum recursion depth ({}) exceeded in schema chain: {}",
                self.config.max_recursion_depth,
                chain.join(" -> ")
            )));
        }

        // Mark as visited and add to chain
        visited.insert(schema.url.clone());
        chain.push(schema.url.clone());

        // Add current schema to resolved list
        resolved.push(schema.clone());

        // Recursively collect base schemas
        if let Some(base_url) = &schema.base {
            let base_fhir_schema = repository
                .get_schema(base_url, None)
                .await
                .map_err(|e| ValidationError::schema_resolution(format!("Failed to load base schema '{}': {}", base_url, e)))?
                .ok_or_else(|| ValidationError::schema_not_found(base_url))?;

            let base_schema = self.convert_to_core_schema(base_fhir_schema)?;
            self.collect_schemas_from_repository(&base_schema, repository, resolved, visited, chain).await?;
        }

        // Remove from chain (backtrack)
        chain.pop();

        Ok(())
    }

    /// Follow references using repository (async version)
    #[cfg(feature = "repository")]
    async fn follow_references_from_repository<R>(
        &self,
        schemas: &mut Vec<Schema>,
        repository: &R,
    ) -> ValidationResult<()>
    where
        R: SchemaRepository + Send + Sync,
    {
        let mut additional_schemas = Vec::new();

        for schema in schemas.iter() {
            if let Some(elements) = &schema.elements {
                for element in elements.values() {
                    // Follow element references
                    if let Some(element_ref) = &element.element_reference {
                        let referenced_fhir_schema = repository
                            .get_schema(element_ref, None)
                            .await
                            .map_err(|e| ValidationError::schema_resolution(format!("Failed to load referenced schema '{}': {}", element_ref, e)))?
                            .ok_or_else(|| ValidationError::schema_not_found(element_ref))?;

                        let referenced_schema = self.convert_to_core_schema(referenced_fhir_schema)?;

                        // Check if we already have this schema
                        if !schemas.iter().any(|s| s.url == referenced_schema.url) {
                            additional_schemas.push(referenced_schema);
                        }
                    }

                    // Follow type references
                    if let Some(element_type) = element.get_element_type() {
                        match element_type {
                            fhirschema_core::ElementType::Simple(type_name) => {
                                // Look for a schema that defines this type
                                if let Some(type_schema) = self.find_schema_by_type_from_repository(&type_name, repository).await? {
                                    if !schemas.iter().any(|s| s.url == type_schema.url) {
                                        additional_schemas.push(type_schema);
                                    }
                                }
                            }
                            fhirschema_core::ElementType::Choice(choices) => {
                                // Follow each choice type
                                for (choice_name, _choice_element) in choices {
                                    if let Some(type_schema) = self.find_schema_by_type_from_repository(&choice_name, repository).await? {
                                        if !schemas.iter().any(|s| s.url == type_schema.url) {
                                            additional_schemas.push(type_schema);
                                        }
                                    }
                                }
                            }
                            fhirschema_core::ElementType::Complex(_) => {
                                // Complex types don't need additional schema resolution
                            }
                            fhirschema_core::ElementType::Reference(_) => {
                                // Reference types don't need additional schema resolution
                            }
                        }
                    }
                }
            }
        }

        // Add additional schemas found through references
        schemas.extend(additional_schemas);

        Ok(())
    }

    /// Find a schema by type name from repository
    #[cfg(feature = "repository")]
    async fn find_schema_by_type_from_repository<R>(
        &self,
        type_name: &str,
        repository: &R,
    ) -> ValidationResult<Option<Schema>>
    where
        R: SchemaRepository + Send + Sync,
    {
        // List all schemas and find one that matches the type
        let schemas = repository
            .list_schemas(None)
            .await
            .map_err(|e| ValidationError::schema_resolution(format!("Failed to list schemas: {}", e)))?;

        for schema_metadata in schemas {
            if let Some(fhir_schema) = repository
                .get_schema(&schema_metadata.url, None)
                .await
                .map_err(|e| ValidationError::schema_resolution(format!("Failed to load schema '{}': {}", schema_metadata.url, e)))?
            {
                let core_schema = self.convert_to_core_schema(fhir_schema)?;
                if core_schema.schema_type == type_name {
                    return Ok(Some(core_schema));
                }
            }
        }

        Ok(None)
    }

    /// Convert FhirSchema to core Schema
    #[cfg(feature = "repository")]
    fn convert_to_core_schema(&self, fhir_schema: fhirschema_core::FhirSchema) -> ValidationResult<Schema> {
        // Convert FhirSchema to the core Schema type used by the validator
        // This is a simplified conversion - in practice, you might need more sophisticated mapping
        Ok(Schema {
            url: fhir_schema.url.unwrap_or_default(),
            schema_type: fhir_schema.name.clone().unwrap_or_default(),
            name: fhir_schema.name.unwrap_or_default(),
            derivation: fhir_schema.derivation.unwrap_or_else(|| "specialization".to_string()),
            base: fhir_schema.base_definition,
            elements: fhir_schema.elements.into_iter().map(|elem| {
                (elem.path.clone(), fhirschema_core::Element {
                    element_type: elem.element_type,
                    min: elem.min,
                    max: elem.max,
                    element_reference: None, // Would need proper mapping
                    ..Default::default()
                })
            }).collect::<HashMap<_, _>>().into(),
            constraints: None, // Would need proper mapping from fhir_schema constraints
            extensions: None,
            additional_properties: None,
            any: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fhirschema_core::{Element, ElementType};
    use std::collections::HashMap;

    fn create_base_schema() -> Schema {
        Schema {
            url: "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
            schema_type: "Patient".to_string(),
            name: "Patient".to_string(),
            derivation: "specialization".to_string(),
            base: None,
            elements: Some({
                let mut elements = HashMap::new();
                elements.insert(
                    "Patient.id".to_string(),
                    Element {
                        element_type: Some("id".to_string()),
                        min: Some(0),
                        max: Some("1".to_string()),
                        ..Default::default()
                    },
                );
                elements
            }),
            constraints: None,
            extensions: None,
            additional_properties: None,
            any: None,
        }
    }

    fn create_constraint_schema() -> Schema {
        Schema {
            url: "http://example.org/StructureDefinition/test-patient".to_string(),
            schema_type: "Patient".to_string(),
            name: "TestPatient".to_string(),
            derivation: "constraint".to_string(),
            base: Some("http://hl7.org/fhir/StructureDefinition/Patient".to_string()),
            elements: Some({
                let mut elements = HashMap::new();
                elements.insert(
                    "Patient.name".to_string(),
                    Element {
                        element_type: Some("HumanName".to_string()),
                        min: Some(1),
                        max: Some("*".to_string()),
                        ..Default::default()
                    },
                );
                elements
            }),
            constraints: None,
            extensions: None,
            additional_properties: None,
            any: None,
        }
    }

    fn create_repository() -> HashMap<String, Schema> {
        let mut repo = HashMap::new();
        let base = create_base_schema();
        let constraint = create_constraint_schema();

        repo.insert(base.url.clone(), base);
        repo.insert(constraint.url.clone(), constraint);
        repo
    }

    #[test]
    fn test_resolver_creation() {
        let config = ValidationConfig::default();
        let resolver = SchemataResolver::new(config);
        assert_eq!(resolver.cache.len(), 0);
    }

    #[test]
    fn test_resolve_simple_schema() {
        let config = ValidationConfig::default();
        let resolver = SchemataResolver::new(config);
        let repo = create_repository();
        let base_schema = repo.get("http://hl7.org/fhir/StructureDefinition/Patient").unwrap();

        let result = resolver.resolve_schemata(base_schema, &repo).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].url, base_schema.url);
    }

    #[test]
    fn test_resolve_constraint_schema() {
        let config = ValidationConfig::default();
        let resolver = SchemataResolver::new(config);
        let repo = create_repository();
        let constraint_schema = repo.get("http://example.org/StructureDefinition/test-patient").unwrap();

        let result = resolver.resolve_schemata(constraint_schema, &repo).unwrap();
        assert_eq!(result.len(), 2); // constraint + base
        assert_eq!(result[0].url, constraint_schema.url);
        assert_eq!(result[1].url, "http://hl7.org/fhir/StructureDefinition/Patient");
    }

    #[test]
    fn test_missing_base_schema() {
        let config = ValidationConfig::default();
        let resolver = SchemataResolver::new(config);
        let mut repo = HashMap::new();

        // Add constraint schema without its base
        let constraint = create_constraint_schema();
        repo.insert(constraint.url.clone(), constraint.clone());

        let result = resolver.resolve_schemata(&constraint, &repo);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::SchemaNotFound { .. }));
    }

    #[test]
    fn test_circular_reference_detection() {
        let config = ValidationConfig::default();
        let resolver = SchemataResolver::new(config);
        let mut repo = HashMap::new();

        // Create circular reference: A -> B -> A
        let mut schema_a = create_base_schema();
        schema_a.url = "http://example.com/A".to_string();
        schema_a.base = Some("http://example.com/B".to_string());

        let mut schema_b = create_base_schema();
        schema_b.url = "http://example.com/B".to_string();
        schema_b.base = Some("http://example.com/A".to_string());

        repo.insert(schema_a.url.clone(), schema_a.clone());
        repo.insert(schema_b.url.clone(), schema_b);

        let result = resolver.resolve_schemata(&schema_a, &repo);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::CircularReference { .. }));
    }

    #[test]
    fn test_is_constraint_schema() {
        let config = ValidationConfig::default();
        let resolver = SchemataResolver::new(config);
        let constraint = create_constraint_schema();
        let base = create_base_schema();

        assert!(resolver.is_constraint_schema(&constraint));
        assert!(!resolver.is_constraint_schema(&base));
    }

    #[test]
    fn test_is_specialization_schema() {
        let config = ValidationConfig::default();
        let resolver = SchemataResolver::new(config);
        let constraint = create_constraint_schema();
        let base = create_base_schema();

        assert!(!resolver.is_specialization_schema(&constraint));
        assert!(resolver.is_specialization_schema(&base));
    }

    #[test]
    fn test_get_base_chain() {
        let config = ValidationConfig::default();
        let resolver = SchemataResolver::new(config);
        let repo = create_repository();
        let constraint_schema = repo.get("http://example.org/StructureDefinition/test-patient").unwrap();

        let chain = resolver.get_base_chain(constraint_schema, &repo).unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].url, "http://hl7.org/fhir/StructureDefinition/Patient");
    }

    #[test]
    fn test_resolve_element_path() {
        let config = ValidationConfig::default();
        let resolver = SchemataResolver::new(config);
        let repo = create_repository();
        let constraint_schema = repo.get("http://example.org/StructureDefinition/test-patient").unwrap();
        let schemata = resolver.resolve_schemata(constraint_schema, &repo).unwrap();

        // Should find element in constraint schema
        let element = resolver.resolve_element_path("Patient.name", &schemata).unwrap();
        assert!(element.is_some());

        // Should find element in base schema
        let element = resolver.resolve_element_path("Patient.id", &schemata).unwrap();
        assert!(element.is_some());

        // Should not find non-existent element
        let element = resolver.resolve_element_path("Patient.nonexistent", &schemata).unwrap();
        assert!(element.is_none());
    }
}
