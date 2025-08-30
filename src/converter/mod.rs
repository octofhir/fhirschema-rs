mod adaptive_converter;
mod choice_types;
mod constraints;
mod context;
mod element_converter;
mod parallel_benchmark;
mod parallel_converter;
mod slicing;
mod structure_definition;
mod test_constraint_preservation;

pub use adaptive_converter::*;
pub use choice_types::*;
pub use constraints::*;
pub use context::*;
pub use element_converter::*;
pub use parallel_converter::*;
pub use slicing::*;
pub use structure_definition::*;

use crate::{FhirSchema, Result};
use octofhir_canonical_manager::CanonicalManager;
use std::sync::Arc;

pub trait StructureDefinitionConverter {
    fn convert(&self, structure_definition: &StructureDefinition) -> Result<FhirSchema>;
    fn convert_with_context(
        &self,
        structure_definition: &StructureDefinition,
        context: &mut ConversionContext,
    ) -> Result<FhirSchema>;
}

#[async_trait::async_trait]
pub trait AsyncStructureDefinitionConverter {
    async fn convert_async(&self, structure_definition: &StructureDefinition)
    -> Result<FhirSchema>;
    async fn convert_with_context_async(
        &self,
        structure_definition: &StructureDefinition,
        context: &mut ConversionContext,
    ) -> Result<FhirSchema>;
}

#[derive(Debug, Clone)]
pub struct FhirSchemaConverter {
    config: ConverterConfig,
}

#[derive(Debug, Clone)]
pub struct ConverterConfig {
    pub expand_choice_types: bool,
    pub include_slicing: bool,
    pub process_constraints: bool,
    pub resolve_profiles: bool,
    pub cache_results: bool,
}

impl Default for ConverterConfig {
    fn default() -> Self {
        Self {
            expand_choice_types: true,
            include_slicing: true,
            process_constraints: true,
            resolve_profiles: true,
            cache_results: true,
        }
    }
}

impl FhirSchemaConverter {
    pub fn new() -> Self {
        Self {
            config: ConverterConfig::default(),
        }
    }

    pub fn with_config(config: ConverterConfig) -> Self {
        Self { config }
    }

    /// Determine the schema class based on kind, derivation, and type according to specification
    fn determine_class(kind: &str, derivation: Option<&str>, type_name: &str) -> String {
        match kind {
            "resource" => match derivation {
                Some("constraint") => "profile".to_string(),
                _ => "resource".to_string(),
            },
            "complex-type" | "primitive-type" => {
                if type_name == "Extension" {
                    "extension".to_string()
                } else {
                    "type".to_string()
                }
            }
            "logical" => "logical".to_string(),
            _ => "resource".to_string(), // fallback
        }
    }

    pub async fn convert_with_canonical_manager(
        &self,
        structure_definition: &StructureDefinition,
        canonical_manager: Arc<CanonicalManager>,
    ) -> Result<FhirSchema> {
        let mut context =
            ConversionContext::with_canonical_manager(&self.config, canonical_manager);
        self.convert_with_context_async(structure_definition, &mut context)
            .await
    }
}

impl Default for FhirSchemaConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl StructureDefinitionConverter for FhirSchemaConverter {
    fn convert(&self, structure_definition: &StructureDefinition) -> Result<FhirSchema> {
        let mut context = ConversionContext::new(&self.config);
        self.convert_with_context(structure_definition, &mut context)
    }

    fn convert_with_context(
        &self,
        structure_definition: &StructureDefinition,
        context: &mut ConversionContext,
    ) -> Result<FhirSchema> {
        context.begin_conversion(structure_definition)?;

        let mut schema = FhirSchema::new(&structure_definition.type_name);

        if let Some(url) = &structure_definition.url {
            schema = schema.with_url(url.clone());
        }

        if let Some(name) = &structure_definition.name {
            schema = schema.with_name(name);
        }

        schema.title = structure_definition.title.clone();
        schema.description = structure_definition.description.clone();
        schema.version = structure_definition.version.clone();
        schema.status = structure_definition.status.clone();

        // Set classification fields according to specification
        schema = schema.with_kind(&structure_definition.kind);
        let class = Self::determine_class(
            &structure_definition.kind,
            structure_definition.derivation.as_deref(),
            &structure_definition.type_name,
        );
        schema = schema.with_class(class);

        if let Some(base_def) = &structure_definition.base_definition {
            schema = schema.with_base(base_def.clone());
        }

        if let Some(abstract_flag) = structure_definition.abstract_ {
            schema = schema.with_abstract(abstract_flag);
        }

        // Legacy fields for backward compatibility
        schema.base_definition = structure_definition.base_definition.clone();
        schema.derivation = structure_definition.derivation.clone();

        // Convert elements
        let element_converter = ElementConverter::new(&self.config);
        for element_def in &structure_definition.elements {
            let elements = element_converter.convert_element(element_def, context)?;
            for (path, element) in elements {
                schema = schema.with_element(path, element);
            }
        }

        // Process constraints
        if self.config.process_constraints {
            let constraint_processor = ConstraintProcessor::new();
            let constraints = constraint_processor
                .process_constraints(&structure_definition.elements, context)?;
            schema.constraints.extend(constraints);
        }

        // Process slicing
        if self.config.include_slicing {
            let slicing_processor = SlicingProcessor::new();
            let slicing_defs =
                slicing_processor.process_slicing(&structure_definition.elements, context)?;
            for (path, slicing) in slicing_defs {
                schema.slicing.insert(path, slicing);
            }
        }

        context.end_conversion(&schema)?;
        Ok(schema)
    }
}

#[async_trait::async_trait]
impl AsyncStructureDefinitionConverter for FhirSchemaConverter {
    async fn convert_async(
        &self,
        structure_definition: &StructureDefinition,
    ) -> Result<FhirSchema> {
        let mut context = ConversionContext::new(&self.config);
        self.convert_with_context_async(structure_definition, &mut context)
            .await
    }

    async fn convert_with_context_async(
        &self,
        structure_definition: &StructureDefinition,
        context: &mut ConversionContext,
    ) -> Result<FhirSchema> {
        context.begin_conversion(structure_definition)?;

        let mut schema = FhirSchema::new(&structure_definition.type_name);

        if let Some(url) = &structure_definition.url {
            schema = schema.with_url(url.clone());
        }

        if let Some(name) = &structure_definition.name {
            schema = schema.with_name(name);
        }

        schema.title = structure_definition.title.clone();
        schema.description = structure_definition.description.clone();
        schema.version = structure_definition.version.clone();
        schema.status = structure_definition.status.clone();

        // Set classification fields according to specification
        schema = schema.with_kind(&structure_definition.kind);
        let class = Self::determine_class(
            &structure_definition.kind,
            structure_definition.derivation.as_deref(),
            &structure_definition.type_name,
        );
        schema = schema.with_class(class);

        if let Some(base_def) = &structure_definition.base_definition {
            schema = schema.with_base(base_def.clone());
        }

        if let Some(abstract_flag) = structure_definition.abstract_ {
            schema = schema.with_abstract(abstract_flag);
        }

        // Legacy fields for backward compatibility
        schema.base_definition = structure_definition.base_definition.clone();
        schema.derivation = structure_definition.derivation.clone();

        // Convert elements (with async profile resolution)
        let element_converter = ElementConverter::new(&self.config);
        for element_def in &structure_definition.elements {
            let elements = element_converter
                .convert_element_async(element_def, context)
                .await?;
            for (path, element) in elements {
                schema = schema.with_element(path, element);
            }
        }

        // Process constraints
        if self.config.process_constraints {
            let constraint_processor = ConstraintProcessor::new();
            let constraints = constraint_processor
                .process_constraints(&structure_definition.elements, context)?;
            schema.constraints.extend(constraints);
        }

        // Process slicing
        if self.config.include_slicing {
            let slicing_processor = SlicingProcessor::new();
            let slicing_defs =
                slicing_processor.process_slicing(&structure_definition.elements, context)?;
            for (path, slicing) in slicing_defs {
                schema.slicing.insert(path, slicing);
            }
        }

        context.end_conversion(&schema)?;
        Ok(schema)
    }
}
