use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use octofhir_canonical_manager::FcmConfig;
use octofhir_fhirschema::package::*;
use octofhir_fhirschema::*;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use url::Url;

/// Create a mock StructureDefinition for benchmarking
fn create_mock_structure_definition(name: &str, element_count: usize) -> StructureDefinition {
    let url = Url::parse(&format!("http://example.com/{name}")).unwrap();

    let mut elements = Vec::with_capacity(element_count);
    for i in 0..element_count {
        let element = ElementDefinition {
            id: None,
            path: format!("{name}.field{i}"),
            representation: None,
            slice_name: None,
            slice_is_constraining: None,
            label: None,
            code: None,
            slicing: None,
            short: None,
            definition: Some(format!("Field {i} for benchmarking")),
            comment: None,
            requirements: None,
            alias: None,
            min: Some(0),
            max: Some("1".to_string()),
            base: None,
            content_reference: None,
            element_type: Some(vec![ElementDefinitionType {
                code: "string".to_string(),
                profile: None,
                target_profile: None,
                aggregation: None,
                versioning: None,
            }]),
            name_reference: None,
            default_value: None,
            meaning_when_missing: None,
            order_meaning: None,
            fixed_value: None,
            pattern_value: None,
            example: None,
            min_value: None,
            max_value: None,
            max_length: None,
            condition: None,
            constraint: None,
            must_support: None,
            is_modifier: None,
            is_modifier_reason: None,
            is_summary: None,
            binding: None,
            mapping: None,
        };
        elements.push(element);
    }

    StructureDefinition {
        resource_type: "StructureDefinition".to_string(),
        id: None,
        url: Some(url),
        identifier: None,
        version: Some("1.0.0".to_string()),
        name: Some(name.to_string()),
        title: Some(format!("{name} Title")),
        status: Some("active".to_string()),
        experimental: None,
        date: None,
        publisher: None,
        contact: None,
        description: Some(format!("Mock {name} for benchmarking")),
        purpose: None,
        copyright: None,
        kind: "resource".to_string(),
        abstract_: Some(false),
        context: None,
        type_name: name.to_string(),
        base_definition: None,
        derivation: None,
        snapshot: None,
        differential: None,
        elements,
    }
}

/// Create multiple mock StructureDefinitions for batch benchmarks
fn create_mock_structure_definitions(
    count: usize,
    elements_per_structure: usize,
) -> Vec<StructureDefinition> {
    (0..count)
        .map(|i| create_mock_structure_definition(&format!("Resource{i}"), elements_per_structure))
        .collect()
}

/// Benchmark Arc vs Direct Schema Cloning
fn bench_arc_vs_direct_cloning(c: &mut Criterion) {
    let schema = create_large_schema();
    let arc_schema = Arc::new(schema.clone());

    let mut group = c.benchmark_group("arc_vs_direct_cloning");

    // Benchmark direct cloning
    group.bench_function("direct_clone", |b| b.iter(|| black_box(schema.clone())));

    // Benchmark Arc cloning
    group.bench_function("arc_clone", |b| {
        b.iter(|| black_box(Arc::clone(&arc_schema)))
    });

    group.finish();
}

/// Benchmark memory pre-allocation vs dynamic allocation
fn bench_memory_preallocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_preallocation");

    const ELEMENT_COUNT: usize = 1000;

    // Dynamic allocation
    group.bench_function("dynamic_allocation", |b| {
        b.iter(|| {
            let mut vec = Vec::new();
            for i in 0..ELEMENT_COUNT {
                vec.push(black_box(format!("Element{i}")));
            }
            vec
        })
    });

    // Pre-allocation
    group.bench_function("preallocation", |b| {
        b.iter(|| {
            let mut vec = Vec::with_capacity(ELEMENT_COUNT);
            for i in 0..ELEMENT_COUNT {
                vec.push(black_box(format!("Element{i}")));
            }
            vec
        })
    });

    group.finish();
}

/// Benchmark hierarchical cache with our optimizations
#[cfg(feature = "memory-storage")]
fn bench_hierarchical_cache_optimizations(c: &mut Criterion) {
    use octofhir_fhirschema::storage::{CacheConfig, HierarchicalCache, MemoryStorage};

    let rt = Runtime::new().unwrap();
    let config = CacheConfig::default();
    let storage = Arc::new(MemoryStorage::new());
    let cache = HierarchicalCache::new(config, storage);

    let schema = create_large_schema();
    let url = Url::parse("http://example.com/CacheTest").unwrap();

    // Pre-populate cache
    rt.block_on(async {
        cache.put(url.clone(), schema.clone()).await.unwrap();
    });

    let mut group = c.benchmark_group("hierarchical_cache");

    group.bench_function("optimized_get", |b| {
        b.iter(|| {
            let rt = Runtime::new().unwrap();
            let url = url.clone();
            rt.block_on(async { black_box(cache.get(&url).await.unwrap()) })
        })
    });

    group.finish();
}

/// Benchmark streaming vs batch processing
fn bench_streaming_vs_batch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let converter = Arc::new(FhirSchemaConverter::new());
    let conversion_options = ConversionOptions::default();

    let mut group = c.benchmark_group("streaming_vs_batch");
    group.measurement_time(Duration::from_secs(10));

    // Test different package sizes
    for size in [50, 100, 200, 500].iter() {
        let structure_definitions = create_mock_structure_definitions(*size, 10);

        let pipeline = ConversionPipeline::new(converter.clone(), 4);

        // Batch processing
        group.bench_with_input(
            BenchmarkId::new("batch", size),
            &structure_definitions,
            |b, structure_definitions| {
                b.iter(|| {
                    rt.block_on(async {
                        black_box(
                            pipeline
                                .convert_batch(structure_definitions, &conversion_options)
                                .await
                                .unwrap(),
                        )
                    })
                })
            },
        );

        // Streaming processing (only for larger sizes)
        if *size >= 100 {
            group.bench_with_input(
                BenchmarkId::new("streaming", size),
                &structure_definitions,
                |b, structure_definitions| {
                    b.iter(|| {
                        rt.block_on(async {
                            black_box(
                                pipeline
                                    .convert_streaming(structure_definitions, &conversion_options)
                                    .await
                                    .unwrap(),
                            )
                        })
                    })
                },
            );
        }
    }

    group.finish();
}

/// Benchmark package manager installation performance
fn bench_package_manager_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("package_manager");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(10);

    // Create a minimal FCM config for testing
    let fcm_config = FcmConfig {
        registry: octofhir_canonical_manager::RegistryConfig::default(),
        packages: vec![],
        storage: octofhir_canonical_manager::StorageConfig {
            cache_dir: std::env::temp_dir().join("bench-cache"),
            index_dir: std::env::temp_dir().join("bench-index"),
            packages_dir: std::env::temp_dir().join("bench-packages"),
            max_cache_size: "100MB".to_string(),
        },
        optimization: Default::default(),
    };

    let pm_config = PackageManagerConfig::default();

    group.bench_function("package_manager_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(
                    FhirSchemaPackageManager::new(fcm_config.clone(), pm_config.clone())
                        .await
                        .unwrap(),
                )
            })
        })
    });

    group.finish();
}

/// Benchmark schema registry O(1) access
fn bench_schema_registry_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = RegistryConfig::default();
    let registry = registry::PackageRegistry::new(config);

    // Create test schemas
    let schema_count = 1000;
    let mut schemas = Vec::with_capacity(schema_count);

    for i in 0..schema_count {
        let mut schema = create_large_schema();
        schema.url = Some(Url::parse(&format!("http://example.com/Schema{i}")).unwrap());
        schema.name = Some(format!("Schema{i}"));
        schemas.push(schema);
    }

    // Register all schemas
    rt.block_on(async {
        let package_id = PackageId::new("test.package", "1.0.0");
        let installed_package = InstalledPackage {
            id: package_id.clone(),
            spec: PackageSpec::registry("test.package", "1.0.0"),
            install_time: chrono::Utc::now(),
            file_path: None,
            checksum: None,
            schemas: schemas.iter().filter_map(|s| s.url.clone()).collect(),
            dependencies: Vec::new(),
            metadata: PackageMetadata::default(),
        };

        registry
            .register_package(installed_package, schemas.clone())
            .await
            .unwrap();
    });

    let mut group = c.benchmark_group("registry_access");
    group.throughput(Throughput::Elements(schema_count as u64));

    // Benchmark O(1) access
    group.bench_function("o1_schema_access", |b| {
        use rand::Rng;
        b.iter_batched(
            || rand::thread_rng().gen_range(0..schema_count),
            |index| {
                let url = format!("http://example.com/Schema{index}");
                rt.block_on(async { black_box(registry.get_schema(&url).await) })
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

/// Create a large schema for benchmarking (reused from existing benchmark)
fn create_large_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("Patient")
        .with_url(Url::parse("http://example.com/Patient").unwrap())
        .with_name("BenchmarkPatient");

    // Add multiple elements
    for i in 0..100 {
        let path = format!("Patient.field{i}");
        let element = Element::new(&path)
            .with_type(ElementType::new("string"))
            .with_cardinality(0, "1");
        schema = schema.with_element(&path, element);
    }

    // Add constraints
    for i in 0..20 {
        let constraint = Constraint::new(
            format!("constraint-{i}"),
            "error",
            format!("Test constraint {i}"),
            "true",
        );
        schema.constraints.push(constraint);
    }

    schema
}

// Group all benchmarks
#[cfg(feature = "memory-storage")]
criterion_group!(
    performance_benches,
    bench_arc_vs_direct_cloning,
    bench_memory_preallocation,
    bench_hierarchical_cache_optimizations,
    bench_streaming_vs_batch,
    bench_package_manager_performance,
    bench_schema_registry_access
);

#[cfg(not(feature = "memory-storage"))]
criterion_group!(
    performance_benches,
    bench_arc_vs_direct_cloning,
    bench_memory_preallocation,
    bench_streaming_vs_batch,
    bench_package_manager_performance,
    bench_schema_registry_access
);

criterion_main!(performance_benches);
