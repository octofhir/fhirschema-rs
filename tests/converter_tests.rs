mod common;

use octofhir_fhirschema::*;
use url::Url;

#[test]
fn test_converter_creation() {
    let _converter = FhirSchemaConverter::new();

    let config = ConverterConfig {
        expand_choice_types: false,
        include_slicing: false,
        process_constraints: false,
        resolve_profiles: false,
        cache_results: false,
    };
    let _converter = FhirSchemaConverter::with_config(config);
    // Test passes if converters can be created without error
}

#[test]
fn test_basic_structure_definition_conversion() {
    let converter = FhirSchemaConverter::new();

    let mut structure_def = StructureDefinition::new("Patient", "resource");
    structure_def.url =
        Some(Url::parse("https://example.com/StructureDefinition/Patient").unwrap());
    structure_def.name = Some("Patient".to_string());
    structure_def.title = Some("Patient Resource".to_string());
    structure_def.description = Some("A patient resource".to_string());
    structure_def.version = Some("1.0.0".to_string());
    structure_def.status = Some("active".to_string());

    // Add a simple element
    let element = ElementDefinition {
        id: Some("Patient.id".to_string()),
        path: "Patient.id".to_string(),
        representation: None,
        slice_name: None,
        slice_is_constraining: None,
        label: None,
        code: None,
        slicing: None,
        short: Some("Logical id".to_string()),
        definition: Some("The logical ID of the resource".to_string()),
        comment: None,
        requirements: None,
        alias: None,
        min: Some(0),
        max: Some("1".to_string()),
        base: None,
        content_reference: None,
        element_type: Some(vec![ElementDefinitionType {
            code: "id".to_string(),
            profile: None,
            target_profile: None,
            aggregation: None,
            versioning: None,
        }]),
        name_reference: None,
        default_value: None,
        meaning_when_missing: None,
        order_meaning: None,
        fixed_value: None,
        pattern_value: None,
        example: None,
        min_value: None,
        max_value: None,
        max_length: None,
        condition: None,
        constraint: None,
        must_support: None,
        is_modifier: None,
        is_modifier_reason: None,
        is_summary: None,
        binding: None,
        mapping: None,
    };

    structure_def.elements = vec![element];

    let result = converter.convert(&structure_def);
    assert!(result.is_ok());

    let schema = result.unwrap();
    assert_eq!(schema.schema_type, "Patient");
    assert_eq!(schema.name, Some("Patient".to_string()));
    assert_eq!(schema.title, Some("Patient Resource".to_string()));
    assert_eq!(schema.description, Some("A patient resource".to_string()));
    assert_eq!(schema.version, Some("1.0.0".to_string()));
    assert_eq!(schema.status, Some("active".to_string()));
    assert_eq!(schema.elements.len(), 1);
    assert!(schema.elements.contains_key("Patient.id"));
}

#[test]
fn test_choice_type_expansion() {
    let converter = FhirSchemaConverter::new();

    let mut structure_def = StructureDefinition::new("TestResource", "resource");
    structure_def.url =
        Some(Url::parse("https://example.com/StructureDefinition/TestResource").unwrap());

    // Add a choice type element
    let choice_element = ElementDefinition {
        id: Some("TestResource.value[x]".to_string()),
        path: "TestResource.value[x]".to_string(),
        representation: None,
        slice_name: None,
        slice_is_constraining: None,
        label: None,
        code: None,
        slicing: None,
        short: Some("Value choice".to_string()),
        definition: Some("A choice of value types".to_string()),
        comment: None,
        requirements: None,
        alias: None,
        min: Some(0),
        max: Some("1".to_string()),
        base: None,
        content_reference: None,
        element_type: Some(vec![
            ElementDefinitionType {
                code: "string".to_string(),
                profile: None,
                target_profile: None,
                aggregation: None,
                versioning: None,
            },
            ElementDefinitionType {
                code: "integer".to_string(),
                profile: None,
                target_profile: None,
                aggregation: None,
                versioning: None,
            },
        ]),
        name_reference: None,
        default_value: None,
        meaning_when_missing: None,
        order_meaning: None,
        fixed_value: None,
        pattern_value: None,
        example: None,
        min_value: None,
        max_value: None,
        max_length: None,
        condition: None,
        constraint: None,
        must_support: None,
        is_modifier: None,
        is_modifier_reason: None,
        is_summary: None,
        binding: None,
        mapping: None,
    };

    structure_def.elements = vec![choice_element];

    let result = converter.convert(&structure_def);
    assert!(result.is_ok());

    let schema = result.unwrap();

    // Should have expanded into valueString and valueInteger
    assert!(schema.elements.contains_key("TestResource.valueString"));
    assert!(schema.elements.contains_key("TestResource.valueInteger"));

    // Check that the types are correct
    let string_element = &schema.elements["TestResource.valueString"];
    assert_eq!(
        string_element.element_type.as_ref().unwrap()[0].code,
        "string"
    );

    let integer_element = &schema.elements["TestResource.valueInteger"];
    assert_eq!(
        integer_element.element_type.as_ref().unwrap()[0].code,
        "integer"
    );
}

#[test]
fn test_constraint_processing() {
    let converter = FhirSchemaConverter::new();

    let mut structure_def = StructureDefinition::new("Patient", "resource");
    structure_def.url =
        Some(Url::parse("https://example.com/StructureDefinition/Patient").unwrap());

    // Add an element with constraints
    let element_with_constraint = ElementDefinition {
        id: Some("Patient.name".to_string()),
        path: "Patient.name".to_string(),
        representation: None,
        slice_name: None,
        slice_is_constraining: None,
        label: None,
        code: None,
        slicing: None,
        short: Some("Patient name".to_string()),
        definition: Some("The name of the patient".to_string()),
        comment: None,
        requirements: None,
        alias: None,
        min: Some(1),
        max: Some("*".to_string()),
        base: None,
        content_reference: None,
        element_type: Some(vec![ElementDefinitionType {
            code: "HumanName".to_string(),
            profile: None,
            target_profile: None,
            aggregation: None,
            versioning: None,
        }]),
        name_reference: None,
        default_value: None,
        meaning_when_missing: None,
        order_meaning: None,
        fixed_value: None,
        pattern_value: None,
        example: None,
        min_value: None,
        max_value: None,
        max_length: None,
        condition: None,
        constraint: Some(vec![ElementDefinitionConstraint {
            key: "pat-1".to_string(),
            requirements: None,
            severity: "error".to_string(),
            human: "Patient must have at least one name".to_string(),
            expression: Some("name.exists()".to_string()),
            xpath: None,
            source: None,
        }]),
        must_support: None,
        is_modifier: None,
        is_modifier_reason: None,
        is_summary: None,
        binding: None,
        mapping: None,
    };

    structure_def.elements = vec![element_with_constraint];

    let result = converter.convert(&structure_def);
    assert!(result.is_ok());

    let schema = result.unwrap();

    // Should have processed the constraint
    assert_eq!(schema.constraints.len(), 1);
    let constraint = &schema.constraints[0];
    assert_eq!(constraint.key, "pat-1");
    assert_eq!(constraint.severity, "error");
    assert_eq!(constraint.human, "Patient must have at least one name");
    assert_eq!(constraint.expression, "name.exists()");
}

#[test]
fn test_conversion_context() {
    let config = ConverterConfig::default();
    let mut context = ConversionContext::new(&config);

    let structure_def = StructureDefinition::new("Test", "resource");

    // Test context lifecycle
    assert!(context.begin_conversion(&structure_def).is_ok());
    assert!(!context.is_element_processed("Test.id"));

    context.mark_element_processed("Test.id");
    assert!(context.is_element_processed("Test.id"));

    context.add_choice_type_expansion(
        "Test.value".to_string(),
        vec![
            "Test.valueString".to_string(),
            "Test.valueInteger".to_string(),
        ],
    );

    let expansions = context.get_choice_type_expansions("Test.value");
    assert!(expansions.is_some());
    assert_eq!(expansions.unwrap().len(), 2);

    let schema = FhirSchema::new("Test");
    assert!(context.end_conversion(&schema).is_ok());

    let stats = context.get_stats();
    assert_eq!(stats.elements_processed, 1);
    assert_eq!(stats.choice_types_expanded, 1);
}
