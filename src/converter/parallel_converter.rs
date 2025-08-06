use super::{FhirSchemaConverter, StructureDefinition, StructureDefinitionConverter};
use crate::{FhirSchema, Result};
use rayon::prelude::*;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use thiserror::Error;

// Global thread pool to avoid recreation overhead
static GLOBAL_THREAD_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn get_or_create_thread_pool(num_threads: usize) -> &'static rayon::ThreadPool {
    GLOBAL_THREAD_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .expect("Failed to create global thread pool")
    })
}

#[derive(Debug, Error)]
pub enum ParallelConversionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Conversion error: {0}")]
    Conversion(String),
    #[error("Channel error: {0}")]
    Channel(String),
}

#[derive(Debug, Clone)]
pub struct ConversionReport {
    pub total_files: usize,
    pub converted: usize,
    pub failed: usize,
    pub duration: Duration,
    pub schemas: Vec<FhirSchema>,
    pub errors: Vec<String>,
}

impl Default for ConversionReport {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversionReport {
    pub fn new() -> Self {
        Self {
            total_files: 0,
            converted: 0,
            failed: 0,
            duration: Duration::default(),
            schemas: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            self.converted as f64 / self.total_files as f64
        }
    }

    pub fn throughput(&self) -> f64 {
        if self.duration.as_secs_f64() == 0.0 {
            0.0
        } else {
            self.converted as f64 / self.duration.as_secs_f64()
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParallelConverterConfig {
    pub worker_threads: usize,
    pub batch_size: usize,
    pub channel_capacity: usize,
    pub enable_progress_reporting: bool,
}

impl Default for ParallelConverterConfig {
    fn default() -> Self {
        Self {
            worker_threads: num_cpus::get().max(1),
            batch_size: 50,
            channel_capacity: 100,
            enable_progress_reporting: true,
        }
    }
}

pub struct ParallelSchemaConverter {
    converter: Arc<FhirSchemaConverter>,
    config: ParallelConverterConfig,
}

impl ParallelSchemaConverter {
    pub fn new(converter: FhirSchemaConverter) -> Self {
        Self {
            converter: Arc::new(converter),
            config: ParallelConverterConfig::default(),
        }
    }

    pub fn with_config(converter: FhirSchemaConverter, config: ParallelConverterConfig) -> Self {
        Self {
            converter: Arc::new(converter),
            config,
        }
    }

    /// Convert a batch of StructureDefinitions in parallel
    pub async fn convert_batch(
        &self,
        definitions: Vec<StructureDefinition>,
    ) -> Result<Vec<FhirSchema>> {
        if definitions.is_empty() {
            return Ok(Vec::new());
        }

        // Use global thread pool to avoid recreation overhead
        let pool = get_or_create_thread_pool(self.config.worker_threads);

        // Process definitions in parallel
        let converter = self.converter.clone();
        let results: Vec<Result<FhirSchema>> = pool.install(|| {
            definitions
                .into_par_iter()
                .map(|def| {
                    // For small batches, skip caching to avoid mutex overhead
                    // Convert directly without cache contention
                    let mut context = super::ConversionContext::new(&converter.config);
                    converter.convert_with_context(&def, &mut context)
                })
                .collect()
        });

        // Collect successful conversions
        let mut schemas = Vec::new();
        for result in results {
            match result {
                Ok(schema) => schemas.push(schema),
                Err(e) => {
                    eprintln!("Warning: Failed to convert schema: {e}");
                }
            }
        }

        Ok(schemas)
    }

    /// Convert an entire package directory in parallel
    pub async fn convert_package(&self, package_path: &Path) -> Result<ConversionReport> {
        let start = Instant::now();
        let mut report = ConversionReport::new();

        // Find all StructureDefinition files
        let files = self.find_structure_definitions(package_path).await?;
        report.total_files = files.len();

        if files.is_empty() {
            report.duration = start.elapsed();
            return Ok(report);
        }

        // Load and parse files in parallel
        let definitions: Vec<StructureDefinition> = files
            .par_iter()
            .filter_map(|path| match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str::<StructureDefinition>(&content) {
                    Ok(def) => Some(def),
                    Err(e) => {
                        eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
                        None
                    }
                },
                Err(e) => {
                    eprintln!("Warning: Failed to read {}: {}", path.display(), e);
                    None
                }
            })
            .collect();

        // Process in batches to manage memory usage
        let batch_size = self.config.batch_size;
        let mut all_schemas = Vec::new();
        let mut total_converted = 0;
        let mut total_failed = 0;

        for (batch_idx, batch) in definitions.chunks(batch_size).enumerate() {
            if self.config.enable_progress_reporting {
                println!(
                    "Processing batch {}/{} ({} definitions)...",
                    batch_idx + 1,
                    definitions.len().div_ceil(batch_size),
                    batch.len()
                );
            }

            match self.convert_batch(batch.to_vec()).await {
                Ok(schemas) => {
                    total_converted += schemas.len();
                    total_failed += batch.len() - schemas.len();
                    all_schemas.extend(schemas);
                }
                Err(e) => {
                    total_failed += batch.len();
                    report
                        .errors
                        .push(format!("Batch {} failed: {}", batch_idx + 1, e));
                }
            }
        }

        report.converted = total_converted;
        report.failed = total_failed;
        report.schemas = all_schemas;
        report.duration = start.elapsed();

        Ok(report)
    }

    /// Find all StructureDefinition JSON files in a directory
    async fn find_structure_definitions(
        &self,
        package_path: &Path,
    ) -> Result<Vec<std::path::PathBuf>> {
        let mut files = Vec::new();

        if !package_path.exists() {
            return Ok(files);
        }

        let mut entries = tokio::fs::read_dir(package_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    // Look for StructureDefinition files
                    if file_name.starts_with("StructureDefinition-") && file_name.ends_with(".json")
                    {
                        files.push(path);
                    }
                }
            }
        }

        Ok(files)
    }

    /// Get converter configuration
    pub fn config(&self) -> &ParallelConverterConfig {
        &self.config
    }

    /// Update converter configuration
    pub fn set_config(&mut self, config: ParallelConverterConfig) {
        self.config = config;
    }
}

impl Default for ParallelSchemaConverter {
    fn default() -> Self {
        Self::new(FhirSchemaConverter::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_structure_definition(id: &str) -> StructureDefinition {
        StructureDefinition {
            resource_type: "StructureDefinition".to_string(),
            id: Some(id.to_string()),
            url: Some(
                format!("http://example.com/StructureDefinition/{id}")
                    .parse()
                    .unwrap(),
            ),
            identifier: None,
            version: None,
            name: Some(id.to_string()),
            title: None,
            status: Some("active".to_string()),
            experimental: None,
            date: None,
            publisher: None,
            contact: None,
            description: None,
            purpose: None,
            copyright: None,
            kind: "resource".to_string(),
            abstract_: Some(false),
            context: None,
            type_name: "Patient".to_string(),
            base_definition: None,
            derivation: None,
            snapshot: None,
            differential: None,
            elements: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_convert_batch_empty() {
        let converter = ParallelSchemaConverter::default();
        let result = converter.convert_batch(vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_convert_batch_single() {
        let converter = ParallelSchemaConverter::default();
        let definitions = vec![create_test_structure_definition("test1")];

        let result = converter.convert_batch(definitions).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_convert_batch_multiple() {
        let converter = ParallelSchemaConverter::default();
        let definitions = vec![
            create_test_structure_definition("test1"),
            create_test_structure_definition("test2"),
            create_test_structure_definition("test3"),
        ];

        let result = converter.convert_batch(definitions).await.unwrap();
        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_convert_package_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let converter = ParallelSchemaConverter::default();

        let report = converter.convert_package(temp_dir.path()).await.unwrap();
        assert_eq!(report.total_files, 0);
        assert_eq!(report.converted, 0);
    }

    #[tokio::test]
    async fn test_convert_package_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let converter = ParallelSchemaConverter::default();

        // Create test StructureDefinition files
        let def1 = create_test_structure_definition("Patient");
        let def2 = create_test_structure_definition("Observation");

        let file1_path = temp_dir.path().join("StructureDefinition-Patient.json");
        let file2_path = temp_dir.path().join("StructureDefinition-Observation.json");

        fs::write(&file1_path, serde_json::to_string_pretty(&def1).unwrap()).unwrap();
        fs::write(&file2_path, serde_json::to_string_pretty(&def2).unwrap()).unwrap();

        let report = converter.convert_package(temp_dir.path()).await.unwrap();
        assert_eq!(report.total_files, 2);
        assert!(report.converted > 0);
    }

    #[test]
    fn test_conversion_report_metrics() {
        let mut report = ConversionReport::new();
        report.total_files = 10;
        report.converted = 8;
        report.failed = 2;
        report.duration = Duration::from_secs(2);

        assert_eq!(report.success_rate(), 0.8);
        assert_eq!(report.throughput(), 4.0);
    }

    #[test]
    fn test_parallel_converter_config() {
        let config = ParallelConverterConfig {
            worker_threads: 4,
            batch_size: 25,
            channel_capacity: 50,
            enable_progress_reporting: false,
        };

        let converter =
            ParallelSchemaConverter::with_config(FhirSchemaConverter::new(), config.clone());

        assert_eq!(converter.config().worker_threads, 4);
        assert_eq!(converter.config().batch_size, 25);
        assert_eq!(converter.config().channel_capacity, 50);
        assert!(!converter.config().enable_progress_reporting);
    }
    #[tokio::test]
    async fn test_memory_usage_large_batch() {
        // Create a large batch to test memory usage
        let large_batch: Vec<StructureDefinition> = (0..1000)
            .map(|i| create_test_structure_definition(&format!("large_test{i}")))
            .collect();

        let converter = ParallelSchemaConverter::new(FhirSchemaConverter::new());

        // Process in smaller batches to ensure memory usage stays bounded
        let batch_size = 50;
        let mut total_converted = 0;

        for chunk in large_batch.chunks(batch_size) {
            let results = converter.convert_batch(chunk.to_vec()).await.unwrap();
            total_converted += results.len();

            // Verify each batch processes successfully
            assert!(!results.is_empty());
        }

        // Verify all items were processed
        assert_eq!(total_converted, large_batch.len());
    }
}
