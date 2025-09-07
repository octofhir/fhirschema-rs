use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchema {
    #[serde(rename = "type")]
    pub schema_type: String,

    pub properties: HashMap<String, FhirSchemaProperty>,

    pub required: Vec<String>,

    pub additional_properties: Option<bool>,

    #[serde(rename = "$schema")]
    pub json_schema_version: Option<String>,

    pub title: Option<String>,

    pub description: Option<String>,

    #[serde(rename = "$id")]
    pub id: Option<String>,

    pub constraints: Vec<FhirConstraint>,

    pub metadata: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaProperty {
    #[serde(rename = "type")]
    pub property_type: Option<String>,

    #[serde(rename = "$ref")]
    pub reference: Option<String>,

    pub items: Option<Box<FhirSchemaProperty>>,

    pub properties: Option<HashMap<String, FhirSchemaProperty>>,

    pub description: Option<String>,

    pub required: Option<Vec<String>>,

    pub minimum: Option<f64>,
    pub maximum: Option<f64>,

    pub min_length: Option<usize>,
    pub max_length: Option<usize>,

    pub pattern: Option<String>,

    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<Value>>,

    pub format: Option<String>,

    pub constraints: Vec<FhirConstraint>,

    pub metadata: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirConstraint {
    pub key: String,

    pub severity: ConstraintSeverity,

    pub human: String,

    pub expression: Option<String>,

    pub xpath: Option<String>,

    pub source: Option<String>,

    pub metadata: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintSeverity {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "information")]
    Information,
}

impl FhirSchema {
    pub fn new(schema_type: &str) -> Self {
        Self {
            schema_type: schema_type.to_string(),
            properties: HashMap::new(),
            required: Vec::new(),
            additional_properties: Some(false),
            json_schema_version: Some("https://json-schema.org/draft/2020-12/schema".to_string()),
            title: None,
            description: None,
            id: None,
            constraints: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    pub fn add_property(&mut self, name: &str, property: FhirSchemaProperty) {
        self.properties.insert(name.to_string(), property);
    }

    pub fn add_required(&mut self, field_name: &str) {
        if !self.required.contains(&field_name.to_string()) {
            self.required.push(field_name.to_string());
        }
    }

    pub fn add_constraint(&mut self, constraint: FhirConstraint) {
        self.constraints.push(constraint);
    }

    pub fn get_property(&self, name: &str) -> Option<&FhirSchemaProperty> {
        self.properties.get(name)
    }

    pub fn is_required(&self, field_name: &str) -> bool {
        self.required.contains(&field_name.to_string())
    }

    pub fn apply_metadata(&mut self, metadata: HashMap<String, Value>) {
        self.metadata.extend(metadata);
    }

    pub fn apply_elements(&mut self, elements: HashMap<String, FhirSchemaProperty>) {
        self.properties.extend(elements);
    }

    pub fn apply_constraints(&mut self, constraints: Vec<FhirConstraint>) {
        self.constraints.extend(constraints);
    }
}

impl FhirSchemaProperty {
    pub fn string() -> Self {
        Self {
            property_type: Some("string".to_string()),
            reference: None,
            items: None,
            properties: None,
            description: None,
            required: None,
            minimum: None,
            maximum: None,
            min_length: None,
            max_length: None,
            pattern: None,
            enum_values: None,
            format: None,
            constraints: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn integer() -> Self {
        Self {
            property_type: Some("integer".to_string()),
            ..Self::string()
        }
    }

    pub fn number() -> Self {
        Self {
            property_type: Some("number".to_string()),
            ..Self::string()
        }
    }

    pub fn boolean() -> Self {
        Self {
            property_type: Some("boolean".to_string()),
            ..Self::string()
        }
    }

    pub fn object() -> Self {
        Self {
            property_type: Some("object".to_string()),
            properties: Some(HashMap::new()),
            ..Self::string()
        }
    }

    pub fn array(item_type: FhirSchemaProperty) -> Self {
        Self {
            property_type: Some("array".to_string()),
            items: Some(Box::new(item_type)),
            ..Self::string()
        }
    }

    pub fn reference(ref_url: &str) -> Self {
        Self {
            property_type: None,
            reference: Some(ref_url.to_string()),
            ..Self::string()
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_pattern(mut self, pattern: &str) -> Self {
        self.pattern = Some(pattern.to_string());
        self
    }

    pub fn with_min_length(mut self, min_length: usize) -> Self {
        self.min_length = Some(min_length);
        self
    }

    pub fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    pub fn with_format(mut self, format: &str) -> Self {
        self.format = Some(format.to_string());
        self
    }

    pub fn add_constraint(&mut self, constraint: FhirConstraint) {
        self.constraints.push(constraint);
    }
}

impl FhirConstraint {
    pub fn new(key: &str, severity: ConstraintSeverity, human: &str) -> Self {
        Self {
            key: key.to_string(),
            severity,
            human: human.to_string(),
            expression: None,
            xpath: None,
            source: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_expression(mut self, expression: &str) -> Self {
        self.expression = Some(expression.to_string());
        self
    }

    pub fn with_xpath(mut self, xpath: &str) -> Self {
        self.xpath = Some(xpath.to_string());
        self
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }
}

impl Default for FhirSchema {
    fn default() -> Self {
        Self::new("object")
    }
}
