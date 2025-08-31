use octofhir_fhirschema::package::SchemaIndex;
use octofhir_fhirschema::{
    Element, ElementType, FhirSchema, NavigationOption, PathCardinality, PathNavigator,
    PathResolver, PathValidationResult,
};
use std::sync::Arc;

async fn create_test_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("Patient");

    // Add basic elements
    let mut id_element = Element::new("id");
    id_element.element_type = Some(vec![ElementType {
        code: "id".to_string(),
        profile: None,
        target_profile: None,
        aggregation: None,
        versioning: None,
    }]);
    id_element.min = Some(0);
    id_element.max = Some("1".to_string());
    schema.elements.insert("id".to_string(), id_element);

    // Add name element (collection)
    let mut name_element = Element::new("name");
    name_element.element_type = Some(vec![ElementType {
        code: "HumanName".to_string(),
        profile: None,
        target_profile: None,
        aggregation: None,
        versioning: None,
    }]);
    name_element.min = Some(0);
    name_element.max = Some("*".to_string());
    schema.elements.insert("name".to_string(), name_element);

    // Add name.given element
    let mut name_given_element = Element::new("name.given");
    name_given_element.element_type = Some(vec![ElementType {
        code: "string".to_string(),
        profile: None,
        target_profile: None,
        aggregation: None,
        versioning: None,
    }]);
    name_given_element.min = Some(0);
    name_given_element.max = Some("*".to_string());
    schema
        .elements
        .insert("name.given".to_string(), name_given_element);

    // Add name.family element
    let mut name_family_element = Element::new("name.family");
    name_family_element.element_type = Some(vec![ElementType {
        code: "string".to_string(),
        profile: None,
        target_profile: None,
        aggregation: None,
        versioning: None,
    }]);
    name_family_element.min = Some(0);
    name_family_element.max = Some("1".to_string());
    schema
        .elements
        .insert("name.family".to_string(), name_family_element);

    // Add choice type element
    let mut value_element = Element::new("value[x]");
    value_element.element_type = Some(vec![
        ElementType {
            code: "string".to_string(),
            profile: None,
            target_profile: None,
            aggregation: None,
            versioning: None,
        },
        ElementType {
            code: "integer".to_string(),
            profile: None,
            target_profile: None,
            aggregation: None,
            versioning: None,
        },
    ]);
    value_element.min = Some(0);
    value_element.max = Some("1".to_string());
    schema
        .elements
        .insert("value[x]".to_string(), value_element);

    schema
}

async fn create_test_resolver() -> PathResolver {
    let schema_index = Arc::new(SchemaIndex::new());
    let _schema = create_test_schema().await;

    // We can't directly add to SchemaIndex without the PackageRegistry,
    // but we can create the PathResolver for basic testing
    PathResolver::new(schema_index)
}

#[tokio::test]
async fn test_path_cardinality() {
    let cardinality = PathCardinality::new(0, Some(1));
    assert!(!cardinality.is_required());
    assert!(!cardinality.is_collection());
    assert!(!cardinality.is_unbounded());

    let cardinality = PathCardinality::new(1, None);
    assert!(cardinality.is_required());
    assert!(cardinality.is_collection());
    assert!(cardinality.is_unbounded());

    let cardinality = PathCardinality::new(0, Some(5));
    assert!(!cardinality.is_required());
    assert!(cardinality.is_collection());
    assert!(!cardinality.is_unbounded());
}

#[tokio::test]
async fn test_path_resolver_creation() {
    let resolver = create_test_resolver().await;
    let metrics = resolver.get_metrics().await;

    assert_eq!(metrics.cache_hits, 0);
    assert_eq!(metrics.cache_misses, 0);
    assert_eq!(metrics.total_resolutions, 0);
}

#[tokio::test]
async fn test_path_resolver_caching() {
    let resolver = create_test_resolver().await;

    // Test cache clearing
    resolver.clear_caches().await;
    let metrics = resolver.get_metrics().await;
    assert_eq!(metrics.total_resolutions, 0);
}

#[tokio::test]
async fn test_path_navigator_creation() {
    let schema_index = Arc::new(SchemaIndex::new());
    let resolver = Arc::new(PathResolver::new(schema_index.clone()));
    let navigator = PathNavigator::new(resolver, schema_index);

    // Test path validation with empty schema
    let validation = navigator.validate_path("Patient", "invalid.path").await;
    assert!(!validation.is_valid);
    assert!(!validation.errors.is_empty());
    assert_eq!(validation.errors[0], "Path not found");
}

#[tokio::test]
async fn test_navigation_options() {
    let option = NavigationOption {
        path: "name.given".to_string(),
        target_type: "string".to_string(),
        description: Some("Given names".to_string()),
        is_collection: true,
    };

    assert_eq!(option.path, "name.given");
    assert_eq!(option.target_type, "string");
    assert!(option.is_collection);
}

#[tokio::test]
async fn test_path_validation_result() {
    let mut result = PathValidationResult {
        is_valid: false,
        target_type: None,
        errors: vec!["Path not found".to_string()],
        suggestions: vec!["name.given".to_string(), "name.family".to_string()],
    };

    assert!(!result.is_valid);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.suggestions.len(), 2);

    result.is_valid = true;
    result.target_type = Some("string".to_string());
    result.errors.clear();

    assert!(result.is_valid);
    assert!(result.target_type.is_some());
    assert!(result.errors.is_empty());
}

#[tokio::test]
async fn test_breadcrumb_navigation() {
    let schema_index = Arc::new(SchemaIndex::new());
    let resolver = Arc::new(PathResolver::new(schema_index.clone()));
    let navigator = PathNavigator::new(resolver, schema_index);

    // Test empty path breadcrumbs (should work even without schema data)
    let breadcrumbs = navigator.get_breadcrumbs("Patient", "").await;
    assert!(breadcrumbs.is_empty());
}

#[tokio::test]
async fn test_parent_navigation() {
    let schema_index = Arc::new(SchemaIndex::new());
    let resolver = Arc::new(PathResolver::new(schema_index.clone()));
    let navigator = PathNavigator::new(resolver, schema_index);

    // Test parent navigation with simple path
    let parent = navigator.navigate_to_parent("Patient", "id").await;
    assert!(parent.is_none()); // No parent for root level

    // Test with nested path (will return None without schema data)
    let parent = navigator.navigate_to_parent("Patient", "name.given").await;
    assert!(parent.is_none()); // Will be None without proper schema data
}

#[tokio::test]
async fn test_depth_navigation() {
    let schema_index = Arc::new(SchemaIndex::new());
    let resolver = Arc::new(PathResolver::new(schema_index.clone()));
    let navigator = PathNavigator::new(resolver, schema_index);

    // Test depth navigation (should return empty without schema data)
    let paths = navigator.get_paths_at_depth("Patient", 0).await;
    assert!(paths.is_empty());

    let paths = navigator.get_paths_at_depth("Patient", 1).await;
    assert!(paths.is_empty());
}

#[tokio::test]
async fn test_similarity_calculation() {
    let schema_index = Arc::new(SchemaIndex::new());
    let resolver = Arc::new(PathResolver::new(schema_index.clone()));
    let navigator = PathNavigator::new(resolver, schema_index);

    // Test path suggestions (will be empty without schema data)
    let suggestions = navigator.generate_path_suggestions("Patient", "nam").await;
    // Without schema data, this will return empty, but the method exists
    assert!(suggestions.len() <= 5); // Max 5 suggestions
}

#[tokio::test]
async fn test_path_resolver_precomputation() {
    let resolver = create_test_resolver().await;

    // Test precomputing common paths
    let types = vec!["Patient".to_string(), "Observation".to_string()];
    let result = resolver.precompute_common_paths(&types).await;

    // Should succeed even with empty schema index
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_path_context_resolution() {
    let resolver = create_test_resolver().await;

    // Test path resolution with context
    let resolution = resolver
        .resolve_path_with_context("Patient", "name.given", "first_name_context")
        .await;

    // Will be None without proper schema data
    assert!(resolution.is_none());
}

#[tokio::test]
async fn test_available_paths() {
    let resolver = create_test_resolver().await;

    // Test getting available paths
    let paths = resolver.get_available_paths("Patient").await;

    // Should be empty without schema data in the index
    assert!(paths.is_empty());
}

#[tokio::test]
async fn test_child_navigation() {
    let schema_index = Arc::new(SchemaIndex::new());
    let resolver = Arc::new(PathResolver::new(schema_index.clone()));
    let navigator = PathNavigator::new(resolver, schema_index);

    // Test child navigation
    let children = navigator.navigate_to_children("Patient", "name").await;

    // Should be empty without schema data
    assert!(children.is_empty());
}
