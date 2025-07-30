//! Main validation engine for FHIRSchema validation

use crate::{
    error::{ValidationError, ValidationResult},
    schemata::SchemataResolver,
    element::ElementValidator,
    primitive::PrimitiveValidator,
    constraint::ConstraintEvaluator,
    slicing::SlicingValidator,
    context::FHIRPathContext,
    ValidationConfig, ValidationOutcome, ValidationIssue, ValidationStats, Severity,
};
use fhirschema_core::Schema;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;

/// Main validation engine for FHIRSchema validation
pub struct Validator {
    /// Validation configuration
    config: ValidationConfig,
    /// Schema repository for resolving schemas
    schemas: HashMap<String, Schema>,
    /// Schemata resolver for schema collection and following
    schemata_resolver: SchemataResolver,
    /// Element validator for individual element validation
    element_validator: ElementValidator,
    /// Primitive validator for FHIR primitive types
    primitive_validator: PrimitiveValidator,
    /// Constraint evaluator for FHIRPath constraints
    constraint_evaluator: ConstraintEvaluator,
    /// Slicing validator for array slicing
    slicing_validator: SlicingValidator,
}

impl Validator {
    /// Create a new validator with default configuration
    pub fn new() -> Self {
        Self::with_config(ValidationConfig::default())
    }

    /// Create a new validator with custom configuration
    pub fn with_config(config: ValidationConfig) -> Self {
        Self {
            config: config.clone(),
            schemas: HashMap::new(),
            schemata_resolver: SchemataResolver::new(config.clone()),
            element_validator: ElementValidator::new(config.clone()),
            primitive_validator: PrimitiveValidator::new(),
            constraint_evaluator: ConstraintEvaluator::new(),
            slicing_validator: SlicingValidator::new(),
        }
    }

    /// Add a schema to the validator's repository
    pub fn add_schema(&mut self, schema: Schema) -> ValidationResult<()> {
        let url = schema.url.clone();
        self.schemas.insert(url, schema);
        Ok(())
    }

    /// Add multiple schemas to the validator's repository
    pub fn add_schemas(&mut self, schemas: Vec<Schema>) -> ValidationResult<()> {
        for schema in schemas {
            self.add_schema(schema)?;
        }
        Ok(())
    }

    /// Get a schema by URL
    pub fn get_schema(&self, url: &str) -> Option<&Schema> {
        self.schemas.get(url)
    }

    /// Validate a FHIR resource against a schema
    pub fn validate(&self, resource: &Value, schema_url: &str) -> ValidationResult<ValidationOutcome> {
        let start_time = Instant::now();
        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        // Get the target schema
        let schema = self.get_schema(schema_url)
            .ok_or_else(|| ValidationError::schema_not_found(schema_url))?;

        // Resolve schemata (collect and follow operations)
        let schemata = self.schemata_resolver.resolve_schemata(schema, &self.schemas)?;

        // Create FHIRPath context for constraint evaluation
        let context = FHIRPathContext::new(resource, resource, resource);

        // Validate the resource against resolved schemata
        self.validate_against_schemata(resource, &schemata, &context, "", &mut issues, &mut stats)?;

        // Calculate validation duration
        stats.duration_ms = start_time.elapsed().as_millis() as u64;

        // Determine overall success
        let success = !issues.iter().any(|issue| issue.severity == Severity::Error);

        Ok(ValidationOutcome {
            success,
            issues,
            stats,
        })
    }

    /// Validate a resource against multiple resolved schemata
    fn validate_against_schemata(
        &self,
        resource: &Value,
        schemata: &[&Schema],
        context: &FHIRPathContext,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
        stats: &mut ValidationStats,
    ) -> ValidationResult<()> {
        // Validate resource structure
        self.validate_resource_structure(resource, path, issues)?;

        // Validate against each schema in the schemata
        for schema in schemata {
            self.validate_against_schema(resource, schema, context, path, issues, stats)?;
        }

        Ok(())
    }

    /// Validate a resource against a single schema
    fn validate_against_schema(
        &self,
        resource: &Value,
        schema: &Schema,
        context: &FHIRPathContext,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
        stats: &mut ValidationStats,
    ) -> ValidationResult<()> {
        // Validate resource type matches schema type
        if let Some(resource_type) = resource.get("resourceType").and_then(|v| v.as_str()) {
            if resource_type != schema.schema_type {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "type-mismatch".to_string(),
                    message: format!("Resource type '{}' does not match schema type '{}'",
                                   resource_type, schema.schema_type),
                    location: path.to_string(),
                    context: None,
                });
                return Ok(());
            }
        }

        // Validate elements if present
        if let Some(elements) = &schema.elements {
            for (element_path, element) in elements {
                let full_path = if path.is_empty() {
                    element_path.clone()
                } else {
                    format!("{}.{}", path, element_path)
                };

                // Extract the value at this path from the resource
                let value = self.extract_value_at_path(resource, element_path);

                // Validate the element
                if self.config.enable_primitives {
                    self.element_validator.validate_element(
                        &value, element, &full_path, issues, stats
                    )?;
                }

                // Validate primitive types
                if self.config.enable_primitives {
                    self.primitive_validator.validate_primitive(
                        &value, element, &full_path, issues, stats
                    )?;
                }

                // Validate constraints
                if self.config.enable_constraints {
                    if let Some(constraints) = &element.constraints {
                        for constraint in constraints.values() {
                            self.constraint_evaluator.evaluate_constraint(
                                constraint, context, &full_path, issues, stats
                            )?;
                        }
                    }
                }

                // Validate slicing if present
                if self.config.enable_slicing {
                    if let Some(slicing) = &element.slicing {
                        self.slicing_validator.validate_slicing(
                            &value, slicing, &full_path, issues, stats
                        )?;
                    }
                }

                stats.elements_validated += 1;
            }
        }

        Ok(())
    }

    /// Validate basic resource structure
    fn validate_resource_structure(
        &self,
        resource: &Value,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
    ) -> ValidationResult<()> {
        // Ensure resource is an object
        if !resource.is_object() {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "invalid-structure".to_string(),
                message: "Resource must be a JSON object".to_string(),
                location: path.to_string(),
                context: None,
            });
            return Ok(());
        }

        // Ensure resourceType is present for root resources
        if path.is_empty() && !resource.get("resourceType").is_some() {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "missing-resource-type".to_string(),
                message: "Resource must have a 'resourceType' property".to_string(),
                location: path.to_string(),
                context: None,
            });
        }

        Ok(())
    }

    /// Extract value at a given path from the resource
    fn extract_value_at_path(&self, resource: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = resource;

        for part in parts.iter().skip(1) { // Skip the resource type part
            match current.get(part) {
                Some(value) => current = value,
                None => return None,
            }
        }

        Some(current.clone())
    }

    /// Get validation statistics
    pub fn get_stats(&self) -> ValidationStats {
        ValidationStats::default()
    }

    /// Clear all schemas from the repository
    pub fn clear_schemas(&mut self) {
        self.schemas.clear();
    }

    /// Get the number of schemas in the repository
    pub fn schema_count(&self) -> usize {
        self.schemas.len()
    }

    /// List all schema URLs in the repository
    pub fn list_schema_urls(&self) -> Vec<String> {
        self.schemas.keys().cloned().collect()
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fhirschema_core::{Element, ElementType};
    use serde_json::json;

    fn create_test_schema() -> Schema {
        let mut elements = HashMap::new();
        elements.insert(
            "Patient.name".to_string(),
            Element {
                element_type: Some("HumanName".to_string()),
                min: Some(1),
                max: Some("*".to_string()),
                short: Some("Patient name".to_string()),
                definition: Some("The name of the patient".to_string()),
                ..Default::default()
            },
        );

        Schema {
            url: "http://example.org/StructureDefinition/test-patient".to_string(),
            schema_type: "Patient".to_string(),
            name: "TestPatient".to_string(),
            derivation: "constraint".to_string(),
            base: Some("http://hl7.org/fhir/StructureDefinition/Patient".to_string()),
            elements: Some(elements),
            constraints: None,
            extensions: None,
            additional_properties: None,
            any: None,
        }
    }

    fn create_test_resource() -> Value {
        json!({
            "resourceType": "Patient",
            "name": [{
                "family": "Doe",
                "given": ["John"]
            }]
        })
    }

    #[test]
    fn test_validator_creation() {
        let validator = Validator::new();
        assert_eq!(validator.schema_count(), 0);
    }

    #[test]
    fn test_add_schema() {
        let mut validator = Validator::new();
        let schema = create_test_schema();
        let url = schema.url.clone();

        validator.add_schema(schema).unwrap();
        assert_eq!(validator.schema_count(), 1);
        assert!(validator.get_schema(&url).is_some());
    }

    #[test]
    fn test_validate_missing_schema() {
        let validator = Validator::new();
        let resource = create_test_resource();

        let result = validator.validate(&resource, "http://nonexistent.com/schema");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::SchemaNotFound { .. }));
    }

    #[test]
    fn test_validate_basic_resource() {
        let mut validator = Validator::new();
        let schema = create_test_schema();
        let url = schema.url.clone();
        validator.add_schema(schema).unwrap();

        let resource = create_test_resource();
        let result = validator.validate(&resource, &url).unwrap();

        // Should succeed with basic validation
        assert!(result.success || result.issues.iter().all(|i| i.severity != Severity::Error));
    }

    #[test]
    fn test_validate_wrong_resource_type() {
        let mut validator = Validator::new();
        let schema = create_test_schema();
        let url = schema.url.clone();
        validator.add_schema(schema).unwrap();

        let resource = json!({
            "resourceType": "Observation",
            "status": "final"
        });

        let result = validator.validate(&resource, &url).unwrap();
        assert!(!result.success);
        assert!(result.issues.iter().any(|i| i.code == "type-mismatch"));
    }

    #[test]
    fn test_clear_schemas() {
        let mut validator = Validator::new();
        validator.add_schema(create_test_schema()).unwrap();
        assert_eq!(validator.schema_count(), 1);

        validator.clear_schemas();
        assert_eq!(validator.schema_count(), 0);
    }
}
