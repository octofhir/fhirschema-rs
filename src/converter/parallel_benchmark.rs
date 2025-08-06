use super::{
    FhirSchemaConverter, ParallelSchemaConverter, StructureDefinition, StructureDefinitionConverter,
};
use std::time::Instant;

/// Comprehensive benchmark to find the break-even point for parallel processing
pub struct ParallelBenchmark;

impl ParallelBenchmark {
    /// Run benchmarks with different dataset sizes to find when parallel becomes beneficial
    pub async fn run_scalability_benchmark() {
        println!("=== Parallel vs Sequential Scalability Benchmark ===\n");

        let test_sizes = vec![10, 50, 100, 200, 500, 1000, 2000, 5000];

        for size in test_sizes {
            println!("Testing with {size} items:");
            let (seq_time, par_time, speedup) = Self::benchmark_size(size).await;

            println!("  Sequential: {seq_time:?}");
            println!("  Parallel:   {par_time:?}");
            println!("  Speedup:    {speedup:.2}x");

            if speedup > 1.0 {
                println!("  ✅ Parallel is FASTER");
            } else {
                println!("  ❌ Sequential is faster ({:.1}x)", 1.0 / speedup);
            }
            println!();
        }
    }

    async fn benchmark_size(size: usize) -> (std::time::Duration, std::time::Duration, f64) {
        // Create test data
        let definitions: Vec<StructureDefinition> = (0..size)
            .map(|i| Self::create_complex_test_definition(&format!("test{i}")))
            .collect();

        // Sequential benchmark
        let sequential_converter = FhirSchemaConverter::new();
        let start = Instant::now();
        let mut sequential_results = Vec::new();
        for def in &definitions {
            if let Ok(schema) = sequential_converter.convert(def) {
                sequential_results.push(schema);
            }
        }
        let sequential_duration = start.elapsed();

        // Parallel benchmark
        let parallel_converter = ParallelSchemaConverter::new(FhirSchemaConverter::new());
        let start = Instant::now();
        let parallel_results = parallel_converter
            .convert_batch(definitions.clone())
            .await
            .unwrap();
        let parallel_duration = start.elapsed();

        // Calculate speedup
        let speedup = if parallel_duration.as_nanos() > 0 {
            sequential_duration.as_nanos() as f64 / parallel_duration.as_nanos() as f64
        } else {
            0.0
        };

        // Verify results are equivalent
        assert_eq!(sequential_results.len(), parallel_results.len());

        (sequential_duration, parallel_duration, speedup)
    }

    /// Create a more complex StructureDefinition to make conversion work more CPU-intensive
    fn create_complex_test_definition(id: &str) -> StructureDefinition {
        use serde_json::json;

        let complex_definition = json!({
            "resourceType": "StructureDefinition",
            "id": id,
            "url": format!("http://example.com/StructureDefinition/{}", id),
            "name": format!("Test{}", id),
            "status": "active",
            "kind": "resource",
            "abstract": false,
            "type": "Patient",
            "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
            "derivation": "constraint",
            "differential": {
                "element": [
                    {
                        "id": format!("{}.identifier", id),
                        "path": "Patient.identifier",
                        "min": 1,
                        "max": "*",
                        "type": [
                            {
                                "code": "Identifier"
                            }
                        ],
                        "constraint": [
                            {
                                "key": "pat-1",
                                "severity": "error",
                                "human": "Patient must have identifier",
                                "expression": "identifier.exists()"
                            }
                        ]
                    },
                    {
                        "id": format!("{}.name", id),
                        "path": "Patient.name",
                        "min": 1,
                        "max": "*",
                        "type": [
                            {
                                "code": "HumanName"
                            }
                        ],
                        "constraint": [
                            {
                                "key": "pat-2",
                                "severity": "error",
                                "human": "Patient must have name",
                                "expression": "name.exists()"
                            }
                        ]
                    },
                    {
                        "id": format!("{}.birthDate", id),
                        "path": "Patient.birthDate",
                        "min": 0,
                        "max": "1",
                        "type": [
                            {
                                "code": "date"
                            }
                        ]
                    },
                    {
                        "id": format!("{}.address", id),
                        "path": "Patient.address",
                        "min": 0,
                        "max": "*",
                        "type": [
                            {
                                "code": "Address"
                            }
                        ],
                        "constraint": [
                            {
                                "key": "pat-3",
                                "severity": "warning",
                                "human": "Address should be complete",
                                "expression": "line.exists() and city.exists() and postalCode.exists()"
                            }
                        ]
                    }
                ]
            }
        });

        serde_json::from_value(complex_definition).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scalability_benchmark() {
        // This test demonstrates when parallel processing becomes beneficial
        ParallelBenchmark::run_scalability_benchmark().await;
    }

    #[tokio::test]
    async fn test_overhead_analysis() {
        println!("=== Parallel Processing Overhead Analysis ===\n");

        // Test very small datasets to understand overhead
        let small_sizes = vec![1, 5, 10, 25, 50];

        for size in small_sizes {
            let (seq_time, par_time, speedup) = ParallelBenchmark::benchmark_size(size).await;
            let overhead = par_time.saturating_sub(seq_time);

            println!(
                "Size {size}: Sequential {seq_time:?}, Parallel {par_time:?}, Overhead {overhead:?}, Speedup {speedup:.2}x"
            );
        }
    }
}
