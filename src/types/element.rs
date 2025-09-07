// Enhanced element type definitions with comprehensive cardinality and constraint support

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementDefinition {
    pub path: String,
    pub element_type: Option<Vec<ElementType>>,
    pub cardinality: Cardinality,
    pub constraints: Vec<crate::types::FhirConstraint>,
    pub slicing: Option<SlicingDefinition>,
    pub binding: Option<BindingDefinition>,
    pub fixed_value: Option<serde_json::Value>,
    pub pattern_value: Option<serde_json::Value>,
    pub default_value: Option<serde_json::Value>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementType {
    pub code: String,
    pub profile: Option<Vec<String>>,
    pub target_profile: Option<Vec<String>>,
    pub aggregation: Option<Vec<AggregationMode>>,
    pub versioning: Option<ReferenceVersionRules>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cardinality {
    pub min: u32,
    pub max: Option<u32>, // None means unbounded
    pub must_support: bool,
    pub is_modifier: bool,
    pub is_summary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlicingDefinition {
    pub discriminator: Vec<SlicingDiscriminator>,
    pub description: Option<String>,
    pub ordered: bool,
    pub rules: SlicingRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlicingDiscriminator {
    pub discriminator_type: DiscriminatorType,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscriminatorType {
    #[serde(rename = "value")]
    Value,
    #[serde(rename = "exists")]
    Exists,
    #[serde(rename = "pattern")]
    Pattern,
    #[serde(rename = "type")]
    Type,
    #[serde(rename = "profile")]
    Profile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SlicingRules {
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "openAtEnd")]
    OpenAtEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingDefinition {
    pub strength: BindingStrength,
    pub description: Option<String>,
    pub value_set: Option<String>,
    pub additional_bindings: Vec<AdditionalBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BindingStrength {
    #[serde(rename = "required")]
    Required,
    #[serde(rename = "extensible")]
    Extensible,
    #[serde(rename = "preferred")]
    Preferred,
    #[serde(rename = "example")]
    Example,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalBinding {
    pub purpose: BindingPurpose,
    pub value_set: String,
    pub documentation: Option<String>,
    pub shortcut: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BindingPurpose {
    #[serde(rename = "maximum")]
    Maximum,
    #[serde(rename = "minimum")]
    Minimum,
    #[serde(rename = "required")]
    Required,
    #[serde(rename = "extensible")]
    Extensible,
    #[serde(rename = "candidate")]
    Candidate,
    #[serde(rename = "current")]
    Current,
    #[serde(rename = "preferred")]
    Preferred,
    #[serde(rename = "ui")]
    Ui,
    #[serde(rename = "starter")]
    Starter,
    #[serde(rename = "component")]
    Component,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationMode {
    #[serde(rename = "contained")]
    Contained,
    #[serde(rename = "referenced")]
    Referenced,
    #[serde(rename = "bundled")]
    Bundled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReferenceVersionRules {
    #[serde(rename = "either")]
    Either,
    #[serde(rename = "independent")]
    Independent,
    #[serde(rename = "specific")]
    Specific,
}

/// Enhanced cardinality validator
#[derive(Debug, Clone)]
pub struct CardinalityValidator {
    pub min_violations: Vec<String>,
    pub max_violations: Vec<String>,
    pub modifier_violations: Vec<String>,
}

/// Constraint evaluation engine for element definitions
#[derive(Debug, Clone)]
pub struct ElementConstraintEvaluator {
    pub constraints: Vec<ConstraintEvaluation>,
    pub binding_violations: Vec<String>,
    pub slicing_violations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ConstraintEvaluation {
    pub constraint_key: String,
    pub severity: crate::types::ConstraintSeverity,
    pub satisfied: bool,
    pub message: String,
    pub path: String,
}

impl ElementDefinition {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            element_type: None,
            cardinality: Cardinality::default(),
            constraints: Vec::new(),
            slicing: None,
            binding: None,
            fixed_value: None,
            pattern_value: None,
            default_value: None,
            metadata: HashMap::new(),
        }
    }

    /// Create an element with specific cardinality
    pub fn with_cardinality(mut self, min: u32, max: Option<u32>) -> Self {
        self.cardinality.min = min;
        self.cardinality.max = max;
        self
    }

    /// Mark element as must support
    pub fn with_must_support(mut self, must_support: bool) -> Self {
        self.cardinality.must_support = must_support;
        self
    }

    /// Mark element as modifier
    pub fn with_modifier(mut self, is_modifier: bool) -> Self {
        self.cardinality.is_modifier = is_modifier;
        self
    }

    /// Mark element as summary
    pub fn with_summary(mut self, is_summary: bool) -> Self {
        self.cardinality.is_summary = is_summary;
        self
    }

    /// Add element type
    pub fn add_element_type(&mut self, element_type: ElementType) {
        if self.element_type.is_none() {
            self.element_type = Some(Vec::new());
        }
        if let Some(ref mut types) = self.element_type {
            types.push(element_type);
        }
    }

    /// Add constraint
    pub fn add_constraint(&mut self, constraint: crate::types::FhirConstraint) {
        self.constraints.push(constraint);
    }

    /// Set slicing definition
    pub fn with_slicing(mut self, slicing: SlicingDefinition) -> Self {
        self.slicing = Some(slicing);
        self
    }

    /// Set binding definition
    pub fn with_binding(mut self, binding: BindingDefinition) -> Self {
        self.binding = Some(binding);
        self
    }

    /// Set fixed value
    pub fn with_fixed_value(mut self, value: serde_json::Value) -> Self {
        self.fixed_value = Some(value);
        self
    }

    /// Set pattern value
    pub fn with_pattern_value(mut self, value: serde_json::Value) -> Self {
        self.pattern_value = Some(value);
        self
    }

    /// Set default value
    pub fn with_default_value(mut self, value: serde_json::Value) -> Self {
        self.default_value = Some(value);
        self
    }

    /// Check if element is required
    pub fn is_required(&self) -> bool {
        self.cardinality.min > 0
    }

    /// Check if element is optional
    pub fn is_optional(&self) -> bool {
        self.cardinality.min == 0
    }

    /// Check if element is repeating
    pub fn is_repeating(&self) -> bool {
        self.cardinality.max.is_none_or(|max| max > 1)
    }

    /// Check if element is choice type
    pub fn is_choice_type(&self) -> bool {
        self.path.contains("[x]")
    }

    /// Check if element is sliced
    pub fn is_sliced(&self) -> bool {
        self.slicing.is_some()
    }

    /// Get maximum cardinality as string
    pub fn max_cardinality_string(&self) -> String {
        match self.cardinality.max {
            Some(max) => max.to_string(),
            None => "*".to_string(),
        }
    }

    /// Validate cardinality against actual count
    pub fn validate_cardinality(&self, actual_count: u32) -> CardinalityValidator {
        let mut validator = CardinalityValidator {
            min_violations: Vec::new(),
            max_violations: Vec::new(),
            modifier_violations: Vec::new(),
        };

        // Check minimum cardinality
        if actual_count < self.cardinality.min {
            validator.min_violations.push(format!(
                "Element '{}' requires minimum {} occurrences, but found {}",
                self.path, self.cardinality.min, actual_count
            ));
        }

        // Check maximum cardinality
        if let Some(max) = self.cardinality.max {
            if actual_count > max {
                validator.max_violations.push(format!(
                    "Element '{}' allows maximum {} occurrences, but found {}",
                    self.path, max, actual_count
                ));
            }
        }

        // Check modifier element requirements
        if self.cardinality.is_modifier && actual_count == 0 {
            validator.modifier_violations.push(format!(
                "Modifier element '{}' must be present when its parent element is present",
                self.path
            ));
        }

        validator
    }

    /// Evaluate element constraints
    pub fn evaluate_constraints(&self, value: &serde_json::Value) -> ElementConstraintEvaluator {
        let mut evaluator = ElementConstraintEvaluator {
            constraints: Vec::new(),
            binding_violations: Vec::new(),
            slicing_violations: Vec::new(),
        };

        // Evaluate each constraint
        for constraint in &self.constraints {
            let evaluation = ConstraintEvaluation {
                constraint_key: constraint.key.clone(),
                severity: constraint.severity.clone(),
                satisfied: self.evaluate_single_constraint(constraint, value),
                message: constraint.human.clone(),
                path: self.path.clone(),
            };
            evaluator.constraints.push(evaluation);
        }

        // Evaluate binding constraints
        if let Some(binding) = &self.binding {
            if let Err(violation) = self.evaluate_binding_constraint(binding, value) {
                evaluator.binding_violations.push(violation);
            }
        }

        // Evaluate fixed value constraints
        if let Some(fixed_value) = &self.fixed_value {
            if value != fixed_value {
                evaluator.constraints.push(ConstraintEvaluation {
                    constraint_key: "fixed-value".to_string(),
                    severity: crate::types::ConstraintSeverity::Error,
                    satisfied: false,
                    message: format!("Value must be exactly {fixed_value}"),
                    path: self.path.clone(),
                });
            }
        }

        evaluator
    }

    /// Evaluate a single constraint
    fn evaluate_single_constraint(
        &self,
        constraint: &crate::types::FhirConstraint,
        _value: &serde_json::Value,
    ) -> bool {
        // This is a placeholder for FHIRPath evaluation
        // In a real implementation, this would evaluate the FHIRPath expression
        // For now, we'll assume constraints are satisfied
        constraint.expression.is_some()
    }

    /// Evaluate binding constraint
    fn evaluate_binding_constraint(
        &self,
        binding: &BindingDefinition,
        _value: &serde_json::Value,
    ) -> Result<(), String> {
        // This is a placeholder for binding validation
        // In a real implementation, this would check if the value is in the bound ValueSet
        match binding.strength {
            BindingStrength::Required => {
                // For required bindings, the value must be from the ValueSet
                // This is a placeholder implementation
                Ok(())
            }
            BindingStrength::Extensible => {
                // For extensible bindings, the value should be from the ValueSet if possible
                Ok(())
            }
            BindingStrength::Preferred | BindingStrength::Example => {
                // For preferred/example bindings, any value is allowed
                Ok(())
            }
        }
    }
}

impl Default for Cardinality {
    fn default() -> Self {
        Self {
            min: 0,
            max: Some(1),
            must_support: false,
            is_modifier: false,
            is_summary: false,
        }
    }
}

impl ElementType {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_string(),
            profile: None,
            target_profile: None,
            aggregation: None,
            versioning: None,
        }
    }

    pub fn with_profile(mut self, profile: &str) -> Self {
        if self.profile.is_none() {
            self.profile = Some(Vec::new());
        }
        if let Some(ref mut profiles) = self.profile {
            profiles.push(profile.to_string());
        }
        self
    }

    pub fn with_target_profile(mut self, target_profile: &str) -> Self {
        if self.target_profile.is_none() {
            self.target_profile = Some(Vec::new());
        }
        if let Some(ref mut profiles) = self.target_profile {
            profiles.push(target_profile.to_string());
        }
        self
    }
}

impl SlicingDefinition {
    pub fn new(rules: SlicingRules) -> Self {
        Self {
            discriminator: Vec::new(),
            description: None,
            ordered: false,
            rules,
        }
    }

    pub fn add_discriminator(&mut self, discriminator_type: DiscriminatorType, path: &str) {
        self.discriminator.push(SlicingDiscriminator {
            discriminator_type,
            path: path.to_string(),
        });
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_ordered(mut self, ordered: bool) -> Self {
        self.ordered = ordered;
        self
    }
}

impl BindingDefinition {
    pub fn new(strength: BindingStrength) -> Self {
        Self {
            strength,
            description: None,
            value_set: None,
            additional_bindings: Vec::new(),
        }
    }

    pub fn with_value_set(mut self, value_set: &str) -> Self {
        self.value_set = Some(value_set.to_string());
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn add_additional_binding(&mut self, binding: AdditionalBinding) {
        self.additional_bindings.push(binding);
    }
}
