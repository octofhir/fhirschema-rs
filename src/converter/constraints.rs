use super::{ConversionContext, ElementDefinition};
use crate::{Constraint, FhirSchemaError, Result};

pub struct ConstraintProcessor;

impl ConstraintProcessor {
    pub fn new() -> Self {
        Self
    }

    pub fn process_constraints(
        &self,
        elements: &[ElementDefinition],
        context: &mut ConversionContext,
    ) -> Result<Vec<Constraint>> {
        let mut result = Vec::new();

        for element in elements {
            if let Some(constraints) = &element.constraint {
                for constraint_def in constraints {
                    let constraint =
                        self.convert_constraint(constraint_def, &element.path, context)?;
                    result.push(constraint);
                    context.increment_constraints_converted();
                }
            }
        }

        Ok(result)
    }

    fn convert_constraint(
        &self,
        constraint_def: &super::ElementDefinitionConstraint,
        element_path: &str,
        context: &mut ConversionContext,
    ) -> Result<Constraint> {
        // Validate constraint severity
        if !self.is_valid_severity(&constraint_def.severity) {
            let error_msg = format!(
                "Invalid constraint severity '{}' for constraint '{}' in element {}",
                constraint_def.severity, constraint_def.key, element_path
            );
            context.add_error(error_msg.clone());
            return Err(FhirSchemaError::Conversion { message: error_msg });
        }

        // Get expression, preferring FHIRPath over XPath
        let expression = self.get_constraint_expression(constraint_def, element_path, context)?;

        let mut constraint = Constraint::new(
            &constraint_def.key,
            &constraint_def.severity,
            &constraint_def.human,
            &expression,
        );

        // Add XPath if present and different from expression
        if let Some(xpath) = &constraint_def.xpath {
            if Some(xpath) != constraint_def.expression.as_ref() {
                constraint = constraint.with_xpath(xpath);
            }
        }

        // Add source if present
        if let Some(source) = &constraint_def.source {
            constraint = constraint.with_source(source);
        }

        // Validate the constraint
        self.validate_constraint(&constraint, element_path, context)?;

        Ok(constraint)
    }

    fn get_constraint_expression(
        &self,
        constraint_def: &super::ElementDefinitionConstraint,
        element_path: &str,
        context: &mut ConversionContext,
    ) -> Result<String> {
        if let Some(expression) = &constraint_def.expression {
            // Validate FHIRPath expression
            self.validate_fhirpath_expression(expression, element_path, context)?;
            Ok(expression.clone())
        } else if let Some(xpath) = &constraint_def.xpath {
            // Fall back to XPath, but warn about it
            context.add_warning(format!(
                "Constraint '{}' in element {} uses XPath instead of FHIRPath expression",
                constraint_def.key, element_path
            ));
            Ok(xpath.clone())
        } else {
            // No expression at all - this is an error
            let error_msg = format!(
                "Constraint '{}' in element {} has neither expression nor xpath",
                constraint_def.key, element_path
            );
            context.add_error(error_msg.clone());
            Err(FhirSchemaError::Conversion { message: error_msg })
        }
    }

    fn validate_fhirpath_expression(
        &self,
        expression: &str,
        element_path: &str,
        context: &mut ConversionContext,
    ) -> Result<()> {
        // Basic validation of FHIRPath expression
        if expression.is_empty() {
            context.add_error(format!(
                "Empty FHIRPath expression in element {element_path}"
            ));
            return Err(FhirSchemaError::Conversion {
                message: "Empty FHIRPath expression".to_string(),
            });
        }

        // Check for common FHIRPath patterns and potential issues
        self.check_fhirpath_patterns(expression, element_path, context);

        Ok(())
    }

    fn check_fhirpath_patterns(
        &self,
        expression: &str,
        element_path: &str,
        context: &mut ConversionContext,
    ) {
        // Check for potentially problematic patterns
        if expression.contains("$this") {
            context.add_info(format!(
                "FHIRPath expression uses $this context in element {element_path}: {expression}"
            ));
        }

        if expression.contains("%resource") {
            context.add_info(format!(
                "FHIRPath expression uses %resource variable in element {element_path}: {expression}"
            ));
        }

        if expression.contains("%rootResource") {
            context.add_info(format!(
                "FHIRPath expression uses %rootResource variable in element {element_path}: {expression}"
            ));
        }

        // Check for complex expressions that might need special handling
        if expression.contains("resolve()") {
            context.add_warning(format!(
                "FHIRPath expression uses resolve() function in element {element_path}: {expression}"
            ));
        }

        if expression.contains("conformsTo()") {
            context.add_warning(format!(
                "FHIRPath expression uses conformsTo() function in element {element_path}: {expression}"
            ));
        }

        // Check bracket matching
        let open_parens = expression.matches('(').count();
        let close_parens = expression.matches(')').count();
        if open_parens != close_parens {
            context.add_error(format!(
                "Unmatched parentheses in FHIRPath expression in element {element_path}: {expression}"
            ));
        }
    }

    fn validate_constraint(
        &self,
        constraint: &Constraint,
        element_path: &str,
        context: &mut ConversionContext,
    ) -> Result<()> {
        // Validate constraint key format
        if !self.is_valid_constraint_key(&constraint.key) {
            context.add_warning(format!(
                "Constraint key '{}' in element {} doesn't follow standard naming convention",
                constraint.key, element_path
            ));
        }

        // Check for duplicate constraint keys within the same element
        // (This would require tracking constraints per element, which we're not doing yet)

        // Validate human-readable message
        if constraint.human.is_empty() {
            context.add_warning(format!(
                "Constraint '{}' in element {} has empty human-readable message",
                constraint.key, element_path
            ));
        }

        Ok(())
    }

    fn is_valid_severity(&self, severity: &str) -> bool {
        matches!(severity, "error" | "warning" | "information")
    }

    fn is_valid_constraint_key(&self, key: &str) -> bool {
        // Standard FHIR constraint keys are typically:
        // - 3-letter resource prefix + hyphen + number (e.g., "pat-1")
        // - or custom format with consistent naming

        if key.is_empty() {
            return false;
        }

        // Allow various formats but check for basic sanity
        !key.contains(' ') && !key.contains('\t') && !key.contains('\n')
    }

    pub fn group_constraints_by_severity<'a>(
        &self,
        constraints: &'a [Constraint],
    ) -> std::collections::HashMap<String, Vec<&'a Constraint>> {
        let mut groups = std::collections::HashMap::new();

        for constraint in constraints {
            groups
                .entry(constraint.severity.clone())
                .or_insert_with(Vec::new)
                .push(constraint);
        }

        groups
    }

    pub fn find_constraint_dependencies(
        &self,
        constraint: &Constraint,
        context: &mut ConversionContext,
    ) -> Result<Vec<String>> {
        let mut dependencies = Vec::new();

        // Analyze the expression for element references
        // This is a simplified version - a full implementation would parse FHIRPath

        let expression = &constraint.expression;

        // Look for simple element references
        if expression.contains('.') && !expression.starts_with("$") {
            // This might reference other elements
            context.add_info(format!(
                "Constraint '{}' may have element dependencies: {}",
                constraint.key, expression
            ));
        }

        // Look for function calls that might reference other resources
        if expression.contains("resolve(") {
            dependencies.push("external_references".to_string());
        }

        if expression.contains("%resource") || expression.contains("%rootResource") {
            dependencies.push("resource_context".to_string());
        }

        Ok(dependencies)
    }
}

impl Default for ConstraintProcessor {
    fn default() -> Self {
        Self::new()
    }
}
