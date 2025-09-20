use octofhir_fhirschema::types::FhirSchema;
use std::collections::HashMap;

#[test]
fn test_minimal_schema() {
    // Test with only the absolute minimum required fields
    let mut test_schemas = HashMap::new();
    let test_schema = FhirSchema {
        // Required fields only
        url: "http://test.com/test".to_string(),
        name: "TestSchema".to_string(),
        type_name: "TestType".to_string(),
        kind: "test".to_string(),
        class: "test-class".to_string(),

        // Set all optional fields to None/empty
        version: None,
        derivation: None,
        base: None,
        abstract_type: None,
        description: None,
        package_name: None,
        package_version: None,
        package_id: None,
        package_meta: None,
        elements: None,
        required: None,
        excluded: None,
        extensions: None,
        constraint: None,
        primitive_type: None,
        choices: None,
    };
    test_schemas.insert("TestSchema".to_string(), test_schema);

    println!("Testing minimal schema serialization...");

    // Test with JSON serialization
    let serialized = serde_json::to_vec(&test_schemas).expect("Minimal serialization should work");
    println!("Serialized {} bytes", serialized.len());

    let deserialized: HashMap<String, FhirSchema> =
        serde_json::from_slice(&serialized).expect("Minimal deserialization should work");

    assert_eq!(deserialized.len(), 1);
    assert!(deserialized.contains_key("TestSchema"));
    println!("Minimal schema test passed");
}
