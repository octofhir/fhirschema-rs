use criterion::{Criterion, criterion_group, criterion_main};
use octofhir_fhirschema::*;
use std::hint::black_box;
use url::Url;

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

fn bench_schema_creation(c: &mut Criterion) {
    c.bench_function("schema_creation", |b| {
        b.iter(|| black_box(create_large_schema()))
    });
}

fn bench_schema_validation(c: &mut Criterion) {
    let schema = create_large_schema();

    c.bench_function("schema_validation", |b| {
        b.iter(|| black_box(schema.validate_structure()).unwrap())
    });
}

fn bench_schema_serialization(c: &mut Criterion) {
    let schema = create_large_schema();

    c.bench_function("schema_json_serialization", |b| {
        b.iter(|| black_box(serde_json::to_string(&schema)).unwrap())
    });
}

#[cfg(feature = "memory-storage")]
fn bench_memory_storage(c: &mut Criterion) {
    use std::sync::Arc;
    use tokio::runtime::Runtime;

    let storage = Arc::new(MemoryStorage::new());
    let schema = create_large_schema();
    let url = Url::parse("http://example.com/Patient").unwrap();

    c.bench_function("memory_storage_put", |b| {
        b.iter(|| {
            let rt = Runtime::new().unwrap();
            let storage = storage.clone();
            let schema = schema.clone();
            let url = url.clone();
            rt.block_on(async { black_box(storage.put(url, schema).await).unwrap() })
        })
    });

    // Pre-populate for get benchmark
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        storage.put(url.clone(), schema.clone()).await.unwrap();
    });

    c.bench_function("memory_storage_get", |b| {
        b.iter(|| {
            let rt = Runtime::new().unwrap();
            let storage = storage.clone();
            let url = url.clone();
            rt.block_on(async { black_box(storage.get(&url).await).unwrap() })
        })
    });
}

#[cfg(feature = "memory-storage")]
criterion_group!(
    benches,
    bench_schema_creation,
    bench_schema_validation,
    bench_schema_serialization,
    bench_memory_storage
);

#[cfg(not(feature = "memory-storage"))]
criterion_group!(
    benches,
    bench_schema_creation,
    bench_schema_validation,
    bench_schema_serialization
);

criterion_main!(benches);
