//! A profile that constrains an element its type inherits (SQL on FHIR's
//! SQLQuery slices `Library.content.extension`; Attachment inherits
//! `extension` from Element) keeps that element's base cardinality: the
//! overlay is merged over the type's whole inheritance chain.

use octofhir_fhirschema::types::FhirSchema;
use octofhir_fhirschema::validation::FhirValidator;
use serde_json::{Value, json};
use std::collections::HashMap;

const ELEMENT: &str = "http://hl7.org/fhir/StructureDefinition/Element";
const SQL_QUERY: &str = "http://example.org/StructureDefinition/SQLQuery";
const SQL_TEXT: &str = "http://example.org/StructureDefinition/sql-text";

fn parse(v: Value) -> FhirSchema {
    serde_json::from_value(v).expect("valid FhirSchema json")
}

fn schemas() -> HashMap<String, FhirSchema> {
    HashMap::from([
        (
            "Element".to_string(),
            parse(json!({
                "url": ELEMENT, "name": "Element", "type": "Element",
                "kind": "complex-type", "class": "type",
                "elements": {
                    "id": {"type": "string"},
                    "extension": {"type": "Extension", "array": true}
                }
            })),
        ),
        (
            "Extension".to_string(),
            parse(json!({
                "url": "http://hl7.org/fhir/StructureDefinition/Extension",
                "name": "Extension", "type": "Extension",
                "kind": "complex-type", "class": "type", "base": ELEMENT,
                "elements": {
                    "url": {"type": "uri"},
                    "valueString": {"type": "string", "choiceOf": "value"},
                    "value": {"choices": ["valueString"]}
                }
            })),
        ),
        (
            "Attachment".to_string(),
            parse(json!({
                "url": "http://hl7.org/fhir/StructureDefinition/Attachment",
                "name": "Attachment", "type": "Attachment",
                "kind": "complex-type", "class": "type", "base": ELEMENT,
                "elements": {
                    "contentType": {"type": "code"},
                    "data": {"type": "base64Binary"}
                }
            })),
        ),
        (
            "Library".to_string(),
            parse(json!({
                "url": "http://hl7.org/fhir/StructureDefinition/Library",
                "name": "Library", "type": "Library",
                "kind": "resource", "class": "resource",
                "elements": {
                    "id": {"type": "id"},
                    "status": {"type": "code"},
                    "content": {"type": "Attachment", "array": true}
                }
            })),
        ),
        (
            SQL_QUERY.to_string(),
            parse(json!({
                "url": SQL_QUERY, "name": "SQLQuery", "type": "Library",
                "kind": "resource", "class": "profile",
                "derivation": "constraint",
                "base": "http://hl7.org/fhir/StructureDefinition/Library",
                "elements": {
                    "content": {
                        "type": "Attachment",
                        "elements": {
                            "extension": {
                                "slicing": {
                                    "discriminator": [{"type": "value", "path": "url"}],
                                    "rules": "open",
                                    "slices": {
                                        "sqlText": {
                                            "match": {"url": SQL_TEXT},
                                            "max": 1,
                                            "schema": {"type": "Extension"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            })),
        ),
    ])
}

fn library(extension: Value) -> Value {
    json!({
        "resourceType": "Library",
        "id": "q",
        "status": "active",
        "content": [{
            "contentType": "application/sql",
            "extension": extension,
            "data": "U0VMRUNUIDE="
        }]
    })
}

#[tokio::test]
async fn inherited_array_element_stays_an_array_under_a_profile() {
    let validator = FhirValidator::from_schemas(schemas(), None);
    let result = validator
        .validate(
            &library(json!([{"url": SQL_TEXT, "valueString": "SELECT 1"}])),
            vec![SQL_QUERY.into()],
        )
        .await;
    assert!(result.valid, "errors: {:?}", result.errors);
}

#[tokio::test]
async fn inherited_array_element_still_rejects_a_single_object() {
    let validator = FhirValidator::from_schemas(schemas(), None);
    let result = validator
        .validate(
            &library(json!({"url": SQL_TEXT, "valueString": "SELECT 1"})),
            vec![SQL_QUERY.into()],
        )
        .await;
    assert!(!result.valid, "a single object where an array belongs");
}

/// The SQLQuery differential lists the slice `Library.content.extension:sqlText`
/// without a slicing root element: the slice's cardinality and profile stay on
/// the slice, not on `extension` itself.
#[test]
fn a_slice_listed_without_its_root_keeps_its_constraints() {
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition",
        "url": SQL_QUERY, "name": "SQLQuery", "type": "Library",
        "kind": "resource", "abstract": false, "derivation": "constraint", "status": "active",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Library",
        "differential": {"element": [
            {"id": "Library", "path": "Library"},
            {"id": "Library.content", "path": "Library.content", "min": 1},
            {"id": "Library.content.extension:sqlText", "path": "Library.content.extension",
             "sliceName": "sqlText", "min": 0, "max": "1",
             "type": [{"code": "Extension", "profile": [SQL_TEXT]}]},
            {"id": "Library.content.data", "path": "Library.content.data", "min": 1}
        ]}
    }))
    .expect("StructureDefinition");
    let schema = octofhir_fhirschema::translate(sd, None).expect("translate");
    let content = &schema.elements.as_ref().unwrap()["content"];
    let nested = content.elements.as_ref().expect("content elements");
    let extension = &nested["extension"];
    assert_eq!(
        extension.max, None,
        "extension itself is not limited: {extension:?}"
    );
    let slicing = extension.slicing.as_ref().expect("slicing");
    let slice = &slicing.slices.as_ref().expect("slices")["sqlText"];
    assert_eq!(
        serde_json::to_value(slice).unwrap()["schema"]["max"],
        json!(1),
        "{slice:?}"
    );
    assert!(nested.contains_key("data"));
}

/// A named slice of a single-valued element, listed without a slicing root (FHIR R4 core
/// `catalog`: `Composition.date:IssueDate` 1..1), constrains the element itself: a single value
/// is never classified into slices.
#[test]
fn a_slice_of_a_single_valued_element_constrains_the_element() {
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition", "status": "active",
        "url": "http://example.org/StructureDefinition/catalog", "name": "Catalog",
        "type": "Composition", "kind": "resource", "abstract": false,
        "derivation": "constraint",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Composition",
        "differential": {"element": [
            {"id": "Composition", "path": "Composition"},
            {"id": "Composition.date:IssueDate", "path": "Composition.date",
             "sliceName": "IssueDate", "min": 1, "max": "1", "mustSupport": true}
        ]}
    }))
    .expect("StructureDefinition");
    let schema = octofhir_fhirschema::translate(sd, None).expect("translate");
    let date = &schema.elements.as_ref().expect("elements")["date"];
    assert_eq!(date.must_support, Some(true), "{date:?}");
    assert!(
        schema.required.iter().flatten().any(|name| name == "date"),
        "date is required: {:?}",
        schema.required
    );
}

/// A required slice requires the element it slices (a complex extension with a 1..1 part, FHIR
/// R4 core `codesystem-alternate`): with no `extension` at all there is nothing to count against
/// the slice's minimum.
#[test]
fn a_required_slice_requires_the_sliced_element() {
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition", "status": "active",
        "url": "http://example.org/StructureDefinition/alternate", "name": "Alternate",
        "type": "Extension", "kind": "complex-type", "abstract": false,
        "derivation": "constraint",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Extension",
        "differential": {"element": [
            {"id": "Extension", "path": "Extension"},
            {"id": "Extension.extension:code", "path": "Extension.extension",
             "sliceName": "code", "min": 1, "max": "1"},
            {"id": "Extension.extension:use", "path": "Extension.extension",
             "sliceName": "use", "min": 0, "max": "*"}
        ]}
    }))
    .expect("StructureDefinition");
    let schema = octofhir_fhirschema::translate(sd, None).expect("translate");
    assert!(
        schema
            .required
            .iter()
            .flatten()
            .any(|name| name == "extension"),
        "extension is required: {:?}",
        schema.required
    );
    let extension = &schema.elements.as_ref().expect("elements")["extension"];
    assert_eq!(extension.max, None, "{extension:?}");
}

/// A slice of a repeating element listed without its root (`Patient.identifier:mrn` 1..1 in a
/// profile whose base sliced `identifier`) describes some items only: `identifier` stays a
/// repeating element without the slice's cardinality.
#[test]
fn a_slice_of_a_repeating_element_does_not_constrain_the_element() {
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition", "status": "active",
        "url": "http://example.org/StructureDefinition/patient", "name": "P",
        "type": "Patient", "kind": "resource", "abstract": false,
        "derivation": "constraint",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
        "differential": {"element": [
            {"id": "Patient", "path": "Patient"},
            {"id": "Patient.identifier:mrn", "path": "Patient.identifier",
             "sliceName": "mrn", "min": 1, "max": "1", "mustSupport": true}
        ]},
        "snapshot": {"element": [
            {"id": "Patient", "path": "Patient", "min": 0, "max": "*",
             "base": {"path": "Patient", "min": 0, "max": "*"}},
            {"id": "Patient.identifier", "path": "Patient.identifier", "min": 0, "max": "*",
             "base": {"path": "Patient.identifier", "min": 0, "max": "*"},
             "slicing": {"discriminator": [{"type": "pattern", "path": "type"}], "rules": "open"}},
            {"id": "Patient.identifier:mrn", "path": "Patient.identifier", "sliceName": "mrn",
             "min": 1, "max": "1",
             "base": {"path": "Patient.identifier", "min": 0, "max": "*"}}
        ]}
    }))
    .expect("StructureDefinition");
    let schema = octofhir_fhirschema::translate(sd, None).expect("translate");
    let identifier = &schema.elements.as_ref().expect("elements")["identifier"];
    assert_eq!(identifier.max, None, "{identifier:?}");
    assert_eq!(identifier.must_support, None, "{identifier:?}");
    let slices = identifier
        .slicing
        .as_ref()
        .and_then(|slicing| slicing.slices.as_ref())
        .expect("slices");
    assert!(slices.contains_key("mrn"), "{slices:?}");
}

/// The converted SQLQuery profile accepts the sql-text extension next to any
/// other extension: an extension slice without a discriminator is sliced by
/// `url` (implied for extensions), so the other one is not counted against it.
#[tokio::test]
async fn converted_profile_accepts_other_extensions_beside_the_slice() {
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition", "status": "active",
        "url": SQL_QUERY, "name": "SQLQuery", "type": "Library",
        "kind": "resource", "abstract": false, "derivation": "constraint",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Library",
        "differential": {"element": [
            {"id": "Library", "path": "Library"},
            {"id": "Library.content.extension:sqlText", "path": "Library.content.extension",
             "sliceName": "sqlText", "min": 0, "max": "1",
             "type": [{"code": "Extension", "profile": [SQL_TEXT]}]}
        ]}
    }))
    .expect("StructureDefinition");
    let mut all = schemas();
    all.insert(
        SQL_QUERY.to_string(),
        octofhir_fhirschema::translate(sd, None).expect("translate"),
    );
    let validator = FhirValidator::from_schemas(all, None);
    let result = validator
        .validate(
            &library(json!([
                {"url": SQL_TEXT, "valueString": "SELECT 1"},
                {"url": "http://example.org/other", "valueString": "x"}
            ])),
            vec![SQL_QUERY.into()],
        )
        .await;
    assert!(result.valid, "errors: {:?}", result.errors);

    let twice = validator
        .validate(
            &library(json!([
                {"url": SQL_TEXT, "valueString": "SELECT 1"},
                {"url": SQL_TEXT, "valueString": "SELECT 2"}
            ])),
            vec![SQL_QUERY.into()],
        )
        .await;
    assert!(!twice.valid, "sqlText is 0..1");
}

/// A profile that constrains `value[x]` without listing its types (US Core 9 smoking status:
/// `Observation.value[x]` min 1, mustSupport) keeps the constraint.
#[test]
fn a_choice_constrained_without_types_is_kept() {
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition", "status": "active",
        "url": "http://example.org/StructureDefinition/obs", "name": "Obs", "type": "Observation",
        "kind": "resource", "abstract": false, "derivation": "constraint",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Observation",
        "differential": {"element": [
            {"id": "Observation", "path": "Observation"},
            {"id": "Observation.value[x]", "path": "Observation.value[x]", "min": 1, "mustSupport": true}
        ]}
    }))
    .expect("StructureDefinition");
    let schema = octofhir_fhirschema::translate(sd, None).expect("translate");
    let value = schema
        .elements
        .as_ref()
        .and_then(|elements| elements.get("value"))
        .unwrap_or_else(|| panic!("value is kept: {:?}", schema.elements));
    assert_eq!(value.must_support, Some(true));
    assert!(
        value.min == Some(1) || schema.required.iter().flatten().any(|name| name == "value"),
        "value is required: {value:?} / {:?}",
        schema.required
    );
}

/// A profile narrowing a repeating element to `max` 1 (`Patient.name` 0..1) keeps the limit,
/// and the validator enforces it.
#[tokio::test]
async fn a_profile_narrowing_max_to_one_is_enforced() {
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition", "status": "active",
        "url": SQL_QUERY, "name": "OneContent", "type": "Library",
        "kind": "resource", "abstract": false, "derivation": "constraint",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Library",
        "differential": {"element": [
            {"id": "Library", "path": "Library"},
            {"id": "Library.content", "path": "Library.content", "max": "1"}
        ]}
    }))
    .expect("StructureDefinition");
    let schema = octofhir_fhirschema::translate(sd, None).expect("translate");
    assert_eq!(schema.elements.as_ref().unwrap()["content"].max, Some(1));
    let mut all = schemas();
    all.insert(SQL_QUERY.to_string(), schema);
    let validator = FhirValidator::from_schemas(all, None);
    let one = library(json!([{"url": SQL_TEXT, "valueString": "SELECT 1"}]));
    assert!(validator.validate(&one, vec![SQL_QUERY.into()]).await.valid);
    let mut two = one.clone();
    let content = two["content"][0].clone();
    two["content"].as_array_mut().unwrap().push(content);
    let result = validator.validate(&two, vec![SQL_QUERY.into()]).await;
    assert!(!result.valid, "two contents where the profile allows one");
}

/// A profile without a snapshot keeps the invariants its differential root declares.
#[test]
fn root_invariants_of_a_differential_only_profile_are_kept() {
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition", "status": "active",
        "url": SQL_QUERY, "name": "Q", "type": "Library",
        "kind": "resource", "abstract": false, "derivation": "constraint",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Library",
        "differential": {"element": [
            {"id": "Library", "path": "Library", "constraint": [{
                "key": "q-1", "severity": "error", "human": "a SQL content",
                "expression": "content.exists()"}]}
        ]}
    }))
    .expect("StructureDefinition");
    let schema = octofhir_fhirschema::translate(sd, None).expect("translate");
    assert!(
        schema
            .constraint
            .as_ref()
            .is_some_and(|c| c.contains_key("q-1")),
        "{:?}",
        schema.constraint
    );
}

/// A sliced backbone element whose base children and slices both carry constraints (US Core
/// Provenance.agent): the base's children stay on the element, each slice keeps its own.
#[test]
fn slices_keep_their_children_apart_from_the_sliced_element() {
    let pattern = |code: &str| json!({"coding": [{"system": "http://terminology.hl7.org/CodeSystem/provenance-participant-type", "code": code}]});
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition", "status": "active",
        "url": "http://example.org/StructureDefinition/prov", "name": "Prov", "type": "Provenance",
        "kind": "resource", "abstract": false, "derivation": "constraint",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Provenance",
        "differential": {"element": [
            {"id": "Provenance", "path": "Provenance"},
            {"id": "Provenance.agent", "path": "Provenance.agent", "mustSupport": true,
             "slicing": {"discriminator": [{"type": "value", "path": "type"}], "rules": "open"}},
            {"id": "Provenance.agent.type", "path": "Provenance.agent.type", "mustSupport": true},
            {"id": "Provenance.agent.who", "path": "Provenance.agent.who", "mustSupport": true},
            {"id": "Provenance.agent:author", "path": "Provenance.agent", "sliceName": "author"},
            {"id": "Provenance.agent:author.type", "path": "Provenance.agent.type", "min": 1,
             "patternCodeableConcept": pattern("author")},
            {"id": "Provenance.agent:transmitter", "path": "Provenance.agent", "sliceName": "transmitter"},
            {"id": "Provenance.agent:transmitter.type", "path": "Provenance.agent.type", "min": 1,
             "patternCodeableConcept": pattern("transmitter")}
        ]}
    }))
    .expect("StructureDefinition");
    let schema = octofhir_fhirschema::translate(sd, None).expect("translate");
    let agent = &schema.elements.as_ref().unwrap()["agent"];
    let children = agent.elements.as_ref().expect("agent keeps its children");
    assert_eq!(children["type"].must_support, Some(true));
    assert_eq!(children["who"].must_support, Some(true));
    let slices = agent.slicing.as_ref().unwrap().slices.as_ref().unwrap();
    for (name, code) in [("author", "author"), ("transmitter", "transmitter")] {
        let slice = serde_json::to_value(&slices[name]).unwrap();
        assert_eq!(
            slice["schema"]["elements"]["type"]["pattern"]["value"],
            pattern(code),
            "{name}: {slice}"
        );
        assert!(
            slice["schema"]["elements"].get("who").is_none(),
            "{name}: {slice}"
        );
        assert_eq!(
            slice["match"]["type"],
            pattern(code),
            "{name} is matched by its type"
        );
    }
}
