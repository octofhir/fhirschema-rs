# Implementing ModelProvider with FhirSchema Bridge

This document shows how to implement ModelProvider traits in bridge libraries using FhirSchema's bridge support functions.

## Architecture Overview

FhirSchema now provides comprehensive bridge support functions that external libraries can use to implement domain-specific ModelProvider traits. This approach provides:

- **Clean Separation**: FhirSchema focuses on core schema management
- **Flexibility**: Bridge libraries implement domain-specific interfaces  
- **Performance**: Direct access to optimized FhirSchema functions with O(1) operations
- **Maintainability**: Clear responsibilities for each library
- **Extensibility**: Easy to add new bridge types for different use cases

## Available Bridge Support Functions

The `FhirSchemaPackageManager` provides comprehensive methods for external integrations:

### Schema Access Methods
- `get_schema(canonical_url)` - O(1) schema lookup
- `get_schemas_by_type(resource_type)` - Get all schemas for a type
- `get_schema_by_type(type_name)` - Get first schema for a type
- `resolve_profile(base_type, profile_url)` - Profile resolution
- `search_schemas(query)` - Full-text schema search

### Resource Type Methods
- `has_resource_type(resource_type)` - O(1) type checking
- `get_resource_types()` - Get all known resource types
- `is_primitive_type(type_name)` - Check primitive types
- `is_complex_type(type_name)` - Check complex types
- `get_base_type(type_name)` - Get inheritance hierarchy
- `is_subtype_of(child, parent)` - Inheritance checking

### Choice Type Methods
- `get_choice_type_options(base_path)` - Get choice expansions
- `resolve_choice_type(base_path, value_type)` - Choice resolution
- `is_choice_type_expansion(path)` - Check if path is choice expansion
- `get_choice_type_base(expanded_path)` - Get base path from expansion

### Path Resolution Methods
- `resolve_element_path(base_type, path)` - Element resolution
- `get_element_cardinality(type_name, path)` - Cardinality info
- `has_element_path(type_name, path)` - Path existence check
- `get_available_paths(type_name)` - Available paths for auto-completion
- `get_element_type(base_type, path)` - Element type resolution

### Type Reflection Methods
- `get_type_properties(type_name)` - All properties for a type
- `get_property_info(type_name, property)` - Single property info
- `get_type_definition(type_name)` - Type definition details

### Constraint Methods  
- `get_type_constraints(type_name)` - Type-level constraints
- `get_element_constraints(type_name, path)` - Element constraints
- `validate_constraint_expression(expression)` - Expression validation

### Utility Methods
- `get_registry_metrics()` - Registry statistics
- `get_path_resolver_metrics()` - Path resolution metrics
- `rebuild_indexes()` - Refresh caches and indexes

## Example: FHIRPath ModelProvider Implementation

Here's how to implement a ModelProvider for FHIRPath using the bridge support functions:

```rust
// In fhirpath-fhirschema-bridge crate

use fhirpath::ModelProvider;
use octofhir_fhirschema::FhirSchemaPackageManager;
use octofhir_fhirschema::{PropertyInfo, BridgeCardinality, ElementInfo};
use async_trait::async_trait;
use std::sync::Arc;

pub struct FhirSchemaModelProvider {
    schema_manager: Arc<FhirSchemaPackageManager>,
}

impl FhirSchemaModelProvider {
    pub fn new(schema_manager: Arc<FhirSchemaPackageManager>) -> Self {
        Self { schema_manager }
    }
}

#[async_trait]
impl ModelProvider for FhirSchemaModelProvider {
    async fn get_schema(&self, canonical_url: &str) -> Option<Arc<FhirSchema>> {
        self.schema_manager.get_schema(canonical_url).await
    }

    async fn get_schemas_by_type(&self, resource_type: &str) -> Vec<Arc<FhirSchema>> {
        self.schema_manager.get_schemas_by_type(resource_type).await
    }

    async fn resolve_profile(&self, base_type: &str, profile_url: &str) -> Option<Arc<FhirSchema>> {
        self.schema_manager.resolve_profile(base_type, profile_url).await
    }

    async fn has_resource_type(&self, resource_type: &str) -> bool {
        self.schema_manager.has_resource_type(resource_type).await
    }

    async fn get_resource_types(&self) -> Vec<String> {
        self.schema_manager.get_resource_types().await
    }

    async fn search_schemas(&self, query: &str) -> Vec<Arc<FhirSchema>> {
        self.schema_manager.search_schemas(query).await
    }

    // Enhanced FHIRPath-specific methods
    async fn get_choice_type_options(&self, base_path: &str) -> Vec<String> {
        self.schema_manager.get_choice_type_options(base_path).await
    }

    async fn resolve_choice_type(&self, base_path: &str, value_type: &str) -> Option<String> {
        self.schema_manager.resolve_choice_type(base_path, value_type).await
    }

    async fn resolve_element_path(&self, base_type: &str, path: &str) -> Option<ElementInfo> {
        self.schema_manager.resolve_element_path(base_type, path).await
    }

    async fn get_element_cardinality(&self, type_name: &str, path: &str) -> Option<BridgeCardinality> {
        self.schema_manager.get_element_cardinality(type_name, path).await
    }

    async fn has_element_path(&self, type_name: &str, path: &str) -> bool {
        self.schema_manager.has_element_path(type_name, path).await
    }

    async fn get_available_paths(&self, type_name: &str) -> Vec<String> {
        self.schema_manager.get_available_paths(type_name).await
    }

    async fn get_element_type(&self, base_type: &str, path: &str) -> Option<String> {
        self.schema_manager.get_element_type(base_type, path).await
    }

    async fn get_type_properties(&self, type_name: &str) -> Vec<PropertyInfo> {
        self.schema_manager.get_type_properties(type_name).await
    }

    async fn is_primitive_type(&self, type_name: &str) -> bool {
        self.schema_manager.is_primitive_type(type_name).await
    }

    async fn is_complex_type(&self, type_name: &str) -> bool {
        self.schema_manager.is_complex_type(type_name).await
    }

    async fn get_base_type(&self, type_name: &str) -> Option<String> {
        self.schema_manager.get_base_type(type_name).await
    }

    async fn is_subtype_of(&self, child_type: &str, parent_type: &str) -> bool {
        self.schema_manager.is_subtype_of(child_type, parent_type).await
    }
}
```

## Example: Validation Engine ModelProvider

For validation engines that need constraint information:

```rust
use validation_engine::ModelProvider;
use octofhir_fhirschema::{FhirSchemaPackageManager, BridgeConstraintInfo};

pub struct ValidationModelProvider {
    schema_manager: Arc<FhirSchemaPackageManager>,
}

#[async_trait]
impl ModelProvider for ValidationModelProvider {
    // ... basic methods same as above ...

    async fn get_type_constraints(&self, type_name: &str) -> Vec<BridgeConstraintInfo> {
        self.schema_manager.get_type_constraints(type_name).await
    }

    async fn get_element_constraints(&self, type_name: &str, path: &str) -> Vec<BridgeConstraintInfo> {
        self.schema_manager.get_element_constraints(type_name, path).await
    }

    async fn validate_constraint_expression(&self, expression: &str) -> ValidationResult {
        self.schema_manager.validate_constraint_expression(expression).await
    }

    async fn get_property_info(&self, type_name: &str, property: &str) -> Option<PropertyInfo> {
        self.schema_manager.get_property_info(type_name, property).await
    }
}
```

## Bridge Support Types

FhirSchema provides specialized types for bridge support:

### PropertyInfo
```rust
pub struct PropertyInfo {
    pub name: String,
    pub element_type: String,
    pub cardinality: BridgeCardinality,
    pub is_collection: bool,
    pub is_required: bool,
    pub is_choice_type: bool,
    pub definition: Option<String>,
}
```

### BridgeCardinality
```rust  
pub struct BridgeCardinality {
    pub min: u32,
    pub max: Option<u32>, // None for unbounded (*)
}

impl BridgeCardinality {
    pub fn is_required(&self) -> bool;
    pub fn is_unbounded(&self) -> bool;
    pub fn is_collection(&self) -> bool;
    pub fn is_optional(&self) -> bool;
    pub fn allows_multiple(&self) -> bool;
}
```

### BridgeConstraintInfo
```rust
pub struct BridgeConstraintInfo {
    pub key: String,
    pub severity: String,
    pub human_description: String,
    pub fhirpath_expression: String,
    pub source: Option<String>,
    pub xpath: Option<String>,
    pub requires_fhirpath: bool,
}
```

### BridgeValidationResult
```rust
pub struct BridgeValidationResult {
    pub is_valid: bool,
    pub errors: Vec<BridgeValidationError>,
    pub warnings: Vec<BridgeValidationWarning>,
    pub metrics: Option<BridgeValidationMetrics>,
}
```

## Performance Characteristics

The bridge support functions are optimized for performance:

- **O(1) Operations**: Schema lookup, resource type checking, primitive type checking
- **Cached Results**: Path resolution, choice type expansions, type hierarchies  
- **Batch Operations**: Multiple schema queries, bulk constraint validation
- **Memory Efficient**: Shared references, lazy loading, compressed storage
- **Concurrent Access**: Thread-safe read operations, minimal locking

## Best Practices

### 1. Error Handling
Always handle missing schemas gracefully:

```rust
async fn get_element_info(&self, type_name: &str, path: &str) -> Result<ElementInfo> {
    match self.schema_manager.resolve_element_path(type_name, path).await {
        Some(info) => Ok(info),
        None => {
            // Try alternative lookups or return appropriate error
            Err(ModelError::ElementNotFound { 
                type_name: type_name.to_string(), 
                path: path.to_string() 
            })
        }
    }
}
```

### 2. Caching
Leverage FhirSchema's internal caching by avoiding redundant calls:

```rust
// Good: Single call gets all properties
let properties = self.schema_manager.get_type_properties("Patient").await;

// Avoid: Multiple individual property calls
// for property in property_names {
//     let info = self.schema_manager.get_property_info("Patient", property).await;
// }
```

### 3. Choice Type Handling
Use the specialized choice type methods:

```rust
async fn resolve_path(&self, base_type: &str, path: &str) -> Option<String> {
    // Check if this is a choice type first
    if path.contains("[x]") {
        let options = self.schema_manager.get_choice_type_options(path).await;
        if !options.is_empty() {
            return Some(options[0].clone()); // or apply selection logic
        }
    }
    
    // Regular path resolution
    self.schema_manager.resolve_element_path(base_type, path)
        .await
        .map(|info| info.path)
}
```

### 4. Metrics and Monitoring
Use the metrics functions for monitoring:

```rust
async fn get_performance_info(&self) -> BridgePerformanceInfo {
    let registry_metrics = self.schema_manager.get_registry_metrics().await;
    let path_metrics = self.schema_manager.get_path_resolver_metrics().await;
    
    BridgePerformanceInfo {
        total_schemas: registry_metrics.total_schemas,
        cache_hit_ratio: registry_metrics.cache_stats.overall_hit_ratio(),
        path_resolution_time: path_metrics.average_resolution_time_ms,
    }
}
```

## Migration Guide

### From Direct ModelProvider Implementation

If you previously implemented ModelProvider directly using FhirSchema internals:

1. **Replace direct trait implementation** with bridge support functions
2. **Update method signatures** to use bridge support types
3. **Remove internal FhirSchema dependencies** from your bridge library
4. **Test performance** - bridge support should be equivalent or faster

### Example Migration

**Before:**
```rust
// Old direct implementation
async fn get_schema(&self, url: &str) -> Option<Arc<FhirSchema>> {
    self.schema_manager.registry.read().await
        .schema_index.by_canonical_url.get(url).cloned()
}
```

**After:**
```rust
// New bridge support function
async fn get_schema(&self, url: &str) -> Option<Arc<FhirSchema>> {
    self.schema_manager.get_schema(url).await
}
```

## Testing Bridge Implementations

Comprehensive test suite for bridge implementations:

```rust
#[tokio::test]
async fn test_bridge_implementation() {
    let manager = create_test_manager().await;
    let provider = FhirSchemaModelProvider::new(manager);
    
    // Test basic schema access
    let schema = provider.get_schema("http://hl7.org/fhir/StructureDefinition/Patient").await;
    assert!(schema.is_some());
    
    // Test resource type checking
    assert!(provider.has_resource_type("Patient").await);
    assert!(!provider.has_resource_type("NonExistentType").await);
    
    // Test choice type resolution
    let choices = provider.get_choice_type_options("value[x]").await;
    assert!(!choices.is_empty());
    
    let resolved = provider.resolve_choice_type("value[x]", "string").await;
    assert_eq!(resolved, Some("valueString".to_string()));
    
    // Test path resolution
    let element = provider.resolve_element_path("Patient", "name.given").await;
    assert!(element.is_some());
    
    // Test cardinality
    let cardinality = provider.get_element_cardinality("Patient", "identifier").await;
    assert!(cardinality.is_some());
    assert!(cardinality.unwrap().allows_multiple());
    
    // Test type properties
    let properties = provider.get_type_properties("Patient").await;
    assert!(!properties.is_empty());
    assert!(properties.iter().any(|p| p.name.contains("identifier")));
}
```

## Conclusion

The bridge support functions provide a clean, performant way to implement ModelProvider traits using FhirSchema. This architecture separates concerns while maintaining high performance and providing comprehensive functionality for FHIR schema operations.