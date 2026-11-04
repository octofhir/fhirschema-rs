//! Slices matched by value through repeating elements, and slices only terminology can tell apart.

use std::collections::HashMap;

use octofhir_fhirschema::validation::FhirValidator;
use octofhir_fhirschema::validation::compiled::{
    BindingStrength, CompiledBinding, CompiledElement, CompiledSlice, SliceClassification,
};
use serde_json::json;

fn validator() -> FhirValidator {
    FhirValidator::from_schemas(HashMap::new(), None)
}

#[test]
fn an_object_pattern_matches_an_element_of_an_array() {
    // The R4 vitalsigns category slice: coding.code and coding.system, where coding repeats.
    let pattern = json!({ "coding": { "system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "vital-signs" } });
    let category = json!({ "coding": [
        { "system": "http://example.org", "code": "x" },
        { "system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "vital-signs", "display": "Vital Signs" }
    ] });
    assert!(FhirValidator::deep_partial_match(&category, &pattern));
    let other = json!({ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory" }] });
    assert!(!FhirValidator::deep_partial_match(&other, &pattern));
}

fn binding_slice(name: &str, value_set: &str) -> CompiledSlice {
    let schema = CompiledElement {
        binding: Some(CompiledBinding {
            value_set: value_set.to_owned(),
            strength: BindingStrength::Required,
            description: None,
        }),
        ..CompiledElement::default()
    };
    CompiledSlice {
        name: name.to_owned(),
        match_value: Some(json!({})),
        min: Some(1),
        max: None,
        schema: Some(Box::new(schema)),
    }
}

#[test]
fn slices_told_apart_by_binding_alone_are_indeterminate_without_terminology() {
    // US Core Condition.category: `us-core` and `screening-assessment`, discriminated by the
    // required binding of each.
    let mut slices = HashMap::new();
    slices.insert(
        "us-core".to_owned(),
        binding_slice(
            "us-core",
            "http://hl7.org/fhir/us/core/ValueSet/us-core-problem-or-health-concern",
        ),
    );
    slices.insert(
        "screening-assessment".to_owned(),
        binding_slice(
            "screening-assessment",
            "http://hl7.org/fhir/us/core/ValueSet/us-core-screening-assessment-condition-category",
        ),
    );
    let item = json!({ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/condition-category", "code": "problem-list-item" }] });
    match validator().classify_slice(&item, &slices) {
        SliceClassification::Indeterminate(mut names) => {
            names.sort();
            assert_eq!(names, vec!["screening-assessment", "us-core"]);
        }
        other => panic!("expected indeterminate, got {other:?}"),
    }
    // A slice matched by value wins over binding-only ones.
    slices.insert(
        "by-value".to_owned(),
        CompiledSlice {
            name: "by-value".to_owned(),
            match_value: Some(json!({ "coding": { "code": "problem-list-item" } })),
            min: None,
            max: None,
            schema: None,
        },
    );
    assert_eq!(
        validator().classify_slice(&item, &slices),
        SliceClassification::Matched("by-value".to_owned())
    );
}
