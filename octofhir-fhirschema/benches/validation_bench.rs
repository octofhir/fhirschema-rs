//! FHIR resource validation benchmarks
//!
//! Run:
//!   cargo bench --bench validation_bench
//!
//! Profiling with flamegraph:
//!   cargo flamegraph --bench validation_bench -- --bench validate_bundle

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use octofhir_fhirschema::{FhirSchemaValidator, FhirValidator, FhirVersion, get_schemas};
use serde_json::{json, Value as JsonValue};
use tokio::runtime::Runtime;

/// Create runtime for async benchmarks
fn create_runtime() -> Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Minimal Patient resource
fn patient_minimal() -> JsonValue {
    json!({
        "resourceType": "Patient",
        "id": "example"
    })
}

/// Simple Patient resource
fn patient_simple() -> JsonValue {
    json!({
        "resourceType": "Patient",
        "id": "example",
        "active": true,
        "name": [{
            "use": "official",
            "family": "Chalmers",
            "given": ["Peter", "James"]
        }],
        "gender": "male",
        "birthDate": "1974-12-25"
    })
}

/// Patient with all typical fields
fn patient_full() -> JsonValue {
    json!({
        "resourceType": "Patient",
        "id": "example-full",
        "meta": {
            "versionId": "1",
            "lastUpdated": "2024-01-01T00:00:00Z"
        },
        "identifier": [{
            "system": "http://hospital.example.org/patients",
            "value": "12345"
        }],
        "active": true,
        "name": [
            {
                "use": "official",
                "family": "Chalmers",
                "given": ["Peter", "James"],
                "prefix": ["Mr."]
            },
            {
                "use": "nickname",
                "given": ["Pete"]
            }
        ],
        "telecom": [
            {"system": "phone", "value": "+1-555-555-5555", "use": "home"},
            {"system": "email", "value": "peter@example.com"}
        ],
        "gender": "male",
        "birthDate": "1974-12-25",
        "deceasedBoolean": false,
        "address": [{
            "use": "home",
            "type": "both",
            "line": ["123 Main St"],
            "city": "Anytown",
            "state": "CA",
            "postalCode": "12345",
            "country": "USA"
        }],
        "maritalStatus": {
            "coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/v3-MaritalStatus",
                "code": "M",
                "display": "Married"
            }]
        },
        "contact": [{
            "relationship": [{
                "coding": [{
                    "system": "http://terminology.hl7.org/CodeSystem/v2-0131",
                    "code": "N"
                }]
            }],
            "name": {"family": "du Marché", "given": ["Bénédicte"]},
            "telecom": [{"system": "phone", "value": "+33 (237) 998327"}]
        }],
        "communication": [{
            "language": {
                "coding": [{
                    "system": "urn:ietf:bcp:47",
                    "code": "en"
                }]
            },
            "preferred": true
        }]
    })
}

/// Observation resource
fn observation_simple() -> JsonValue {
    json!({
        "resourceType": "Observation",
        "id": "example",
        "status": "final",
        "code": {
            "coding": [{
                "system": "http://loinc.org",
                "code": "29463-7",
                "display": "Body Weight"
            }]
        },
        "subject": {
            "reference": "Patient/example"
        },
        "effectiveDateTime": "2024-01-01T00:00:00Z",
        "valueQuantity": {
            "value": 70.0,
            "unit": "kg",
            "system": "http://unitsofmeasure.org",
            "code": "kg"
        }
    })
}

/// Bundle with N resources
fn bundle_with_resources(count: usize) -> JsonValue {
    let entries: Vec<JsonValue> = (0..count)
        .map(|i| {
            json!({
                "fullUrl": format!("urn:uuid:patient-{}", i),
                "resource": {
                    "resourceType": "Patient",
                    "id": format!("patient-{}", i),
                    "active": true,
                    "name": [{
                        "family": format!("Family{}", i),
                        "given": [format!("Given{}", i)]
                    }],
                    "gender": if i % 2 == 0 { "male" } else { "female" },
                    "birthDate": "1990-01-01"
                }
            })
        })
        .collect();

    json!({
        "resourceType": "Bundle",
        "id": "bundle-example",
        "type": "collection",
        "entry": entries
    })
}

/// Benchmark: schema lookup (isolated)
fn bench_schema_lookup(c: &mut Criterion) {
    let schemas = get_schemas(FhirVersion::R4);

    c.bench_function("schema_lookup_by_name", |b| {
        b.iter(|| {
            let _ = black_box(schemas.get("Patient"));
        });
    });

    c.bench_function("schema_lookup_miss", |b| {
        b.iter(|| {
            let _ = black_box(schemas.get("NonExistentType"));
        });
    });
}

/// Benchmark: Patient validation
fn bench_validate_patient(c: &mut Criterion) {
    let rt = create_runtime();
    let schemas = get_schemas(FhirVersion::R4).clone();
    let validator = FhirSchemaValidator::new(schemas, None);

    let patient_min = patient_minimal();
    let patient_simple = patient_simple();
    let patient_full = patient_full();

    let mut group = c.benchmark_group("validate_patient");

    group.bench_function("minimal", |b| {
        b.iter(|| {
            rt.block_on(async {
                validator
                    .validate(black_box(&patient_min), vec!["Patient".to_string()])
                    .await
            })
        });
    });

    group.bench_function("simple", |b| {
        b.iter(|| {
            rt.block_on(async {
                validator
                    .validate(black_box(&patient_simple), vec!["Patient".to_string()])
                    .await
            })
        });
    });

    group.bench_function("full", |b| {
        b.iter(|| {
            rt.block_on(async {
                validator
                    .validate(black_box(&patient_full), vec!["Patient".to_string()])
                    .await
            })
        });
    });

    group.finish();
}

/// Benchmark: Observation validation
fn bench_validate_observation(c: &mut Criterion) {
    let rt = create_runtime();
    let schemas = get_schemas(FhirVersion::R4).clone();
    let validator = FhirSchemaValidator::new(schemas, None);

    let observation = observation_simple();

    c.bench_function("validate_observation", |b| {
        b.iter(|| {
            rt.block_on(async {
                validator
                    .validate(black_box(&observation), vec!["Observation".to_string()])
                    .await
            })
        });
    });
}

/// Benchmark: Bundle validation with varying sizes
fn bench_validate_bundle(c: &mut Criterion) {
    let rt = create_runtime();
    let schemas = get_schemas(FhirVersion::R4).clone();
    let validator = FhirSchemaValidator::new(schemas, None);

    let mut group = c.benchmark_group("validate_bundle");

    for count in [1, 10, 50, 100].iter() {
        let bundle = bundle_with_resources(*count);

        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &bundle, |b, bundle| {
            b.iter(|| {
                rt.block_on(async {
                    validator
                        .validate(black_box(bundle), vec!["Bundle".to_string()])
                        .await
                })
            });
        });
    }

    group.finish();
}

/// Benchmark: throughput (resources per second)
fn bench_throughput(c: &mut Criterion) {
    let rt = create_runtime();
    let schemas = get_schemas(FhirVersion::R4).clone();
    let validator = FhirSchemaValidator::new(schemas, None);

    // Подготовим batch разных ресурсов
    let resources: Vec<JsonValue> = (0..100)
        .map(|i| {
            if i % 3 == 0 {
                patient_simple()
            } else if i % 3 == 1 {
                observation_simple()
            } else {
                patient_full()
            }
        })
        .collect();

    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Elements(100));

    group.bench_function("mixed_100_resources", |b| {
        b.iter(|| {
            rt.block_on(async {
                for resource in &resources {
                    let resource_type = resource
                        .get("resourceType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Patient");
                    let _ = validator
                        .validate(black_box(resource), vec![resource_type.to_string()])
                        .await;
                }
            })
        });
    });

    group.finish();
}

/// Benchmark: validator creation
fn bench_validator_creation(c: &mut Criterion) {
    let schemas = get_schemas(FhirVersion::R4).clone();

    c.bench_function("validator_creation", |b| {
        b.iter(|| {
            let _ = black_box(FhirSchemaValidator::new(schemas.clone(), None));
        });
    });
}

criterion_group!(
    benches,
    bench_schema_lookup,
    bench_validate_patient,
    bench_validate_observation,
    bench_validate_bundle,
    bench_throughput,
    bench_validator_creation,
);

criterion_main!(benches);
