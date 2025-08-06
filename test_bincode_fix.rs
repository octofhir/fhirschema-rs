use octofhir_fhirschema::types::FhirSchema;

#[cfg(feature = "bincode")]
fn main() {
    // Create a test schema
    let schema = FhirSchema::new("Patient")
        .with_name("TestPatient".to_string());

    // Test bincode serialization
    let encoded = bincode::encode_to_vec(&schema, bincode::config::standard())
        .expect("Failed to encode schema with bincode");

    println!("✓ Successfully encoded FhirSchema with bincode ({} bytes)", encoded.len());

    // Test bincode deserialization
    let (decoded, _): (FhirSchema, _) = bincode::decode_from_slice(&encoded, bincode::config::standard())
        .expect("Failed to decode schema with bincode");

    println!("✓ Successfully decoded FhirSchema with bincode");

    // Verify the data is correct
    assert_eq!(schema.schema_type, decoded.schema_type);
    assert_eq!(schema.name, decoded.name);

    println!("✓ Bincode serialization/deserialization working correctly!");
}

#[cfg(not(feature = "bincode"))]
fn main() {
    println!("Bincode feature not enabled - skipping test");
}
