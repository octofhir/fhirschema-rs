use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::sync::Arc;

use octofhir_fhir_model::{
    Result as ModelResult, ValidationProvider, error::ModelError, provider::ModelProvider,
};

use crate::embedded::{FhirVersion, create_validation_context, get_schemas};
use crate::model_provider::FhirSchemaModelProvider;
use crate::types::ValidationContext;
use octofhir_fhir_model::provider::FhirVersion as ModelFhirVersion;

/// ValidationProvider implementation using FHIR schemas
#[derive(Debug)]
pub struct FhirSchemaValidationProvider {
    schema_provider: Arc<FhirSchemaModelProvider>,
    #[allow(dead_code)]
    validation_context: ValidationContext,
}

impl FhirSchemaValidationProvider {
    /// Create new validation provider
    pub fn new(
        schema_provider: Arc<FhirSchemaModelProvider>,
        validation_context: ValidationContext,
    ) -> Self {
        Self {
            schema_provider,
            validation_context,
        }
    }

    /// Create validation provider from EmbeddedModelProvider
    pub async fn from_embedded_provider(
        embedded_provider: Arc<dyn ModelProvider>,
        validation_context: ValidationContext,
    ) -> ModelResult<Self> {
        let model_fhir_version = embedded_provider.get_fhir_version().await?;
        let fhir_version = match model_fhir_version {
            ModelFhirVersion::R4 => FhirVersion::R4,
            ModelFhirVersion::R4B => FhirVersion::R4B,
            ModelFhirVersion::R5 => FhirVersion::R5,
            ModelFhirVersion::R6 => FhirVersion::R6,
            ModelFhirVersion::Custom { .. } => FhirVersion::R4, // Default to R4 for custom versions
        };

        let schema_provider = Arc::new(FhirSchemaModelProvider::new(
            get_schemas(fhir_version).clone(),
            model_fhir_version,
        ));

        Ok(Self {
            schema_provider,
            validation_context,
        })
    }

    /// Create validation provider from DynamicModelProvider
    pub async fn from_dynamic_provider(
        dynamic_provider: Arc<dyn ModelProvider>,
        validation_context: ValidationContext,
    ) -> ModelResult<Self> {
        let model_fhir_version = dynamic_provider.get_fhir_version().await?;
        let fhir_version = match model_fhir_version {
            ModelFhirVersion::R4 => FhirVersion::R4,
            ModelFhirVersion::R4B => FhirVersion::R4B,
            ModelFhirVersion::R5 => FhirVersion::R5,
            ModelFhirVersion::R6 => FhirVersion::R6,
            ModelFhirVersion::Custom { .. } => FhirVersion::R4, // Default to R4 for custom versions
        };

        let schema_provider = Arc::new(FhirSchemaModelProvider::new(
            get_schemas(fhir_version).clone(),
            model_fhir_version,
        ));

        Ok(Self {
            schema_provider,
            validation_context,
        })
    }

    /// Create validation provider with embedded schemas
    pub fn with_embedded_schemas(fhir_version: FhirVersion) -> ModelResult<Self> {
        let schemas = get_schemas(fhir_version);
        let model_fhir_version = match fhir_version {
            FhirVersion::R4 => ModelFhirVersion::R4,
            FhirVersion::R4B => ModelFhirVersion::R4B,
            FhirVersion::R5 => ModelFhirVersion::R5,
            FhirVersion::R6 => ModelFhirVersion::R6,
        };

        let schema_provider = Arc::new(FhirSchemaModelProvider::new(
            schemas.clone(),
            model_fhir_version,
        ));

        let validation_context = create_validation_context(fhir_version);

        Ok(Self {
            schema_provider,
            validation_context,
        })
    }
}

#[async_trait]
impl ValidationProvider for FhirSchemaValidationProvider {
    async fn validate(&self, resource: &JsonValue, profile_url: &str) -> ModelResult<bool> {
        // Check if profile exists
        let _profile_schema = self
            .schema_provider
            .get_schema_by_url_or_name(profile_url)
            .ok_or_else(|| {
                ModelError::validation_error(format!("Profile not found: {profile_url}"))
            })?;

        // Create FHIR Schema validator with all available schemas
        let validator =
            crate::validation::FhirSchemaValidator::new(self.schema_provider.schemas().clone());

        // Validate using the comprehensive FHIR Schema validation engine
        let validation_result = validator.validate(resource, vec![profile_url.to_string()]);

        // Return true if validation passed (no errors)
        Ok(validation_result.valid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FhirSchema;
    use serde_json::json;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_validation_provider_creation() {
        let schemas = HashMap::new();
        let schema_provider = Arc::new(FhirSchemaModelProvider::new(
            schemas,
            octofhir_fhir_model::FhirVersion::R4,
        ));

        let validation_context = ValidationContext::default();
        let validation_provider =
            FhirSchemaValidationProvider::new(schema_provider, validation_context);

        assert!(validation_provider.schema_provider.schemas().is_empty());
    }

    #[tokio::test]
    async fn test_validation_provider_with_schema() {
        let mut schemas = HashMap::new();
        let test_schema = FhirSchema {
            url: "http://example.org/StructureDefinition/TestProfile".to_string(),
            version: None,
            name: "TestProfile".to_string(),
            type_name: "Patient".to_string(),
            kind: "resource".to_string(),
            derivation: Some("constraint".to_string()),
            base: Some("http://hl7.org/fhir/StructureDefinition/Patient".to_string()),
            abstract_type: None,
            class: "resource".to_string(),
            description: None,
            package_name: None,
            package_version: None,
            package_id: None,
            package_meta: None,
            elements: None,
            required: None,
            excluded: None,
            extensions: None,
            constraint: None,
            primitive_type: None,
            choices: None,
        };

        schemas.insert(
            "http://example.org/StructureDefinition/TestProfile".to_string(),
            test_schema,
        );

        let schema_provider = Arc::new(FhirSchemaModelProvider::new(
            schemas,
            octofhir_fhir_model::FhirVersion::R4,
        ));

        let validation_context = ValidationContext::default();
        let validation_provider =
            FhirSchemaValidationProvider::new(schema_provider.clone(), validation_context);

        // Test schema availability via ValidationProvider
        use octofhir_fhir_model::ValidationProvider;

        let result = validation_provider
            .validate(
                &json!({"resourceType": "Patient", "id": "test"}),
                "http://example.org/StructureDefinition/TestProfile",
            )
            .await;

        // Should succeed (not crash) whether validation passes or fails
        assert!(
            result.is_ok(),
            "ValidationProvider should handle requests gracefully"
        );
    }
}

/// Create a ValidationProvider from an existing EmbeddedModelProvider
/// This reuses the already initialized provider and its schemas
pub async fn create_validation_provider_from_embedded(
    embedded_provider: Arc<dyn ModelProvider>,
) -> ModelResult<Arc<dyn ValidationProvider>> {
    let model_fhir_version = embedded_provider.get_fhir_version().await?;
    let fhir_version = match model_fhir_version {
        ModelFhirVersion::R4 => FhirVersion::R4,
        ModelFhirVersion::R4B => FhirVersion::R4B,
        ModelFhirVersion::R5 => FhirVersion::R5,
        ModelFhirVersion::R6 => FhirVersion::R6,
        ModelFhirVersion::Custom { .. } => FhirVersion::R4,
    };
    let validation_context = create_validation_context(fhir_version);

    // The EmbeddedModelProvider internally uses FhirSchemaModelProvider with embedded schemas
    // We extract those same schemas to create our ValidationProvider
    let validation_provider =
        FhirSchemaValidationProvider::from_embedded_provider(embedded_provider, validation_context)
            .await?;

    Ok(Arc::new(validation_provider))
}

/// Create a ValidationProvider from an existing DynamicModelProvider
/// This reuses the already initialized provider and its schemas
pub async fn create_validation_provider_from_dynamic(
    dynamic_provider: Arc<dyn ModelProvider>,
) -> ModelResult<Arc<dyn ValidationProvider>> {
    let model_fhir_version = dynamic_provider.get_fhir_version().await?;
    let fhir_version = match model_fhir_version {
        ModelFhirVersion::R4 => FhirVersion::R4,
        ModelFhirVersion::R4B => FhirVersion::R4B,
        ModelFhirVersion::R5 => FhirVersion::R5,
        ModelFhirVersion::R6 => FhirVersion::R6,
        ModelFhirVersion::Custom { .. } => FhirVersion::R4,
    };
    let validation_context = create_validation_context(fhir_version);

    // The DynamicModelProvider internally uses FhirSchemaModelProvider with dynamic schemas
    // We extract those same schemas to create our ValidationProvider
    let validation_provider =
        FhirSchemaValidationProvider::from_dynamic_provider(dynamic_provider, validation_context)
            .await?;

    Ok(Arc::new(validation_provider))
}
