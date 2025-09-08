use octofhir_fhirschema::provider::EmbeddedModelProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the provider
    let provider = EmbeddedModelProvider::r4().await?;

    println!("Testing property validation fix...");

    // Test 1: Valid property "given" on HumanName should succeed
    match provider.navigate_typed_path("HumanName", "given").await {
        Ok(_) => println!("✓ Valid property 'given' on HumanName: SUCCESS"),
        Err(e) => println!("✗ Valid property 'given' on HumanName: FAILED - {e}"),
    }

    // Test 2: Invalid property "given1" on HumanName should fail
    match provider.navigate_typed_path("HumanName", "given1").await {
        Ok(_) => println!("✗ Invalid property 'given1' on HumanName: SHOULD HAVE FAILED"),
        Err(e) => println!("✓ Invalid property 'given1' on HumanName: CORRECTLY FAILED - {e}"),
    }

    // Test 3: Valid property "family" on HumanName should succeed
    match provider.navigate_typed_path("HumanName", "family").await {
        Ok(_) => println!("✓ Valid property 'family' on HumanName: SUCCESS"),
        Err(e) => println!("✗ Valid property 'family' on HumanName: FAILED - {e}"),
    }

    // Test 4: Invalid property "nonexistent" on HumanName should fail
    match provider
        .navigate_typed_path("HumanName", "nonexistent")
        .await
    {
        Ok(_) => println!("✗ Invalid property 'nonexistent' on HumanName: SHOULD HAVE FAILED"),
        Err(e) => println!("✓ Invalid property 'nonexistent' on HumanName: CORRECTLY FAILED - {e}"),
    }

    // Test 5: Invalid type should fail
    match provider
        .navigate_typed_path("NonExistentType", "someProperty")
        .await
    {
        Ok(_) => println!("✗ Invalid type 'NonExistentType': SHOULD HAVE FAILED"),
        Err(e) => println!("✓ Invalid type 'NonExistentType': CORRECTLY FAILED - {e}"),
    }

    Ok(())
}
