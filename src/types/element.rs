use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use url::Url;

use super::Constraint;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Element {
    pub path: String,
    pub definition: Option<String>,
    pub short: Option<String>,
    pub comment: Option<String>,

    pub min: Option<u32>,
    pub max: Option<String>,

    #[serde(rename = "type")]
    pub element_type: Option<Vec<ElementType>>,

    pub fixed: Option<serde_json::Value>,
    pub pattern: Option<serde_json::Value>,

    #[serde(default)]
    pub constraints: Vec<Constraint>,

    pub binding: Option<Binding>,

    #[serde(default)]
    pub mapping: Vec<Mapping>,

    #[serde(default)]
    pub is_modifier: bool,

    #[serde(default)]
    pub is_summary: bool,

    #[serde(flatten)]
    pub extensions: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementType {
    pub code: String,
    pub profile: Option<Vec<Url>>,
    pub target_profile: Option<Vec<Url>>,
    pub aggregation: Option<Vec<String>>,
    pub versioning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Binding {
    pub strength: String,
    pub description: Option<String>,
    pub value_set: Option<Url>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, bincode::Encode, bincode::Decode)]
pub struct Mapping {
    pub identity: String,
    pub language: Option<String>,
    pub map: String,
    pub comment: Option<String>,
}

impl Element {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            definition: None,
            short: None,
            comment: None,
            min: None,
            max: None,
            element_type: None,
            fixed: None,
            pattern: None,
            constraints: Vec::new(),
            binding: None,
            mapping: Vec::new(),
            is_modifier: false,
            is_summary: false,
            extensions: HashMap::new(),
        }
    }

    pub fn with_type(mut self, element_type: ElementType) -> Self {
        match &mut self.element_type {
            Some(types) => types.push(element_type),
            None => self.element_type = Some(vec![element_type]),
        }
        self
    }

    pub fn with_cardinality(mut self, min: u32, max: impl Into<String>) -> Self {
        self.min = Some(min);
        self.max = Some(max.into());
        self
    }

    pub fn with_binding(mut self, binding: Binding) -> Self {
        self.binding = Some(binding);
        self
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.path.is_empty() {
            return Err(crate::FhirSchemaError::Validation {
                message: "Element path cannot be empty".to_string(),
            });
        }

        if let Some(min) = self.min {
            if let Some(max_str) = &self.max {
                if max_str != "*" {
                    if let Ok(max_num) = max_str.parse::<u32>() {
                        if min > max_num {
                            return Err(crate::FhirSchemaError::Validation {
                                message: format!(
                                    "Min cardinality ({min}) cannot be greater than max ({max_num})"
                                ),
                            });
                        }
                    }
                }
            }
        }

        for constraint in &self.constraints {
            constraint.validate()?;
        }

        Ok(())
    }
}

impl ElementType {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            profile: None,
            target_profile: None,
            aggregation: None,
            versioning: None,
        }
    }

    pub fn with_profile(mut self, profile: Url) -> Self {
        match &mut self.profile {
            Some(profiles) => profiles.push(profile),
            None => self.profile = Some(vec![profile]),
        }
        self
    }
}

impl Binding {
    pub fn new(strength: impl Into<String>) -> Self {
        Self {
            strength: strength.into(),
            description: None,
            value_set: None,
        }
    }

    pub fn with_value_set(mut self, value_set: Url) -> Self {
        self.value_set = Some(value_set);
        self
    }
}

impl fmt::Display for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Element({})", self.path)?;
        if let Some(types) = &self.element_type {
            if !types.is_empty() {
                write!(f, ": {}", types[0].code)?;
                if types.len() > 1 {
                    write!(f, " | {} more", types.len() - 1)?;
                }
            }
        }
        if let Some(min) = self.min {
            write!(f, " [{}..{}]", min, self.max.as_deref().unwrap_or("*"))?;
        }
        Ok(())
    }
}
