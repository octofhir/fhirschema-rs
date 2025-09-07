// Constraint mapping for FHIR constraints to JSON Schema constraints

use crate::error::{FhirSchemaError, Result};
use crate::types::{ConstraintSeverity, FhirConstraint};
use serde_json::Value;
use std::collections::HashMap;

pub struct ConstraintMapper {
    // Cache for converted constraints to avoid duplicate processing
    constraint_cache: std::sync::RwLock<HashMap<String, FhirConstraint>>,
}

impl ConstraintMapper {
    pub fn new() -> Self {
        Self {
            constraint_cache: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Convert a FHIR constraint definition to a FhirConstraint
    pub async fn convert_constraint(&self, constraint: &Value) -> Result<FhirConstraint> {
        // Extract constraint key for caching
        let key = constraint
            .get("key")
            .and_then(|k| k.as_str())
            .unwrap_or("unknown");

        // Create cache key with full constraint content for uniqueness
        let cache_key = format!("{}_{}", key, constraint.to_string().len());

        // Check cache first
        {
            let cache = self
                .constraint_cache
                .read()
                .map_err(|_| FhirSchemaError::Runtime {
                    message: "Failed to acquire constraint cache read lock".to_string(),
                })?;

            if let Some(cached_constraint) = cache.get(&cache_key) {
                return Ok(cached_constraint.clone());
            }
        }

        // Convert the constraint
        let fhir_constraint = self.convert_constraint_internal(constraint).await?;

        // Cache the result
        {
            let mut cache =
                self.constraint_cache
                    .write()
                    .map_err(|_| FhirSchemaError::Runtime {
                        message: "Failed to acquire constraint cache write lock".to_string(),
                    })?;

            cache.insert(cache_key, fhir_constraint.clone());
        }

        Ok(fhir_constraint)
    }

    /// Internal constraint conversion logic
    async fn convert_constraint_internal(&self, constraint: &Value) -> Result<FhirConstraint> {
        // Extract basic constraint information
        let key = constraint
            .get("key")
            .and_then(|k| k.as_str())
            .unwrap_or("unknown");

        let human = constraint
            .get("human")
            .and_then(|h| h.as_str())
            .unwrap_or("Constraint");

        let severity = self.parse_constraint_severity(constraint)?;

        let mut fhir_constraint = FhirConstraint::new(key, severity, human);

        // Add FHIRPath expression if available
        if let Some(expression) = constraint.get("expression").and_then(|e| e.as_str()) {
            fhir_constraint = fhir_constraint.with_expression(expression);

            // Try to convert FHIRPath to additional JSON Schema constraints
            if let Ok(additional_constraints) =
                self.extract_json_schema_constraints(expression).await
            {
                for (meta_key, meta_value) in additional_constraints {
                    fhir_constraint.metadata.insert(meta_key, meta_value);
                }
            }
        }

        // Add XPath expression if available (legacy support)
        if let Some(xpath) = constraint.get("xpath").and_then(|x| x.as_str()) {
            fhir_constraint = fhir_constraint.with_xpath(xpath);
        }

        // Add source information
        if let Some(source) = constraint.get("source").and_then(|s| s.as_str()) {
            fhir_constraint = fhir_constraint.with_source(source);
        }

        // Add additional metadata
        if let Some(requirements) = constraint.get("requirements").and_then(|r| r.as_str()) {
            fhir_constraint.metadata.insert(
                "fhir_requirements".to_string(),
                Value::String(requirements.to_string()),
            );
        }

        Ok(fhir_constraint)
    }

    /// Parse constraint severity from FHIR constraint
    fn parse_constraint_severity(&self, constraint: &Value) -> Result<ConstraintSeverity> {
        let severity_str = constraint
            .get("severity")
            .and_then(|s| s.as_str())
            .unwrap_or("error");

        match severity_str.to_lowercase().as_str() {
            "error" => Ok(ConstraintSeverity::Error),
            "warning" => Ok(ConstraintSeverity::Warning),
            "information" | "info" => Ok(ConstraintSeverity::Information),
            _ => {
                // Default to error for unknown severity
                eprintln!("Unknown constraint severity '{severity_str}', defaulting to error");
                Ok(ConstraintSeverity::Error)
            }
        }
    }

    /// Extract JSON Schema constraints from FHIRPath expressions
    async fn extract_json_schema_constraints(
        &self,
        expression: &str,
    ) -> Result<HashMap<String, Value>> {
        let mut constraints = HashMap::new();

        // Simple pattern matching for common FHIRPath expressions
        // This is a basic implementation - a full FHIRPath parser would be more robust

        // Pattern: exists() -> required
        if expression.contains("exists()") && !expression.contains("not") {
            constraints.insert("json_schema_required".to_string(), Value::Bool(true));
        }

        // Pattern: empty() -> not required
        if expression.contains("empty()") && !expression.contains("not") {
            constraints.insert("json_schema_required".to_string(), Value::Bool(false));
        }

        // Pattern: length() constraints
        if let Some(length_constraint) = self.extract_length_constraint(expression) {
            constraints.insert("json_schema_length".to_string(), length_constraint);
        }

        // Pattern: count() constraints
        if let Some(count_constraint) = self.extract_count_constraint(expression) {
            constraints.insert("json_schema_count".to_string(), count_constraint);
        }

        // Pattern: matches() regular expressions
        if let Some(regex_pattern) = self.extract_regex_pattern(expression) {
            constraints.insert(
                "json_schema_pattern".to_string(),
                Value::String(regex_pattern),
            );
        }

        // Pattern: value comparisons
        if let Some(value_constraints) = self.extract_value_constraints(expression) {
            for (key, value) in value_constraints {
                constraints.insert(key, value);
            }
        }

        // Pattern: type checks
        if let Some(type_constraint) = self.extract_type_constraint(expression) {
            constraints.insert(
                "json_schema_type".to_string(),
                Value::String(type_constraint),
            );
        }

        Ok(constraints)
    }

    /// Extract length constraints from FHIRPath expressions
    fn extract_length_constraint(&self, expression: &str) -> Option<Value> {
        // Look for patterns like "length() <= 255" or "length() >= 1"
        if expression.contains("length()") {
            // Simple regex patterns for common length constraints
            if let Some(captures) = regex::Regex::new(r"length\(\)\s*([<>=]+)\s*(\d+)")
                .ok()?
                .captures(expression)
            {
                let operator = captures.get(1)?.as_str();
                let value: u64 = captures.get(2)?.as_str().parse().ok()?;

                let mut constraint = HashMap::new();
                match operator {
                    "<=" | "<" => {
                        constraint.insert("max".to_string(), Value::Number(value.into()));
                    }
                    ">=" | ">" => {
                        constraint.insert("min".to_string(), Value::Number(value.into()));
                    }
                    "=" | "==" => {
                        constraint.insert("exact".to_string(), Value::Number(value.into()));
                    }
                    _ => return None,
                }

                return serde_json::to_value(constraint).ok();
            }
        }
        None
    }

    /// Extract count constraints from FHIRPath expressions
    fn extract_count_constraint(&self, expression: &str) -> Option<Value> {
        // Look for patterns like "count() <= 1" or "count() >= 0"
        if expression.contains("count()") {
            if let Some(captures) = regex::Regex::new(r"count\(\)\s*([<>=]+)\s*(\d+)")
                .ok()?
                .captures(expression)
            {
                let operator = captures.get(1)?.as_str();
                let value: u64 = captures.get(2)?.as_str().parse().ok()?;

                let mut constraint = HashMap::new();
                match operator {
                    "<=" | "<" => {
                        constraint.insert("maxItems".to_string(), Value::Number(value.into()));
                    }
                    ">=" | ">" => {
                        constraint.insert("minItems".to_string(), Value::Number(value.into()));
                    }
                    "=" | "==" => {
                        constraint.insert("exactItems".to_string(), Value::Number(value.into()));
                    }
                    _ => return None,
                }

                return serde_json::to_value(constraint).ok();
            }
        }
        None
    }

    /// Extract regex patterns from FHIRPath matches() expressions
    fn extract_regex_pattern(&self, expression: &str) -> Option<String> {
        // Look for patterns like "matches('^[A-Z]{3}$')"
        if let Some(captures) = regex::Regex::new(r#"matches\(['\"]([^'\"]+)['\"]\)"#)
            .ok()?
            .captures(expression)
        {
            return Some(captures.get(1)?.as_str().to_string());
        }
        None
    }

    /// Extract value constraints from FHIRPath expressions
    fn extract_value_constraints(&self, expression: &str) -> Option<HashMap<String, Value>> {
        let mut constraints = HashMap::new();

        // Look for numeric comparisons like "value <= 100" or "value >= 0"
        if let Some(captures) = regex::Regex::new(r"value\s*([<>=]+)\s*(-?\d+\.?\d*)")
            .ok()?
            .captures(expression)
        {
            let operator = captures.get(1)?.as_str();
            let value_str = captures.get(2)?.as_str();

            if let Ok(value) = value_str.parse::<f64>() {
                match operator {
                    "<=" | "<" => {
                        constraints.insert(
                            "json_schema_maximum".to_string(),
                            Value::Number(serde_json::Number::from_f64(value)?),
                        );
                    }
                    ">=" | ">" => {
                        constraints.insert(
                            "json_schema_minimum".to_string(),
                            Value::Number(serde_json::Number::from_f64(value)?),
                        );
                    }
                    "=" | "==" => {
                        constraints.insert(
                            "json_schema_const".to_string(),
                            Value::Number(serde_json::Number::from_f64(value)?),
                        );
                    }
                    _ => {}
                }
            }
        }

        if constraints.is_empty() {
            None
        } else {
            Some(constraints)
        }
    }

    /// Extract type constraints from FHIRPath expressions
    fn extract_type_constraint(&self, expression: &str) -> Option<String> {
        // Look for type checks like "is(string)" or "as(boolean)"
        if let Some(captures) = regex::Regex::new(r"(?:is|as)\((\w+)\)")
            .ok()?
            .captures(expression)
        {
            let fhir_type = captures.get(1)?.as_str();

            // Map FHIR types to JSON Schema types
            let json_type = match fhir_type.to_lowercase().as_str() {
                "string" | "code" | "id" | "uri" | "url" | "canonical" => "string",
                "boolean" => "boolean",
                "integer" | "positiveint" | "unsignedint" => "integer",
                "decimal" => "number",
                _ => return None,
            };

            return Some(json_type.to_string());
        }
        None
    }

    /// Convert multiple constraints in batch for efficiency
    pub async fn convert_constraints_batch(
        &self,
        constraints: &[Value],
    ) -> Result<Vec<FhirConstraint>> {
        let mut results = Vec::new();

        for constraint in constraints {
            match self.convert_constraint(constraint).await {
                Ok(fhir_constraint) => results.push(fhir_constraint),
                Err(e) => {
                    // Log error but continue processing others
                    eprintln!("Failed to convert constraint: {e}");
                }
            }
        }

        Ok(results)
    }

    /// Clear the constraint cache (useful for testing or memory management)
    pub fn clear_cache(&self) -> Result<()> {
        let mut cache = self
            .constraint_cache
            .write()
            .map_err(|_| FhirSchemaError::Runtime {
                message: "Failed to acquire constraint cache write lock for clearing".to_string(),
            })?;

        cache.clear();
        Ok(())
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> Result<(usize, usize)> {
        let cache = self
            .constraint_cache
            .read()
            .map_err(|_| FhirSchemaError::Runtime {
                message: "Failed to acquire constraint cache read lock for stats".to_string(),
            })?;

        let size = cache.len();
        let capacity = cache.capacity();

        Ok((size, capacity))
    }
}

impl Default for ConstraintMapper {
    fn default() -> Self {
        Self::new()
    }
}
