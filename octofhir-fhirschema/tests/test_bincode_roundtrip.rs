use octofhir_fhirschema::types::FhirSchema;
use std::collections::HashMap;

#[test]
fn test_json_roundtrip() {
    // Create a simple test schema matching the actual FhirSchema structure
    let mut test_schemas = HashMap::new();
    let test_schema = FhirSchema {
        url: "http://test.com/test".to_string(),
        version: Some("1.0.0".to_string()),
        name: "TestSchema".to_string(),
        type_name: "TestType".to_string(),
        kind: "test".to_string(),
        derivation: None,
        base: None,
        abstract_type: None,
        class: "test-class".to_string(),
        description: None,
        package_name: None,
        package_version: None,
        package_id: None,
        package_meta: None,
        elements: Some(HashMap::new()),
        required: None,
        excluded: None,
        extensions: None,
        constraint: None,
        primitive_type: None,
        choices: None,
    };
    test_schemas.insert("TestSchema".to_string(), test_schema);

    // Test JSON serialization
    let serialized = serde_json::to_vec(&test_schemas).expect("JSON serialization should work");

    println!("JSON serialized {} bytes", serialized.len());

    // Test JSON deserialization
    let deserialized: HashMap<String, FhirSchema> =
        serde_json::from_slice(&serialized).expect("JSON deserialization should work");

    assert_eq!(deserialized.len(), 1);
    assert!(deserialized.contains_key("TestSchema"));
    println!("JSON round-trip test passed");
}

#[test]
fn test_json_pretty_roundtrip() {
    // Create a simple test schema matching the actual FhirSchema structure
    let mut test_schemas = HashMap::new();
    let test_schema = FhirSchema {
        url: "http://test.com/test".to_string(),
        version: Some("1.0.0".to_string()),
        name: "TestSchema".to_string(),
        type_name: "TestType".to_string(),
        kind: "test".to_string(),
        derivation: None,
        base: None,
        abstract_type: None,
        class: "test-class".to_string(),
        description: None,
        package_name: None,
        package_version: None,
        package_id: None,
        package_meta: None,
        elements: Some(HashMap::new()),
        required: None,
        excluded: None,
        extensions: None,
        constraint: None,
        primitive_type: None,
        choices: None,
    };
    test_schemas.insert("TestSchema".to_string(), test_schema);

    // Test pretty JSON serialization
    let serialized =
        serde_json::to_string_pretty(&test_schemas).expect("Pretty JSON serialization should work");

    println!("Pretty JSON serialized {} bytes", serialized.len());

    // Test JSON deserialization from string
    let deserialized: HashMap<String, FhirSchema> =
        serde_json::from_str(&serialized).expect("Pretty JSON deserialization should work");

    assert_eq!(deserialized.len(), 1);
    assert!(deserialized.contains_key("TestSchema"));
    println!("Pretty JSON round-trip test passed");
}
