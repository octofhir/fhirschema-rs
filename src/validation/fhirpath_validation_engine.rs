// Copyright 2024 OctoFHIR Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Enhanced validation engine with FHIRPath constraint evaluation support
//!
//! This validation engine can accept an optional FHIRPath engine via dependency injection
//! to evaluate complex constraint expressions, avoiding circular dependencies between
//! fhirschema and fhirpath libraries.

use crate::validation::{ValidationContext, ValidationEngine, ValidationResult};
use crate::{Constraint, FhirSchema, Result};
use octofhir_fhir_model::constraints::ConstraintSeverity;
use octofhir_fhir_model::{
    ConstraintInfo, FhirPathEngine, FhirPathEngineCapabilities, FhirPathEvaluationConfig,
    FhirPathEvaluationContext,
};
use serde_json::Value;
use std::sync::Arc;

/// Validation engine with optional FHIRPath constraint evaluation
#[derive(Debug)]
pub struct FhirPathValidationEngine {
    /// Base validation engine for schema validation
    base_engine: crate::validation::FhirSchemaValidationEngine,
    /// Optional FHIRPath engine for constraint evaluation
    fhirpath_engine: Option<Arc<dyn FhirPathEngine>>,
    /// Configuration for FHIRPath evaluation
    fhirpath_config: FhirPathEvaluationConfig,
    /// Whether to use FHIRPath engine for all constraints or only complex ones
    use_fhirpath_for_all: bool,
}

impl FhirPathValidationEngine {
    /// Create a new enhanced validation engine without FHIRPath support
    pub fn new() -> Self {
        Self {
            base_engine: crate::validation::FhirSchemaValidationEngine::new(),
            fhirpath_engine: None,
            fhirpath_config: FhirPathEvaluationConfig::default(),
            use_fhirpath_for_all: false,
        }
    }

    /// Create a new enhanced validation engine with strict mode
    pub fn new_strict() -> Self {
        Self {
            base_engine: crate::validation::FhirSchemaValidationEngine::new_strict(),
            fhirpath_engine: None,
            fhirpath_config: FhirPathEvaluationConfig::default(),
            use_fhirpath_for_all: false,
        }
    }

    /// Create a new enhanced validation engine with FHIRPath support
    pub fn with_fhirpath_engine(fhirpath_engine: Arc<dyn FhirPathEngine>) -> Self {
        Self {
            base_engine: crate::validation::FhirSchemaValidationEngine::new(),
            fhirpath_engine: Some(fhirpath_engine),
            fhirpath_config: FhirPathEvaluationConfig::default(),
            use_fhirpath_for_all: false,
        }
    }

    /// Create a new enhanced validation engine with FHIRPath support and custom configuration
    pub fn with_fhirpath_engine_and_config(
        fhirpath_engine: Arc<dyn FhirPathEngine>,
        fhirpath_config: FhirPathEvaluationConfig,
    ) -> Self {
        Self {
            base_engine: crate::validation::FhirSchemaValidationEngine::new(),
            fhirpath_engine: Some(fhirpath_engine),
            fhirpath_config,
            use_fhirpath_for_all: false,
        }
    }

    /// Create a new enhanced validation engine with strict mode and FHIRPath support
    pub fn with_fhirpath_engine_strict(fhirpath_engine: Arc<dyn FhirPathEngine>) -> Self {
        Self {
            base_engine: crate::validation::FhirSchemaValidationEngine::new_strict(),
            fhirpath_engine: Some(fhirpath_engine),
            fhirpath_config: FhirPathEvaluationConfig::default(),
            use_fhirpath_for_all: false,
        }
    }

    /// Set whether to use FHIRPath engine for all constraints or only complex ones
    pub fn with_fhirpath_for_all(mut self, use_for_all: bool) -> Self {
        self.use_fhirpath_for_all = use_for_all;
        self
    }

    /// Set the FHIRPath evaluation configuration
    pub fn with_fhirpath_config(mut self, config: FhirPathEvaluationConfig) -> Self {
        self.fhirpath_config = config;
        self
    }

    /// Check if this engine has FHIRPath support
    pub fn has_fhirpath_support(&self) -> bool {
        self.fhirpath_engine.is_some()
    }

    /// Evaluate a single constraint using the appropriate method
    async fn evaluate_constraint_with_engine(
        &self,
        resource: &Value,
        constraint: &Constraint,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Convert fhirschema Constraint to fhir-model ConstraintInfo
        let constraint_info = self.convert_constraint_to_info(constraint);

        // Determine whether to use FHIRPath engine or fallback
        let should_use_fhirpath = self.fhirpath_engine.is_some()
            && (self.use_fhirpath_for_all || self.is_complex_expression(&constraint.expression));

        if should_use_fhirpath {
            if let Some(ref fhirpath_engine) = self.fhirpath_engine {
                let evaluation_context = FhirPathEvaluationContext::new(resource.clone())
                    .with_path(context.current_path.clone());

                match fhirpath_engine
                    .evaluate_constraint(
                        resource,
                        &constraint_info,
                        &evaluation_context,
                        &self.fhirpath_config,
                    )
                    .await
                {
                    Ok(result) => {
                        if !result.is_success() {
                            // Add validation issue based on constraint severity
                            match constraint.severity.as_str() {
                                "error" => {
                                    context.add_error(&constraint.key, &constraint.human);
                                }
                                "warning" => {
                                    context.add_warning(&constraint.key, &constraint.human);
                                }
                                "information" => {
                                    context.add_warning(&constraint.key, &constraint.human);
                                }
                                _ => {
                                    context.add_warning(
                                        &constraint.key,
                                        format!(
                                            "Constraint failed (unknown severity '{}'): {}",
                                            constraint.severity, constraint.human
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // FHIRPath evaluation failed - add as warning and fall back to basic evaluation
                        context.add_warning(
                            format!("{}-fhirpath-error", constraint.key),
                            format!(
                                "FHIRPath evaluation error: {e}. Falling back to basic evaluation."
                            ),
                        );

                        // Fall back to basic constraint evaluation
                        self.base_engine
                            .validate_constraint(resource, constraint, context)?;
                    }
                }
            }
        } else {
            // Use basic constraint evaluation
            self.base_engine
                .validate_constraint(resource, constraint, context)?;
        }

        Ok(())
    }

    /// Check if a FHIRPath expression is complex (needs full engine evaluation)
    fn is_complex_expression(&self, expression: &str) -> bool {
        // Simple heuristics to identify complex expressions
        // Complex expressions typically contain:
        // - Lambda functions (where, select, all, any, etc.)
        // - Complex operators (|, &, implies, etc.)
        // - Nested paths with multiple dots and complex navigation
        // - Function calls beyond simple exists()

        let complex_indicators = [
            ".where(",
            ".select(",
            ".all(",
            ".any(",
            ".first(",
            ".last(",
            ".skip(",
            ".take(",
            ".aggregate(",
            ".combine(",
            ".distinct(",
            ".intersect(",
            ".exclude(",
            ".union(",
            " implies ",
            " and ",
            " or ",
            " xor ",
            ".as(",
            ".is(",
            ".ofType(",
            "$this",
            "%context",
            "%resource",
            "%rootResource",
            ".resolve()",
            ".extension(",
            ".hasValue()",
            ".trace(",
        ];

        // Count dots to detect complex navigation
        let dot_count = expression.matches('.').count();

        // Check for complex indicators
        let has_complex_indicator = complex_indicators
            .iter()
            .any(|indicator| expression.contains(indicator));

        // Consider it complex if:
        // 1. It has complex indicators, OR
        // 2. It has more than 3 dots (complex navigation), OR
        // 3. It contains parentheses beyond simple function calls
        has_complex_indicator
            || dot_count > 3
            || (expression.contains('(') && !self.is_simple_function_call(expression))
    }

    /// Check if an expression is a simple function call
    fn is_simple_function_call(&self, expression: &str) -> bool {
        // Simple function calls we can handle with basic evaluation
        let simple_functions = [
            ".exists()",
            "exists(",
            ".empty()",
            "empty(",
            ".count()",
            "count(",
        ];

        simple_functions
            .iter()
            .any(|func| expression.contains(func))
    }

    /// Convert fhirschema Constraint to fhir-model ConstraintInfo
    fn convert_constraint_to_info(&self, constraint: &Constraint) -> ConstraintInfo {
        let severity = match constraint.severity.as_str() {
            "error" => ConstraintSeverity::Error,
            "warning" => ConstraintSeverity::Warning,
            "information" => ConstraintSeverity::Information,
            _ => ConstraintSeverity::Warning, // Default to warning for unknown severities
        };

        let mut constraint_info = ConstraintInfo::new(
            constraint.key.clone(),
            severity,
            constraint.human.clone(),
            constraint.expression.clone(),
        );

        if let Some(ref xpath) = constraint.xpath {
            constraint_info = constraint_info.with_xpath(xpath.clone());
        }

        if let Some(ref source) = constraint.source {
            constraint_info = constraint_info.with_source(source.clone());
        }

        constraint_info
    }

    /// Validate all constraints in a schema using the enhanced engine
    async fn validate_schema_constraints(
        &self,
        resource: &Value,
        schema: &FhirSchema,
        context: &mut ValidationContext,
    ) -> Result<()> {
        // Validate schema-level constraints
        for constraint in &schema.constraints {
            context.push_path(&constraint.key);
            self.evaluate_constraint_with_engine(resource, constraint, context)
                .await?;
            context.pop_path();
        }

        // Validate element-level constraints
        for (element_path, element) in &schema.elements {
            context.push_path(element_path);

            for constraint in &element.constraints {
                self.evaluate_constraint_with_engine(resource, constraint, context)
                    .await?;
            }

            context.pop_path();
        }

        Ok(())
    }
}

impl Default for FhirPathValidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationEngine for FhirPathValidationEngine {
    fn validate_resource(&self, resource: &Value, schema: &FhirSchema) -> Result<ValidationResult> {
        // For now, use the base engine for synchronous validation
        // In a real async implementation, this would be handled differently
        self.base_engine.validate_resource(resource, schema)
    }

    fn validate_resource_with_schemas(
        &self,
        resource: &Value,
        schemas: &[&FhirSchema],
    ) -> Result<ValidationResult> {
        // For now, use the base engine for synchronous validation
        // In a real async implementation, this would be handled differently
        self.base_engine
            .validate_resource_with_schemas(resource, schemas)
    }
}

impl FhirPathValidationEngine {
    /// Async version of validate_resource that can use FHIRPath engine
    pub async fn validate_resource_async(
        &self,
        resource: &Value,
        schema: &FhirSchema,
    ) -> Result<ValidationResult> {
        let mut context = ValidationContext::new(resource.clone());
        context.add_schema(&schema.schema_type, schema.clone());

        // Perform basic validation using the base engine
        self.base_engine
            .validate_resource_with_context(resource, schema, &mut context)?;

        // Perform enhanced constraint validation if FHIRPath engine is available
        if self.fhirpath_engine.is_some() {
            self.validate_schema_constraints(resource, schema, &mut context)
                .await?;
        }

        Ok(context.into_result())
    }

    /// Async version of validate_resource_with_schemas
    pub async fn validate_resource_with_schemas_async(
        &self,
        resource: &Value,
        schemas: &[&FhirSchema],
    ) -> Result<ValidationResult> {
        let mut context = ValidationContext::new(resource.clone());

        // Add all schemas to the context
        for schema in schemas {
            context.add_schema(&schema.schema_type, (*schema).clone());
        }

        let mut final_result = ValidationResult::success();

        // Validate against each schema
        for schema in schemas {
            let mut schema_context = context.clone();

            // Basic validation
            self.base_engine.validate_resource_with_context(
                resource,
                schema,
                &mut schema_context,
            )?;

            // Enhanced constraint validation if available
            if self.fhirpath_engine.is_some() {
                self.validate_schema_constraints(resource, schema, &mut schema_context)
                    .await?;
            }

            final_result.merge(schema_context.into_result());
        }

        Ok(final_result)
    }

    /// Get information about the FHIRPath engine capabilities
    pub fn get_fhirpath_capabilities(&self) -> Option<FhirPathEngineCapabilities> {
        self.fhirpath_engine
            .as_ref()
            .map(|engine| engine.get_capabilities())
    }

    /// Get statistics about constraint evaluation performance
    pub fn get_constraint_stats(&self) -> ConstraintValidationStats {
        ConstraintValidationStats {
            has_fhirpath_engine: self.fhirpath_engine.is_some(),
            use_fhirpath_for_all: self.use_fhirpath_for_all,
            fhirpath_timeout_ms: self.fhirpath_config.timeout_ms,
            fhirpath_max_recursion: self.fhirpath_config.max_recursion_depth,
        }
    }
}

/// Statistics about constraint validation configuration
#[derive(Debug, Clone)]
pub struct ConstraintValidationStats {
    /// Whether a FHIRPath engine is available
    pub has_fhirpath_engine: bool,
    /// Whether FHIRPath is used for all constraints
    pub use_fhirpath_for_all: bool,
    /// FHIRPath evaluation timeout in milliseconds
    pub fhirpath_timeout_ms: u64,
    /// FHIRPath maximum recursion depth
    pub fhirpath_max_recursion: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FhirSchema;

    #[test]
    fn test_enhanced_engine_creation() {
        let engine = FhirPathValidationEngine::new();
        assert!(!engine.has_fhirpath_support());

        let strict_engine = FhirPathValidationEngine::new_strict();
        assert!(!strict_engine.has_fhirpath_support());
        assert!(strict_engine.base_engine.strict_mode);
    }

    #[test]
    fn test_complex_expression_detection() {
        let engine = FhirPathValidationEngine::new();

        // Simple expressions
        assert!(!engine.is_complex_expression("name.exists()"));
        assert!(!engine.is_complex_expression("count() > 0"));
        assert!(!engine.is_complex_expression("value = 'test'"));

        // Complex expressions
        assert!(engine.is_complex_expression("name.where(use = 'official').exists()"));
        assert!(engine.is_complex_expression("telecom.where(system = 'email').value"));
        assert!(engine.is_complex_expression("contact.all(name.exists())"));
        assert!(
            engine.is_complex_expression("extension.where(url = 'http://example.com').exists()")
        );
        assert!(engine.is_complex_expression("value implies reason.exists()"));
    }

    #[test]
    fn test_constraint_conversion() {
        let engine = FhirPathValidationEngine::new();

        let constraint = Constraint::new("pat-1", "error", "Name must exist", "name.exists()")
            .with_xpath("//name")
            .with_source("http://example.com/Patient");

        let constraint_info = engine.convert_constraint_to_info(&constraint);

        assert_eq!(constraint_info.key, "pat-1");
        assert_eq!(constraint_info.severity, ConstraintSeverity::Error);
        assert_eq!(constraint_info.human, "Name must exist");
        assert_eq!(constraint_info.expression, "name.exists()");
        assert_eq!(constraint_info.xpath.as_deref(), Some("//name"));
        assert_eq!(
            constraint_info.source.as_deref(),
            Some("http://example.com/Patient")
        );
    }

    #[tokio::test]
    async fn test_validation_without_fhirpath_engine() {
        let engine = FhirPathValidationEngine::new();
        let resource = serde_json::json!({
            "resourceType": "Patient",
            "name": [{"family": "Doe"}]
        });

        let mut schema = FhirSchema::new("Patient");
        schema.constraints.push(Constraint::new(
            "pat-1",
            "error",
            "Name must exist",
            "name.exists()",
        ));

        let result = engine
            .validate_resource_async(&resource, &schema)
            .await
            .unwrap();

        // Should validate successfully using basic validation
        assert!(result.is_valid);
        assert_eq!(result.error_count, 0);
    }
}
