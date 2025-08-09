use crate::converter::{FhirSchemaConverter, StructureDefinition, StructureDefinitionConverter};
use crate::error::Result;
use crate::package::{ConversionError, ConversionErrorType, ConversionOptions, ConversionResults};
use crate::types::FhirSchema;

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;

/// Conversion pipeline for batch processing of StructureDefinitions
pub struct ConversionPipeline {
    converter: Arc<FhirSchemaConverter>,
    semaphore: Arc<Semaphore>,
    progress_tracker: Option<Arc<dyn ProgressTracker>>,
}

/// Progress tracking trait for conversion operations
pub trait ProgressTracker: Send + Sync {
    fn update_progress(&self, completed: usize, total: usize, current_item: &str);
    fn set_error(&self, error: &str);
    fn set_completed(&self);
}

/// Batch conversion result
pub struct BatchConversionResult {
    pub schemas: Vec<FhirSchema>,
    pub results: ConversionResults,
}

impl ConversionPipeline {
    /// Create a new conversion pipeline
    pub fn new(converter: Arc<FhirSchemaConverter>, max_concurrent: usize) -> Self {
        Self {
            converter,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            progress_tracker: None,
        }
    }

    /// Create a new conversion pipeline with progress tracking
    pub fn with_progress_tracker(
        converter: Arc<FhirSchemaConverter>,
        max_concurrent: usize,
        progress_tracker: Arc<dyn ProgressTracker>,
    ) -> Self {
        Self {
            converter,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            progress_tracker: Some(progress_tracker),
        }
    }

    /// Convert multiple StructureDefinitions in parallel
    pub async fn convert_batch(
        &self,
        structure_definitions: &[StructureDefinition],
        options: &ConversionOptions,
    ) -> Result<BatchConversionResult> {
        let start_time = Instant::now();
        let total_count = structure_definitions.len();

        // Filter StructureDefinitions based on options
        let filtered_definitions =
            self.filter_structure_definitions(structure_definitions, options);

        let mut tasks = Vec::with_capacity(filtered_definitions.len());

        for (index, structure_def) in filtered_definitions.iter().enumerate() {
            let converter = self.converter.clone();
            let semaphore = self.semaphore.clone();
            let structure_def = structure_def.clone();
            let progress_tracker = self.progress_tracker.clone();

            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();

                if let Some(tracker) = &progress_tracker {
                    let name = structure_def.name.as_deref().unwrap_or("Unknown");
                    tracker.update_progress(index, total_count, name);
                }

                let convert_start = Instant::now();
                let result = converter.convert(&structure_def);
                let convert_duration = convert_start.elapsed();

                (result, convert_duration)
            });

            tasks.push(task);
        }

        // Collect results with pre-allocated capacity
        let mut schemas = Vec::with_capacity(filtered_definitions.len());
        let mut failed_conversions = Vec::new(); // Unknown capacity, keep as-is
        let mut total_conversion_time = std::time::Duration::ZERO;
        let mut conversion_times = Vec::with_capacity(tasks.len());

        for (i, task) in tasks.into_iter().enumerate() {
            match task.await {
                Ok((result, duration)) => {
                    total_conversion_time += duration;
                    conversion_times.push(duration.as_millis());

                    match result {
                        Ok(schema) => schemas.push(schema),
                        Err(e) => {
                            let structure_def = &filtered_definitions[i];
                            failed_conversions.push(ConversionError {
                                structure_definition_url: structure_def
                                    .url
                                    .as_ref()
                                    .map(|u| u.to_string())
                                    .unwrap_or_else(|| "unknown".to_string()),
                                error_message: e.to_string(),
                                error_type: ConversionErrorType::ConversionFailure,
                            });
                        }
                    }
                }
                Err(e) => {
                    let structure_def = &filtered_definitions[i];
                    failed_conversions.push(ConversionError {
                        structure_definition_url: structure_def
                            .url
                            .as_ref()
                            .map(|u| u.to_string())
                            .unwrap_or_else(|| "unknown".to_string()),
                        error_message: format!("Task execution failed: {e}"),
                        error_type: ConversionErrorType::ConversionFailure,
                    });
                }
            }
        }

        // Calculate performance statistics
        let performance_stats = if !conversion_times.is_empty() {
            crate::package::ConversionStats {
                avg_conversion_time_ms: conversion_times.iter().sum::<u128>() as f64
                    / conversion_times.len() as f64,
                max_conversion_time_ms: *conversion_times.iter().max().unwrap_or(&0),
                min_conversion_time_ms: *conversion_times.iter().min().unwrap_or(&0),
                memory_usage_mb: self.estimate_memory_usage_mb(&conversion_times),
                parallel_efficiency: self
                    .calculate_parallel_efficiency(&conversion_times, start_time.elapsed()),
            }
        } else {
            Default::default()
        };

        let results = ConversionResults {
            total_structure_definitions: structure_definitions.len(),
            converted_schemas: schemas.len(),
            skipped: structure_definitions.len() - filtered_definitions.len(),
            failed: failed_conversions,
            conversion_time: start_time.elapsed(),
            performance_stats,
        };

        if let Some(tracker) = &self.progress_tracker {
            if results.failed.is_empty() {
                tracker.set_completed();
            } else {
                tracker.set_error(&format!("{} conversions failed", results.failed.len()));
            }
        }

        Ok(BatchConversionResult { schemas, results })
    }

    /// Convert using streaming approach for large packages (memory-efficient)
    pub async fn convert_streaming(
        &self,
        structure_definitions: &[StructureDefinition],
        options: &ConversionOptions,
    ) -> Result<BatchConversionResult> {
        const BATCH_SIZE: usize = 50; // Process in chunks of 50 schemas

        let start_time = Instant::now();
        let total_count = structure_definitions.len();

        // Filter StructureDefinitions based on options
        let filtered_definitions =
            self.filter_structure_definitions(structure_definitions, options);

        let mut all_schemas = Vec::with_capacity(filtered_definitions.len());
        let mut all_failed_conversions = Vec::new();
        let mut total_conversion_time = std::time::Duration::ZERO;
        let mut all_conversion_times = Vec::new();

        if let Some(tracker) = &self.progress_tracker {
            tracker.update_progress(0, total_count, "Starting streaming conversion");
        }

        // Process in batches to control memory usage
        for (batch_index, batch) in filtered_definitions.chunks(BATCH_SIZE).enumerate() {
            let batch_start = batch_index * BATCH_SIZE;

            if let Some(tracker) = &self.progress_tracker {
                let _progress = (batch_start as f64 / filtered_definitions.len() as f64) * 100.0;
                tracker.update_progress(
                    batch_start,
                    total_count,
                    &format!(
                        "Processing batch {}/{}",
                        batch_index + 1,
                        filtered_definitions.len().div_ceil(BATCH_SIZE)
                    ),
                );
            }

            // Process current batch
            let mut batch_tasks = Vec::with_capacity(batch.len());

            for (index_in_batch, structure_def) in batch.iter().enumerate() {
                let converter = self.converter.clone();
                let semaphore = self.semaphore.clone();
                let structure_def = structure_def.clone();
                let global_index = batch_start + index_in_batch;

                let task = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    let convert_start = Instant::now();
                    // Use the synchronous convert method in async context
                    let result = tokio::task::spawn_blocking({
                        let converter = converter.clone();
                        let structure_def = structure_def.clone();
                        move || converter.convert(&structure_def)
                    })
                    .await
                    .unwrap();
                    let convert_time = convert_start.elapsed();
                    (global_index, result, convert_time.as_millis())
                });

                batch_tasks.push(task);
            }

            // Collect batch results
            for (i, task) in batch_tasks.into_iter().enumerate() {
                match task.await {
                    Ok((_global_index, Ok(schema), conversion_time)) => {
                        all_schemas.push(schema);
                        all_conversion_times.push(conversion_time);
                        total_conversion_time +=
                            std::time::Duration::from_millis(conversion_time as u64);
                    }
                    Ok((global_index, Err(e), conversion_time)) => {
                        all_conversion_times.push(conversion_time);
                        let structure_def = &filtered_definitions[global_index];
                        all_failed_conversions.push(crate::package::ConversionError {
                            structure_definition_url: structure_def
                                .url
                                .as_ref()
                                .map(|u| u.to_string())
                                .unwrap_or_else(|| format!("Unknown-{global_index}")),
                            error_message: e.to_string(),
                            error_type: crate::package::ConversionErrorType::ConversionFailure,
                        });
                    }
                    Err(e) => {
                        let structure_def = &filtered_definitions[batch_start + i];
                        all_failed_conversions.push(crate::package::ConversionError {
                            structure_definition_url: structure_def
                                .url
                                .as_ref()
                                .map(|u| u.to_string())
                                .unwrap_or_else(|| format!("Unknown-{}", batch_start + i)),
                            error_message: format!("Task execution failed: {e}"),
                            error_type: crate::package::ConversionErrorType::ConversionFailure,
                        });
                    }
                }
            }

            // Force garbage collection of batch data by dropping references
            // This helps keep memory usage stable during streaming
        }

        // Calculate performance statistics
        let performance_stats = if !all_conversion_times.is_empty() {
            crate::package::ConversionStats {
                avg_conversion_time_ms: all_conversion_times.iter().sum::<u128>() as f64
                    / all_conversion_times.len() as f64,
                max_conversion_time_ms: *all_conversion_times.iter().max().unwrap(),
                min_conversion_time_ms: *all_conversion_times.iter().min().unwrap(),
                memory_usage_mb: self.estimate_memory_usage_mb(&all_conversion_times),
                parallel_efficiency: self
                    .calculate_parallel_efficiency(&all_conversion_times, start_time.elapsed()),
            }
        } else {
            Default::default()
        };

        let results = ConversionResults {
            total_structure_definitions: structure_definitions.len(),
            converted_schemas: all_schemas.len(),
            skipped: structure_definitions.len() - filtered_definitions.len(),
            failed: all_failed_conversions,
            conversion_time: start_time.elapsed(),
            performance_stats,
        };

        if let Some(tracker) = &self.progress_tracker {
            if results.failed.is_empty() {
                tracker.set_completed();
            } else {
                tracker.set_error(&format!("{} conversions failed", results.failed.len()));
            }
        }

        Ok(BatchConversionResult {
            schemas: all_schemas,
            results,
        })
    }

    /// Filter StructureDefinitions based on conversion options
    fn filter_structure_definitions(
        &self,
        structure_definitions: &[StructureDefinition],
        options: &ConversionOptions,
    ) -> Vec<StructureDefinition> {
        structure_definitions
            .iter()
            .filter(|sd| {
                // Filter by resource type if specified
                if !options.resource_type_filter.is_empty()
                    && !options.resource_type_filter.contains(&sd.type_name)
                {
                    return false;
                }

                // Filter by profile type if specified
                if !options.profile_type_filter.is_empty() {
                    let profile_type = match sd.kind.as_str() {
                        "resource" => crate::package::ProfileTypeFilter::Resource,
                        "complex-type" | "primitive-type" => {
                            if sd.type_name == "Extension" {
                                crate::package::ProfileTypeFilter::Extension
                            } else {
                                crate::package::ProfileTypeFilter::Type
                            }
                        }
                        "logical" => crate::package::ProfileTypeFilter::Logical,
                        _ => crate::package::ProfileTypeFilter::Resource,
                    };

                    if !options.profile_type_filter.contains(&profile_type) {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect()
    }

    /// Calculate parallel processing efficiency
    fn calculate_parallel_efficiency(
        &self,
        conversion_times: &[u128],
        total_elapsed: std::time::Duration,
    ) -> f64 {
        if conversion_times.is_empty() {
            return 0.0;
        }

        let total_work_time: u128 = conversion_times.iter().sum();
        let total_elapsed_ms = total_elapsed.as_millis();

        if total_elapsed_ms == 0 {
            return 0.0;
        }

        // Theoretical maximum efficiency is the number of concurrent workers
        let max_concurrent = self.semaphore.available_permits() + conversion_times.len();
        let theoretical_minimum_time = total_work_time / max_concurrent as u128;

        if theoretical_minimum_time == 0 {
            return 0.0;
        }

        (theoretical_minimum_time as f64 / total_elapsed_ms as f64).min(1.0)
    }

    /// Estimate memory usage for conversion operations
    fn estimate_memory_usage_mb(&self, conversion_times: &[u128]) -> f64 {
        if conversion_times.is_empty() {
            return 0.0;
        }

        // Estimate memory based on concurrent operations and average schema size
        let concurrent_ops = self
            .semaphore
            .available_permits()
            .min(conversion_times.len());
        let avg_schema_size_kb = 15.0; // Estimated average FhirSchema size in KB
        let overhead_factor = 1.5; // Account for conversion overhead

        (concurrent_ops as f64 * avg_schema_size_kb * overhead_factor) / 1024.0
    }
}

/// Simple progress tracker implementation
pub struct SimpleProgressTracker {
    name: String,
}

impl SimpleProgressTracker {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl ProgressTracker for SimpleProgressTracker {
    fn update_progress(&self, completed: usize, total: usize, current_item: &str) {
        let percentage = (completed as f64 / total as f64 * 100.0) as u32;
        println!(
            "{}: Converting {} ({}/{}): {}%",
            self.name,
            current_item,
            completed + 1,
            total,
            percentage
        );
    }

    fn set_error(&self, error: &str) {
        println!("{}: Error - {}", self.name, error);
    }

    fn set_completed(&self) {
        println!("{}: Conversion completed successfully", self.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::{ConverterConfig, FhirSchemaConverter};

    #[tokio::test]
    async fn test_conversion_pipeline() {
        let converter = Arc::new(FhirSchemaConverter::with_config(ConverterConfig::default()));
        let pipeline = ConversionPipeline::new(converter, 4);

        let structure_definitions = vec![]; // Empty for test
        let options = ConversionOptions::default();

        let result = pipeline
            .convert_batch(&structure_definitions, &options)
            .await;
        assert!(result.is_ok());

        let batch_result = result.unwrap();
        assert_eq!(batch_result.schemas.len(), 0);
        assert_eq!(batch_result.results.total_structure_definitions, 0);
    }

    #[test]
    fn test_simple_progress_tracker() {
        let tracker = SimpleProgressTracker::new("test");
        tracker.update_progress(1, 10, "TestResource");
        tracker.set_completed();
        // This test mainly ensures the tracker doesn't panic
    }
}
