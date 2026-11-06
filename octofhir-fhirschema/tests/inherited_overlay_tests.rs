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

/// Type slices of a choice (US Core 9 Smoking Status: `value[x]` 1..1 with
/// `value[x]:valueQuantity` and `value[x]:valueCodeableConcept`) constrain their typed elements
/// and do not narrow the choice to one type.
#[test]
fn type_slices_of_a_choice_constrain_the_typed_elements() {
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition", "status": "active",
        "url": "http://example.org/StructureDefinition/smoking", "name": "Smoking",
        "type": "Observation", "kind": "resource", "abstract": false,
        "derivation": "constraint",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Observation",
        "differential": {"element": [
            {"id": "Observation", "path": "Observation"},
            {"id": "Observation.value[x]", "path": "Observation.value[x]", "min": 1},
            {"id": "Observation.value[x]:valueQuantity", "path": "Observation.value[x]",
             "sliceName": "valueQuantity", "min": 0, "max": "1", "mustSupport": true,
             "type": [{"code": "Quantity"}]},
            {"id": "Observation.value[x]:valueCodeableConcept", "path": "Observation.value[x]",
             "sliceName": "valueCodeableConcept", "min": 0, "max": "1",
             "type": [{"code": "CodeableConcept"}]}
        ]}
    }))
    .expect("StructureDefinition");
    let schema = octofhir_fhirschema::translate(sd, None).expect("translate");
    let elements = schema.elements.as_ref().expect("elements");
    let quantity = &elements["valueQuantity"];
    assert_eq!(
        quantity.type_name.as_deref(),
        Some("Quantity"),
        "{quantity:?}"
    );
    assert_eq!(quantity.must_support, Some(true), "{quantity:?}");
    let value = &elements["value"];
    assert!(
        value
            .choices
            .as_ref()
            .is_none_or(|choices| choices.len() != 1),
        "the choice is not narrowed to one type: {value:?}"
    );
}

/// A choice narrowed by its root and then type-sliced (FHIR R6 `bp`: `value[x]` Quantity, then
/// `value[x]:valueQuantity`) keeps the root's narrowing: the element entered again is merged,
/// not replaced.
#[test]
fn a_type_slice_keeps_the_choice_its_root_narrowed() {
    let sd: octofhir_fhirschema::StructureDefinition = serde_json::from_value(json!({
        "resourceType": "StructureDefinition", "status": "active",
        "url": "http://example.org/StructureDefinition/bp", "name": "BP",
        "type": "Observation", "kind": "resource", "abstract": false,
        "derivation": "constraint",
        "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Observation",
        "differential": {"element": [
            {"id": "Observation", "path": "Observation"},
            {"id": "Observation.value[x]", "path": "Observation.value[x]", "mustSupport": true,
             "type": [{"code": "Quantity"}]},
            {"id": "Observation.value[x]:valueQuantity", "path": "Observation.value[x]",
             "sliceName": "valueQuantity", "short": "In mm[Hg]",
             "type": [{"code": "Quantity"}]}
        ]}
    }))
    .expect("StructureDefinition");
    let schema = octofhir_fhirschema::translate(sd, None).expect("translate");
    let elements = schema.elements.as_ref().expect("elements");
    let value = &elements["value"];
    assert_eq!(
        value.choices.as_deref(),
        Some(&["valueQuantity".to_string()][..]),
        "{value:?}"
    );
    assert_eq!(value.must_support, Some(true), "{value:?}");
    let quantity = &elements["valueQuantity"];
    assert_eq!(quantity.short.as_deref(), Some("In mm[Hg]"), "{quantity:?}");
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

const TRACE: &str = "http://example.org/StructureDefinition/Trace";

fn overlays() -> HashMap<String, FhirSchema> {
    let mut schemas = schemas();
    schemas.insert(
        "Reference".to_string(),
        parse(json!({
            "url": "http://hl7.org/fhir/StructureDefinition/Reference",
            "name": "Reference", "type": "Reference",
            "kind": "complex-type", "class": "type", "base": ELEMENT,
            "elements": {
                "reference": {"type": "string"},
                "display": {"type": "string"}
            }
        })),
    );
    schemas.insert(
        "Trace".to_string(),
        parse(json!({
            "url": "http://example.org/StructureDefinition/TraceBase",
            "name": "Trace", "type": "Trace", "kind": "resource", "class": "resource",
            "elements": {
                "target": {"type": "Reference", "array": true},
                "form": {"type": "canonical"}
            }
        })),
    );
    schemas.insert(
        TRACE.to_string(),
        parse(json!({
            "url": TRACE, "name": "TraceProfile", "type": "Trace",
            "kind": "resource", "class": "profile", "derivation": "constraint",
            "base": "http://example.org/StructureDefinition/TraceBase",
            "elements": {
                "target": {"elements": {"reference": {"short": "What it points at"}}},
                "form": {"elements": {"extension": {"type": "Extension", "array": true}}}
            }
        })),
    );
    schemas
}

/// US Core Provenance says something of `target.reference`, and of nothing else in it.
#[tokio::test]
async fn an_overlay_on_a_reference_keeps_its_other_elements() {
    let validator = FhirValidator::from_schemas(overlays(), None);
    let resource = json!({"resourceType": "Trace", "target": [{"display": "A patient"}]});
    let result = validator
        .validate(&resource, vec!["Trace".to_string(), TRACE.to_string()])
        .await;
    assert!(result.valid, "{:?}", result.errors);
}

/// US Core QuestionnaireResponse slices `questionnaire.extension`; the element stays a string.
#[tokio::test]
async fn an_overlay_on_a_primitive_leaves_it_a_primitive() {
    let validator = FhirValidator::from_schemas(overlays(), None);
    let resource = json!({"resourceType": "Trace", "form": "http://example.org/form"});
    let result = validator
        .validate(&resource, vec!["Trace".to_string(), TRACE.to_string()])
        .await;
    assert!(result.valid, "{:?}", result.errors);
}

/// US Core Patient requires `identifier.system`, in the element and not at the root.
#[tokio::test]
async fn a_required_child_of_an_element_is_enforced() {
    let mut schemas = overlays();
    schemas.insert(
        "Trace".to_string(),
        parse(json!({
            "url": "http://example.org/StructureDefinition/TraceBase",
            "name": "Trace", "type": "Trace", "kind": "resource", "class": "resource",
            "elements": {
                "step": {
                    "array": true, "required": ["name"],
                    "elements": {"name": {"type": "string"}, "note": {"type": "string"}}
                }
            }
        })),
    );
    let validator = FhirValidator::from_schemas(schemas, None);
    let check = async |step: Value| {
        let resource = json!({"resourceType": "Trace", "step": [step]});
        validator
            .validate(&resource, vec!["Trace".to_string()])
            .await
    };
    assert!(check(json!({"name": "a"})).await.valid);
    let missing = check(json!({"note": "b"})).await;
    assert!(!missing.valid);
    assert_eq!(
        missing.errors[0].message.as_deref(),
        Some("Required element 'name' is missing")
    );
}

#[tokio::test]
async fn a_reference_holds_nothing_else() {
    let validator = FhirValidator::from_schemas(overlays(), None);
    let check = async |target: Value| {
        let resource = json!({"resourceType": "Trace", "target": [target]});
        validator
            .validate(&resource, vec!["Trace".to_string()])
            .await
    };
    assert!(
        check(json!({"reference": "Patient/1", "_display": {"id": "d"}}))
            .await
            .valid
    );
    let unknown = check(json!({"reference": "Patient/1", "noSuchElement": true})).await;
    assert!(!unknown.valid);
    assert_eq!(
        unknown.errors[0].message.as_deref(),
        Some("Unknown element 'noSuchElement'")
    );
}

/// US Core's blood pressure says of `component:systolic` that its `valueQuantity` has a unit.
#[tokio::test]
async fn an_item_is_held_to_what_its_slice_says() {
    let mut schemas = overlays();
    schemas.insert(
        "Trace".to_string(),
        parse(json!({
            "url": "http://example.org/StructureDefinition/TraceBase",
            "name": "Trace", "type": "Trace", "kind": "resource", "class": "resource",
            "elements": {
                "step": {
                    "array": true,
                    "elements": {"kind": {"type": "string"}, "note": {"type": "string"}},
                    "slicing": {
                        "discriminator": [{"type": "value", "path": "kind"}],
                        "rules": "open",
                        "slices": {
                            "first": {
                                "match": {"kind": "first"},
                                "schema": {"required": ["note"]}
                            }
                        }
                    }
                }
            }
        })),
    );
    let validator = FhirValidator::from_schemas(schemas, None);
    let check = async |step: Value| {
        let resource = json!({"resourceType": "Trace", "step": [step]});
        validator
            .validate(&resource, vec!["Trace".to_string()])
            .await
    };
    assert!(check(json!({"kind": "first", "note": "a"})).await.valid);
    assert!(check(json!({"kind": "second"})).await.valid);
    let missing = check(json!({"kind": "first"})).await;
    assert_eq!(
        missing.errors[0].message.as_deref(),
        Some("Required element 'note' is missing"),
        "{:?}",
        missing.errors
    );
}

#[tokio::test]
async fn a_choice_holds_one_type() {
    let mut schemas = overlays();
    schemas.insert(
        "Trace".to_string(),
        parse(json!({
            "url": "http://example.org/StructureDefinition/TraceBase",
            "name": "Trace", "type": "Trace", "kind": "resource", "class": "resource",
            "elements": {
                "value": {"choices": ["valueString", "valueBoolean"]},
                "valueString": {"type": "string", "choiceOf": "value"},
                "valueBoolean": {"type": "boolean", "choiceOf": "value"}
            }
        })),
    );
    let validator = FhirValidator::from_schemas(schemas, None);
    let check = async |value: Value| validator.validate(&value, vec!["Trace".to_string()]).await;
    assert!(
        check(json!({"resourceType": "Trace", "valueString": "a"}))
            .await
            .valid
    );
    let both =
        check(json!({"resourceType": "Trace", "valueString": "a", "valueBoolean": true})).await;
    assert!(!both.valid);
    assert!(
        both.errors[0]
            .message
            .as_deref()
            .is_some_and(|message| message.starts_with("Choice 'value' holds more than one type"))
    );
}

/// IPS fixes `DiagnosticReport.status`; a value that differs is an error, not only a slice miss.
#[tokio::test]
async fn fixed_and_pattern_values_hold_outside_slices_too() {
    let mut schemas = overlays();
    schemas.insert(
        "Trace".to_string(),
        parse(json!({
            "url": "http://example.org/StructureDefinition/TraceBase",
            "name": "Trace", "type": "Trace", "kind": "resource", "class": "resource",
            "elements": {
                "status": {"type": "code", "pattern": {"type": "code", "value": "final"}},
                "code": {
                    "pattern": {"type": "CodeableConcept", "value": {"coding": [{"code": "x"}]}},
                    "elements": {
                        "coding": {"array": true, "elements": {"code": {"type": "code"}}}
                    }
                }
            }
        })),
    );
    let validator = FhirValidator::from_schemas(schemas, None);
    let check = async |value: Value| validator.validate(&value, vec!["Trace".to_string()]).await;
    let good = json!({"resourceType": "Trace", "status": "final",
        "code": {"coding": [{"code": "y"}, {"code": "x"}]}});
    assert!(check(good).await.valid);
    assert!(
        !check(json!({"resourceType": "Trace", "status": "other"}))
            .await
            .valid
    );
    let other = json!({"resourceType": "Trace", "code": {"coding": [{"code": "y"}]}});
    assert!(!check(other).await.valid);
}
