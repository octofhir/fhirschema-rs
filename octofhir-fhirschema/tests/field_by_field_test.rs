use octofhir_fhirschema::types::FhirSchema;
use std::collections::HashMap;

#[test]
fn test_field_by_field() {
    println!("Testing FhirSchema fields one by one...");

    // Test 1: Only required String fields
    {
        let mut test_schemas = HashMap::new();
        let test_schema = FhirSchema {
            url: "http://test.com/test".to_string(),
            name: "TestSchema".to_string(),
            type_name: "TestType".to_string(),
            kind: "test".to_string(),
            class: "test-class".to_string(),

            // All optional fields None
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

        let serialized =
            serde_json::to_vec(&test_schemas).expect("Required fields serialization should work");

        let _deserialized: HashMap<String, FhirSchema> = serde_json::from_slice(&serialized)
            .expect("Required fields deserialization should work");

        println!("✓ Required string fields work");
    }

    // Test 2: Add simple optional String fields
    {
        let mut test_schemas = HashMap::new();
        let test_schema = FhirSchema {
            url: "http://test.com/test".to_string(),
            name: "TestSchema".to_string(),
            type_name: "TestType".to_string(),
            kind: "test".to_string(),
            class: "test-class".to_string(),
            version: Some("1.0.0".to_string()),
            derivation: Some("constraint".to_string()),
            base: Some("Element".to_string()),
            description: Some("Test description".to_string()),
            package_name: Some("test.package".to_string()),
            package_version: Some("1.0".to_string()),
            package_id: Some("test-pkg".to_string()),
            primitive_type: Some("string".to_string()),

            // Still None
            abstract_type: None,
            package_meta: None,
            elements: None,
            required: None,
            excluded: None,
            extensions: None,
            constraint: None,
            choices: None,
        };
        test_schemas.insert("TestSchema".to_string(), test_schema);

        let serialized = serde_json::to_vec(&test_schemas)
            .expect("String optional fields serialization should work");

        let _deserialized: HashMap<String, FhirSchema> = serde_json::from_slice(&serialized)
            .expect("String optional fields deserialization should work");

        println!("✓ Optional string fields work");
    }

    // Test 3: Add boolean field
    {
        let mut test_schemas = HashMap::new();
        let test_schema = FhirSchema {
            url: "http://test.com/test".to_string(),
            name: "TestSchema".to_string(),
            type_name: "TestType".to_string(),
            kind: "test".to_string(),
            class: "test-class".to_string(),
            version: Some("1.0.0".to_string()),
            derivation: Some("constraint".to_string()),
            base: Some("Element".to_string()),
            description: Some("Test description".to_string()),
            package_name: Some("test.package".to_string()),
            package_version: Some("1.0".to_string()),
            package_id: Some("test-pkg".to_string()),
            primitive_type: Some("string".to_string()),
            abstract_type: Some(false),

            // Still None
            package_meta: None,
            elements: None,
            required: None,
            excluded: None,
            extensions: None,
            constraint: None,
            choices: None,
        };
        test_schemas.insert("TestSchema".to_string(), test_schema);

        let serialized =
            serde_json::to_vec(&test_schemas).expect("Boolean field serialization should work");

        let _deserialized: HashMap<String, FhirSchema> =
            serde_json::from_slice(&serialized).expect("Boolean field deserialization should work");

        println!("✓ Boolean field works");
    }

    // Test 4: Add simple Vec<String> fields
    {
        let mut test_schemas = HashMap::new();
        let test_schema = FhirSchema {
            url: "http://test.com/test".to_string(),
            name: "TestSchema".to_string(),
            type_name: "TestType".to_string(),
            kind: "test".to_string(),
            class: "test-class".to_string(),
            version: Some("1.0.0".to_string()),
            derivation: Some("constraint".to_string()),
            base: Some("Element".to_string()),
            description: Some("Test description".to_string()),
            package_name: Some("test.package".to_string()),
            package_version: Some("1.0".to_string()),
            package_id: Some("test-pkg".to_string()),
            primitive_type: Some("string".to_string()),
            abstract_type: Some(false),
            required: Some(vec!["id".to_string()]),
            excluded: Some(vec!["deprecated".to_string()]),

            // Still None
            package_meta: None,
            elements: None,
            extensions: None,
            constraint: None,
            choices: None,
        };
        test_schemas.insert("TestSchema".to_string(), test_schema);

        let serialized = serde_json::to_vec(&test_schemas)
            .expect("Vec<String> fields serialization should work");

        let _deserialized: HashMap<String, FhirSchema> = serde_json::from_slice(&serialized)
            .expect("Vec<String> fields deserialization should work");

        println!("✓ Vec<String> fields work");
    }

    println!("All basic field tests passed!");
}
