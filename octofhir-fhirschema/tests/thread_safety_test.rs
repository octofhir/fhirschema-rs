use octofhir_fhirschema::{EmbeddedSchemaProvider, ModelProvider};
use std::sync::Arc;
use std::thread;
use tokio::runtime::Runtime;

#[test]
fn test_embedded_provider_convenience_methods() {
    // Test all convenience methods
    let r4_provider = EmbeddedSchemaProvider::r4();
    let r4b_provider = EmbeddedSchemaProvider::r4b();
    let r5_provider = EmbeddedSchemaProvider::r5();
    let r6_provider = EmbeddedSchemaProvider::r6();

    // Verify they have schemas loaded
    assert!(
        r4_provider.schema_count() > 0,
        "R4 provider should have schemas"
    );
    assert!(
        r4b_provider.schema_count() > 0,
        "R4B provider should have schemas"
    );
    assert!(
        r5_provider.schema_count() > 0,
        "R5 provider should have schemas"
    );
    assert!(
        r6_provider.schema_count() > 0,
        "R6 provider should have schemas"
    );

    println!("✅ All convenience methods work:");
    println!("  - R4: {} schemas", r4_provider.schema_count());
    println!("  - R4B: {} schemas", r4b_provider.schema_count());
    println!("  - R5: {} schemas", r5_provider.schema_count());
    println!("  - R6: {} schemas", r6_provider.schema_count());
}

#[test]
fn test_provider_send_sync() {
    // This test verifies that our providers implement Send + Sync
    // These are compile-time checks - if any provider doesn't implement
    // Send + Sync, this test will fail to compile

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    fn assert_send_sync<T: Send + Sync>() {}

    // Verify at compile time that our providers are Send + Sync
    assert_send::<EmbeddedSchemaProvider>();
    assert_sync::<EmbeddedSchemaProvider>();
    assert_send_sync::<EmbeddedSchemaProvider>();

    println!("✅ EmbeddedSchemaProvider implements Send + Sync (automatically derived)");
}

#[test]
fn test_multithreaded_usage() {
    // Test actual multithreaded usage
    let provider = Arc::new(EmbeddedSchemaProvider::r4());
    let mut handles = vec![];

    // Spawn multiple threads that use the provider
    for i in 0..4 {
        let provider_clone = Arc::clone(&provider);
        let handle = thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            rt.block_on(async move {
                // Test basic provider operations in each thread
                let schema_count = provider_clone.schema_count();
                let version = provider_clone.version();

                // Try to get a common type
                let result = provider_clone.get_type("Patient").await;

                println!(
                    "Thread {}: {} schemas, version {:?}, Patient type: {}",
                    i,
                    schema_count,
                    version,
                    result.is_ok() && result.unwrap().is_some()
                );

                // Return some result to verify thread completed successfully
                schema_count > 0
            })
        });
        handles.push(handle);
    }

    // Wait for all threads to complete and verify they all succeeded
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result, "Thread should have successfully accessed provider");
    }

    println!("✅ Provider works correctly across multiple threads");
}

#[tokio::test]
async fn test_async_multithreaded_usage() {
    // Test async multithreaded usage with tokio
    let provider = Arc::new(EmbeddedSchemaProvider::r4());
    let mut tasks = vec![];

    // Spawn multiple async tasks
    for i in 0..4 {
        let provider_clone = Arc::clone(&provider);
        let task = tokio::spawn(async move {
            // Test provider operations
            let patient_type = provider_clone.get_type("Patient").await.unwrap();
            let observation_type = provider_clone.get_type("Observation").await.unwrap();

            println!(
                "Task {}: Patient={}, Observation={}",
                i,
                patient_type.is_some(),
                observation_type.is_some()
            );

            patient_type.is_some() && observation_type.is_some()
        });
        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        let result = task.await.unwrap();
        assert!(result, "Async task should have successfully used provider");
    }

    println!("✅ Provider works correctly with async multithreading");
}
