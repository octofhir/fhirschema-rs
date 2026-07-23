//! Tests for `targetProfile` conformance validation (Phase 4b).
//!
//! A reference that declares one or more `targetProfile`s is dereferenced via
//! the reference resolver and the referenced resource is validated against those
//! profiles with OR-semantics. These tests use hand-built schemas and an
//! in-memory resolver so no network or storage is involved.

use async_trait::async_trait;
use octofhir_fhirschema::reference::{
    ReferenceResolutionResult, ReferenceResolver, ReferenceResult,
};
use octofhir_fhirschema::types::FhirSchema;
use octofhir_fhirschema::validation::FhirValidator;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

const PATIENT_URL: &str = "http://hl7.org/fhir/StructureDefinition/Patient";
const OBSERVATION_URL: &str = "http://hl7.org/fhir/StructureDefinition/Observation";
const SPECIAL_PATIENT: &str = "http://example.org/SpecialPatient";
const OTHER_PATIENT: &str = "http://example.org/OtherPatient";

/// In-memory resolver: `fetch_resource` returns a stored body by exact
/// reference string, `None` otherwise (unresolvable).
struct MapResolver {
    bodies: HashMap<String, Arc<Value>>,
}

impl MapResolver {
    fn new(bodies: Vec<(&str, Value)>) -> Arc<Self> {
        Arc::new(Self {
            bodies: bodies
                .into_iter()
                .map(|(k, v)| (k.to_string(), Arc::new(v)))
                .collect(),
        })
    }
}

#[async_trait]
impl ReferenceResolver for MapResolver {
    async fn resource_exists(&self, _rt: &str, _id: &str) -> ReferenceResult<bool> {
        Ok(true)
    }

    async fn resolve_reference(
        &self,
        _reference: &str,
    ) -> ReferenceResult<ReferenceResolutionResult> {
        // Existence (Phase 4) is orthogonal to targetProfile conformance
        // (Phase 4b); treat every reference as existing so these tests isolate
        // the conformance behavior driven by `fetch_resource`.
        Ok(ReferenceResolutionResult::skipped())
    }

    async fn fetch_resource(&self, reference: &str) -> ReferenceResult<Option<Arc<Value>>> {
        Ok(self.bodies.get(reference).cloned())
    }
}

fn parse(v: Value) -> FhirSchema {
    serde_json::from_value(v).expect("valid FhirSchema json")
}

/// Build the schema set. `subject_targets` are the `targetProfile`s declared on
/// `Observation.subject`.
fn schemas(subject_targets: &[&str]) -> HashMap<String, FhirSchema> {
    let mut m = HashMap::new();

    m.insert(
        "Patient".to_string(),
        parse(json!({
            "url": PATIENT_URL, "name": "Patient", "type": "Patient",
            "kind": "resource", "class": "resource",
            "elements": {
                "id": {"type": "id"},
                "active": {"type": "boolean"},
                "birthDate": {"type": "date"},
                "link": {"type": "Reference", "array": true, "refers": [SPECIAL_PATIENT]}
            }
        })),
    );

    m.insert(
        "Observation".to_string(),
        parse(json!({
            "url": OBSERVATION_URL, "name": "Observation", "type": "Observation",
            "kind": "resource", "class": "resource",
            "elements": {
                "id": {"type": "id"},
                "status": {"type": "code"},
                "subject": {"type": "Reference", "refers": subject_targets}
            }
        })),
    );

    // SpecialPatient: a Patient that must carry `active`.
    m.insert(
        SPECIAL_PATIENT.to_string(),
        parse(json!({
            "url": SPECIAL_PATIENT, "name": "SpecialPatient", "type": "Patient",
            "kind": "resource", "class": "profile",
            "derivation": "constraint", "base": PATIENT_URL,
            "required": ["active"]
        })),
    );

    // OtherPatient: a Patient that must carry `birthDate`.
    m.insert(
        OTHER_PATIENT.to_string(),
        parse(json!({
            "url": OTHER_PATIENT, "name": "OtherPatient", "type": "Patient",
            "kind": "resource", "class": "profile",
            "derivation": "constraint", "base": PATIENT_URL,
            "required": ["birthDate"]
        })),
    );

    m
}

fn observation(subject_ref: &str) -> Value {
    json!({
        "resourceType": "Observation",
        "id": "obs1",
        "status": "final",
        "subject": {"reference": subject_ref}
    })
}

fn validator(subject_targets: &[&str], resolver: Arc<MapResolver>) -> FhirValidator {
    FhirValidator::from_schemas(schemas(subject_targets), None)
        .with_reference_resolver(resolver)
        .with_target_profile_validation(true)
}

const MISMATCH: &str = "FS1017";

#[tokio::test]
async fn conforming_reference_passes() {
    let resolver = MapResolver::new(vec![(
        "Patient/1",
        json!({"resourceType": "Patient", "id": "1", "active": true}),
    )]);
    let v = validator(&[SPECIAL_PATIENT], resolver);
    let result = v
        .validate(&observation("Patient/1"), vec!["Observation".into()])
        .await;
    assert!(result.valid, "expected valid, errors: {:?}", result.errors);
}

#[tokio::test]
async fn nonconforming_reference_errors() {
    // Patient without `active` violates SpecialPatient.
    let resolver = MapResolver::new(vec![(
        "Patient/1",
        json!({"resourceType": "Patient", "id": "1"}),
    )]);
    let v = validator(&[SPECIAL_PATIENT], resolver);
    let result = v
        .validate(&observation("Patient/1"), vec!["Observation".into()])
        .await;
    assert!(!result.valid, "expected mismatch error");
    let mismatch = result.errors.iter().find(|e| e.error_type == MISMATCH);
    assert!(
        mismatch.is_some(),
        "expected {MISMATCH}, got {:?}",
        result.errors
    );
    let path = mismatch.unwrap().path.last().and_then(|v| v.as_str());
    assert_eq!(path, Some("reference"));
}

#[tokio::test]
async fn or_semantics_passes_on_second_target() {
    // Body satisfies OtherPatient (has birthDate) but not SpecialPatient (no active).
    let resolver = MapResolver::new(vec![(
        "Patient/1",
        json!({"resourceType": "Patient", "id": "1", "birthDate": "1980-01-01"}),
    )]);
    let v = validator(&[SPECIAL_PATIENT, OTHER_PATIENT], resolver);
    let result = v
        .validate(&observation("Patient/1"), vec!["Observation".into()])
        .await;
    assert!(
        result.valid,
        "OR-semantics: should pass via OtherPatient, errors: {:?}",
        result.errors
    );
}

#[tokio::test]
async fn unresolvable_reference_warns_not_errors() {
    // No body registered → fetch returns None → warning, still valid.
    let resolver = MapResolver::new(vec![]);
    let v = validator(&[SPECIAL_PATIENT], resolver);
    let result = v
        .validate(&observation("Patient/99"), vec!["Observation".into()])
        .await;
    assert!(
        result.valid,
        "unresolvable ref must not fail validity, errors: {:?}",
        result.errors
    );
    assert!(
        result.warnings.iter().any(|w| w.error_type == MISMATCH),
        "expected an unresolvable warning, warnings: {:?}",
        result.warnings
    );
}

#[tokio::test]
async fn reference_cycle_terminates() {
    // Patient/1.link -> Patient/2.link -> Patient/1. Both conform. The visited
    // cycle-guard must stop the recursion rather than loop forever.
    let resolver = MapResolver::new(vec![
        (
            "Patient/1",
            json!({"resourceType": "Patient", "id": "1", "active": true,
                   "link": [{"reference": "Patient/2"}]}),
        ),
        (
            "Patient/2",
            json!({"resourceType": "Patient", "id": "2", "active": true,
                   "link": [{"reference": "Patient/1"}]}),
        ),
    ]);
    let v = validator(&[SPECIAL_PATIENT], resolver);
    let result = v
        .validate(&observation("Patient/1"), vec!["Observation".into()])
        .await;
    assert!(
        result.valid,
        "cycle should validate cleanly, errors: {:?}",
        result.errors
    );
}

#[tokio::test]
async fn disabled_by_default_no_fetch() {
    // Without with_target_profile_validation, a non-conforming target is ignored.
    let resolver = MapResolver::new(vec![(
        "Patient/1",
        json!({"resourceType": "Patient", "id": "1"}),
    )]);
    let v = FhirValidator::from_schemas(schemas(&[SPECIAL_PATIENT]), None)
        .with_reference_resolver(resolver);
    let result = v
        .validate(&observation("Patient/1"), vec!["Observation".into()])
        .await;
    assert!(
        result.valid,
        "targetProfile off: must not check conformance"
    );
    assert!(result.errors.iter().all(|e| e.error_type != MISMATCH));
}
