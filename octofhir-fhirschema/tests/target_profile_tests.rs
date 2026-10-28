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
use std::sync::atomic::{AtomicUsize, Ordering};

const PATIENT_URL: &str = "http://hl7.org/fhir/StructureDefinition/Patient";
const OBSERVATION_URL: &str = "http://hl7.org/fhir/StructureDefinition/Observation";
const SPECIAL_PATIENT: &str = "http://example.org/SpecialPatient";
const OTHER_PATIENT: &str = "http://example.org/OtherPatient";

/// In-memory resolver: `fetch_resource` returns a stored body by exact
/// reference string, `None` otherwise (unresolvable).
struct MapResolver {
    bodies: HashMap<String, Arc<Value>>,
    /// Count of `fetch_resource` calls, to assert de-duplication.
    fetches: AtomicUsize,
}

impl MapResolver {
    fn new(bodies: Vec<(&str, Value)>) -> Arc<Self> {
        Arc::new(Self {
            bodies: bodies
                .into_iter()
                .map(|(k, v)| (k.to_string(), Arc::new(v)))
                .collect(),
            fetches: AtomicUsize::new(0),
        })
    }

    fn fetches(&self) -> usize {
        self.fetches.load(Ordering::Relaxed)
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
        self.fetches.fetch_add(1, Ordering::Relaxed);
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

#[tokio::test]
async fn duplicate_reference_dereferenced_once() {
    // The same target referenced from several sites (here two `Patient.link`
    // array entries) must be dereferenced and re-validated a single time, not
    // once per site. Guards the batch/dedup/memoize path: per-site fetching
    // dominated write latency for reference-heavy resources like EoB.
    let resolver = MapResolver::new(vec![(
        "Patient/1",
        json!({"resourceType": "Patient", "id": "1", "active": true}),
    )]);
    let counter = resolver.clone();
    // `link` declares targetProfile SPECIAL_PATIENT (requires `active`).
    let v = FhirValidator::from_schemas(schemas(&[SPECIAL_PATIENT]), None)
        .with_reference_resolver(resolver)
        .with_target_profile_validation(true);
    let patient = json!({
        "resourceType": "Patient",
        "id": "src",
        "link": [
            {"reference": "Patient/1"},
            {"reference": "Patient/1"}
        ]
    });
    let result = v.validate(&patient, vec!["Patient".into()]).await;
    assert!(result.valid, "expected valid, errors: {:?}", result.errors);
    assert_eq!(
        counter.fetches(),
        1,
        "same reference must be dereferenced once, not per site"
    );
}

/// Resolver that answers only from declared `meta.profile` and treats any body
/// fetch as a failure, so a test using it proves the fast path was taken.
struct DeclaredOnlyResolver {
    declared: HashMap<String, Vec<String>>,
    fetches: AtomicUsize,
}

impl DeclaredOnlyResolver {
    fn new(declared: Vec<(&str, Vec<&str>)>) -> Arc<Self> {
        Arc::new(Self {
            declared: declared
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.into_iter().map(String::from).collect()))
                .collect(),
            fetches: AtomicUsize::new(0),
        })
    }

    fn fetches(&self) -> usize {
        self.fetches.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl ReferenceResolver for DeclaredOnlyResolver {
    async fn resource_exists(&self, _rt: &str, _id: &str) -> ReferenceResult<bool> {
        Ok(true)
    }

    async fn resolve_reference(
        &self,
        _reference: &str,
    ) -> ReferenceResult<ReferenceResolutionResult> {
        Ok(ReferenceResolutionResult::skipped())
    }

    async fn fetch_resource(&self, _reference: &str) -> ReferenceResult<Option<Arc<Value>>> {
        self.fetches.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    async fn fetch_profiles_batch(&self, references: &[String]) -> HashMap<String, Vec<String>> {
        references
            .iter()
            .filter_map(|r| self.declared.get(r).map(|p| (r.clone(), p.clone())))
            .collect()
    }
}

fn declared_validator(
    subject_targets: &[&str],
    resolver: Arc<DeclaredOnlyResolver>,
) -> FhirValidator {
    FhirValidator::from_schemas(schemas(subject_targets), None)
        .with_reference_resolver(resolver)
        .with_target_profile_validation(true)
}

#[tokio::test]
async fn declared_profile_settles_without_fetching_body() {
    // The target declares SpecialPatient. That claim was verified when the
    // resource was written, so conformance follows without dereferencing it —
    // note the body would NOT satisfy SpecialPatient if it were fetched, since
    // this resolver returns nothing.
    let resolver = DeclaredOnlyResolver::new(vec![("Patient/1", vec![SPECIAL_PATIENT])]);
    let v = declared_validator(&[SPECIAL_PATIENT], resolver.clone());
    let result = v
        .validate(&observation("Patient/1"), vec!["Observation".into()])
        .await;
    assert!(result.valid, "expected valid, errors: {:?}", result.errors);
    assert_eq!(
        resolver.fetches(),
        0,
        "declared profile must settle without a body fetch"
    );
}

#[tokio::test]
async fn declared_profile_matches_across_version_suffix() {
    // meta.profile pins a version, targetProfile does not: still a match.
    let resolver = DeclaredOnlyResolver::new(vec![(
        "Patient/1",
        vec![&format!("{SPECIAL_PATIENT}|1.2.0")],
    )]);
    let v = declared_validator(&[SPECIAL_PATIENT], resolver.clone());
    let result = v
        .validate(&observation("Patient/1"), vec!["Observation".into()])
        .await;
    assert!(result.valid, "expected valid, errors: {:?}", result.errors);
    assert_eq!(resolver.fetches(), 0);
}

#[tokio::test]
async fn unrelated_declared_profile_falls_back_to_fetch() {
    // Declaring some *other* profile settles nothing, so the body is still
    // dereferenced — the fast path only ever removes work it can answer.
    let resolver = DeclaredOnlyResolver::new(vec![("Patient/1", vec![OTHER_PATIENT])]);
    let v = declared_validator(&[SPECIAL_PATIENT], resolver.clone());
    let _ = v
        .validate(&observation("Patient/1"), vec!["Observation".into()])
        .await;
    assert_eq!(
        resolver.fetches(),
        1,
        "unsettled reference must still be fetched"
    );
}

const RESOURCE_URL: &str = "http://hl7.org/fhir/StructureDefinition/Resource";
const DOMAIN_RESOURCE_URL: &str = "http://hl7.org/fhir/StructureDefinition/DomainResource";

fn abstract_base_validator(target: &str, resolver: Arc<MapResolver>) -> FhirValidator {
    let mut schemas = schemas(&[target]);
    for (name, url) in [
        ("Resource", RESOURCE_URL),
        ("DomainResource", DOMAIN_RESOURCE_URL),
    ] {
        schemas.insert(
            name.to_string(),
            parse(json!({
                "url": url, "name": name, "type": name,
                "kind": "resource", "class": "resource",
                "elements": {"id": {"type": "id"}}
            })),
        );
    }
    FhirValidator::from_schemas(schemas, None)
        .with_reference_resolver(resolver)
        .with_target_profile_validation(true)
}

#[tokio::test]
async fn abstract_resource_target_covers_any_type() {
    let resolver = MapResolver::new(vec![(
        "Patient/1",
        json!({"resourceType": "Patient", "id": "1", "active": true}),
    )]);
    let v = abstract_base_validator(RESOURCE_URL, resolver.clone());
    let result = v
        .validate(&observation("Patient/1"), vec!["Observation".into()])
        .await;
    assert!(result.valid, "expected valid, errors: {:?}", result.errors);
    assert_eq!(resolver.fetches(), 0, "an abstract base settles by type");
}

#[tokio::test]
async fn domain_resource_target_excludes_bundle() {
    let resolver = MapResolver::new(vec![(
        "Bundle/1",
        json!({"resourceType": "Bundle", "id": "1"}),
    )]);
    let v = abstract_base_validator(DOMAIN_RESOURCE_URL, resolver);
    let result = v
        .validate(&observation("Bundle/1"), vec!["Observation".into()])
        .await;
    assert!(
        result.errors.iter().any(|e| e.error_type == MISMATCH),
        "expected {MISMATCH}, got {:?}",
        result.errors
    );
}
