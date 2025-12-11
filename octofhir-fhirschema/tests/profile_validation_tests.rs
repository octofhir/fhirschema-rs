//! Profile validation tests for FHIR Schema validator
//!
//! Tests for Phase 3: Profile Validation Enhancements
//! - Profile chain resolution
//! - Deep schema merging
//! - Multiple profile validation
//! - Conflict detection

use octofhir_fhirschema::embedded::{FhirVersion, get_schemas};
use octofhir_fhirschema::types::{
    FhirSchema, FhirSchemaElement, FhirSchemaPattern, FhirSchemaSliceMatch, FhirSchemaSlicing,
};
use octofhir_fhirschema::validation::FhirSchemaValidator;
use serde_json::json;
use std::collections::HashMap;

/// Helper to create a minimal FhirSchema
/// For profiles, type_name should be the base resource type (e.g., "Patient")
fn create_schema(
    url: &str,
    name: &str,
    type_name: &str, // Separate from name - for profiles this is the resource type
    kind: &str,
    base: Option<&str>,
    elements: Option<HashMap<String, FhirSchemaElement>>,
) -> FhirSchema {
    FhirSchema {
        url: url.to_string(),
        version: Some("1.0.0".to_string()),
        name: name.to_string(),
        type_name: type_name.to_string(),
        kind: kind.to_string(),
        derivation: if base.is_some() {
            Some("constraint".to_string())
        } else {
            None
        },
        base: base.map(|s| s.to_string()),
        abstract_type: None,
        class: if base.is_some() {
            "profile".to_string()
        } else {
            "resource".to_string()
        },
        description: None,
        package_name: None,
        package_version: None,
        package_id: None,
        package_meta: None,
        elements,
        required: None,
        excluded: None,
        extensions: None,
        constraint: None,
        primitive_type: None,
        choices: None,
    }
}

/// Helper to create a minimal FhirSchemaElement
fn create_element(
    type_name: Option<&str>,
    min: Option<i32>,
    max: Option<i32>,
) -> FhirSchemaElement {
    // In FHIR:
    // - max: None means unbounded (*) which is an array
    // - max > 1 is also an array
    // - max == 1 is single value (not an array)
    let is_array = match max {
        None => Some(true), // unbounded = array
        Some(m) if m > 1 => Some(true),
        Some(1) => None, // single value, not array
        _ => None,
    };
    FhirSchemaElement {
        type_name: type_name.map(|s| s.to_string()),
        min,
        max,
        array: is_array,
        ..Default::default()
    }
}

// =============================================================================
// Test 1: Single Profile Validation
// =============================================================================

#[tokio::test]
async fn test_single_profile_validation() {
    // Create base Patient schema
    let mut base_elements = HashMap::new();
    base_elements.insert(
        "id".to_string(),
        create_element(Some("string"), Some(0), Some(1)),
    );
    base_elements.insert(
        "name".to_string(),
        create_element(Some("HumanName"), Some(0), None),
    );
    base_elements.insert(
        "gender".to_string(),
        create_element(Some("code"), Some(0), Some(1)),
    );

    let patient_schema = create_schema(
        "http://hl7.org/fhir/StructureDefinition/Patient",
        "Patient",
        "Patient", // type_name = name for base resources
        "resource",
        None,
        Some(base_elements),
    );

    // Create profile that requires gender
    let mut profile_elements = HashMap::new();
    profile_elements.insert(
        "gender".to_string(),
        create_element(Some("code"), Some(1), Some(1)), // min=1 makes it required
    );

    let mut profile = create_schema(
        "http://example.org/fhir/StructureDefinition/RequiredGenderPatient",
        "RequiredGenderPatient",
        "Patient", // type_name = base resource type for profiles
        "resource",
        Some("http://hl7.org/fhir/StructureDefinition/Patient"),
        Some(profile_elements),
    );
    profile.required = Some(vec!["gender".to_string()]);

    // Build validator
    let mut schemas = HashMap::new();
    schemas.insert("Patient".to_string(), patient_schema);
    schemas.insert("RequiredGenderPatient".to_string(), profile);

    let validator = FhirSchemaValidator::new(schemas, None);

    // Test: Resource with gender should pass
    let valid_patient = json!({
        "resourceType": "Patient",
        "gender": "male"
    });

    let result = validator
        .validate_with_profiles(&valid_patient, vec!["RequiredGenderPatient".to_string()])
        .await;

    assert!(
        result.valid,
        "Patient with gender should validate against profile: {:?}",
        result.errors
    );

    // Test: Resource without gender should fail
    let invalid_patient = json!({
        "resourceType": "Patient",
        "id": "test"
    });

    let result = validator
        .validate_with_profiles(&invalid_patient, vec!["RequiredGenderPatient".to_string()])
        .await;

    assert!(
        !result.valid,
        "Patient without gender should fail validation against profile"
    );
}

// =============================================================================
// Test 2: Profile Chain Resolution
// =============================================================================

#[tokio::test]
async fn test_profile_chain_resolution() {
    // Create base Patient
    let mut base_elements = HashMap::new();
    base_elements.insert(
        "id".to_string(),
        create_element(Some("string"), Some(0), Some(1)),
    );
    base_elements.insert(
        "name".to_string(),
        create_element(Some("HumanName"), Some(0), None),
    );
    base_elements.insert(
        "birthDate".to_string(),
        create_element(Some("date"), Some(0), Some(1)),
    );
    base_elements.insert(
        "gender".to_string(),
        create_element(Some("code"), Some(0), Some(1)),
    );

    let patient = create_schema(
        "http://hl7.org/fhir/StructureDefinition/Patient",
        "Patient",
        "Patient",
        "resource",
        None,
        Some(base_elements),
    );

    // Intermediate profile: requires name
    let mut profile_a_elements = HashMap::new();
    profile_a_elements.insert(
        "name".to_string(),
        create_element(Some("HumanName"), Some(1), None),
    );

    let mut profile_a = create_schema(
        "http://example.org/fhir/StructureDefinition/NamedPatient",
        "NamedPatient",
        "Patient",
        "resource",
        Some("http://hl7.org/fhir/StructureDefinition/Patient"),
        Some(profile_a_elements),
    );
    profile_a.required = Some(vec!["name".to_string()]);

    // Derived profile: also requires birthDate
    let mut profile_b_elements = HashMap::new();
    profile_b_elements.insert(
        "birthDate".to_string(),
        create_element(Some("date"), Some(1), Some(1)),
    );

    let mut profile_b = create_schema(
        "http://example.org/fhir/StructureDefinition/FullPatient",
        "FullPatient",
        "Patient",
        "resource",
        Some("http://example.org/fhir/StructureDefinition/NamedPatient"),
        Some(profile_b_elements),
    );
    profile_b.required = Some(vec!["birthDate".to_string()]);

    // Build validator
    let mut schemas = HashMap::new();
    schemas.insert("Patient".to_string(), patient);
    schemas.insert("NamedPatient".to_string(), profile_a);
    schemas.insert("FullPatient".to_string(), profile_b);

    let validator = FhirSchemaValidator::new(schemas, None);

    // Resolve profile chain
    let chain = validator
        .resolve_profile_chain("FullPatient")
        .expect("Should resolve profile chain");

    // Chain should be: Patient -> NamedPatient -> FullPatient
    assert_eq!(chain.len(), 3, "Chain should have 3 schemas");
    assert_eq!(chain[0].name, "Patient");
    assert_eq!(chain[1].name, "NamedPatient");
    assert_eq!(chain[2].name, "FullPatient");

    // Merge chain and verify constraints are combined
    let merged = validator
        .merge_profile_chain(&chain)
        .expect("Should merge chain");

    // Merged schema should have required from both profiles
    let required = merged.required.unwrap_or_default();
    assert!(
        required.contains(&"name".to_string()),
        "Merged schema should require 'name'"
    );
    assert!(
        required.contains(&"birthDate".to_string()),
        "Merged schema should require 'birthDate'"
    );
}

// =============================================================================
// Test 3: Cycle Detection in Profile Chain
// =============================================================================

#[tokio::test]
async fn test_profile_chain_cycle_detection() {
    // Create circular profile chain: A -> B -> A

    let profile_a = create_schema(
        "http://example.org/fhir/StructureDefinition/ProfileA",
        "ProfileA",
        "SomeResource",
        "resource",
        Some("http://example.org/fhir/StructureDefinition/ProfileB"),
        None,
    );

    let profile_b = create_schema(
        "http://example.org/fhir/StructureDefinition/ProfileB",
        "ProfileB",
        "SomeResource",
        "resource",
        Some("http://example.org/fhir/StructureDefinition/ProfileA"),
        None,
    );

    let mut schemas = HashMap::new();
    schemas.insert("ProfileA".to_string(), profile_a);
    schemas.insert("ProfileB".to_string(), profile_b);

    let validator = FhirSchemaValidator::new(schemas, None);

    // Attempt to resolve chain - should fail with cycle detection
    let result = validator.resolve_profile_chain("ProfileA");

    assert!(result.is_err(), "Should detect cycle in profile chain");

    let error = result.unwrap_err();
    assert!(
        error.message.as_ref().unwrap().contains("Cycle detected"),
        "Error should mention cycle: {:?}",
        error
    );
}

// =============================================================================
// Test 4: Multiple Compatible Profiles
// =============================================================================

#[tokio::test]
async fn test_multiple_compatible_profiles() {
    // HumanName complex type schema
    let mut human_name_elements = HashMap::new();
    human_name_elements.insert(
        "family".to_string(),
        create_element(Some("string"), Some(0), Some(1)),
    );
    human_name_elements.insert(
        "given".to_string(),
        create_element(Some("string"), Some(0), None),
    );

    let human_name = create_schema(
        "http://hl7.org/fhir/StructureDefinition/HumanName",
        "HumanName",
        "HumanName",
        "complex-type",
        None,
        Some(human_name_elements),
    );

    // Base Patient
    let mut base_elements = HashMap::new();
    base_elements.insert(
        "id".to_string(),
        create_element(Some("string"), Some(0), Some(1)),
    );
    base_elements.insert(
        "name".to_string(),
        create_element(Some("HumanName"), Some(0), None),
    );
    base_elements.insert(
        "gender".to_string(),
        create_element(Some("code"), Some(0), Some(1)),
    );
    base_elements.insert(
        "birthDate".to_string(),
        create_element(Some("date"), Some(0), Some(1)),
    );

    let patient = create_schema(
        "http://hl7.org/fhir/StructureDefinition/Patient",
        "Patient",
        "Patient",
        "resource",
        None,
        Some(base_elements),
    );

    // Profile A: requires name
    let mut profile_a_elements = HashMap::new();
    profile_a_elements.insert(
        "name".to_string(),
        create_element(Some("HumanName"), Some(1), None),
    );
    let mut profile_a = create_schema(
        "http://example.org/ProfileA",
        "ProfileA",
        "Patient",
        "resource",
        Some("http://hl7.org/fhir/StructureDefinition/Patient"),
        Some(profile_a_elements),
    );
    profile_a.required = Some(vec!["name".to_string()]);

    // Profile B: requires birthDate (compatible with A)
    let mut profile_b_elements = HashMap::new();
    profile_b_elements.insert(
        "birthDate".to_string(),
        create_element(Some("date"), Some(1), Some(1)),
    );
    let mut profile_b = create_schema(
        "http://example.org/ProfileB",
        "ProfileB",
        "Patient",
        "resource",
        Some("http://hl7.org/fhir/StructureDefinition/Patient"),
        Some(profile_b_elements),
    );
    profile_b.required = Some(vec!["birthDate".to_string()]);

    let mut schemas = HashMap::new();
    schemas.insert("HumanName".to_string(), human_name);
    schemas.insert("Patient".to_string(), patient);
    schemas.insert("ProfileA".to_string(), profile_a);
    schemas.insert("ProfileB".to_string(), profile_b);

    let validator = FhirSchemaValidator::new(schemas, None);

    // No conflicts should be detected
    let profile_a_chain = validator.resolve_profile_chain("ProfileA").unwrap();
    let profile_b_chain = validator.resolve_profile_chain("ProfileB").unwrap();

    let merged_a = validator.merge_profile_chain(&profile_a_chain).unwrap();
    let merged_b = validator.merge_profile_chain(&profile_b_chain).unwrap();

    let conflicts = validator.detect_profile_conflicts(&[merged_a, merged_b]);
    assert!(
        conflicts.is_none(),
        "Compatible profiles should have no conflicts"
    );

    // Validate resource that satisfies both profiles
    let valid_patient = json!({
        "resourceType": "Patient",
        "name": [{"family": "Doe"}],
        "birthDate": "1990-01-01"
    });

    let result = validator
        .validate_with_profiles(
            &valid_patient,
            vec!["ProfileA".to_string(), "ProfileB".to_string()],
        )
        .await;

    assert!(
        result.valid,
        "Patient satisfying both profiles should pass: {:?}",
        result.errors
    );
}

// =============================================================================
// Test 5: Conflicting Profiles
// =============================================================================

#[tokio::test]
async fn test_conflicting_profiles() {
    // Base Patient
    let mut base_elements = HashMap::new();
    base_elements.insert(
        "gender".to_string(),
        create_element(Some("code"), Some(0), Some(1)),
    );

    let patient = create_schema(
        "http://hl7.org/fhir/StructureDefinition/Patient",
        "Patient",
        "Patient",
        "resource",
        None,
        Some(base_elements),
    );

    // Profile A: requires gender=male (via pattern)
    let mut profile_a_elements = HashMap::new();
    let mut gender_elem_a = create_element(Some("code"), Some(1), Some(1));
    gender_elem_a.pattern = Some(FhirSchemaPattern {
        type_name: "code".to_string(),
        value: json!("male"),
        string: Some("male".to_string()),
    });
    profile_a_elements.insert("gender".to_string(), gender_elem_a);

    let profile_a = create_schema(
        "http://example.org/MalePatient",
        "MalePatient",
        "Patient",
        "resource",
        Some("http://hl7.org/fhir/StructureDefinition/Patient"),
        Some(profile_a_elements),
    );

    // Profile B: requires gender=female (via pattern) - CONFLICTS with A
    let mut profile_b_elements = HashMap::new();
    let mut gender_elem_b = create_element(Some("code"), Some(1), Some(1));
    gender_elem_b.pattern = Some(FhirSchemaPattern {
        type_name: "code".to_string(),
        value: json!("female"),
        string: Some("female".to_string()),
    });
    profile_b_elements.insert("gender".to_string(), gender_elem_b);

    let profile_b = create_schema(
        "http://example.org/FemalePatient",
        "FemalePatient",
        "Patient",
        "resource",
        Some("http://hl7.org/fhir/StructureDefinition/Patient"),
        Some(profile_b_elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("Patient".to_string(), patient);
    schemas.insert("MalePatient".to_string(), profile_a);
    schemas.insert("FemalePatient".to_string(), profile_b);

    let validator = FhirSchemaValidator::new(schemas, None);

    // Detect conflicts
    let profile_a_chain = validator.resolve_profile_chain("MalePatient").unwrap();
    let profile_b_chain = validator.resolve_profile_chain("FemalePatient").unwrap();

    let merged_a = validator.merge_profile_chain(&profile_a_chain).unwrap();
    let merged_b = validator.merge_profile_chain(&profile_b_chain).unwrap();

    let conflicts = validator.detect_profile_conflicts(&[merged_a, merged_b]);
    assert!(
        conflicts.is_some(),
        "Conflicting profiles should be detected"
    );

    let conflict_list = conflicts.unwrap();
    assert!(
        !conflict_list.is_empty(),
        "Should have at least one conflict"
    );
    assert!(
        conflict_list[0].contains("gender"),
        "Conflict should mention 'gender'"
    );
}

// =============================================================================
// Test 6: Deep Element Merge
// =============================================================================

#[tokio::test]
async fn test_deep_element_merge() {
    // Base with nested elements
    let mut contact_elem = create_element(None, Some(0), None);
    let mut contact_children = HashMap::new();
    contact_children.insert(
        "name".to_string(),
        create_element(Some("HumanName"), Some(0), Some(1)),
    );
    contact_children.insert(
        "telecom".to_string(),
        create_element(Some("ContactPoint"), Some(0), None),
    );
    contact_elem.elements = Some(contact_children);

    let mut base_elements = HashMap::new();
    base_elements.insert("contact".to_string(), contact_elem);

    let base = create_schema(
        "http://base",
        "Base",
        "Base",
        "resource",
        None,
        Some(base_elements),
    );

    // Profile with nested element constraints
    let mut profile_contact_elem = create_element(None, Some(1), None); // require contact
    let mut profile_contact_children = HashMap::new();
    profile_contact_children.insert(
        "name".to_string(),
        create_element(Some("HumanName"), Some(1), Some(1)), // require name
    );
    profile_contact_elem.elements = Some(profile_contact_children);

    let mut profile_elements = HashMap::new();
    profile_elements.insert("contact".to_string(), profile_contact_elem);

    let profile = create_schema(
        "http://profile",
        "Profile",
        "Base",
        "resource",
        Some("http://base"),
        Some(profile_elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("Base".to_string(), base.clone());
    schemas.insert("Profile".to_string(), profile.clone());

    let validator = FhirSchemaValidator::new(schemas, None);

    // Merge and verify deep merge
    let merged = validator.merge_schemas(&base, &profile);

    // Check merged has contact.name with min=1 from profile
    let contact = merged.elements.as_ref().unwrap().get("contact").unwrap();
    assert_eq!(
        contact.min,
        Some(1),
        "Contact should have min=1 from profile"
    );

    let contact_elements = contact.elements.as_ref().unwrap();

    // Name should have min=1 from profile
    let name = contact_elements.get("name").unwrap();
    assert_eq!(
        name.min,
        Some(1),
        "contact.name should have min=1 from profile"
    );

    // Telecom should still exist from base
    assert!(
        contact_elements.contains_key("telecom"),
        "contact.telecom should still exist from base"
    );
}

// =============================================================================
// Test 7: Slicing Merge
// =============================================================================

#[tokio::test]
async fn test_slicing_merge() {
    // Base with slicing definition
    let mut identifier_elem = create_element(Some("Identifier"), Some(0), None);
    identifier_elem.array = Some(true);
    identifier_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: Some(vec![]),
        rules: Some("open".to_string()),
        ordered: Some(false),
        slices: Some(HashMap::new()),
    });

    let mut base_elements = HashMap::new();
    base_elements.insert("identifier".to_string(), identifier_elem);

    let base = create_schema(
        "http://base",
        "Base",
        "Base",
        "resource",
        None,
        Some(base_elements),
    );

    // Profile adds slices
    let mut profile_identifier_elem = create_element(Some("Identifier"), Some(1), None);
    profile_identifier_elem.array = Some(true);

    let mut slices = HashMap::new();
    slices.insert(
        "MRN".to_string(),
        FhirSchemaSliceMatch {
            match_value: Some(json!({"system": "http://hospital.org/mrn"})),
            schema: None,
            min: Some(1),
            max: Some(1),
        },
    );

    profile_identifier_elem.slicing = Some(FhirSchemaSlicing {
        discriminator: None,
        rules: Some("closed".to_string()), // Override to closed
        ordered: None,
        slices: Some(slices),
    });

    let mut profile_elements = HashMap::new();
    profile_elements.insert("identifier".to_string(), profile_identifier_elem);

    let profile = create_schema(
        "http://profile",
        "Profile",
        "Base",
        "resource",
        Some("http://base"),
        Some(profile_elements),
    );

    let mut schemas = HashMap::new();
    schemas.insert("Base".to_string(), base.clone());
    schemas.insert("Profile".to_string(), profile.clone());

    let validator = FhirSchemaValidator::new(schemas, None);

    // Merge and verify slicing merge
    let merged = validator.merge_schemas(&base, &profile);

    let identifier = merged.elements.as_ref().unwrap().get("identifier").unwrap();
    let slicing = identifier.slicing.as_ref().unwrap();

    // Rules should be "closed" from profile (overlay priority)
    assert_eq!(
        slicing.rules,
        Some("closed".to_string()),
        "Slicing rules should be 'closed' from profile"
    );

    // MRN slice should exist
    let slices = slicing.slices.as_ref().unwrap();
    assert!(slices.contains_key("MRN"), "MRN slice should exist");

    let mrn_slice = slices.get("MRN").unwrap();
    assert_eq!(mrn_slice.min, Some(1), "MRN slice min should be 1");
}

// =============================================================================
// Test 8: Derivation Types (constraint vs specialization)
// =============================================================================

#[tokio::test]
async fn test_derivation_types() {
    // Base resource
    let mut base_elements = HashMap::new();
    base_elements.insert(
        "id".to_string(),
        create_element(Some("string"), Some(0), Some(1)),
    );
    base_elements.insert(
        "status".to_string(),
        create_element(Some("code"), Some(0), Some(1)),
    );

    let base = create_schema(
        "http://base/Resource",
        "BaseResource",
        "BaseResource",
        "resource",
        None,
        Some(base_elements),
    );

    // Constraint profile (restricts)
    let mut constraint_elements = HashMap::new();
    constraint_elements.insert(
        "status".to_string(),
        create_element(Some("code"), Some(1), Some(1)), // Make required
    );

    let mut constraint = create_schema(
        "http://profile/Constraint",
        "ConstraintProfile",
        "BaseResource",
        "resource",
        Some("http://base/Resource"),
        Some(constraint_elements),
    );
    constraint.derivation = Some("constraint".to_string());

    let mut schemas = HashMap::new();
    schemas.insert("BaseResource".to_string(), base);
    schemas.insert("ConstraintProfile".to_string(), constraint);

    let validator = FhirSchemaValidator::new(schemas, None);

    // Resolve chain
    let chain = validator
        .resolve_profile_chain("ConstraintProfile")
        .expect("Should resolve constraint profile chain");

    assert_eq!(chain.len(), 2);
    assert_eq!(chain[1].derivation, Some("constraint".to_string()));

    // Merged profile should make status required
    let merged = validator.merge_profile_chain(&chain).unwrap();
    let status = merged.elements.as_ref().unwrap().get("status").unwrap();
    assert_eq!(
        status.min,
        Some(1),
        "Constraint profile should make status required"
    );
}

// =============================================================================
// Test 9: Schema Not Found Error
// =============================================================================

#[tokio::test]
async fn test_schema_not_found_error() {
    let schemas = HashMap::new();
    let validator = FhirSchemaValidator::new(schemas, None);

    let result = validator.resolve_profile_chain("http://nonexistent/Profile");

    assert!(result.is_err(), "Should fail for non-existent schema");
    let error = result.unwrap_err();
    assert!(
        error.message.as_ref().unwrap().contains("Schema not found"),
        "Error should mention schema not found"
    );
}

// =============================================================================
// Test 10: Embedded R4 Schema Profile Chain
// =============================================================================

#[tokio::test]
async fn test_embedded_r4_profile_chain() {
    let schemas = get_schemas(FhirVersion::R4);
    let validator = FhirSchemaValidator::new(schemas.clone(), None);

    // Patient should have a base chain to DomainResource -> Resource -> Base
    let chain = validator.resolve_profile_chain("Patient");

    assert!(chain.is_ok(), "Should resolve Patient chain");
    let chain = chain.unwrap();

    // Chain should include Patient and its base types
    assert!(!chain.is_empty(), "Chain should not be empty");
    assert_eq!(
        chain.last().unwrap().name,
        "Patient",
        "Last should be Patient"
    );
}
