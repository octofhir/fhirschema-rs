use futures::future::try_join_all;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task;

use crate::conversion::{ConstraintMapper, ElementConverter, StructDefProcessor};
use crate::core::{ConversionMetadata, ConversionResult, PerformanceConfig};
use crate::error::{FhirSchemaError, Result};
use crate::types::{FhirSchema, TypeResolver};
use crate::utils::performance::Timer;

pub struct ConversionEngine {
    canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
    type_resolver: Arc<TypeResolver>,
    element_processor: Arc<ElementConverter>,
    constraint_mapper: Arc<ConstraintMapper>,
    structdef_processor: Arc<StructDefProcessor>,
    config: PerformanceConfig,
}

impl ConversionEngine {
    pub async fn new(
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
        config: &PerformanceConfig,
    ) -> Result<Self> {
        let type_resolver = Arc::new(TypeResolver::new(Arc::clone(&canonical_manager)).await?);

        Ok(Self {
            canonical_manager: Arc::clone(&canonical_manager),
            type_resolver: Arc::clone(&type_resolver),
            element_processor: Arc::new(ElementConverter::with_type_resolver(Arc::clone(
                &type_resolver,
            ))),
            constraint_mapper: Arc::new(ConstraintMapper::new()),
            structdef_processor: Arc::new(StructDefProcessor::new()),
            config: config.clone(),
        })
    }

    /// High-performance parallel batch conversion with backpressure control
    pub async fn convert_batch(&self, structure_defs: Vec<Value>) -> Result<Vec<ConversionResult>> {
        if structure_defs.is_empty() {
            return Ok(Vec::new());
        }

        let timer = Timer::new();
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_conversions));

        // Create tasks for parallel processing
        let tasks: Vec<_> = structure_defs
            .into_iter()
            .enumerate()
            .map(|(index, structure_def)| {
                let engine = Arc::new(self.clone());
                let sem = Arc::clone(&semaphore);

                task::spawn(async move {
                    let _permit = sem.acquire().await.map_err(|e| FhirSchemaError::Runtime {
                        message: format!("Failed to acquire semaphore: {e}"),
                    })?;

                    let result = engine.convert_single(structure_def).await;

                    // Add batch context to result
                    match result {
                        Ok(mut conv_result) => {
                            conv_result.metadata.structure_definition_url =
                                Some(format!("batch_item_{index}"));
                            Ok(conv_result)
                        }
                        Err(e) => Err(e),
                    }
                })
            })
            .collect();

        // Wait for all tasks to complete
        let results = try_join_all(tasks)
            .await
            .map_err(|e| FhirSchemaError::Runtime {
                message: format!("Batch conversion task failed: {e}"),
            })?;

        // Collect results and handle errors
        let mut final_results = Vec::new();
        let mut errors = Vec::new();

        for result in results {
            match result {
                Ok(conversion_result) => final_results.push(conversion_result),
                Err(e) => {
                    errors.push(e);
                    // Push a failure result to maintain order
                    final_results.push(ConversionResult::failure(vec![
                        crate::error::ValidationError::ConstraintViolation {
                            path: "batch_conversion".to_string(),
                            message: "Conversion failed".to_string(),
                        },
                    ]));
                }
            }
        }

        // Log batch performance
        let duration = timer.elapsed();
        #[cfg(feature = "performance-metrics")]
        tracing::info!(
            "Batch conversion completed: {} items in {:?}ms, {} errors",
            final_results.len(),
            duration.as_millis(),
            errors.len()
        );
        #[cfg(not(feature = "performance-metrics"))]
        {
            let _ = duration; // Prevent unused variable warning
            let _ = errors.len(); // Prevent unused variable warning
        }

        Ok(final_results)
    }

    /// Single conversion with comprehensive processing
    pub async fn convert_single(&self, structure_def: Value) -> Result<ConversionResult> {
        let timer = Timer::new();
        let mut metadata = ConversionMetadata::default();

        // Validate input
        if !structure_def.is_object() {
            return Ok(ConversionResult::failure(vec![
                crate::error::ValidationError::TypeMismatch {
                    path: "root".to_string(),
                    expected: "Object".to_string(),
                    actual: "Non-object".to_string(),
                },
            ]));
        }

        // Extract basic information
        let resource_type = structure_def
            .get("resourceType")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        if resource_type != "StructureDefinition" {
            return Ok(ConversionResult::failure(vec![
                crate::error::ValidationError::TypeMismatch {
                    path: "resourceType".to_string(),
                    expected: "StructureDefinition".to_string(),
                    actual: resource_type.to_string(),
                },
            ]));
        }

        let url = structure_def
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        metadata.structure_definition_url = Some(url.to_string());

        // Apply timeout wrapper
        let conversion_future = self.convert_with_processors(&structure_def, &mut metadata);

        let result = tokio::time::timeout(self.config.conversion_timeout, conversion_future).await;

        let final_result = match result {
            Ok(Ok(schema)) => {
                metadata.duration_ms = Some(timer.elapsed_ms() as u64);
                ConversionResult::success(schema).with_metadata(metadata)
            }
            Ok(Err(e)) => {
                metadata.duration_ms = Some(timer.elapsed_ms() as u64);
                ConversionResult::failure(vec![
                    crate::error::ValidationError::ConstraintViolation {
                        path: "conversion".to_string(),
                        message: e.to_string(),
                    },
                ])
                .with_metadata(metadata)
            }
            Err(_) => {
                metadata.duration_ms = Some(timer.elapsed_ms() as u64);
                ConversionResult::failure(vec![
                    crate::error::ValidationError::ConstraintViolation {
                        path: "conversion".to_string(),
                        message: format!(
                            "Conversion timed out after {:?}",
                            self.config.conversion_timeout
                        ),
                    },
                ])
                .with_metadata(metadata)
            }
        };

        Ok(final_result)
    }

    /// Internal conversion with parallel processing of different aspects
    async fn convert_with_processors(
        &self,
        structure_def: &Value,
        metadata: &mut ConversionMetadata,
    ) -> Result<FhirSchema> {
        let name = structure_def
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        let mut schema = FhirSchema::new("object");

        // Parallel processing of different aspects using try_join!
        let (sd_metadata, elements, constraints): (
            std::collections::HashMap<String, Value>,
            std::collections::HashMap<String, crate::types::FhirSchemaProperty>,
            Vec<crate::types::FhirConstraint>,
        ) = tokio::try_join!(
            self.structdef_processor.process_metadata(structure_def),
            self.structdef_processor.process_elements(structure_def),
            self.structdef_processor.process_constraints(structure_def)
        )?;

        // Apply results to schema
        schema.apply_metadata(sd_metadata);
        schema.apply_elements(elements.clone());
        schema.apply_constraints(constraints.clone());

        // Set schema metadata
        schema = schema
            .with_title(name)
            .with_description(&format!("Schema for {name}"));

        if let Some(url) = structure_def.get("url").and_then(|v| v.as_str()) {
            schema = schema.with_id(url);
        }

        // Update conversion metadata
        metadata.processed_elements = elements.len();
        metadata.applied_constraints = constraints.len();
        metadata.resolved_types = self.count_resolved_types(&elements);

        Ok(schema)
    }

    /// Count resolved types in the elements
    fn count_resolved_types(
        &self,
        elements: &std::collections::HashMap<String, crate::types::FhirSchemaProperty>,
    ) -> usize {
        elements
            .values()
            .map(|prop| {
                1 + if let Some(items) = &prop.items {
                    self.count_property_types(items)
                } else {
                    0
                } + if let Some(properties) = &prop.properties {
                    self.count_resolved_types(properties)
                } else {
                    0
                }
            })
            .sum()
    }

    fn count_property_types(&self, prop: &crate::types::FhirSchemaProperty) -> usize {
        1 + if let Some(properties) = &prop.properties {
            self.count_resolved_types(properties)
        } else {
            0
        }
    }
}

impl Clone for ConversionEngine {
    fn clone(&self) -> Self {
        Self {
            canonical_manager: Arc::clone(&self.canonical_manager),
            type_resolver: Arc::clone(&self.type_resolver),
            element_processor: Arc::clone(&self.element_processor),
            constraint_mapper: Arc::clone(&self.constraint_mapper),
            structdef_processor: Arc::clone(&self.structdef_processor),
            config: self.config.clone(),
        }
    }
}

impl std::fmt::Debug for ConversionEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversionEngine")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
