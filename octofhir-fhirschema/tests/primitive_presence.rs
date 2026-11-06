//! A primitive with only its `_name` sibling (extensions, no value) is present: FHIR JSON,
//! "Representations of primitive values".

use std::collections::HashMap;

use octofhir_fhirschema::embedded::{FhirVersion, get_schemas};
use octofhir_fhirschema::validation::FhirValidator;
use serde_json::json;

#[tokio::test]
async fn a_required_choice_with_only_extensions_is_present() {
    let schemas: HashMap<_, _> = get_schemas(FhirVersion::R4)
        .iter()
        .map(|(name, schema)| (name.clone(), schema.clone()))
        .collect();
    let validator = FhirValidator::from_schemas(schemas, None);
    let immunization = json!({
        "resourceType": "Immunization",
        "status": "completed",
        "vaccineCode": { "coding": [{ "system": "http://hl7.org/fhir/sid/cvx", "code": "140" }] },
        "patient": { "reference": "Patient/1" },
        "_occurrenceDateTime": { "extension": [{
            "url": "http://hl7.org/fhir/StructureDefinition/data-absent-reason",
            "valueCode": "unknown"
        }] }
    });
    let result = validator
        .validate(&immunization, vec!["Immunization".to_owned()])
        .await;
    let missing: Vec<_> = result
        .errors
        .iter()
        .filter(|e| {
            e.message
                .as_deref()
                .is_some_and(|m| m.contains("occurrence"))
        })
        .collect();
    assert!(missing.is_empty(), "{missing:#?}");
    // Without it, occurrence is missing.
    let mut without = immunization.clone();
    without
        .as_object_mut()
        .unwrap()
        .remove("_occurrenceDateTime");
    let result = validator
        .validate(&without, vec!["Immunization".to_owned()])
        .await;
    assert!(
        result
            .errors
            .iter()
            .any(|e| e.message.as_deref() == Some("Required element 'occurrence' is missing"))
    );
}

#[tokio::test]
async fn a_reference_with_only_an_extension_is_present() {
    let schemas: HashMap<_, _> = get_schemas(FhirVersion::R4)
        .iter()
        .map(|(name, schema)| (name.clone(), schema.clone()))
        .collect();
    let validator = FhirValidator::from_schemas(schemas, None);
    let observation = |performer: serde_json::Value| {
        json!({
            "resourceType": "Observation",
            "status": "final",
            "code": { "text": "x" },
            "performer": [performer]
        })
    };
    let reference_errors = |result: &octofhir_fhirschema::types::ValidationResult| {
        result
            .errors
            .iter()
            .filter(|e| {
                e.message
                    .as_deref()
                    .is_some_and(|m| m.contains("Reference must have"))
            })
            .count()
    };
    let absent = observation(json!({ "extension": [{
        "url": "http://hl7.org/fhir/StructureDefinition/data-absent-reason",
        "valueCode": "unknown"
    }] }));
    let result = validator
        .validate(&absent, vec!["Observation".to_owned()])
        .await;
    assert_eq!(reference_errors(&result), 0, "{:#?}", result.errors);
    // With nothing at all, it is still an empty reference.
    let result = validator
        .validate(&observation(json!({})), vec!["Observation".to_owned()])
        .await;
    assert_eq!(reference_errors(&result), 1, "{:#?}", result.errors);
}
