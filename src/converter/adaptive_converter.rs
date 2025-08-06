use super::{
    FhirSchemaConverter, ParallelSchemaConverter, StructureDefinition, StructureDefinitionConverter,
};
use crate::{FhirSchema, Result};
use std::time::Instant;

/// Adaptive converter that automatically chooses between sequential and parallel processing
/// based on dataset characteristics and performance heuristics
pub struct AdaptiveSchemaConverter {
    sequential_converter: FhirSchemaConverter,
    parallel_converter: ParallelSchemaConverter,
    threshold_size: usize,
}

impl AdaptiveSchemaConverter {
    /// Create a new adaptive converter with default thresholds
    pub fn new(converter: FhirSchemaConverter) -> Self {
        let parallel_converter = ParallelSchemaConverter::new(converter.clone());
        Self {
            sequential_converter: converter,
            parallel_converter,
            threshold_size: 10000, // Based on analysis, parallel is never beneficial below this
        }
    }

    /// Create an adaptive converter with custom threshold
    pub fn with_threshold(converter: FhirSchemaConverter, threshold_size: usize) -> Self {
        let parallel_converter = ParallelSchemaConverter::new(converter.clone());
        Self {
            sequential_converter: converter,
            parallel_converter,
            threshold_size,
        }
    }

    /// Convert a batch of StructureDefinitions using the optimal strategy
    pub async fn convert_batch(
        &self,
        definitions: Vec<StructureDefinition>,
    ) -> Result<Vec<FhirSchema>> {
        if definitions.is_empty() {
            return Ok(Vec::new());
        }

        let strategy = self.choose_strategy(&definitions);

        match strategy {
            ConversionStrategy::Sequential => self.convert_sequential(definitions),
            ConversionStrategy::Parallel => {
                self.parallel_converter.convert_batch(definitions).await
            }
        }
    }

    /// Convert using sequential processing
    fn convert_sequential(&self, definitions: Vec<StructureDefinition>) -> Result<Vec<FhirSchema>> {
        let mut results = Vec::with_capacity(definitions.len());

        for definition in definitions {
            match self.sequential_converter.convert(&definition) {
                Ok(schema) => results.push(schema),
                Err(e) => {
                    eprintln!("Warning: Failed to convert schema: {e}");
                }
            }
        }

        Ok(results)
    }

    /// Choose the optimal conversion strategy based on dataset characteristics
    fn choose_strategy(&self, definitions: &[StructureDefinition]) -> ConversionStrategy {
        let size = definitions.len();

        // Based on our analysis, parallel processing is never beneficial for typical FHIR conversions
        // due to low computational intensity and high overhead
        if size < self.threshold_size {
            ConversionStrategy::Sequential
        } else {
            // Even for large datasets, sequential is typically better for FHIR schema conversion
            // Only use parallel if explicitly configured with a very low threshold
            if self.threshold_size < 1000 {
                ConversionStrategy::Parallel
            } else {
                ConversionStrategy::Sequential
            }
        }
    }

    /// Get performance statistics for the last conversion
    pub async fn benchmark_strategies(
        &self,
        definitions: Vec<StructureDefinition>,
    ) -> StrategyBenchmark {
        if definitions.is_empty() {
            return StrategyBenchmark::default();
        }

        // Test sequential
        let start = Instant::now();
        let sequential_results = self
            .convert_sequential(definitions.clone())
            .unwrap_or_default();
        let sequential_duration = start.elapsed();

        // Test parallel
        let start = Instant::now();
        let parallel_results = self
            .parallel_converter
            .convert_batch(definitions.clone())
            .await
            .unwrap_or_default();
        let parallel_duration = start.elapsed();

        let speedup = if parallel_duration.as_nanos() > 0 {
            sequential_duration.as_nanos() as f64 / parallel_duration.as_nanos() as f64
        } else {
            0.0
        };

        StrategyBenchmark {
            dataset_size: definitions.len(),
            sequential_duration,
            parallel_duration,
            speedup_ratio: speedup,
            sequential_results: sequential_results.len(),
            parallel_results: parallel_results.len(),
            recommended_strategy: if speedup > 1.0 {
                ConversionStrategy::Parallel
            } else {
                ConversionStrategy::Sequential
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConversionStrategy {
    Sequential,
    Parallel,
}

#[derive(Debug, Clone)]
pub struct StrategyBenchmark {
    pub dataset_size: usize,
    pub sequential_duration: std::time::Duration,
    pub parallel_duration: std::time::Duration,
    pub speedup_ratio: f64,
    pub sequential_results: usize,
    pub parallel_results: usize,
    pub recommended_strategy: ConversionStrategy,
}

impl Default for StrategyBenchmark {
    fn default() -> Self {
        Self {
            dataset_size: 0,
            sequential_duration: std::time::Duration::ZERO,
            parallel_duration: std::time::Duration::ZERO,
            speedup_ratio: 0.0,
            sequential_results: 0,
            parallel_results: 0,
            recommended_strategy: ConversionStrategy::Sequential,
        }
    }
}

impl StrategyBenchmark {
    pub fn print_analysis(&self) {
        println!("=== Strategy Benchmark Analysis ===");
        println!("Dataset size: {} items", self.dataset_size);
        println!(
            "Sequential: {:?} ({} results)",
            self.sequential_duration, self.sequential_results
        );
        println!(
            "Parallel:   {:?} ({} results)",
            self.parallel_duration, self.parallel_results
        );
        println!("Speedup ratio: {:.2}x", self.speedup_ratio);

        if self.speedup_ratio > 1.0 {
            println!("✅ Parallel is {:.1}x faster", self.speedup_ratio);
        } else {
            println!("❌ Sequential is {:.1}x faster", 1.0 / self.speedup_ratio);
        }

        println!("Recommended strategy: {:?}", self.recommended_strategy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_definition(id: &str) -> StructureDefinition {
        use serde_json::json;

        let definition = json!({
            "resourceType": "StructureDefinition",
            "id": id,
            "url": format!("http://example.com/StructureDefinition/{}", id),
            "name": format!("Test{}", id),
            "status": "active",
            "kind": "resource",
            "abstract": false,
            "type": "Patient",
            "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
            "derivation": "constraint"
        });

        serde_json::from_value(definition).unwrap()
    }

    #[tokio::test]
    async fn test_adaptive_converter_small_dataset() {
        let converter = AdaptiveSchemaConverter::new(FhirSchemaConverter::new());

        let definitions: Vec<StructureDefinition> = (0..10)
            .map(|i| create_test_definition(&format!("test{i}")))
            .collect();

        let results = converter.convert_batch(definitions).await.unwrap();
        assert_eq!(results.len(), 10);
    }

    #[tokio::test]
    async fn test_adaptive_converter_strategy_selection() {
        let converter = AdaptiveSchemaConverter::with_threshold(FhirSchemaConverter::new(), 5);

        // Small dataset should use sequential
        let small_definitions: Vec<StructureDefinition> = (0..3)
            .map(|i| create_test_definition(&format!("small{i}")))
            .collect();

        let strategy = converter.choose_strategy(&small_definitions);
        assert_eq!(strategy, ConversionStrategy::Sequential);

        // Large dataset with low threshold might use parallel (but our analysis shows it shouldn't)
        let large_definitions: Vec<StructureDefinition> = (0..100)
            .map(|i| create_test_definition(&format!("large{i}")))
            .collect();

        let strategy = converter.choose_strategy(&large_definitions);
        // With very low threshold (5), large dataset (100) should use parallel
        // But our analysis shows sequential is still better, so we expect parallel here
        // since the threshold logic allows it, even though it's not optimal
        assert_eq!(strategy, ConversionStrategy::Parallel);
    }
}
