use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use url::Url;

use super::{Constraint, Element, Slicing};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FhirSchema {
    #[serde(rename = "$schema")]
    pub schema_version: Option<String>,

    pub url: Option<Url>,
    pub name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub status: Option<String>,

    #[serde(rename = "type")]
    pub schema_type: String,

    // Classification fields according to converter specification
    pub kind: Option<String>,
    pub class: Option<String>,
    pub base: Option<Url>,
    #[serde(rename = "abstract")]
    pub abstract_type: Option<bool>,

    // Legacy fields for backward compatibility
    pub base_definition: Option<Url>,
    pub derivation: Option<String>,

    pub elements: HashMap<String, Element>,

    #[serde(default)]
    pub constraints: Vec<Constraint>,

    #[serde(default)]
    pub slicing: HashMap<String, Slicing>,

    #[serde(flatten)]
    pub extensions: HashMap<String, serde_json::Value>,
}

impl FhirSchema {
    pub fn new(schema_type: impl Into<String>) -> Self {
        Self {
            schema_version: Some("https://json-schema.org/draft/2020-12/schema".to_string()),
            url: None,
            name: None,
            title: None,
            description: None,
            version: None,
            status: None,
            schema_type: schema_type.into(),
            kind: None,
            class: None,
            base: None,
            abstract_type: None,
            base_definition: None,
            derivation: None,
            elements: HashMap::new(),
            constraints: Vec::new(),
            slicing: HashMap::new(),
            extensions: HashMap::new(),
        }
    }

    pub fn with_url(mut self, url: Url) -> Self {
        self.url = Some(url);
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_element(mut self, path: impl Into<String>, element: Element) -> Self {
        self.elements.insert(path.into(), element);
        self
    }

    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }

    pub fn with_base(mut self, base: Url) -> Self {
        self.base = Some(base);
        self
    }

    pub fn with_abstract(mut self, abstract_type: bool) -> Self {
        self.abstract_type = Some(abstract_type);
        self
    }

    pub fn validate_structure(&self) -> crate::Result<()> {
        if self.schema_type.is_empty() {
            return Err(crate::FhirSchemaError::Validation {
                message: "Schema type cannot be empty".to_string(),
            });
        }

        for (path, element) in &self.elements {
            element.validate()?;
            if path.is_empty() {
                return Err(crate::FhirSchemaError::Validation {
                    message: "Element path cannot be empty".to_string(),
                });
            }
        }

        for constraint in &self.constraints {
            constraint.validate()?;
        }

        Ok(())
    }
}

impl fmt::Display for FhirSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FhirSchema({})", self.schema_type)?;
        if let Some(name) = &self.name {
            write!(f, " - {name}")?;
        }
        if let Some(url) = &self.url {
            write!(f, " [{url}]")?;
        }
        Ok(())
    }
}

impl bincode::Encode for FhirSchema {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        let json_bytes = serde_json::to_vec(self)
            .map_err(|_| bincode::error::EncodeError::Other("JSON serialization failed"))?;
        json_bytes.encode(encoder)
    }
}

impl<Context> bincode::Decode<Context> for FhirSchema {
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let json_bytes = Vec::<u8>::decode(decoder)?;
        serde_json::from_slice(&json_bytes)
            .map_err(|_| bincode::error::DecodeError::Other("JSON deserialization failed"))
    }
}

impl<'de, Context> bincode::BorrowDecode<'de, Context> for FhirSchema {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let json_bytes = Vec::<u8>::borrow_decode(decoder)?;
        serde_json::from_slice(&json_bytes)
            .map_err(|_| bincode::error::DecodeError::Other("JSON deserialization failed"))
    }
}
