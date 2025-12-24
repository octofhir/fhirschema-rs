//! Mock FHIRPath evaluator for unit testing
//!
//! This provides a simple mock implementation of FhirPathEvaluator
//! that can be used in unit tests without requiring the full FHIRPath engine.

use async_trait::async_trait;
use octofhir_fhir_model::{
    error::ModelError, CompiledExpression, EvaluationResult, FhirPathConstraint, FhirPathEvaluator,
    ModelProvider, ValidationError, ValidationResult as FhirPathValidationResult,
};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Mock FHIRPath evaluator that always returns valid
pub struct AlwaysValidEvaluator;

#[async_trait]
impl FhirPathEvaluator for AlwaysValidEvaluator {
    async fn validate_constraints(
        &self,
        _resource: &JsonValue,
        _constraints: &[FhirPathConstraint],
    ) -> Result<FhirPathValidationResult, ModelError> {
        Ok(FhirPathValidationResult {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
        })
    }

    async fn evaluate(
        &self,
        _expression: &str,
        _resource: &JsonValue,
    ) -> Result<EvaluationResult, ModelError> {
        Ok(EvaluationResult::Empty)
    }

    async fn evaluate_with_variables(
        &self,
        _expression: &str,
        _resource: &JsonValue,
        _variables: &HashMap<String, EvaluationResult>,
    ) -> Result<EvaluationResult, ModelError> {
        // Return true for constraint validation (constraints pass when expression is truthy)
        Ok(EvaluationResult::Boolean(true, None))
    }

    async fn compile(&self, expression: &str) -> Result<CompiledExpression, ModelError> {
        Ok(CompiledExpression {
            expression: expression.to_string(),
            compiled_form: expression.to_string(),
            is_valid: true,
        })
    }

    async fn validate_expression(
        &self,
        _expression: &str,
    ) -> Result<octofhir_fhir_model::ValidationResult, ModelError> {
        Ok(octofhir_fhir_model::ValidationResult {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
        })
    }

    fn model_provider(&self) -> &dyn ModelProvider {
        unimplemented!("Mock evaluator doesn't provide a model provider")
    }
}

/// Mock FHIRPath evaluator that always returns invalid with specific errors
pub struct AlwaysInvalidEvaluator {
    pub error_message: String,
}

impl AlwaysInvalidEvaluator {
    pub fn new(error_message: impl Into<String>) -> Self {
        Self {
            error_message: error_message.into(),
        }
    }
}

#[async_trait]
impl FhirPathEvaluator for AlwaysInvalidEvaluator {
    async fn validate_constraints(
        &self,
        _resource: &JsonValue,
        constraints: &[FhirPathConstraint],
    ) -> Result<FhirPathValidationResult, ModelError> {
        let errors: Vec<ValidationError> = constraints
            .iter()
            .map(|c| ValidationError {
                message: format!("{}: {}", c.key, self.error_message),
                code: Some(c.key.clone()),
                location: None,
                severity: c.severity,
            })
            .collect();

        Ok(FhirPathValidationResult {
            is_valid: false,
            errors,
            warnings: vec![],
        })
    }

    async fn evaluate(
        &self,
        _expression: &str,
        _resource: &JsonValue,
    ) -> Result<EvaluationResult, ModelError> {
        Ok(EvaluationResult::Empty)
    }

    async fn evaluate_with_variables(
        &self,
        _expression: &str,
        _resource: &JsonValue,
        _variables: &HashMap<String, EvaluationResult>,
    ) -> Result<EvaluationResult, ModelError> {
        // Return false for constraint validation (constraints fail when expression is falsy)
        Ok(EvaluationResult::Boolean(false, None))
    }

    async fn compile(&self, expression: &str) -> Result<CompiledExpression, ModelError> {
        Ok(CompiledExpression {
            expression: expression.to_string(),
            compiled_form: expression.to_string(),
            is_valid: true,
        })
    }

    async fn validate_expression(
        &self,
        _expression: &str,
    ) -> Result<octofhir_fhir_model::ValidationResult, ModelError> {
        Ok(octofhir_fhir_model::ValidationResult {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
        })
    }

    fn model_provider(&self) -> &dyn ModelProvider {
        unimplemented!("Mock evaluator doesn't provide a model provider")
    }
}

/// Mock FHIRPath evaluator with configurable responses per expression
///
/// Note: The validator calls `evaluate_with_variables` with the expression string,
/// so responses are keyed by expression, not constraint key.
pub struct ConfigurableEvaluator {
    responses: HashMap<String, bool>,
}

impl ConfigurableEvaluator {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    /// Set whether a specific key (constraint key or expression) should pass or fail.
    ///
    /// For `validate_constraints`, use the constraint key (e.g., "pat-1").
    /// For `evaluate_with_variables`, use the expression (e.g., "name.exists()").
    pub fn set_constraint_result(&mut self, key: impl Into<String>, passes: bool) {
        self.responses.insert(key.into(), passes);
    }

    /// Alias for set_constraint_result - use the expression string as the key.
    pub fn set_expression_result(&mut self, expression: impl Into<String>, passes: bool) {
        self.set_constraint_result(expression, passes);
    }
}

#[async_trait]
impl FhirPathEvaluator for ConfigurableEvaluator {
    async fn validate_constraints(
        &self,
        _resource: &JsonValue,
        constraints: &[FhirPathConstraint],
    ) -> Result<FhirPathValidationResult, ModelError> {
        let mut errors = vec![];

        for constraint in constraints {
            let passes = self.responses.get(&constraint.key).copied().unwrap_or(true);
            if !passes {
                errors.push(ValidationError {
                    message: format!("Constraint {} failed", constraint.key),
                    code: Some(constraint.key.clone()),
                    location: None,
                    severity: constraint.severity,
                });
            }
        }

        Ok(FhirPathValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings: vec![],
        })
    }

    async fn evaluate(
        &self,
        _expression: &str,
        _resource: &JsonValue,
    ) -> Result<EvaluationResult, ModelError> {
        Ok(EvaluationResult::Empty)
    }

    async fn evaluate_with_variables(
        &self,
        expression: &str,
        _resource: &JsonValue,
        _variables: &HashMap<String, EvaluationResult>,
    ) -> Result<EvaluationResult, ModelError> {
        // Look up the result by expression, defaulting to true (pass) if not configured
        let passes = self.responses.get(expression).copied().unwrap_or(true);
        Ok(EvaluationResult::Boolean(passes, None))
    }

    async fn compile(&self, expression: &str) -> Result<CompiledExpression, ModelError> {
        Ok(CompiledExpression {
            expression: expression.to_string(),
            compiled_form: expression.to_string(),
            is_valid: true,
        })
    }

    async fn validate_expression(
        &self,
        _expression: &str,
    ) -> Result<octofhir_fhir_model::ValidationResult, ModelError> {
        Ok(octofhir_fhir_model::ValidationResult {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
        })
    }

    fn model_provider(&self) -> &dyn ModelProvider {
        unimplemented!("Mock evaluator doesn't provide a model provider")
    }
}

impl Default for ConfigurableEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_always_valid_evaluator() {
        let evaluator = AlwaysValidEvaluator;
        let constraint = FhirPathConstraint::new(
            "test-1".to_string(),
            "Test constraint".to_string(),
            "true".to_string(),
        );

        let result = evaluator
            .validate_constraints(&JsonValue::Object(Default::default()), &[constraint])
            .await
            .unwrap();

        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_always_invalid_evaluator() {
        let evaluator = AlwaysInvalidEvaluator::new("Always fails");
        let constraint = FhirPathConstraint::new(
            "test-1".to_string(),
            "Test constraint".to_string(),
            "false".to_string(),
        );

        let result = evaluator
            .validate_constraints(&JsonValue::Object(Default::default()), &[constraint])
            .await
            .unwrap();

        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].message.contains("Always fails"));
    }

    #[tokio::test]
    async fn test_configurable_evaluator() {
        let mut evaluator = ConfigurableEvaluator::new();
        evaluator.set_constraint_result("pass-1", true);
        evaluator.set_constraint_result("fail-1", false);

        let constraints = vec![
            FhirPathConstraint::new(
                "pass-1".to_string(),
                "Should pass".to_string(),
                "true".to_string(),
            ),
            FhirPathConstraint::new(
                "fail-1".to_string(),
                "Should fail".to_string(),
                "false".to_string(),
            ),
        ];

        let result = evaluator
            .validate_constraints(&JsonValue::Object(Default::default()), &constraints)
            .await
            .unwrap();

        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].code.as_ref().unwrap(), "fail-1");
    }
}
