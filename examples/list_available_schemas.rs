use octofhir_fhir_model::provider::ModelProvider;
use octofhir_fhirschema::provider::EmbeddedModelProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the provider
    let provider = EmbeddedModelProvider::r4().await?;

    println!("Available resource types:");
    let resource_types = provider.get_available_resource_types();
    for (i, resource_type) in resource_types.iter().take(20).enumerate() {
        println!("{}. {}", i + 1, resource_type);
    }

    if resource_types.len() > 20 {
        println!("... and {} more", resource_types.len() - 20);
    }

    println!("\nTotal: {} resource types", resource_types.len());

    // Test with Patient resource
    println!("\nTesting with Patient resource:");

    // Test 1: Valid property "name" on Patient should succeed
    match provider.navigate_typed_path("Patient", "name").await {
        Ok(_) => println!("✓ Valid property 'name' on Patient: SUCCESS"),
        Err(e) => println!("✗ Valid property 'name' on Patient: FAILED - {e}"),
    }

    // Test 2: Invalid property "name1" on Patient should fail
    match provider.navigate_typed_path("Patient", "name1").await {
        Ok(_) => println!("✗ Invalid property 'name1' on Patient: SHOULD HAVE FAILED"),
        Err(e) => println!("✓ Invalid property 'name1' on Patient: CORRECTLY FAILED - {e}"),
    }

    // Test 3: Valid property "id" on Patient should succeed
    match provider.navigate_typed_path("Patient", "id").await {
        Ok(_) => println!("✓ Valid property 'id' on Patient: SUCCESS"),
        Err(e) => println!("✗ Valid property 'id' on Patient: FAILED - {e}"),
    }

    // Test 4: Invalid property "given1" on Patient should fail (this was our original failing case)
    match provider.navigate_typed_path("Patient", "given1").await {
        Ok(_) => println!("✗ Invalid property 'given1' on Patient: SHOULD HAVE FAILED"),
        Err(e) => println!("✓ Invalid property 'given1' on Patient: CORRECTLY FAILED - {e}"),
    }

    // Test 5: Valid property "gender" on Patient should succeed
    match provider.navigate_typed_path("Patient", "gender").await {
        Ok(_) => println!("✓ Valid property 'gender' on Patient: SUCCESS"),
        Err(e) => println!("✗ Valid property 'gender' on Patient: FAILED - {e}"),
    }

    // Test 6: Invalid property "invalidProperty123" on Patient should fail
    match provider
        .navigate_typed_path("Patient", "invalidProperty123")
        .await
    {
        Ok(_) => println!("✗ Invalid property 'invalidProperty123' on Patient: SHOULD HAVE FAILED"),
        Err(e) => {
            println!("✓ Invalid property 'invalidProperty123' on Patient: CORRECTLY FAILED - {e}")
        }
    }

    Ok(())
}
