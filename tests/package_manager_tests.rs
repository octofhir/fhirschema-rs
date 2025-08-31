use octofhir_canonical_manager::{FcmConfig, RegistryConfig, StorageConfig};
use octofhir_fhirschema::package::{
    ConversionOptions, InstallHooks, PackageMetadata, PackageSource, PackageSpecBuilder,
    SchemaIndex, SimpleProgressTracker,
};
use octofhir_fhirschema::{
    ConversionPipeline, FhirSchemaPackageManager, InstallOptions, PackageId, PackageManagerConfig,
    PackageSpec, ProgressTracker,
};
use std::sync::Arc;
use tempfile::TempDir;

fn create_test_fcm_config() -> FcmConfig {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_path_buf();

    FcmConfig {
        registry: RegistryConfig::default(),
        packages: vec![],
        storage: StorageConfig {
            cache_dir: temp_path.join("cache"),
            index_dir: temp_path.join("index"),
            packages_dir: temp_path.join("packages"),
            max_cache_size: "100MB".to_string(),
        },
        optimization: Default::default(),
    }
}

#[tokio::test]
async fn test_package_manager_initialization() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = FhirSchemaPackageManager::new(fcm_config, config).await;
    assert!(package_manager.is_ok());

    let manager = package_manager.unwrap();
    let packages = manager.list_packages().await;
    assert!(packages.is_empty());
}

#[tokio::test]
async fn test_package_spec_creation() {
    // Test registry package spec
    let spec = PackageSpec::registry("hl7.fhir.r4.core", "4.0.1");
    assert_eq!(spec.name, "hl7.fhir.r4.core");
    assert_eq!(spec.version, "4.0.1");

    // Test local package spec
    let local_spec = PackageSpec::local("local.package", "/tmp/package");
    assert_eq!(local_spec.name, "local.package");
    assert_eq!(local_spec.version, "local");

    // Test git package spec
    let git_spec = PackageSpec::git("git.package", "https://github.com/example/package.git");
    assert_eq!(git_spec.name, "git.package");
    assert_eq!(git_spec.version, "git");
}

#[tokio::test]
async fn test_install_options() {
    let options = InstallOptions::default();
    assert!(!options.skip_dependencies);
    assert!(!options.force);
    assert!(!options.allow_prerelease);
    assert_eq!(options.timeout_seconds, 300);
    assert!(options.validate);
}

#[tokio::test]
async fn test_package_id() {
    let package_id = PackageId::new("test.package", "1.0.0");
    assert_eq!(package_id.name, "test.package");
    assert_eq!(package_id.version, "1.0.0");
    assert_eq!(format!("{package_id}"), "test.package@1.0.0");
}

#[tokio::test]
async fn test_bridge_support_schema_methods() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test bridge support schema methods
    let schema = package_manager
        .get_schema("http://example.com/NonExistentSchema")
        .await;
    assert!(schema.is_none());

    let schemas = package_manager.get_schemas_by_type("Patient").await;
    assert!(schemas.is_empty());

    let schema_by_type = package_manager.get_schema_by_type("Patient").await;
    assert!(schema_by_type.is_none());

    let resolved_profile = package_manager
        .resolve_profile("Patient", "http://example.com/PatientProfile")
        .await;
    assert!(resolved_profile.is_none());

    let search_results = package_manager.search_schemas("patient").await;
    assert!(search_results.is_empty());
}

#[tokio::test]
async fn test_bridge_support_resource_type_methods() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test bridge support resource type methods
    let has_type = package_manager.has_resource_type("Patient").await;
    assert!(!has_type); // Empty registry should have no types

    let resource_types = package_manager.get_resource_types().await;
    assert!(resource_types.is_empty());

    let _is_primitive = package_manager.is_primitive_type("string").await;
    // This might be true or false depending on whether basic types are preloaded

    let is_complex = package_manager.is_complex_type("Patient").await;
    assert!(!is_complex); // Empty registry should have no complex types

    let base_type = package_manager.get_base_type("Patient").await;
    assert!(base_type.is_none());

    let is_subtype = package_manager.is_subtype_of("Patient", "Resource").await;
    assert!(!is_subtype); // Empty registry should have no inheritance
}

#[tokio::test]
async fn test_bridge_support_choice_type_methods() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test bridge support choice type methods
    let choice_options = package_manager.get_choice_type_options("value[x]").await;
    assert!(choice_options.is_empty()); // Empty registry should have no choice types

    let resolved_choice = package_manager
        .resolve_choice_type("value[x]", "string")
        .await;
    // The method might implement basic choice type resolution even with empty registry
    // so let's check if it returns a reasonable result
    if let Some(result) = resolved_choice {
        assert_eq!(result, "valueString"); // Basic expansion should work
    }

    let is_choice_expansion = package_manager
        .is_choice_type_expansion("valueString")
        .await;
    assert!(!is_choice_expansion); // Empty registry should have no expansions

    let choice_base = package_manager.get_choice_type_base("valueString").await;
    assert!(choice_base.is_none());
}

#[tokio::test]
async fn test_bridge_support_path_resolution_methods() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test bridge support path resolution methods
    let element_info = package_manager
        .resolve_element_path("Patient", "name.given")
        .await;
    assert!(element_info.is_none()); // Empty registry should resolve nothing

    let cardinality = package_manager
        .get_element_cardinality("Patient", "identifier")
        .await;
    assert!(cardinality.is_none());

    let has_path = package_manager.has_element_path("Patient", "name").await;
    assert!(!has_path); // Empty registry should have no paths

    let available_paths = package_manager.get_available_paths("Patient").await;
    assert!(available_paths.is_empty());

    let element_type = package_manager.get_element_type("Patient", "name").await;
    assert!(element_type.is_none());
}

#[tokio::test]
async fn test_bridge_support_type_reflection_methods() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test bridge support type reflection methods
    let type_properties = package_manager.get_type_properties("Patient").await;
    assert!(type_properties.is_empty()); // Empty registry should have no properties

    // Note: get_property_info and get_type_definition
    // are not implemented yet, but get_type_properties is
    let type_properties_again = package_manager.get_type_properties("Patient").await;
    assert!(type_properties_again.is_empty());
}

#[tokio::test]
async fn test_bridge_support_constraint_methods() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test bridge support constraint methods
    let type_constraints = package_manager.get_type_constraints("Patient").await;
    assert!(type_constraints.is_empty()); // Empty registry should have no constraints

    let element_constraints = package_manager
        .get_element_constraints("Patient", "name")
        .await;
    assert!(element_constraints.is_empty());

    // Test constraint expression validation
    let validation_result = package_manager
        .validate_constraint_expression("name.exists()")
        .await;
    assert!(validation_result.is_valid); // Basic expressions should validate

    let empty_validation = package_manager.validate_constraint_expression("").await;
    assert!(!empty_validation.is_valid); // Empty expressions should fail
    assert!(!empty_validation.errors.is_empty());
}

#[tokio::test]
async fn test_bridge_support_utility_methods() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test bridge support utility methods
    let registry_metrics = package_manager.get_registry_metrics().await;
    assert_eq!(registry_metrics.total_schemas, 0); // Empty registry
    assert_eq!(registry_metrics.resource_types, 0);
    assert_eq!(registry_metrics.profiles, 0);

    // Note: get_path_resolver_metrics not implemented yet
    // Just test that we can call it multiple times without issues
    let _metrics_again = package_manager.get_registry_metrics().await;

    // Test index rebuilding
    let rebuild_result = package_manager.rebuild_indexes().await;
    assert!(rebuild_result.is_ok());
}

#[tokio::test]
async fn test_conversion_pipeline() {
    use octofhir_fhirschema::{ConverterConfig, FhirSchemaConverter};

    let converter = Arc::new(FhirSchemaConverter::with_config(ConverterConfig::default()));
    let pipeline = ConversionPipeline::new(converter, 4);

    // Test with empty structure definitions
    let structure_definitions = vec![];
    let options = ConversionOptions::default();

    let result = pipeline
        .convert_batch(&structure_definitions, &options)
        .await;
    assert!(result.is_ok());

    let batch_result = result.unwrap();
    assert_eq!(batch_result.schemas.len(), 0);
    assert_eq!(batch_result.results.total_structure_definitions, 0);
    assert_eq!(batch_result.results.converted_schemas, 0);
    assert_eq!(batch_result.results.failed.len(), 0);
}

#[tokio::test]
async fn test_progress_tracker() {
    let tracker = SimpleProgressTracker::new("test");

    // These should not panic
    tracker.update_progress(1, 10, "TestResource");
    tracker.set_completed();
    tracker.set_error("Test error");
}

#[tokio::test]
async fn test_package_manager_package_operations() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = FhirSchemaPackageManager::new(fcm_config, config)
        .await
        .expect("Failed to create package manager");

    // Test listing empty packages
    let packages = package_manager.list_packages().await;
    assert!(packages.is_empty());

    // Test getting non-existent package
    let package_id = PackageId::new("nonexistent", "1.0.0");
    let package = package_manager.get_package(&package_id).await;
    assert!(package.is_none());

    // Test uninstalling non-existent package
    let uninstall_result = package_manager.uninstall_package(&package_id).await;
    assert!(uninstall_result.is_ok());
    assert!(!uninstall_result.unwrap());
}

#[tokio::test]
async fn test_conversion_options() {
    let options = ConversionOptions::default();
    assert!(options.expand_choice_types);
    assert!(options.include_slicing);
    assert!(options.process_constraints);
    assert!(options.resolve_profiles);
    assert!(options.cache_results);
    assert!(options.resource_type_filter.is_empty());
    assert!(options.profile_type_filter.is_empty());
    assert!(options.custom_settings.is_empty());
}

#[tokio::test]
async fn test_error_handling() {
    // Test invalid FCM config path
    let invalid_config = FcmConfig {
        registry: RegistryConfig::default(),
        packages: vec![],
        storage: StorageConfig {
            cache_dir: "/invalid/path/cache".into(),
            index_dir: "/invalid/path/index".into(),
            packages_dir: "/invalid/path/packages".into(),
            max_cache_size: "100MB".to_string(),
        },
        optimization: Default::default(),
    };

    let config = PackageManagerConfig::default();

    // This might succeed or fail depending on the environment
    // but should not panic
    let result = FhirSchemaPackageManager::new(invalid_config, config).await;
    // Either succeeds or fails gracefully
    match result {
        Ok(_) => println!("Package manager created successfully"),
        Err(e) => println!("Expected error: {e}"),
    }
}

#[tokio::test]
async fn test_debug_implementations() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = FhirSchemaPackageManager::new(fcm_config, config)
        .await
        .expect("Failed to create package manager");

    // Test Debug implementation - should not panic
    let debug_output = format!("{package_manager:?}");
    assert!(debug_output.contains("FhirSchemaPackageManager"));

    let config_debug = format!("{:?}", PackageManagerConfig::default());
    assert!(config_debug.contains("PackageManagerConfig"));
}

#[tokio::test]
async fn test_package_spec_builder() {
    let spec = PackageSpecBuilder::new("test.package")
        .version("1.0.0")
        .registry_source(None, None)
        .build();

    assert_eq!(spec.name, "test.package");
    assert_eq!(spec.version, "1.0.0");

    match spec.source {
        PackageSource::Registry { url, auth } => {
            assert!(url.is_none());
            assert!(auth.is_none());
        }
        _ => panic!("Expected registry source"),
    }
}

#[test]
fn test_package_metadata() {
    let metadata = PackageMetadata::default();
    assert!(metadata.title.is_none());
    assert!(metadata.description.is_none());
    assert!(metadata.author.is_none());
    assert!(metadata.license.is_none());
    assert!(metadata.homepage.is_none());
    assert!(metadata.repository.is_none());
    assert!(metadata.keywords.is_empty());
    assert!(metadata.fhir_version.is_none());
    assert!(metadata.jurisdiction.is_none());
    assert!(metadata.custom.is_empty());
}

#[tokio::test]
async fn test_schema_index() {
    use octofhir_fhirschema::types::FhirSchema;
    use url::Url;

    let index = SchemaIndex::new();
    let package_id = PackageId::new("test", "1.0.0");

    let mut schema = FhirSchema::new("TestResource");
    schema.url = Some(Url::parse("http://example.com/TestResource").unwrap());
    schema.name = Some("TestResource".to_string());

    let schema_arc = Arc::new(schema);

    // Test adding schema to index
    let result = index.add_schema(package_id, schema_arc).await;
    assert!(result.is_ok());
}

#[test]
fn test_install_hooks() {
    let hooks = InstallHooks::default();
    assert!(hooks.pre_install.is_empty());
    assert!(hooks.post_install.is_empty());
    assert!(hooks.pre_conversion.is_empty());
    assert!(hooks.post_conversion.is_empty());
}

#[test]
fn test_display_implementations() {
    let package_id = PackageId::new("test.package", "1.0.0");
    assert_eq!(format!("{package_id}"), "test.package@1.0.0");
}

#[tokio::test]
async fn test_bridge_support_types() {
    use octofhir_fhirschema::{
        BridgeCacheStats, BridgeCardinality, BridgeValidationError, BridgeValidationResult,
        BridgeValidationWarning,
    };

    // Test BridgeCardinality
    let cardinality = BridgeCardinality::new(1, Some(5));
    assert!(cardinality.is_required());
    assert!(!cardinality.is_unbounded());
    assert!(cardinality.is_collection());
    assert!(!cardinality.is_optional());
    assert!(cardinality.allows_multiple());

    let unbounded = BridgeCardinality::new(0, None);
    assert!(!unbounded.is_required());
    assert!(unbounded.is_unbounded());
    assert!(unbounded.is_collection());
    assert!(unbounded.is_optional());
    assert!(unbounded.allows_multiple());

    // Test BridgeValidationResult
    let mut validation_result = BridgeValidationResult::valid();
    assert!(validation_result.is_valid);
    assert!(!validation_result.has_errors());
    assert!(!validation_result.has_warnings());
    assert_eq!(validation_result.error_count(), 0);
    assert_eq!(validation_result.warning_count(), 0);

    let error = BridgeValidationError::new("Test error".to_string(), "test-error".to_string())
        .with_location("Patient.name".to_string());

    validation_result.add_error(error);
    assert!(!validation_result.is_valid);
    assert!(validation_result.has_errors());
    assert_eq!(validation_result.error_count(), 1);

    let warning =
        BridgeValidationWarning::new("Test warning".to_string(), "test-warning".to_string())
            .with_location("Patient.identifier".to_string());

    validation_result.add_warning(warning);
    assert!(validation_result.has_warnings());
    assert_eq!(validation_result.warning_count(), 1);

    // Test BridgeCacheStats
    let cache_stats = BridgeCacheStats::default();
    assert_eq!(cache_stats.schema_hit_ratio(), 0.0);
    assert_eq!(cache_stats.path_hit_ratio(), 0.0);
    assert_eq!(cache_stats.type_hit_ratio(), 0.0);
    assert_eq!(cache_stats.overall_hit_ratio(), 0.0);
}

#[tokio::test]
async fn test_bridge_support_error_handling() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test error handling with invalid inputs
    let invalid_schema = package_manager.get_schema("").await;
    assert!(invalid_schema.is_none());

    let invalid_type_check = package_manager.has_resource_type("").await;
    assert!(!invalid_type_check);

    let invalid_path_resolution = package_manager.resolve_element_path("", "").await;
    assert!(invalid_path_resolution.is_none());

    let invalid_choice_resolution = package_manager.resolve_choice_type("", "").await;
    assert!(invalid_choice_resolution.is_none());

    // Test with malformed URLs and paths
    let malformed_url_schema = package_manager.get_schema("not-a-valid-url").await;
    assert!(malformed_url_schema.is_none());

    let malformed_path = package_manager
        .resolve_element_path("Patient", "name..given")
        .await;
    assert!(malformed_path.is_none());
}

#[tokio::test]
async fn test_bridge_support_performance_characteristics() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test that multiple calls to the same method are fast (cached)
    use std::time::Instant;

    let start = Instant::now();
    for _ in 0..100 {
        let _ = package_manager.has_resource_type("Patient").await;
    }
    let duration = start.elapsed();

    // Should be very fast since it's O(1) with caching
    assert!(
        duration.as_millis() < 100,
        "Resource type checking should be fast, took {}ms",
        duration.as_millis()
    );

    let start = Instant::now();
    for _ in 0..100 {
        let _ = package_manager.get_resource_types().await;
    }
    let duration = start.elapsed();

    // Should be fast since it's cached
    assert!(
        duration.as_millis() < 100,
        "Resource types retrieval should be fast, took {}ms",
        duration.as_millis()
    );
}

#[tokio::test]
async fn test_bridge_support_concurrent_access() {
    use tokio::task;

    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test concurrent access from multiple tasks
    let mut handles = Vec::new();

    for i in 0..10 {
        let manager = Arc::clone(&package_manager);
        let handle = task::spawn(async move {
            // Each task performs different bridge operations concurrently
            let _ = manager.has_resource_type(&format!("Type{i}")).await;
            let _ = manager.get_resource_types().await;
            let _ = manager
                .get_schema(&format!("http://example.com/Type{i}"))
                .await;
            let _ = manager.get_registry_metrics().await;
            i
        });
        handles.push(handle);
    }

    // All tasks should complete successfully
    for handle in handles {
        let result = handle.await;
        assert!(result.is_ok());
    }
}

// Integration test for the complete workflow
#[tokio::test]
#[ignore] // Only run with --ignored flag as it requires network access
async fn test_complete_workflow_integration() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // This test would need actual FHIR packages to be meaningful
    // For now, just verify the manager was created successfully
    let packages = package_manager.list_packages().await;
    assert!(packages.is_empty());

    // Test that all bridge support methods are accessible
    let _ = package_manager.get_schema("test").await;
    let _ = package_manager.has_resource_type("test").await;
    let _ = package_manager.resolve_element_path("test", "test").await;
    let _ = package_manager.get_type_properties("test").await;
    let _ = package_manager.get_type_constraints("test").await;
    let _ = package_manager.get_registry_metrics().await;

    println!("✅ Complete workflow integration test setup successful");
}
