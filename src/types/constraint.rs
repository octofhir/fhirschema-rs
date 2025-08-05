use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Constraint {
    pub key: String,
    pub severity: String,
    pub human: String,
    pub expression: String,
    pub xpath: Option<String>,
    pub source: Option<String>,

    #[serde(flatten)]
    pub extensions: HashMap<String, serde_json::Value>,
}

impl Constraint {
    pub fn new(
        key: impl Into<String>,
        severity: impl Into<String>,
        human: impl Into<String>,
        expression: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            severity: severity.into(),
            human: human.into(),
            expression: expression.into(),
            xpath: None,
            source: None,
            extensions: HashMap::new(),
        }
    }

    pub fn with_xpath(mut self, xpath: impl Into<String>) -> Self {
        self.xpath = Some(xpath.into());
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.key.is_empty() {
            return Err(crate::FhirSchemaError::Validation {
                message: "Constraint key cannot be empty".to_string(),
            });
        }

        if self.expression.is_empty() {
            return Err(crate::FhirSchemaError::Validation {
                message: "Constraint expression cannot be empty".to_string(),
            });
        }

        let valid_severities = ["error", "warning", "information"];
        if !valid_severities.contains(&self.severity.as_str()) {
            return Err(crate::FhirSchemaError::Validation {
                message: format!("Invalid constraint severity: {}", self.severity),
            });
        }

        Ok(())
    }
}
