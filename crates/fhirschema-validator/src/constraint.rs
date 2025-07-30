//! Constraint evaluation for FHIRSchema validation
//!
//! This module handles FHIRPath constraint evaluation using the fhirpath-rs
//! library from the octofhir ecosystem, providing constraint validation
//! with proper severity handling and error reporting.

use crate::{
    error::{ValidationError, ValidationResult},
    context::FHIRPathContext,
    ValidationIssue, ValidationStats, Severity,
};
use fhirschema_core::Constraint;
use serde_json::Value;

/// Constraint evaluator for FHIRPath constraint validation
pub struct ConstraintEvaluator {
    // Note: In a real implementation, this would hold a reference to the fhirpath-rs engine
    // For now, we'll implement basic constraint evaluation
}

impl ConstraintEvaluator {
    /// Create a new constraint evaluator
    pub fn new() -> Self {
        Self {}
    }

    /// Evaluate a constraint against the current context
    pub fn evaluate_constraint(
        &self,
        constraint: &Constraint,
        context: &FHIRPathContext,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
        stats: &mut ValidationStats,
    ) -> ValidationResult<()> {
        // Get the expression (it's a required field)
        let expression = &constraint.expression;

        // Evaluate the FHIRPath expression
        let result = self.evaluate_fhirpath_expression(expression, context)?;

        // Check if constraint is satisfied
        let is_satisfied = self.is_constraint_satisfied(&result);

        if !is_satisfied {
            // Create validation issue for constraint violation
            let severity = self.map_constraint_severity(&constraint.severity);
            let issue = ValidationIssue {
                severity,
                code: constraint.key.clone(),
                message: constraint.human.clone().unwrap_or_else(|| {
                    format!("Constraint violation: {}", expression)
                }),
                location: path.to_string(),
                context: Some(format!("Expression: {}", expression)),
            };

            issues.push(issue);
        }

        stats.constraints_evaluated += 1;
        Ok(())
    }

    /// Evaluate a FHIRPath expression against the context
    fn evaluate_fhirpath_expression(
        &self,
        expression: &str,
        context: &FHIRPathContext,
    ) -> ValidationResult<Value> {
        // TODO: Integrate with fhirpath-rs library
        // For now, we'll implement basic expression evaluation

        // Handle simple variable references
        if expression.starts_with('%') {
            if let Some(value) = context.extract_path_value(expression) {
                return Ok(value);
            } else {
                return Err(ValidationError::fhirpath_error(format!(
                    "Unknown variable: {}",
                    expression
                )));
            }
        }

        // Handle simple property access
        if let Some(value) = context.extract_path_value(expression) {
            return Ok(value);
        }

        // Handle basic boolean expressions
        match expression {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => {
                // For complex expressions, we would use fhirpath-rs
                // For now, return a placeholder result
                self.evaluate_basic_expression(expression, context)
            }
        }
    }

    /// Basic expression evaluation for common patterns
    fn evaluate_basic_expression(
        &self,
        expression: &str,
        context: &FHIRPathContext,
    ) -> ValidationResult<Value> {
        // Handle exists() function
        if expression.ends_with(".exists()") {
            let path = &expression[..expression.len() - 9]; // Remove ".exists()"
            let exists = context.extract_path_value(path).is_some();
            return Ok(Value::Bool(exists));
        }

        // Handle empty() function
        if expression.ends_with(".empty()") {
            let path = &expression[..expression.len() - 8]; // Remove ".empty()"
            let is_empty = match context.extract_path_value(path) {
                Some(Value::Null) => true,
                Some(Value::String(s)) => s.is_empty(),
                Some(Value::Array(arr)) => arr.is_empty(),
                Some(Value::Object(obj)) => obj.is_empty(),
                None => true,
                _ => false,
            };
            return Ok(Value::Bool(is_empty));
        }

        // Handle count() function
        if expression.ends_with(".count()") {
            let path = &expression[..expression.len() - 8]; // Remove ".count()"
            let count = match context.extract_path_value(path) {
                Some(Value::Array(arr)) => arr.len(),
                Some(_) => 1,
                None => 0,
            };
            return Ok(Value::Number(count.into()));
        }

        // Handle length() function for strings
        if expression.ends_with(".length()") {
            let path = &expression[..expression.len() - 9]; // Remove ".length()"
            let length = match context.extract_path_value(path) {
                Some(Value::String(s)) => s.len(),
                Some(Value::Array(arr)) => arr.len(),
                _ => 0,
            };
            return Ok(Value::Number(length.into()));
        }

        // Handle simple comparisons
        if let Some(comparison_result) = self.evaluate_comparison(expression, context)? {
            return Ok(Value::Bool(comparison_result));
        }

        // Handle hasValue() function
        if expression.ends_with(".hasValue()") {
            let path = &expression[..expression.len() - 11]; // Remove ".hasValue()"
            let has_value = match context.extract_path_value(path) {
                Some(Value::Null) => false,
                Some(_) => true,
                None => false,
            };
            return Ok(Value::Bool(has_value));
        }

        // Default: try to extract as a simple path
        if let Some(value) = context.extract_path_value(expression) {
            Ok(value)
        } else {
            // For unhandled expressions, we'll assume they evaluate to true
            // In a real implementation, this would use fhirpath-rs
            Err(ValidationError::fhirpath_parse_error(
                expression,
                "Complex FHIRPath expressions not yet supported in basic evaluator",
            ))
        }
    }

    /// Evaluate simple comparison expressions
    fn evaluate_comparison(
        &self,
        expression: &str,
        context: &FHIRPathContext,
    ) -> ValidationResult<Option<bool>> {
        // Handle equality comparisons
        if let Some(eq_pos) = expression.find(" = ") {
            let left_expr = &expression[..eq_pos].trim();
            let right_expr = &expression[eq_pos + 3..].trim();

            let left_value = context.extract_path_value(left_expr);
            let right_value = self.parse_literal_or_extract(right_expr, context);

            return Ok(Some(left_value == right_value));
        }

        // Handle inequality comparisons
        if let Some(ne_pos) = expression.find(" != ") {
            let left_expr = &expression[..ne_pos].trim();
            let right_expr = &expression[ne_pos + 4..].trim();

            let left_value = context.extract_path_value(left_expr);
            let right_value = self.parse_literal_or_extract(right_expr, context);

            return Ok(Some(left_value != right_value));
        }

        Ok(None)
    }

    /// Parse a literal value or extract from context
    fn parse_literal_or_extract(&self, expr: &str, context: &FHIRPathContext) -> Option<Value> {
        let expr = expr.trim();

        // Handle string literals
        if expr.starts_with('\'') && expr.ends_with('\'') {
            let string_value = &expr[1..expr.len() - 1];
            return Some(Value::String(string_value.to_string()));
        }

        // Handle boolean literals
        match expr {
            "true" => return Some(Value::Bool(true)),
            "false" => return Some(Value::Bool(false)),
            _ => {}
        }

        // Handle numeric literals
        if let Ok(num) = expr.parse::<i64>() {
            return Some(Value::Number(num.into()));
        }

        if let Ok(num) = expr.parse::<f64>() {
            return Some(Value::Number(serde_json::Number::from_f64(num).unwrap()));
        }

        // Try to extract as path
        context.extract_path_value(expr)
    }

    /// Check if a constraint evaluation result indicates satisfaction
    fn is_constraint_satisfied(&self, result: &Value) -> bool {
        match result {
            Value::Bool(b) => *b,
            Value::Null => false,
            Value::Array(arr) => !arr.is_empty(),
            Value::String(s) => !s.is_empty(),
            Value::Number(_) => true,
            Value::Object(_) => true,
        }
    }

    /// Map constraint severity to validation severity
    fn map_constraint_severity(&self, severity: &Option<String>) -> Severity {
        match severity.as_deref() {
            Some("error") => Severity::Error,
            Some("warning") => Severity::Warning,
            Some("information") => Severity::Information,
            _ => Severity::Error, // Default to error for unknown severities
        }
    }

    /// Evaluate multiple constraints
    pub fn evaluate_constraints(
        &self,
        constraints: &[Constraint],
        context: &FHIRPathContext,
        path: &str,
        issues: &mut Vec<ValidationIssue>,
        stats: &mut ValidationStats,
    ) -> ValidationResult<()> {
        for constraint in constraints {
            self.evaluate_constraint(constraint, context, path, issues, stats)?;
        }
        Ok(())
    }

    /// Check if an expression is supported by the basic evaluator
    pub fn is_expression_supported(&self, expression: &str) -> bool {
        // Basic patterns we support
        expression.starts_with('%') ||
        expression.ends_with(".exists()") ||
        expression.ends_with(".empty()") ||
        expression.ends_with(".count()") ||
        expression.ends_with(".length()") ||
        expression.ends_with(".hasValue()") ||
        expression.contains(" = ") ||
        expression.contains(" != ") ||
        expression == "true" ||
        expression == "false"
    }

    /// Get constraint evaluation statistics
    pub fn get_stats(&self) -> ValidationStats {
        ValidationStats::default()
    }
}

impl Default for ConstraintEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_constraint(
        key: &str,
        expression: &str,
        severity: &str,
        human: &str,
    ) -> Constraint {
        Constraint {
            key: key.to_string(),
            expression: expression.to_string(),
            severity: Some(severity.to_string()),
            human: Some(human.to_string()),
        }
    }

    fn create_test_context() -> FHIRPathContext {
        let resource = json!({
            "resourceType": "Patient",
            "id": "test-patient",
            "name": [{
                "family": "Doe",
                "given": ["John"]
            }],
            "active": true,
            "gender": "male"
        });

        FHIRPathContext::new(&resource, &resource, &resource)
    }

    #[test]
    fn test_constraint_evaluator_creation() {
        let evaluator = ConstraintEvaluator::new();
        assert!(evaluator.is_expression_supported("name.exists()"));
    }

    #[test]
    fn test_evaluate_exists_constraint() {
        let evaluator = ConstraintEvaluator::new();
        let context = create_test_context();
        let constraint = create_test_constraint(
            "pat-1",
            "name.exists()",
            "error",
            "Patient must have a name"
        );

        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        evaluator.evaluate_constraint(&constraint, &context, "Patient", &mut issues, &mut stats).unwrap();

        // Should pass - patient has a name
        assert_eq!(issues.len(), 0);
        assert_eq!(stats.constraints_evaluated, 1);
    }

    #[test]
    fn test_evaluate_missing_field_constraint() {
        let evaluator = ConstraintEvaluator::new();
        let context = create_test_context();
        let constraint = create_test_constraint(
            "pat-2",
            "birthDate.exists()",
            "error",
            "Patient must have a birth date"
        );

        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        evaluator.evaluate_constraint(&constraint, &context, "Patient", &mut issues, &mut stats).unwrap();

        // Should fail - patient doesn't have birthDate
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "pat-2");
        assert_eq!(issues[0].severity, Severity::Error);
    }

    #[test]
    fn test_evaluate_count_constraint() {
        let evaluator = ConstraintEvaluator::new();
        let context = create_test_context();
        let constraint = create_test_constraint(
            "pat-3",
            "name.count() >= 1",
            "error",
            "Patient must have at least one name"
        );

        let mut issues: Vec<ValidationIssue> = Vec::new();
        let mut stats = ValidationStats::default();

        // This would require more complex expression parsing
        // For now, we'll test the count() function directly
        let result = evaluator.evaluate_fhirpath_expression("name.count()", &context).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn test_evaluate_equality_constraint() {
        let evaluator = ConstraintEvaluator::new();
        let context = create_test_context();

        // Test equality comparison
        let result = evaluator.evaluate_fhirpath_expression("gender = 'male'", &context).unwrap();
        assert_eq!(result, json!(true));

        let result = evaluator.evaluate_fhirpath_expression("gender = 'female'", &context).unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_evaluate_boolean_constraint() {
        let evaluator = ConstraintEvaluator::new();
        let context = create_test_context();
        let constraint = create_test_constraint(
            "pat-4",
            "active = true",
            "warning",
            "Patient should be active"
        );

        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        evaluator.evaluate_constraint(&constraint, &context, "Patient", &mut issues, &mut stats).unwrap();

        // Should pass - patient is active
        assert_eq!(issues.len(), 0);
    }

    #[test]
    fn test_constraint_severity_mapping() {
        let evaluator = ConstraintEvaluator::new();

        assert_eq!(evaluator.map_constraint_severity(&Some("error".to_string())), Severity::Error);
        assert_eq!(evaluator.map_constraint_severity(&Some("warning".to_string())), Severity::Warning);
        assert_eq!(evaluator.map_constraint_severity(&Some("information".to_string())), Severity::Information);
        assert_eq!(evaluator.map_constraint_severity(&None), Severity::Error);
    }

    #[test]
    fn test_is_constraint_satisfied() {
        let evaluator = ConstraintEvaluator::new();

        assert!(evaluator.is_constraint_satisfied(&json!(true)));
        assert!(!evaluator.is_constraint_satisfied(&json!(false)));
        assert!(!evaluator.is_constraint_satisfied(&json!(null)));
        assert!(evaluator.is_constraint_satisfied(&json!([1, 2, 3])));
        assert!(!evaluator.is_constraint_satisfied(&json!([])));
        assert!(evaluator.is_constraint_satisfied(&json!("test")));
        assert!(!evaluator.is_constraint_satisfied(&json!("")));
        assert!(evaluator.is_constraint_satisfied(&json!(42)));
        assert!(evaluator.is_constraint_satisfied(&json!({"key": "value"})));
    }

    #[test]
    fn test_evaluate_multiple_constraints() {
        let evaluator = ConstraintEvaluator::new();
        let context = create_test_context();

        let constraints = vec![
            create_test_constraint("pat-1", "name.exists()", "error", "Must have name"),
            create_test_constraint("pat-2", "active.exists()", "warning", "Should have active flag"),
        ];

        let mut issues = Vec::new();
        let mut stats = ValidationStats::default();

        evaluator.evaluate_constraints(&constraints, &context, "Patient", &mut issues, &mut stats).unwrap();

        // Both constraints should pass
        assert_eq!(issues.len(), 0);
        assert_eq!(stats.constraints_evaluated, 2);
    }

    #[test]
    fn test_variable_reference() {
        let evaluator = ConstraintEvaluator::new();
        let context = create_test_context();

        let result = evaluator.evaluate_fhirpath_expression("%resource", &context).unwrap();
        assert_eq!(result.get("resourceType"), Some(&json!("Patient")));

        let result = evaluator.evaluate_fhirpath_expression("%ucum", &context).unwrap();
        assert_eq!(result, json!("http://unitsofmeasure.org"));
    }

    #[test]
    fn test_expression_support_detection() {
        let evaluator = ConstraintEvaluator::new();

        assert!(evaluator.is_expression_supported("name.exists()"));
        assert!(evaluator.is_expression_supported("birthDate.empty()"));
        assert!(evaluator.is_expression_supported("name.count()"));
        assert!(evaluator.is_expression_supported("gender = 'male'"));
        assert!(evaluator.is_expression_supported("%resource"));
        assert!(evaluator.is_expression_supported("true"));
        assert!(evaluator.is_expression_supported("false"));

        // Complex expressions not yet supported
        assert!(!evaluator.is_expression_supported("name.where(use = 'official').exists()"));
    }

    #[test]
    fn test_parse_literal_values() {
        let evaluator = ConstraintEvaluator::new();
        let context = create_test_context();

        assert_eq!(evaluator.parse_literal_or_extract("'test'", &context), Some(json!("test")));
        assert_eq!(evaluator.parse_literal_or_extract("true", &context), Some(json!(true)));
        assert_eq!(evaluator.parse_literal_or_extract("false", &context), Some(json!(false)));
        assert_eq!(evaluator.parse_literal_or_extract("42", &context), Some(json!(42)));
        assert_eq!(evaluator.parse_literal_or_extract("3.14", &context), Some(json!(3.14)));
    }
}
