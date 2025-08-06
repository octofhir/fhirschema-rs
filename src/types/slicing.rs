use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Slicing {
    pub discriminator: Vec<Discriminator>,
    pub description: Option<String>,
    pub ordered: Option<bool>,
    pub rules: String,

    #[serde(flatten)]
    pub extensions: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, bincode::Encode, bincode::Decode)]
pub struct Discriminator {
    #[serde(rename = "type")]
    pub discriminator_type: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Slice {
    pub name: String,
    pub definition: Option<String>,
    pub short: Option<String>,
    pub comment: Option<String>,

    pub min: Option<u32>,
    pub max: Option<String>,

    #[serde(flatten)]
    pub extensions: HashMap<String, serde_json::Value>,
}

impl Slicing {
    pub fn new(rules: impl Into<String>) -> Self {
        Self {
            discriminator: Vec::new(),
            description: None,
            ordered: None,
            rules: rules.into(),
            extensions: HashMap::new(),
        }
    }

    pub fn with_discriminator(mut self, discriminator: Discriminator) -> Self {
        self.discriminator.push(discriminator);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn validate(&self) -> crate::Result<()> {
        let valid_rules = ["open", "closed", "openAtEnd"];
        if !valid_rules.contains(&self.rules.as_str()) {
            return Err(crate::FhirSchemaError::Validation {
                message: format!("Invalid slicing rules: {}", self.rules),
            });
        }

        if self.discriminator.is_empty() {
            return Err(crate::FhirSchemaError::Validation {
                message: "Slicing must have at least one discriminator".to_string(),
            });
        }

        for discriminator in &self.discriminator {
            discriminator.validate()?;
        }

        Ok(())
    }
}

impl Discriminator {
    pub fn new(discriminator_type: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            discriminator_type: discriminator_type.into(),
            path: path.into(),
        }
    }

    pub fn validate(&self) -> crate::Result<()> {
        let valid_types = ["value", "exists", "pattern", "type", "profile"];
        if !valid_types.contains(&self.discriminator_type.as_str()) {
            return Err(crate::FhirSchemaError::Validation {
                message: format!("Invalid discriminator type: {}", self.discriminator_type),
            });
        }

        if self.path.is_empty() {
            return Err(crate::FhirSchemaError::Validation {
                message: "Discriminator path cannot be empty".to_string(),
            });
        }

        Ok(())
    }
}

impl Slice {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            definition: None,
            short: None,
            comment: None,
            min: None,
            max: None,
            extensions: HashMap::new(),
        }
    }

    pub fn with_cardinality(mut self, min: u32, max: impl Into<String>) -> Self {
        self.min = Some(min);
        self.max = Some(max.into());
        self
    }
}
