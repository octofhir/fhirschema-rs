pub mod cache;
pub mod composite;
pub mod fhir_model_provider;
pub mod navigation;
pub mod schema_builder;
pub mod type_reflection;

#[cfg(feature = "embedded-providers")]
pub mod embedded;

#[cfg(feature = "dynamic-caching")]
pub mod dynamic;

pub use cache::ModelProviderCache;
pub use composite::CompositeModelProvider;
pub use fhir_model_provider::{
    ChoiceResolution, FhirSchemaModelProvider, NavigationResult, TypeHierarchy,
};
pub use navigation::NavigationEngine;
pub use schema_builder::{SchemaBuilder, SchemaBuilderResult};

#[cfg(feature = "embedded-providers")]
pub use embedded::EmbeddedModelProvider;

#[cfg(feature = "dynamic-caching")]
pub use dynamic::{DynamicModelProvider, DynamicProviderConfig};
