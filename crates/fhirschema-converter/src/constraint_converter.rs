//! Constraint converter for transforming FHIR constraints to FHIRSchema format.

use fhirschema_core::Constraint;
use crate::{Result, Error};
use serde_json::Value;
use regex::Regex;

/// Converter for FHIRPath constraint transformation.
pub struct ConstraintConverter {
    // Cache for compiled regex patterns
    fhirpath_validator: FHIRPathValidator,
}

/// Simple FHIRPath expression validator
struct FHIRPathValidator {
    // Basic patterns for FHIRPath validation
    identifier_pattern: Regex,
    function_pattern: Regex,
    operator_pattern: Regex,
}

impl ConstraintConverter {
    /// Create a new constraint converter.
    pub fn new() -> Result<Self> {
        Ok(Self {
            fhirpath_validator: FHIRPathValidator::new()?,
        })
    }

    /// Convert FHIR constraint to FHIRSchema Constraint.
    pub fn convert(&self, constraint_json: &Value) -> Result<Constraint> {
        let key = constraint_json.get("key")
            .and_then(|k| k.as_str())
            .ok_or_else(|| Error::Conversion("Constraint missing 'key' field".to_string()))?;

        let expression = constraint_json.get("expression")
            .and_then(|e| e.as_str())
            .ok_or_else(|| Error::Conversion("Constraint missing 'expression' field".to_string()))?;

        let human = constraint_json.get("human")
            .and_then(|h| h.as_str())
            .ok_or_else(|| Error::Conversion("Constraint missing 'human' field".to_string()))?;

        let severity = constraint_json.get("severity")
            .and_then(|s| s.as_str())
            .unwrap_or("error");

        // Validate FHIRPath expression
        self.validate_fhirpath_expression(expression)?;

        let mut constraint = Constraint::new(
            key.to_string(),
            expression.to_string(),
        );

        constraint.human = Some(human.to_string());
        constraint.severity = Some(severity.to_string());

        Ok(constraint)
    }

    /// Convert multiple constraints from element definition.
    pub fn convert_constraints(&self, constraints_array: &[Value]) -> Result<Vec<Constraint>> {
        let mut converted_constraints = Vec::new();

        for constraint_json in constraints_array {
            match self.convert(constraint_json) {
                Ok(constraint) => converted_constraints.push(constraint),
                Err(e) => {
                    // Log warning but continue processing other constraints
                    eprintln!("Warning: Failed to convert constraint: {}", e);
                }
            }
        }

        Ok(converted_constraints)
    }

    /// Validate FHIRPath expression syntax.
    pub fn validate_fhirpath_expression(&self, expression: &str) -> Result<()> {
        self.fhirpath_validator.validate(expression)
    }

    /// Extract constraints from element definition.
    pub fn extract_constraints_from_element(&self, element: &Value) -> Result<Vec<Constraint>> {
        let mut constraints = Vec::new();

        // Extract from constraint array
        if let Some(constraint_array) = element.get("constraint").and_then(|c| c.as_array()) {
            let element_constraints = self.convert_constraints(constraint_array)?;
            constraints.extend(element_constraints);
        }

        // Extract from condition array (invariants)
        if let Some(condition_array) = element.get("condition").and_then(|c| c.as_array()) {
            for condition in condition_array {
                if let Some(condition_str) = condition.as_str() {
                    // Create a constraint from condition reference
                    let mut constraint = Constraint::new(
                        condition_str.to_string(),
                        format!("condition('{}')", condition_str),
                    );
                    constraint.human = Some(format!("Condition {} must be satisfied", condition_str));
                    constraints.push(constraint);
                }
            }
        }

        Ok(constraints)
    }

    /// Check if expression uses supported FHIRPath features.
    pub fn check_expression_support(&self, expression: &str) -> Vec<String> {
        let mut unsupported_features = Vec::new();

        // Check for complex features that might not be fully supported
        if expression.contains("$this") {
            unsupported_features.push("$this context variable".to_string());
        }

        if expression.contains("resolve()") {
            unsupported_features.push("resolve() function".to_string());
        }

        if expression.contains("conformsTo()") {
            unsupported_features.push("conformsTo() function".to_string());
        }

        if expression.contains("memberOf()") {
            unsupported_features.push("memberOf() function".to_string());
        }

        unsupported_features
    }
}

impl FHIRPathValidator {
    fn new() -> Result<Self> {
        Ok(Self {
            identifier_pattern: Regex::new(r"^[A-Za-z][A-Za-z0-9]*$")
                .map_err(|e| Error::Conversion(format!("Failed to compile regex: {}", e)))?,
            function_pattern: Regex::new(r"[a-zA-Z][a-zA-Z0-9]*\s*\(")
                .map_err(|e| Error::Conversion(format!("Failed to compile regex: {}", e)))?,
            operator_pattern: Regex::new(r"(\+|\-|\*|\/|=|!=|<|>|<=|>=|and|or|xor|implies)")
                .map_err(|e| Error::Conversion(format!("Failed to compile regex: {}", e)))?,
        })
    }

    fn validate(&self, expression: &str) -> Result<()> {
        if expression.trim().is_empty() {
            return Err(Error::Conversion("FHIRPath expression cannot be empty".to_string()));
        }

        // Basic syntax validation
        self.validate_parentheses(expression)?;
        self.validate_quotes(expression)?;
        self.validate_basic_syntax(expression)?;

        Ok(())
    }

    fn validate_parentheses(&self, expression: &str) -> Result<()> {
        let mut depth = 0;
        for ch in expression.chars() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth < 0 {
                        return Err(Error::Conversion("Unmatched closing parenthesis in FHIRPath expression".to_string()));
                    }
                }
                _ => {}
            }
        }

        if depth != 0 {
            return Err(Error::Conversion("Unmatched opening parenthesis in FHIRPath expression".to_string()));
        }

        Ok(())
    }

    fn validate_quotes(&self, expression: &str) -> Result<()> {
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut chars = expression.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '\'' if !in_double_quote => {
                    in_single_quote = !in_single_quote;
                }
                '"' if !in_single_quote => {
                    in_double_quote = !in_double_quote;
                }
                '\\' if in_single_quote || in_double_quote => {
                    // Skip escaped character
                    chars.next();
                }
                _ => {}
            }
        }

        if in_single_quote {
            return Err(Error::Conversion("Unmatched single quote in FHIRPath expression".to_string()));
        }

        if in_double_quote {
            return Err(Error::Conversion("Unmatched double quote in FHIRPath expression".to_string()));
        }

        Ok(())
    }

    fn validate_basic_syntax(&self, expression: &str) -> Result<()> {
        // Check for obviously invalid patterns
        if expression.contains("..") && !expression.contains("...") {
            return Err(Error::Conversion("Invalid '..' operator in FHIRPath expression".to_string()));
        }

        if expression.starts_with('.') && !expression.starts_with("..") {
            return Err(Error::Conversion("FHIRPath expression cannot start with '.'".to_string()));
        }

        if expression.ends_with('.') {
            return Err(Error::Conversion("FHIRPath expression cannot end with '.'".to_string()));
        }

        Ok(())
    }
}

impl Default for ConstraintConverter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            fhirpath_validator: FHIRPathValidator {
                identifier_pattern: Regex::new(r"^[A-Za-z][A-Za-z0-9]*$").unwrap(),
                function_pattern: Regex::new(r"[a-zA-Z][a-zA-Z0-9]*\s*\(").unwrap(),
                operator_pattern: Regex::new(r"(\+|\-|\*|\/|=|!=|<|>|<=|>=|and|or|xor|implies)").unwrap(),
            }
        })
    }
}
