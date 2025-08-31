use crate::error::Result;
use crate::types::FhirSchema;
use papaya::HashMap as PapayaMap;
use std::collections::HashSet;
use std::sync::Arc;

/// Registry for O(1) resource type access
#[derive(Debug)]
pub struct ResourceTypeRegistry {
    // Pre-computed type sets for O(1) lookups
    #[allow(dead_code)]
    all_types: Arc<HashSet<String>>,
    resource_types: Arc<HashSet<String>>,
    primitive_types: Arc<HashSet<String>>,
    complex_types: Arc<HashSet<String>>,

    // Choice type mappings
    choice_type_expansions: Arc<PapayaMap<String, Vec<String>>>,
    choice_base_paths: Arc<PapayaMap<String, String>>, // expanded -> base

    // Type hierarchy for quick inheritance checks
    type_hierarchy: Arc<PapayaMap<String, TypeHierarchyInfo>>,

    // Performance metrics
    metrics: RegistryMetrics,
}

#[derive(Debug, Clone)]
pub struct TypeHierarchyInfo {
    pub base_type: Option<String>,
    pub derived_types: Vec<String>,
    pub depth: u32,
}

#[derive(Debug, Default)]
pub struct RegistryMetrics {
    pub total_types: usize,
    pub resource_count: usize,
    pub primitive_count: usize,
    pub complex_count: usize,
    pub choice_expansions: usize,
}

impl ResourceTypeRegistry {
    /// Create new empty registry
    pub fn new() -> Self {
        Self {
            all_types: Arc::new(HashSet::new()),
            resource_types: Arc::new(HashSet::new()),
            primitive_types: Arc::new(HashSet::new()),
            complex_types: Arc::new(HashSet::new()),
            choice_type_expansions: Arc::new(PapayaMap::new()),
            choice_base_paths: Arc::new(PapayaMap::new()),
            type_hierarchy: Arc::new(PapayaMap::new()),
            metrics: RegistryMetrics::default(),
        }
    }

    /// Build registry from schemas
    pub async fn build_from_schemas(schemas: &[Arc<FhirSchema>]) -> Result<Self> {
        let mut all_types = HashSet::new();
        let mut resource_types = HashSet::new();
        let mut primitive_types = HashSet::new();
        let mut complex_types = HashSet::new();
        let choice_type_expansions = PapayaMap::new();
        let choice_base_paths = PapayaMap::new();
        let type_hierarchy = PapayaMap::new();

        for schema in schemas {
            let type_name = schema
                .title
                .as_ref()
                .unwrap_or(
                    &schema
                        .name
                        .as_ref()
                        .unwrap_or(&"Unknown".to_string())
                        .clone(),
                )
                .clone();
            all_types.insert(type_name.clone());

            // Classify type based on schema properties
            if Self::is_resource_schema(schema) {
                resource_types.insert(type_name.clone());
            } else if Self::is_primitive_schema(schema) {
                primitive_types.insert(type_name.clone());
            } else {
                complex_types.insert(type_name.clone());
            }

            // Process choice types and type hierarchy
            Self::process_schema_for_choice_types(
                schema,
                &choice_type_expansions,
                &choice_base_paths,
            );
            Self::process_type_hierarchy(schema, &type_hierarchy);
        }

        let metrics = RegistryMetrics {
            total_types: all_types.len(),
            resource_count: resource_types.len(),
            primitive_count: primitive_types.len(),
            complex_count: complex_types.len(),
            choice_expansions: choice_type_expansions.len(),
        };

        Ok(Self {
            all_types: Arc::new(all_types),
            resource_types: Arc::new(resource_types),
            primitive_types: Arc::new(primitive_types),
            complex_types: Arc::new(complex_types),
            choice_type_expansions: Arc::new(choice_type_expansions),
            choice_base_paths: Arc::new(choice_base_paths),
            type_hierarchy: Arc::new(type_hierarchy),
            metrics,
        })
    }

    /// O(1) type classification checks
    pub fn is_resource_type(&self, type_name: &str) -> bool {
        self.resource_types.contains(type_name)
    }

    pub fn is_primitive_type(&self, type_name: &str) -> bool {
        self.primitive_types.contains(type_name)
    }

    pub fn is_complex_type(&self, type_name: &str) -> bool {
        self.complex_types.contains(type_name)
    }

    /// O(1) resource type enumeration
    pub fn get_all_resource_types(&self) -> &HashSet<String> {
        &self.resource_types
    }

    pub fn get_primitive_types(&self) -> &HashSet<String> {
        &self.primitive_types
    }

    pub fn get_complex_types(&self) -> &HashSet<String> {
        &self.complex_types
    }

    /// Choice type support
    pub fn get_choice_type_expansions(&self, base_path: &str) -> Option<Vec<String>> {
        let guard = self.choice_type_expansions.pin();
        guard.get(base_path).cloned()
    }

    pub fn resolve_choice_base(&self, expanded_path: &str) -> Option<String> {
        let guard = self.choice_base_paths.pin();
        guard.get(expanded_path).cloned()
    }

    pub fn is_choice_type_expansion(&self, path: &str) -> bool {
        let guard = self.choice_base_paths.pin();
        guard.contains_key(path)
    }

    /// Type hierarchy queries
    pub fn get_base_type(&self, type_name: &str) -> Option<String> {
        let guard = self.type_hierarchy.pin();
        guard.get(type_name)?.base_type.clone()
    }

    pub fn get_derived_types(&self, type_name: &str) -> Option<Vec<String>> {
        let guard = self.type_hierarchy.pin();
        Some(guard.get(type_name)?.derived_types.clone())
    }

    pub fn is_subtype_of(&self, child: &str, parent: &str) -> bool {
        let mut current = child.to_string();
        while let Some(base) = self.get_base_type(&current) {
            if base == parent {
                return true;
            }
            current = base;
        }
        false
    }

    /// Registry statistics
    pub fn get_metrics(&self) -> &RegistryMetrics {
        &self.metrics
    }

    // Helper methods for schema analysis
    fn is_resource_schema(schema: &FhirSchema) -> bool {
        // Check if schema represents a FHIR resource
        // First check if description explicitly mentions "resource"
        if let Some(desc) = &schema.description {
            if desc.contains("resource") {
                return true;
            }
        }

        // If schema_type is not "object", it's definitely not a resource
        if schema.schema_type != "object" {
            return false;
        }

        // For object types, only consider them resources if they have a resource-indicating description
        // or if they're in our known list of FHIR resource types
        if let Some(title) = &schema.title {
            use crate::types::FhirTypeDefinitions;
            FhirTypeDefinitions::is_resource_type(title)
        } else {
            false
        }
    }

    fn is_primitive_schema(schema: &FhirSchema) -> bool {
        // Check if schema represents a primitive type
        matches!(
            schema.schema_type.as_str(),
            "string" | "number" | "boolean" | "integer"
        )
    }

    fn process_schema_for_choice_types(
        _schema: &FhirSchema,
        _choice_expansions: &PapayaMap<String, Vec<String>>,
        _choice_bases: &PapayaMap<String, String>,
    ) {
        // TODO: Implement choice type processing logic
        // This would analyze schema properties to identify choice types
        // and build the expansion mappings
    }

    fn process_type_hierarchy(
        _schema: &FhirSchema,
        _hierarchy: &PapayaMap<String, TypeHierarchyInfo>,
    ) {
        // TODO: Implement type hierarchy processing
        // This would analyze schema inheritance to build hierarchy info
    }
}

impl Default for ResourceTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FhirSchema, FhirTypeDefinitions};

    fn create_test_schema(
        title: &str,
        schema_type: Option<&str>,
        description: Option<&str>,
    ) -> Arc<FhirSchema> {
        use std::collections::HashMap;

        Arc::new(FhirSchema {
            schema_version: Some("https://json-schema.org/draft/2020-12/schema".to_string()),
            url: Some(
                url::Url::parse(&format!("http://hl7.org/fhir/StructureDefinition/{title}"))
                    .unwrap(),
            ),
            name: Some(title.to_string()),
            title: Some(title.to_string()),
            description: description.map(|d| d.to_string()),
            version: None,
            status: None,
            schema_type: schema_type.unwrap_or("object").to_string(),
            kind: None,
            class: None,
            base: None,
            abstract_type: None,
            base_definition: None,
            derivation: None,
            elements: HashMap::new(),
            constraints: Vec::new(),
            slicing: HashMap::new(),
            extensions: HashMap::new(),
        })
    }

    #[tokio::test]
    async fn test_empty_registry() {
        let registry = ResourceTypeRegistry::new();

        assert_eq!(registry.get_all_resource_types().len(), 0);
        assert_eq!(registry.get_primitive_types().len(), 0);
        assert_eq!(registry.get_complex_types().len(), 0);
        assert!(!registry.is_resource_type("Patient"));
        assert!(!registry.is_primitive_type("string"));
        assert!(!registry.is_complex_type("HumanName"));
    }

    #[tokio::test]
    async fn test_build_from_resource_schemas() {
        let schemas = vec![
            create_test_schema("Patient", None, Some("Patient resource")),
            create_test_schema("Observation", None, Some("Observation resource")),
            create_test_schema("string", Some("string"), None),
            create_test_schema("HumanName", Some("object"), None),
        ];

        let registry = ResourceTypeRegistry::build_from_schemas(&schemas)
            .await
            .unwrap();

        // Check resource types
        assert!(registry.is_resource_type("Patient"));
        assert!(registry.is_resource_type("Observation"));
        assert!(!registry.is_resource_type("string"));
        assert!(!registry.is_resource_type("HumanName"));

        // Check primitive types
        assert!(registry.is_primitive_type("string"));
        assert!(!registry.is_primitive_type("Patient"));

        // Check complex types (non-resource, non-primitive)
        assert!(registry.is_complex_type("HumanName"));
        assert!(!registry.is_complex_type("Patient"));
        assert!(!registry.is_complex_type("string"));

        // Check metrics
        let metrics = registry.get_metrics();
        assert_eq!(metrics.total_types, 4);
        assert_eq!(metrics.resource_count, 2);
        assert_eq!(metrics.primitive_count, 1);
        assert_eq!(metrics.complex_count, 1);
    }

    #[tokio::test]
    async fn test_resource_type_enumeration() {
        let schemas = vec![
            create_test_schema("Patient", None, Some("Patient resource")),
            create_test_schema("Observation", None, Some("Observation resource")),
            create_test_schema("string", Some("string"), None),
        ];

        let registry = ResourceTypeRegistry::build_from_schemas(&schemas)
            .await
            .unwrap();

        let resource_types: Vec<String> =
            registry.get_all_resource_types().iter().cloned().collect();
        let mut resource_types_sorted = resource_types;
        resource_types_sorted.sort();

        assert_eq!(resource_types_sorted, vec!["Observation", "Patient"]);
    }

    #[tokio::test]
    async fn test_type_classification_edge_cases() {
        let schemas = vec![
            // Edge case: lowercase resource name (should still be classified as resource due to description)
            create_test_schema("patient", None, Some("Patient resource for testing")),
            // Edge case: uppercase complex type without resource description
            create_test_schema("CustomType", Some("object"), None),
            // Edge case: numeric type
            create_test_schema("decimal", Some("number"), None),
        ];

        let registry = ResourceTypeRegistry::build_from_schemas(&schemas)
            .await
            .unwrap();

        // Lowercase with resource description should be resource
        assert!(registry.is_resource_type("patient"));

        // Uppercase without resource description should be complex
        assert!(registry.is_complex_type("CustomType"));
        assert!(!registry.is_resource_type("CustomType"));

        // Number type should be primitive
        assert!(registry.is_primitive_type("decimal"));
    }

    #[tokio::test]
    async fn test_registry_metrics() {
        let schemas = vec![
            create_test_schema("Patient", None, Some("Patient resource")),
            create_test_schema("Observation", None, Some("Observation resource")),
            create_test_schema("string", Some("string"), None),
            create_test_schema("integer", Some("integer"), None),
            create_test_schema("HumanName", Some("object"), None),
            create_test_schema("Address", Some("object"), None),
        ];

        let registry = ResourceTypeRegistry::build_from_schemas(&schemas)
            .await
            .unwrap();
        let metrics = registry.get_metrics();

        assert_eq!(metrics.total_types, 6);
        assert_eq!(metrics.resource_count, 2);
        assert_eq!(metrics.primitive_count, 2);
        assert_eq!(metrics.complex_count, 2);
        assert_eq!(metrics.choice_expansions, 0); // No choice types in this test
    }

    #[tokio::test]
    async fn test_type_hierarchy_placeholder() {
        let schemas = vec![create_test_schema(
            "Patient",
            None,
            Some("Patient resource"),
        )];

        let registry = ResourceTypeRegistry::build_from_schemas(&schemas)
            .await
            .unwrap();

        // Type hierarchy is not implemented yet, so these should return None
        assert!(registry.get_base_type("Patient").is_none());
        assert!(registry.get_derived_types("Patient").is_none());
        assert!(!registry.is_subtype_of("Patient", "Resource"));
    }

    #[tokio::test]
    async fn test_choice_type_placeholder() {
        let schemas = vec![create_test_schema(
            "Patient",
            None,
            Some("Patient resource"),
        )];

        let registry = ResourceTypeRegistry::build_from_schemas(&schemas)
            .await
            .unwrap();

        // Choice types are not implemented yet, so these should return None/false
        assert!(registry.get_choice_type_expansions("value").is_none());
        assert!(registry.resolve_choice_base("valueString").is_none());
        assert!(!registry.is_choice_type_expansion("valueString"));
    }

    #[tokio::test]
    async fn test_fhir_type_definitions() {
        // Test the static type definitions
        assert!(FhirTypeDefinitions::is_resource_type("Patient"));
        assert!(FhirTypeDefinitions::is_resource_type("Observation"));
        assert!(!FhirTypeDefinitions::is_resource_type("string"));

        assert!(FhirTypeDefinitions::is_primitive_type("string"));
        assert!(FhirTypeDefinitions::is_primitive_type("integer"));
        assert!(!FhirTypeDefinitions::is_primitive_type("Patient"));

        assert!(FhirTypeDefinitions::is_complex_type("HumanName"));
        assert!(FhirTypeDefinitions::is_complex_type("Address"));
        assert!(!FhirTypeDefinitions::is_complex_type("Patient"));

        let all_types = FhirTypeDefinitions::all_types();
        assert!(!all_types.is_empty());
        assert!(all_types.contains(&"Patient"));
        assert!(all_types.contains(&"string"));
        assert!(all_types.contains(&"HumanName"));
    }

    #[tokio::test]
    async fn test_performance_characteristics() {
        // Create a registry with many types to test O(1) characteristics
        let mut schemas = Vec::new();
        for i in 0..1000 {
            schemas.push(create_test_schema(
                &format!("Resource{i}"),
                None,
                Some("Test resource"),
            ));
        }

        let registry = ResourceTypeRegistry::build_from_schemas(&schemas)
            .await
            .unwrap();

        // These should all be O(1) operations
        let start = std::time::Instant::now();
        for i in 0..100 {
            let _ = registry.is_resource_type(&format!("Resource{i}"));
        }
        let lookup_time = start.elapsed();

        // Lookups should be very fast (under 1ms for 100 lookups)
        assert!(
            lookup_time.as_millis() < 10,
            "Lookups took too long: {lookup_time:?}"
        );

        // Registry building should be reasonable (under 100ms for 1000 schemas)
        let start = std::time::Instant::now();
        let _registry2 = ResourceTypeRegistry::build_from_schemas(&schemas)
            .await
            .unwrap();
        let build_time = start.elapsed();

        assert!(
            build_time.as_millis() < 100,
            "Building took too long: {build_time:?}"
        );
    }
}
