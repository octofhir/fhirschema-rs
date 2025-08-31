#[cfg(test)]
mod tests {
    use super::super::{
        constraints::ConstraintProcessor,
        context::ConversionContext,
        structure_definition::{ElementDefinition, ElementDefinitionConstraint},
    };
    use crate::converter::ConverterConfig;

    #[test]
    fn test_fhirpath_expression_preservation() {
        let processor = ConstraintProcessor::new();
        let config = ConverterConfig::default();
        let mut context = ConversionContext::new(&config);

        // Create test ElementDefinition with various FHIRPath expressions
        let test_elements = vec![
            create_element_with_constraint(
                "Patient.name",
                "pat-1",
                "error",
                "Name is required",
                Some("name.exists()"), // Simple exists check
                None,
            ),
            create_element_with_constraint(
                "Patient.contact",
                "pat-2",
                "warning",
                "Contact must have name or organization",
                Some("name.exists() or organization.exists()"), // Boolean logic
                None,
            ),
            create_element_with_constraint(
                "Patient.telecom",
                "pat-3",
                "error",
                "Phone number validation",
                Some("where(system='phone').value.matches('[0-9]{3}-[0-9]{3}-[0-9]{4}')"), // Complex pattern
                Some("//telecom[system='phone']"), // XPath fallback
            ),
            create_element_with_constraint(
                "Patient.identifier",
                "pat-4",
                "information",
                "Identifier system context",
                Some("system.exists() implies value.exists()"), // Conditional logic
                None,
            ),
            create_element_with_constraint(
                "Patient.extension",
                "pat-5",
                "error",
                "Extension validation",
                Some(
                    "extension.where(url = 'http://example.com/patient-type').value.as(CodeableConcept).coding.code.exists()",
                ), // Deep navigation
                None,
            ),
        ];

        // Process constraints
        let result = processor.process_constraints(&test_elements, &mut context);

        assert!(result.is_ok(), "Constraint processing should succeed");
        let constraints = result.unwrap();

        // Verify all constraints were processed
        assert_eq!(constraints.len(), 5, "Should process all 5 constraints");

        // Verify each constraint preserved the correct FHIRPath expression
        let constraint_by_key: std::collections::HashMap<String, _> =
            constraints.iter().map(|c| (c.key.clone(), c)).collect();

        // Test 1: Simple exists check
        let pat1 = constraint_by_key.get("pat-1").expect("pat-1 should exist");
        assert_eq!(pat1.expression, "name.exists()");
        assert_eq!(pat1.severity, "error");
        assert_eq!(pat1.human, "Name is required");
        assert!(pat1.xpath.is_none());

        // Test 2: Boolean logic
        let pat2 = constraint_by_key.get("pat-2").expect("pat-2 should exist");
        assert_eq!(pat2.expression, "name.exists() or organization.exists()");
        assert_eq!(pat2.severity, "warning");

        // Test 3: Complex pattern with XPath
        let pat3 = constraint_by_key.get("pat-3").expect("pat-3 should exist");
        assert_eq!(
            pat3.expression,
            "where(system='phone').value.matches('[0-9]{3}-[0-9]{3}-[0-9]{4}')"
        );
        assert_eq!(pat3.severity, "error");
        assert_eq!(pat3.xpath.as_deref(), Some("//telecom[system='phone']"));

        // Test 4: Conditional logic
        let pat4 = constraint_by_key.get("pat-4").expect("pat-4 should exist");
        assert_eq!(pat4.expression, "system.exists() implies value.exists()");
        assert_eq!(pat4.severity, "information");

        // Test 5: Deep navigation
        let pat5 = constraint_by_key.get("pat-5").expect("pat-5 should exist");
        assert_eq!(
            pat5.expression,
            "extension.where(url = 'http://example.com/patient-type').value.as(CodeableConcept).coding.code.exists()"
        );

        // Verify context collected appropriate warnings/info
        let stats = context.get_stats();
        let _warnings = &stats.warnings;
        let errors = &stats.errors;

        // Should have no errors for valid constraints
        assert!(
            errors.is_empty(),
            "Should have no errors for valid constraints"
        );

        println!("✅ All FHIRPath expressions correctly preserved:");
        for constraint in &constraints {
            println!("  - {}: {}", constraint.key, constraint.expression);
        }
    }

    #[test]
    fn test_xpath_fallback_when_no_fhirpath() {
        let processor = ConstraintProcessor::new();
        let mut context = ConversionContext::new(&ConverterConfig::default());

        // Create element with only XPath (no FHIRPath expression)
        let test_elements = vec![create_element_with_constraint(
            "Patient.birthDate",
            "pat-xpath",
            "error",
            "Birth date validation",
            None,                               // No FHIRPath expression
            Some("@value castable as xs:date"), // Only XPath
        )];

        let result = processor.process_constraints(&test_elements, &mut context);

        assert!(result.is_ok(), "Should handle XPath fallback gracefully");
        let constraints = result.unwrap();

        assert_eq!(constraints.len(), 1);
        let constraint = &constraints[0];

        // Should use XPath as expression when FHIRPath is not available
        assert_eq!(constraint.expression, "@value castable as xs:date");
        assert_eq!(constraint.key, "pat-xpath");

        // Should have a warning about using XPath instead of FHIRPath
        let warnings = &context.get_stats().warnings;
        assert!(
            warnings
                .iter()
                .any(|msg| msg.contains("XPath instead of FHIRPath")),
            "Should warn about XPath fallback"
        );

        println!(
            "✅ XPath fallback working correctly: {}",
            constraint.expression
        );
    }

    #[test]
    fn test_missing_expression_error() {
        let processor = ConstraintProcessor::new();
        let mut context = ConversionContext::new(&ConverterConfig::default());

        // Create element with constraint but no expression or xpath
        let test_elements = vec![create_element_with_constraint(
            "Patient.gender",
            "pat-empty",
            "error",
            "Gender validation",
            None, // No FHIRPath
            None, // No XPath either
        )];

        let result = processor.process_constraints(&test_elements, &mut context);

        // Should fail because there's no expression at all
        assert!(
            result.is_err(),
            "Should error when no expression is provided"
        );

        let errors = &context.get_stats().errors;
        assert!(
            errors
                .iter()
                .any(|msg| msg.contains("neither expression nor xpath")),
            "Should report missing expression error"
        );

        println!("✅ Missing expression correctly detected and reported");
    }

    #[test]
    fn test_complex_fhirpath_patterns_detected() {
        let processor = ConstraintProcessor::new();
        let mut context = ConversionContext::new(&ConverterConfig::default());

        // Test various FHIRPath patterns that should be detected
        let test_elements = vec![
            create_element_with_constraint(
                "Patient",
                "pat-this",
                "error",
                "This context test",
                Some("$this.name.exists()"),
                None,
            ),
            create_element_with_constraint(
                "Patient",
                "pat-resource",
                "error",
                "Resource variable test",
                Some("%resource.identifier.exists()"),
                None,
            ),
            create_element_with_constraint(
                "Patient",
                "pat-root",
                "error",
                "Root resource test",
                Some("%rootResource.meta.versionId.exists()"),
                None,
            ),
            create_element_with_constraint(
                "Patient.managingOrganization",
                "pat-resolve",
                "warning",
                "Reference resolution test",
                Some("resolve().name.exists()"),
                None,
            ),
            create_element_with_constraint(
                "Patient",
                "pat-conforms",
                "warning",
                "Conformance test",
                Some("conformsTo()"),
                None,
            ),
        ];

        let result = processor.process_constraints(&test_elements, &mut context);

        assert!(
            result.is_ok(),
            "Should process complex patterns successfully"
        );
        let constraints = result.unwrap();
        assert_eq!(constraints.len(), 5);

        // Check that appropriate info/warning messages were generated
        let warnings = &context.get_stats().warnings;

        // Should warn about resolve() function
        assert!(
            warnings
                .iter()
                .any(|msg| msg.contains("resolve() function")),
            "Should warn about resolve() usage"
        );

        // Should warn about conformsTo() function
        assert!(
            warnings
                .iter()
                .any(|msg| msg.contains("conformsTo() function")),
            "Should warn about conformsTo() usage"
        );

        println!("✅ Complex FHIRPath patterns correctly detected:");
        for msg in warnings {
            if msg.contains("FHIRPath expression") {
                println!("  ⚠️ {msg}");
            }
        }
    }

    #[test]
    fn test_parentheses_validation() {
        let processor = ConstraintProcessor::new();
        let mut context = ConversionContext::new(&ConverterConfig::default());

        // Test expressions with unmatched parentheses
        let test_elements = vec![
            create_element_with_constraint(
                "Patient.name",
                "pat-unmatched-1",
                "error",
                "Unmatched opening paren",
                Some("name.where(use = 'official'.exists()"), // Missing closing paren
                None,
            ),
            create_element_with_constraint(
                "Patient.contact",
                "pat-unmatched-2",
                "error",
                "Unmatched closing paren",
                Some("contact.all(name.exists()))"), // Extra closing paren
                None,
            ),
            create_element_with_constraint(
                "Patient.telecom",
                "pat-matched",
                "error",
                "Properly matched parens",
                Some("telecom.where(system = 'email').value.exists()"), // Correct
                None,
            ),
        ];

        let result = processor.process_constraints(&test_elements, &mut context);

        // Should still succeed (warnings, not errors)
        assert!(
            result.is_ok(),
            "Should process with paren validation warnings"
        );

        let errors = &context.get_stats().errors;

        // Should detect unmatched parentheses
        assert!(
            errors
                .iter()
                .any(|msg| msg.contains("Unmatched parentheses")),
            "Should detect unmatched parentheses"
        );

        // Should have detected both unmatched cases
        let unmatched_errors: Vec<_> = errors
            .iter()
            .filter(|msg| msg.contains("Unmatched parentheses"))
            .collect();
        assert_eq!(
            unmatched_errors.len(),
            2,
            "Should detect both unmatched parentheses cases"
        );

        println!("✅ Parentheses validation working correctly:");
        for error in &unmatched_errors {
            println!("  ❌ {error}");
        }
    }

    // Helper function to create ElementDefinition with constraint
    fn create_element_with_constraint(
        path: &str,
        key: &str,
        severity: &str,
        human: &str,
        expression: Option<&str>,
        xpath: Option<&str>,
    ) -> ElementDefinition {
        let constraint = ElementDefinitionConstraint {
            key: key.to_string(),
            requirements: None,
            severity: severity.to_string(),
            human: human.to_string(),
            expression: expression.map(|s| s.to_string()),
            xpath: xpath.map(|s| s.to_string()),
            source: None,
        };

        ElementDefinition {
            id: None,
            path: path.to_string(),
            representation: None,
            slice_name: None,
            slice_is_constraining: None,
            label: None,
            code: None,
            slicing: None,
            short: None,
            definition: None,
            comment: None,
            requirements: None,
            alias: None,
            min: None,
            max: None,
            base: None,
            content_reference: None,
            element_type: None,
            name_reference: None,
            default_value: None,
            meaning_when_missing: None,
            order_meaning: None,
            fixed_value: None,
            pattern_value: None,
            example: None,
            min_value: None,
            max_value: None,
            max_length: None,
            condition: None,
            constraint: Some(vec![constraint]),
            must_support: None,
            is_modifier: None,
            is_modifier_reason: None,
            is_summary: None,
            binding: None,
            mapping: None,
        }
    }
}
