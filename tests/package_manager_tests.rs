use octofhir_canonical_manager::{FcmConfig, RegistryConfig, StorageConfig};
use octofhir_fhirschema::package::{
    ConversionOptions, InstallHooks, PackageMetadata, PackageSource, PackageSpecBuilder,
    SchemaIndex, SimpleProgressTracker,
};
use octofhir_fhirschema::{
    ConversionPipeline, FhirSchemaPackageManager, InstallOptions, ModelProvider, PackageId,
    PackageManagerConfig, PackageSpec, ProgressTracker,
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
    assert_eq!(options.max_parallel, 4);
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
async fn test_model_provider_interface() {
    let fcm_config = create_test_fcm_config();
    let config = PackageManagerConfig::default();

    let package_manager = Arc::new(
        FhirSchemaPackageManager::new(fcm_config, config)
            .await
            .expect("Failed to create package manager"),
    );

    // Test ModelProvider methods
    let schema = package_manager
        .get_schema("http://example.com/NonExistentSchema")
        .await;
    assert!(schema.is_none());

    let schemas = package_manager.get_schemas_by_type("Patient").await;
    assert!(schemas.is_empty());

    let has_type = package_manager.has_resource_type("Patient").await;
    assert!(!has_type);

    let resource_types = package_manager.get_resource_types().await;
    assert!(resource_types.is_empty());

    let search_results = package_manager.search_schemas("patient").await;
    assert!(search_results.is_empty());
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

    println!("✅ Complete workflow integration test setup successful");
}
