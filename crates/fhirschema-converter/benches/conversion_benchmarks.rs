//! Performance benchmarks for FHIRSchema conversion.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fhirschema_converter::StructureDefinitionConverter;
use serde_json::json;

/// Generate a simple StructureDefinition for benchmarking.
fn generate_simple_structure_definition(name: &str) -> String {
    json!({
        "resourceType": "StructureDefinition",
        "url": format!("http://example.org/StructureDefinition/{}", name),
        "name": name,
        "status": "active",
        "kind": "resource",
        "type": "Patient",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
        "derivation": "constraint",
        "differential": {
            "element": [
                {
                    "id": "Patient",
                    "path": "Patient"
                },
                {
                    "id": "Patient.name",
                    "path": "Patient.name",
                    "short": "Patient name",
                    "definition": "The name of the patient",
                    "min": 1,
                    "max": "*"
                }
            ]
        }
    }).to_string()
}

/// Generate a complex StructureDefinition with multiple elements for benchmarking.
fn generate_complex_structure_definition(name: &str, element_count: usize) -> String {
    let mut elements = vec![
        json!({
            "id": "Patient",
            "path": "Patient"
        })
    ];

    // Add multiple elements to test performance with larger profiles
    for i in 0..element_count {
        elements.push(json!({
            "id": format!("Patient.extension:ext{}", i),
            "path": "Patient.extension",
            "sliceName": format!("ext{}", i),
            "short": format!("Extension {}", i),
            "definition": format!("Custom extension number {}", i),
            "min": 0,
            "max": "1",
            "type": [
                {
                    "code": "Extension",
                    "profile": [format!("http://example.org/StructureDefinition/ext-{}", i)]
                }
            ]
        }));
    }

    json!({
        "resourceType": "StructureDefinition",
        "url": format!("http://example.org/StructureDefinition/{}", name),
        "name": name,
        "status": "active",
        "kind": "resource",
        "type": "Patient",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
        "derivation": "constraint",
        "differential": {
            "element": elements
        }
    }).to_string()
}

/// Generate a StructureDefinition with slicing for benchmarking.
fn generate_sliced_structure_definition(name: &str, slice_count: usize) -> String {
    let mut elements = vec![
        json!({
            "id": "Patient",
            "path": "Patient"
        }),
        json!({
            "id": "Patient.identifier",
            "path": "Patient.identifier",
            "slicing": {
                "discriminator": [
                    {
                        "type": "value",
                        "path": "system"
                    }
                ],
                "rules": "open",
                "ordered": false,
                "description": "Slice by identifier system"
            },
            "min": 1
        })
    ];

    // Add multiple slices
    for i in 0..slice_count {
        elements.push(json!({
            "id": format!("Patient.identifier:slice{}", i),
            "path": "Patient.identifier",
            "sliceName": format!("slice{}", i),
            "short": format!("Identifier slice {}", i),
            "definition": format!("Identifier slice number {}", i),
            "min": 0,
            "max": "1",
            "fixedValue": {
                "system": format!("http://example.org/slice-{}", i)
            }
        }));
    }

    json!({
        "resourceType": "StructureDefinition",
        "url": format!("http://example.org/StructureDefinition/{}", name),
        "name": name,
        "status": "active",
        "kind": "resource",
        "type": "Patient",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
        "derivation": "constraint",
        "differential": {
            "element": elements
        }
    }).to_string()
}

/// Benchmark simple StructureDefinition conversion.
fn bench_simple_conversion(c: &mut Criterion) {
    let converter = StructureDefinitionConverter::new();
    let structure_def = generate_simple_structure_definition("SimplePatient");

    c.bench_function("simple_conversion", |b| {
        b.iter(|| {
            let result = converter.convert(black_box(&structure_def));
            black_box(result)
        })
    });
}

/// Benchmark conversion with varying element counts.
fn bench_element_count_scaling(c: &mut Criterion) {
    let converter = StructureDefinitionConverter::new();
    let mut group = c.benchmark_group("element_count_scaling");

    for element_count in [10, 50, 100, 200, 500].iter() {
        let structure_def = generate_complex_structure_definition(
            &format!("ComplexPatient{}", element_count),
            *element_count
        );

        group.bench_with_input(
            BenchmarkId::new("elements", element_count),
            element_count,
            |b, _| {
                b.iter(|| {
                    let result = converter.convert(black_box(&structure_def));
                    black_box(result)
                })
            },
        );
    }
    group.finish();
}

/// Benchmark conversion with varying slice counts.
fn bench_slice_count_scaling(c: &mut Criterion) {
    let converter = StructureDefinitionConverter::new();
    let mut group = c.benchmark_group("slice_count_scaling");

    for slice_count in [5, 10, 25, 50, 100].iter() {
        let structure_def = generate_sliced_structure_definition(
            &format!("SlicedPatient{}", slice_count),
            *slice_count
        );

        group.bench_with_input(
            BenchmarkId::new("slices", slice_count),
            slice_count,
            |b, _| {
                b.iter(|| {
                    let result = converter.convert(black_box(&structure_def));
                    black_box(result)
                })
            },
        );
    }
    group.finish();
}

/// Benchmark converter creation overhead.
fn bench_converter_creation(c: &mut Criterion) {
    c.bench_function("converter_creation", |b| {
        b.iter(|| {
            let converter = StructureDefinitionConverter::new();
            black_box(converter)
        })
    });
}

/// Benchmark JSON parsing overhead.
fn bench_json_parsing(c: &mut Criterion) {
    let structure_def = generate_simple_structure_definition("SimplePatient");

    c.bench_function("json_parsing", |b| {
        b.iter(|| {
            let parsed: serde_json::Value = serde_json::from_str(black_box(&structure_def)).unwrap();
            black_box(parsed)
        })
    });
}

/// Benchmark memory usage patterns with repeated conversions.
fn bench_memory_usage(c: &mut Criterion) {
    let converter = StructureDefinitionConverter::new();
    let structure_defs: Vec<String> = (0..100)
        .map(|i| generate_simple_structure_definition(&format!("Patient{}", i)))
        .collect();

    c.bench_function("batch_conversion_100", |b| {
        b.iter(|| {
            for structure_def in &structure_defs {
                let result = converter.convert(black_box(structure_def));
                black_box(result);
            }
        })
    });
}

/// Benchmark constraint processing performance.
fn bench_constraint_processing(c: &mut Criterion) {
    let converter = StructureDefinitionConverter::new();

    let structure_def_with_constraints = json!({
        "resourceType": "StructureDefinition",
        "url": "http://example.org/StructureDefinition/constrained-patient",
        "name": "ConstrainedPatient",
        "status": "active",
        "kind": "resource",
        "type": "Patient",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
        "derivation": "constraint",
        "differential": {
            "element": [
                {
                    "id": "Patient",
                    "path": "Patient"
                },
                {
                    "id": "Patient.name",
                    "path": "Patient.name",
                    "constraint": [
                        {
                            "key": "name-1",
                            "severity": "error",
                            "human": "Name must be present",
                            "expression": "family.exists() or given.exists()"
                        },
                        {
                            "key": "name-2",
                            "severity": "warning",
                            "human": "Should have both family and given names",
                            "expression": "family.exists() and given.exists()"
                        },
                        {
                            "key": "name-3",
                            "severity": "error",
                            "human": "Family name should not be empty",
                            "expression": "family.empty().not()"
                        }
                    ]
                }
            ]
        }
    }).to_string();

    c.bench_function("constraint_processing", |b| {
        b.iter(|| {
            let result = converter.convert(black_box(&structure_def_with_constraints));
            black_box(result)
        })
    });
}

criterion_group!(
    benches,
    bench_simple_conversion,
    bench_element_count_scaling,
    bench_slice_count_scaling,
    bench_converter_creation,
    bench_json_parsing,
    bench_memory_usage,
    bench_constraint_processing
);

criterion_main!(benches);
