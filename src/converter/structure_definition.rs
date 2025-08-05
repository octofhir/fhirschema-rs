use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StructureDefinition {
    #[serde(rename = "resourceType")]
    pub resource_type: String,

    pub id: Option<String>,
    pub url: Option<Url>,
    pub identifier: Option<Vec<Identifier>>,
    pub version: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub experimental: Option<bool>,
    pub date: Option<String>,
    pub publisher: Option<String>,
    pub contact: Option<Vec<ContactDetail>>,
    pub description: Option<String>,
    pub purpose: Option<String>,
    pub copyright: Option<String>,

    pub kind: String,
    pub abstract_: Option<bool>,
    pub context: Option<Vec<StructureDefinitionContext>>,

    #[serde(rename = "type")]
    pub type_name: String,

    #[serde(rename = "baseDefinition")]
    pub base_definition: Option<Url>,

    pub derivation: Option<String>,

    pub snapshot: Option<StructureDefinitionSnapshot>,
    pub differential: Option<StructureDefinitionDifferential>,

    #[serde(skip_deserializing, skip_serializing)]
    pub elements: Vec<ElementDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StructureDefinitionSnapshot {
    pub element: Vec<ElementDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StructureDefinitionDifferential {
    pub element: Vec<ElementDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementDefinition {
    pub id: Option<String>,
    pub path: String,
    pub representation: Option<Vec<String>>,

    #[serde(rename = "sliceName")]
    pub slice_name: Option<String>,

    #[serde(rename = "sliceIsConstraining")]
    pub slice_is_constraining: Option<bool>,

    pub label: Option<String>,
    pub code: Option<Vec<Coding>>,
    pub slicing: Option<ElementDefinitionSlicing>,
    pub short: Option<String>,
    pub definition: Option<String>,
    pub comment: Option<String>,
    pub requirements: Option<String>,
    pub alias: Option<Vec<String>>,

    pub min: Option<u32>,
    pub max: Option<String>,

    pub base: Option<ElementDefinitionBase>,

    #[serde(rename = "contentReference")]
    pub content_reference: Option<String>,

    #[serde(rename = "type")]
    pub element_type: Option<Vec<ElementDefinitionType>>,

    #[serde(rename = "nameReference")]
    pub name_reference: Option<String>,

    #[serde(rename = "defaultValue")]
    pub default_value: Option<serde_json::Value>,

    #[serde(rename = "meaningWhenMissing")]
    pub meaning_when_missing: Option<String>,

    #[serde(rename = "orderMeaning")]
    pub order_meaning: Option<String>,

    #[serde(rename = "fixed")]
    pub fixed_value: Option<serde_json::Value>,

    #[serde(rename = "pattern")]
    pub pattern_value: Option<serde_json::Value>,

    pub example: Option<Vec<ElementDefinitionExample>>,

    #[serde(rename = "minValue")]
    pub min_value: Option<serde_json::Value>,

    #[serde(rename = "maxValue")]
    pub max_value: Option<serde_json::Value>,

    #[serde(rename = "maxLength")]
    pub max_length: Option<u32>,

    pub condition: Option<Vec<String>>,
    pub constraint: Option<Vec<ElementDefinitionConstraint>>,

    #[serde(rename = "mustSupport")]
    pub must_support: Option<bool>,

    #[serde(rename = "isModifier")]
    pub is_modifier: Option<bool>,

    #[serde(rename = "isModifierReason")]
    pub is_modifier_reason: Option<String>,

    #[serde(rename = "isSummary")]
    pub is_summary: Option<bool>,

    pub binding: Option<ElementDefinitionBinding>,
    pub mapping: Option<Vec<ElementDefinitionMapping>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementDefinitionSlicing {
    pub discriminator: Option<Vec<ElementDefinitionSlicingDiscriminator>>,
    pub description: Option<String>,
    pub ordered: Option<bool>,
    pub rules: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementDefinitionSlicingDiscriminator {
    #[serde(rename = "type")]
    pub discriminator_type: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementDefinitionBase {
    pub path: String,
    pub min: u32,
    pub max: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementDefinitionType {
    pub code: String,
    pub profile: Option<Vec<Url>>,

    #[serde(rename = "targetProfile")]
    pub target_profile: Option<Vec<Url>>,

    pub aggregation: Option<Vec<String>>,
    pub versioning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementDefinitionExample {
    pub label: String,

    // Handle FHIR choice type pattern for value[x]
    #[serde(rename = "valueBase64Binary")]
    pub value_base64_binary: Option<String>,
    #[serde(rename = "valueBoolean")]
    pub value_boolean: Option<bool>,
    #[serde(rename = "valueCanonical")]
    pub value_canonical: Option<String>,
    #[serde(rename = "valueCode")]
    pub value_code: Option<String>,
    #[serde(rename = "valueDate")]
    pub value_date: Option<String>,
    #[serde(rename = "valueDateTime")]
    pub value_date_time: Option<String>,
    #[serde(rename = "valueDecimal")]
    pub value_decimal: Option<f64>,
    #[serde(rename = "valueId")]
    pub value_id: Option<String>,
    #[serde(rename = "valueInstant")]
    pub value_instant: Option<String>,
    #[serde(rename = "valueInteger")]
    pub value_integer: Option<i32>,
    #[serde(rename = "valueMarkdown")]
    pub value_markdown: Option<String>,
    #[serde(rename = "valueOid")]
    pub value_oid: Option<String>,
    #[serde(rename = "valuePositiveInt")]
    pub value_positive_int: Option<u32>,
    #[serde(rename = "valueString")]
    pub value_string: Option<String>,
    #[serde(rename = "valueTime")]
    pub value_time: Option<String>,
    #[serde(rename = "valueUnsignedInt")]
    pub value_unsigned_int: Option<u32>,
    #[serde(rename = "valueUri")]
    pub value_uri: Option<String>,
    #[serde(rename = "valueUrl")]
    pub value_url: Option<String>,
    #[serde(rename = "valueUuid")]
    pub value_uuid: Option<String>,

    // Complex types
    #[serde(rename = "valueAddress")]
    pub value_address: Option<serde_json::Value>,
    #[serde(rename = "valueAge")]
    pub value_age: Option<serde_json::Value>,
    #[serde(rename = "valueAnnotation")]
    pub value_annotation: Option<serde_json::Value>,
    #[serde(rename = "valueAttachment")]
    pub value_attachment: Option<serde_json::Value>,
    #[serde(rename = "valueCodeableConcept")]
    pub value_codeable_concept: Option<serde_json::Value>,
    #[serde(rename = "valueCoding")]
    pub value_coding: Option<serde_json::Value>,
    #[serde(rename = "valueContactPoint")]
    pub value_contact_point: Option<serde_json::Value>,
    #[serde(rename = "valueCount")]
    pub value_count: Option<serde_json::Value>,
    #[serde(rename = "valueDistance")]
    pub value_distance: Option<serde_json::Value>,
    #[serde(rename = "valueDuration")]
    pub value_duration: Option<serde_json::Value>,
    #[serde(rename = "valueHumanName")]
    pub value_human_name: Option<serde_json::Value>,
    #[serde(rename = "valueIdentifier")]
    pub value_identifier: Option<serde_json::Value>,
    #[serde(rename = "valueMoney")]
    pub value_money: Option<serde_json::Value>,
    #[serde(rename = "valuePeriod")]
    pub value_period: Option<serde_json::Value>,
    #[serde(rename = "valueQuantity")]
    pub value_quantity: Option<serde_json::Value>,
    #[serde(rename = "valueRange")]
    pub value_range: Option<serde_json::Value>,
    #[serde(rename = "valueRatio")]
    pub value_ratio: Option<serde_json::Value>,
    #[serde(rename = "valueReference")]
    pub value_reference: Option<serde_json::Value>,
    #[serde(rename = "valueSampledData")]
    pub value_sampled_data: Option<serde_json::Value>,
    #[serde(rename = "valueSignature")]
    pub value_signature: Option<serde_json::Value>,
    #[serde(rename = "valueTiming")]
    pub value_timing: Option<serde_json::Value>,
    #[serde(rename = "valueContactDetail")]
    pub value_contact_detail: Option<serde_json::Value>,
    #[serde(rename = "valueContributor")]
    pub value_contributor: Option<serde_json::Value>,
    #[serde(rename = "valueDataRequirement")]
    pub value_data_requirement: Option<serde_json::Value>,
    #[serde(rename = "valueExpression")]
    pub value_expression: Option<serde_json::Value>,
    #[serde(rename = "valueParameterDefinition")]
    pub value_parameter_definition: Option<serde_json::Value>,
    #[serde(rename = "valueRelatedArtifact")]
    pub value_related_artifact: Option<serde_json::Value>,
    #[serde(rename = "valueTriggerDefinition")]
    pub value_trigger_definition: Option<serde_json::Value>,
    #[serde(rename = "valueUsageContext")]
    pub value_usage_context: Option<serde_json::Value>,
    #[serde(rename = "valueDosage")]
    pub value_dosage: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementDefinitionConstraint {
    pub key: String,
    pub requirements: Option<String>,
    pub severity: String,
    pub human: String,
    pub expression: Option<String>,
    pub xpath: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementDefinitionBinding {
    pub strength: String,
    pub description: Option<String>,

    #[serde(rename = "valueSet")]
    pub value_set: Option<Url>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementDefinitionMapping {
    pub identity: String,
    pub language: Option<String>,
    pub map: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StructureDefinitionContext {
    #[serde(rename = "type")]
    pub context_type: String,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Identifier {
    pub use_: Option<String>,
    #[serde(rename = "type")]
    pub identifier_type: Option<CodeableConcept>,
    pub system: Option<Url>,
    pub value: Option<String>,
    pub period: Option<Period>,
    pub assigner: Option<Box<Reference>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContactDetail {
    pub name: Option<String>,
    pub telecom: Option<Vec<ContactPoint>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContactPoint {
    pub system: Option<String>,
    pub value: Option<String>,
    pub use_: Option<String>,
    pub rank: Option<u32>,
    pub period: Option<Period>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeableConcept {
    pub coding: Option<Vec<Coding>>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Coding {
    pub system: Option<Url>,
    pub version: Option<String>,
    pub code: Option<String>,
    pub display: Option<String>,
    #[serde(rename = "userSelected")]
    pub user_selected: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Period {
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reference {
    pub reference: Option<String>,
    #[serde(rename = "type")]
    pub reference_type: Option<String>,
    pub identifier: Option<Box<Identifier>>,
    pub display: Option<String>,
}

impl StructureDefinition {
    pub fn new(type_name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            resource_type: "StructureDefinition".to_string(),
            id: None,
            url: None,
            identifier: None,
            version: None,
            name: None,
            title: None,
            status: None,
            experimental: None,
            date: None,
            publisher: None,
            contact: None,
            description: None,
            purpose: None,
            copyright: None,
            kind: kind.into(),
            abstract_: None,
            context: None,
            type_name: type_name.into(),
            base_definition: None,
            derivation: None,
            snapshot: None,
            differential: None,
            elements: Vec::new(),
        }
    }

    pub fn with_url(mut self, url: Url) -> Self {
        self.url = Some(url);
        self
    }

    pub fn with_elements(mut self, elements: Vec<ElementDefinition>) -> Self {
        self.elements = elements;
        self
    }

    pub fn extract_elements(&mut self) -> crate::Result<()> {
        if let Some(snapshot) = &self.snapshot {
            self.elements = snapshot.element.clone();
        } else if let Some(differential) = &self.differential {
            self.elements = differential.element.clone();
        } else {
            return Err(crate::FhirSchemaError::Conversion {
                message: "StructureDefinition has neither snapshot nor differential".to_string(),
            });
        }
        Ok(())
    }
}
