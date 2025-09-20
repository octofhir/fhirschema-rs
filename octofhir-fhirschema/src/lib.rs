pub mod action_calculator;
pub mod choice_handler;
pub mod converter;
pub mod element_transformer;
pub mod embedded;
pub mod error;
pub mod model_provider;
pub mod path_parser;
pub mod stack_processor;
pub mod types;
pub mod validation;
pub mod validation_provider;
pub mod validator;

pub use converter::translate;
pub use embedded::{
    FhirVersion, SchemaInfo, create_validation_context, get_schema, get_schema_info,
    get_schema_names, get_schemas, has_schema, list_primitives, list_resources,
};
pub use error::{FhirSchemaError, Result};
pub use types::{
    FhirSchema, FhirSchemaElement, StructureDefinition, ValidationContext, ValidationError,
    ValidationResult,
};
pub use validation::{FhirSchemaErrorCode, FhirSchemaValidationContext, FhirSchemaValidator};
pub use validator::validate;

// Model Provider exports
pub use model_provider::{DynamicSchemaProvider, EmbeddedSchemaProvider, FhirSchemaModelProvider};

// Validation Provider exports
pub use validation_provider::FhirSchemaValidationProvider;

// Convenience functions for creating ValidationProvider from existing ModelProviders
pub use validation_provider::{
    create_validation_provider_from_dynamic, create_validation_provider_from_embedded,
};

// Re-export key types from fhir-model-rs for convenience
pub use octofhir_fhir_model::error::{ModelError, Result as ModelResult};
pub use octofhir_fhir_model::provider::{
    ElementInfo, FhirVersion as ModelFhirVersion, ModelProvider, TypeInfo,
};
