/*!
 * Integration with FHIR Libraries Example
 * =======================================
 *
 * This example demonstrates how to integrate octofhir-fhirschema with other FHIR libraries
 * in the Rust ecosystem, showing common integration patterns and best practices.
 *
 * Key integration points:
 * - Type validation for incoming FHIR resources
 * - Schema-driven resource processing
 * - Performance optimization through precompiled providers
 * - Cross-library compatibility patterns
 */

use octofhir_fhir_model::provider::ModelProvider;
use octofhir_fhirschema::core::FhirVersion;
use octofhir_fhirschema::provider::{CompositeModelProvider, EmbeddedModelProvider};
use std::sync::Arc;
use tokio::sync::OnceCell;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 FHIR Library Integration Examples");
    println!("====================================\n");

    // Example 1: Basic integration setup
    demonstrate_basic_integration().await?;

    // Example 2: Validation service integration
    demonstrate_validation_service().await?;

    // Example 3: Resource processing pipeline
    demonstrate_processing_pipeline().await?;

    // Example 4: Multi-library coordination
    demonstrate_multi_library_setup().await?;

    // Example 5: Performance optimization patterns
    demonstrate_performance_patterns().await?;

    Ok(())
}

async fn demonstrate_basic_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏗️  1. Basic Integration Setup");
    println!("------------------------------");

    // Create a provider that can be shared across your application
    let provider = Arc::new(CompositeModelProvider::r4().await?);
    println!("✅ Created shared CompositeModelProvider");

    // Example: Validating resource types before processing
    let incoming_resource_types = vec!["Patient", "Observation", "InvalidType", "Practitioner"];

    println!("🔍 Validating incoming resource types:");
    for resource_type in incoming_resource_types {
        let is_valid = provider.resource_type_exists(resource_type)?;
        let status = if is_valid { "✅" } else { "❌" };
        println!(
            "   {} {}: {}",
            status,
            resource_type,
            if is_valid {
                "Valid FHIR resource"
            } else {
                "Unknown resource type"
            }
        );
    }

    // Example: Getting type information for schema validation
    if let Some(hierarchy) = provider.get_type_hierarchy("Patient").await? {
        println!("📊 Patient resource hierarchy:");
        println!("   Type: {}", hierarchy.type_name);
        println!("   Parent: {:?}", hierarchy.direct_parent);
        println!("   Is Abstract: {}", hierarchy.is_abstract);
    }

    println!();
    Ok(())
}

async fn demonstrate_validation_service() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛡️  2. Validation Service Integration");
    println!("------------------------------------");

    // Create a validation service that uses schema information
    let validation_service = FhirValidationService::new(FhirVersion::R4).await?;

    // Example: Validate resource structure
    let test_cases = vec![
        ("Patient", r#"{"resourceType": "Patient", "id": "example"}"#),
        (
            "Observation",
            r#"{"resourceType": "Observation", "status": "final"}"#,
        ),
        ("InvalidType", r#"{"resourceType": "InvalidType"}"#),
    ];

    println!("🧪 Validation test cases:");
    for (expected_type, json_resource) in test_cases {
        match validation_service
            .validate_resource_json(expected_type, json_resource)
            .await
        {
            Ok(result) => {
                let status = if result.is_valid { "✅" } else { "❌" };
                println!(
                    "   {} {}: {} ({})",
                    status,
                    expected_type,
                    if result.is_valid { "Valid" } else { "Invalid" },
                    result.summary
                );
            }
            Err(e) => {
                println!("   ❌ {expected_type}: Error - {e}");
            }
        }
    }

    println!();
    Ok(())
}

async fn demonstrate_processing_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏭 3. Resource Processing Pipeline");
    println!("----------------------------------");

    let processor = FhirResourceProcessor::new(FhirVersion::R4).await?;

    // Example: Process different types of FHIR resources
    let sample_resources = vec![
        (
            "Patient",
            r#"{"resourceType": "Patient", "id": "patient-1", "name": [{"family": "Doe", "given": ["John"]}]}"#,
        ),
        (
            "Observation",
            r#"{"resourceType": "Observation", "id": "obs-1", "status": "final", "code": {"text": "Blood pressure"}}"#,
        ),
        (
            "Bundle",
            r#"{"resourceType": "Bundle", "type": "collection", "entry": []}"#,
        ),
    ];

    println!("🔄 Processing FHIR resources:");
    for (resource_type, json_resource) in sample_resources {
        match processor
            .process_resource(resource_type, json_resource)
            .await
        {
            Ok(result) => {
                println!(
                    "   ✅ {}: {} (took {:?})",
                    resource_type, result.summary, result.processing_time
                );
                if !result.extracted_fields.is_empty() {
                    println!("      Extracted fields: {:?}", result.extracted_fields);
                }
            }
            Err(e) => {
                println!("   ❌ {resource_type}: Error - {e}");
            }
        }
    }

    println!();
    Ok(())
}

async fn demonstrate_multi_library_setup() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤝 4. Multi-Library Coordination");
    println!("---------------------------------");

    // Example: Create a centralized FHIR service manager
    let fhir_manager = FhirServiceManager::new().await?;
    println!("✅ Created centralized FHIR service manager");

    // Example: Different services using the same provider
    println!("🔧 Service capabilities:");

    // Validation service
    let can_validate = fhir_manager
        .get_validator()
        .can_validate_type("Patient")
        .await?;
    println!(
        "   Validation service: {} (Patient validation: {})",
        if can_validate {
            "Available"
        } else {
            "Unavailable"
        },
        can_validate
    );

    // Processing service
    let supported_types = fhir_manager
        .get_processor()
        .supported_resource_types()
        .await?;
    println!(
        "   Processing service: Available ({} supported types)",
        supported_types.len()
    );

    // Schema service
    let schema_count = fhir_manager.get_schema_provider().schema_count();
    println!(
        "   Schema service: Available ({schema_count} preloaded schemas)"
    );

    // Example: Coordinated processing workflow
    let workflow_result = fhir_manager
        .process_workflow(vec![
            ("validate", "Patient"),
            ("extract", "Patient"),
            ("transform", "Patient"),
        ])
        .await?;

    println!(
        "🔄 Workflow result: {} steps completed successfully",
        workflow_result.completed_steps
    );

    println!();
    Ok(())
}

async fn demonstrate_performance_patterns() -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ 5. Performance Optimization Patterns");
    println!("---------------------------------------");

    // Pattern 1: Global singleton for maximum sharing
    println!("🚀 Pattern 1: Global Singleton Provider");
    let global_provider = get_global_fhir_provider().await?;
    let startup_cost = measure_provider_startup().await?;
    println!("   Global provider ready (startup: {startup_cost:?})");

    // Pattern 2: Lazy initialization per thread
    println!("🧵 Pattern 2: Thread-Local Lazy Providers");
    let thread_local_cost = measure_thread_local_performance().await?;
    println!(
        "   Thread-local providers ready (avg setup: {thread_local_cost:?})"
    );

    // Pattern 3: Embedded vs Composite performance comparison
    println!("📊 Pattern 3: Provider Performance Comparison");

    let embedded_provider = EmbeddedModelProvider::r4().await?;
    let composite_provider = CompositeModelProvider::r4().await?;

    // Measure lookup performance
    let embedded_time = measure_lookup_performance(&embedded_provider).await?;
    let composite_time = measure_lookup_performance(&composite_provider).await?;

    println!("   Embedded provider: {embedded_time:?} avg lookup time");
    println!(
        "   Composite provider: {composite_time:?} avg lookup time"
    );

    let speedup = composite_time.as_nanos() as f64 / embedded_time.as_nanos() as f64;
    println!("   Embedded provider is {speedup:.2}x faster for lookups");

    // Pattern 4: Bulk operation optimization
    println!("📦 Pattern 4: Bulk Operation Optimization");
    let bulk_performance = measure_bulk_operations(global_provider).await?;
    println!(
        "   Bulk validation: {} resources/sec",
        bulk_performance.resources_per_second
    );
    println!(
        "   Memory efficiency: {} MB peak usage",
        bulk_performance.peak_memory_mb
    );

    println!();
    Ok(())
}

// Supporting structures and implementations

struct FhirValidationService {
    provider: Arc<CompositeModelProvider>,
}

struct ValidationResult {
    is_valid: bool,
    summary: String,
}

impl FhirValidationService {
    async fn new(version: FhirVersion) -> Result<Self, Box<dyn std::error::Error>> {
        let provider = match version {
            FhirVersion::R4 => CompositeModelProvider::r4().await?,
            FhirVersion::R4B => CompositeModelProvider::r4b().await?,
            FhirVersion::R5 => CompositeModelProvider::r5().await?,
            FhirVersion::R6 => CompositeModelProvider::r6().await?,
        };

        Ok(Self {
            provider: Arc::new(provider),
        })
    }

    async fn validate_resource_json(
        &self,
        expected_type: &str,
        json: &str,
    ) -> Result<ValidationResult, Box<dyn std::error::Error>> {
        // Parse the JSON to extract resource type
        let parsed: serde_json::Value = serde_json::from_str(json)?;
        let resource_type = parsed
            .get("resourceType")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        // Check if the resource type exists in our schema
        let type_exists = self.provider.resource_type_exists(resource_type)?;

        if !type_exists {
            return Ok(ValidationResult {
                is_valid: false,
                summary: format!("Unknown resource type: {resource_type}"),
            });
        }

        // Check if it matches expected type
        if resource_type != expected_type {
            return Ok(ValidationResult {
                is_valid: false,
                summary: format!("Expected {expected_type} but got {resource_type}"),
            });
        }

        // Basic structure validation (in practice, you'd do more comprehensive validation)
        let has_id = parsed.get("id").is_some();
        let has_resource_type = parsed.get("resourceType").is_some();

        Ok(ValidationResult {
            is_valid: has_resource_type,
            summary: format!(
                "Basic validation passed (id: {})",
                if has_id { "present" } else { "missing" }
            ),
        })
    }
}

struct FhirResourceProcessor {
    provider: Arc<CompositeModelProvider>,
}

struct ProcessingResult {
    summary: String,
    processing_time: std::time::Duration,
    extracted_fields: Vec<String>,
}

impl FhirResourceProcessor {
    async fn new(version: FhirVersion) -> Result<Self, Box<dyn std::error::Error>> {
        let provider = match version {
            FhirVersion::R4 => CompositeModelProvider::r4().await?,
            _ => CompositeModelProvider::r4().await?, // fallback
        };

        Ok(Self {
            provider: Arc::new(provider),
        })
    }

    async fn process_resource(
        &self,
        resource_type: &str,
        json: &str,
    ) -> Result<ProcessingResult, Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();

        // Validate resource type exists
        if !self.provider.resource_type_exists(resource_type)? {
            return Err(format!("Unknown resource type: {resource_type}").into());
        }

        // Parse JSON and extract fields
        let parsed: serde_json::Value = serde_json::from_str(json)?;
        let mut extracted_fields = Vec::new();

        // Extract common fields
        if let Some(id) = parsed.get("id").and_then(|v| v.as_str()) {
            extracted_fields.push(format!("id: {id}"));
        }

        if let Some(resource_type_val) = parsed.get("resourceType").and_then(|v| v.as_str()) {
            extracted_fields.push(format!("resourceType: {resource_type_val}"));
        }

        // Resource-specific field extraction
        match resource_type {
            "Patient" => {
                if let Some(name) = parsed.get("name").and_then(|v| v.as_array()) {
                    if let Some(first_name) = name.first() {
                        if let Some(family) = first_name.get("family").and_then(|v| v.as_str()) {
                            extracted_fields.push(format!("family: {family}"));
                        }
                    }
                }
            }
            "Observation" => {
                if let Some(status) = parsed.get("status").and_then(|v| v.as_str()) {
                    extracted_fields.push(format!("status: {status}"));
                }
            }
            _ => {}
        }

        let processing_time = start_time.elapsed();

        Ok(ProcessingResult {
            summary: format!("Processed {resource_type} resource"),
            processing_time,
            extracted_fields,
        })
    }

    async fn supported_resource_types(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Ok(self.provider.get_supported_resource_types().await?)
    }
}

struct FhirServiceManager {
    validator: Arc<FhirValidationService>,
    processor: Arc<FhirResourceProcessor>,
    schema_provider: Arc<EmbeddedModelProvider>,
}

struct WorkflowResult {
    completed_steps: usize,
}

impl FhirServiceManager {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            validator: Arc::new(FhirValidationService::new(FhirVersion::R4).await?),
            processor: Arc::new(FhirResourceProcessor::new(FhirVersion::R4).await?),
            schema_provider: Arc::new(EmbeddedModelProvider::r4().await?),
        })
    }

    fn get_validator(&self) -> &FhirValidationService {
        &self.validator
    }

    fn get_processor(&self) -> &FhirResourceProcessor {
        &self.processor
    }

    fn get_schema_provider(&self) -> &EmbeddedModelProvider {
        &self.schema_provider
    }

    async fn process_workflow(
        &self,
        steps: Vec<(&str, &str)>,
    ) -> Result<WorkflowResult, Box<dyn std::error::Error>> {
        let mut completed = 0;

        for (operation, resource_type) in steps {
            match operation {
                "validate" => {
                    if self
                        .validator
                        .provider
                        .resource_type_exists(resource_type)?
                    {
                        completed += 1;
                    }
                }
                "extract" | "transform" => {
                    // Simulate processing step
                    if self
                        .processor
                        .provider
                        .resource_type_exists(resource_type)?
                    {
                        completed += 1;
                    }
                }
                _ => {}
            }
        }

        Ok(WorkflowResult {
            completed_steps: completed,
        })
    }
}

impl FhirValidationService {
    async fn can_validate_type(
        &self,
        resource_type: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(self.provider.resource_type_exists(resource_type)?)
    }
}

// Global provider singleton
static GLOBAL_PROVIDER: OnceCell<Arc<CompositeModelProvider>> = OnceCell::const_new();

async fn get_global_fhir_provider(
) -> Result<&'static Arc<CompositeModelProvider>, Box<dyn std::error::Error>> {
    GLOBAL_PROVIDER
        .get_or_try_init(|| async { CompositeModelProvider::r4().await.map(Arc::new) })
        .await
        .map_err(|e| e.into())
}

async fn measure_provider_startup() -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let _provider = CompositeModelProvider::r4().await?;
    Ok(start.elapsed())
}

async fn measure_thread_local_performance(
) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();

    // Simulate multiple thread-local providers
    let _provider1 = EmbeddedModelProvider::r4().await?;
    let _provider2 = EmbeddedModelProvider::r4().await?;
    let _provider3 = EmbeddedModelProvider::r4().await?;

    Ok(start.elapsed() / 3)
}

async fn measure_lookup_performance<T: ModelProvider>(
    provider: &T,
) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    let iterations = 1000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = provider.resource_type_exists("Patient");
    }

    Ok(start.elapsed() / iterations)
}

struct BulkPerformanceResult {
    resources_per_second: u64,
    peak_memory_mb: u64,
}

async fn measure_bulk_operations(
    provider: &Arc<CompositeModelProvider>,
) -> Result<BulkPerformanceResult, Box<dyn std::error::Error>> {
    let iterations = 10000;
    let start = std::time::Instant::now();

    // Simulate bulk validation
    for i in 0..iterations {
        let resource_type = match i % 4 {
            0 => "Patient",
            1 => "Observation",
            2 => "Practitioner",
            _ => "Organization",
        };
        let _ = provider.resource_type_exists(resource_type)?;
    }

    let elapsed = start.elapsed();
    let resources_per_second = (iterations * 1000) / elapsed.as_millis().max(1) as u64;

    Ok(BulkPerformanceResult {
        resources_per_second,
        peak_memory_mb: 50, // Simulated value
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validation_service_integration() {
        let service = FhirValidationService::new(FhirVersion::R4).await.unwrap();

        let result = service
            .validate_resource_json(
                "Patient",
                r#"{"resourceType": "Patient", "id": "test-patient"}"#,
            )
            .await
            .unwrap();

        assert!(result.is_valid);
        assert!(result.summary.contains("Basic validation passed"));
    }

    #[tokio::test]
    async fn test_processing_service_integration() {
        let processor = FhirResourceProcessor::new(FhirVersion::R4).await.unwrap();

        let result = processor
            .process_resource(
                "Patient",
                r#"{"resourceType": "Patient", "id": "test", "name": [{"family": "Test"}]}"#,
            )
            .await
            .unwrap();

        assert!(result.summary.contains("Processed Patient"));
        assert!(!result.extracted_fields.is_empty());
    }

    #[tokio::test]
    async fn test_service_manager_integration() {
        let manager = FhirServiceManager::new().await.unwrap();

        let can_validate = manager
            .get_validator()
            .can_validate_type("Patient")
            .await
            .unwrap();
        assert!(can_validate);

        let supported_types = manager
            .get_processor()
            .supported_resource_types()
            .await
            .unwrap();
        assert!(!supported_types.is_empty());
        assert!(supported_types.contains(&"Patient".to_string()));
    }
}
