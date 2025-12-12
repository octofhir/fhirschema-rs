//! FhirSchemaModelProvider implementation for FHIRPath evaluation
//!
//! This implementation provides the production-ready ModelProvider using FHIR schemas
//! with schema-driven type checking following FHIR schema patterns.

use async_trait::async_trait;
use std::collections::HashMap;

use octofhir_fhir_model::{
    Result as ModelResult,
    provider::{ElementInfo, FhirVersion as ModelFhirVersion, ModelProvider, TypeInfo},
};

use crate::types::FhirSchema;

/// Navigation result for testing purposes
#[derive(Debug)]
pub struct NavigationResult {
    pub success: bool,
    pub result_type: Option<TypeInfo>,
}

/// FHIR to FHIRPath type mapping - essential for type conversion
const TYPE_MAPPING: &[(&str, &str)] = &[
    ("boolean", "Boolean"),
    ("integer", "Integer"),
    ("string", "String"),
    ("decimal", "Decimal"),
    ("uri", "String"),
    ("url", "String"),
    ("canonical", "String"),
    ("base64Binary", "String"),
    ("instant", "DateTime"),
    ("date", "Date"),
    ("dateTime", "DateTime"),
    ("time", "Time"),
    ("code", "String"),
    ("oid", "String"),
    ("id", "String"),
    ("markdown", "String"),
    ("unsignedInt", "Integer"),
    ("positiveInt", "Integer"),
    ("uuid", "String"),
    ("xhtml", "String"),
    ("Quantity", "Quantity"),
    ("SimpleQuantity", "Quantity"),
    ("Money", "Quantity"),
    ("Duration", "Quantity"),
    ("Age", "Quantity"),
    ("Distance", "Quantity"),
    ("Count", "Quantity"),
    // FHIRPath specific types
    ("Any", "Any"),
];

/// Production-ready FhirSchemaModelProvider with schema-driven functionality
#[derive(Debug)]
pub struct FhirSchemaModelProvider {
    schemas: HashMap<String, FhirSchema>,
    type_mapping: HashMap<String, String>,
    fhir_version: ModelFhirVersion,
    /// URL to schema name mapping for O(1) lookup by URL
    url_to_name: HashMap<String, String>,
    /// Reverse mapping for FHIRPath types back to FHIR types
    reverse_type_mapping: HashMap<String, String>,
}

impl FhirSchemaModelProvider {
    /// Get the nested elements of a backbone element by parent type and element path.
    ///
    /// Backbone elements in FHIR schemas have their nested elements stored inline
    /// within the parent schema (not as separate top-level schemas). This method
    /// navigates into the schema structure to extract those nested elements.
    ///
    /// # Arguments
    /// * `parent_type` - The parent type name (e.g., "Task", "Patient")
    /// * `element_path` - Dot-separated path to the backbone element (e.g., "input", "contact.telecom")
    ///
    /// # Returns
    /// A vector of ElementInfo for all children of the backbone element
    pub fn get_backbone_element_children(
        &self,
        parent_type: &str,
        element_path: &str,
    ) -> Vec<octofhir_fhir_model::provider::ElementInfo> {
        use octofhir_fhir_model::provider::ElementInfo;

        let mut result = Vec::new();

        // Get the parent schema
        let Some(schema) = self.get_schema(parent_type) else {
            return result;
        };

        // Navigate to the backbone element through the path
        let path_parts: Vec<&str> = element_path.split('.').collect();
        let mut current_elements = schema.elements.as_ref();

        for part in &path_parts {
            let Some(elements) = current_elements else {
                return result;
            };

            let Some(element) = elements.get(*part) else {
                return result;
            };

            // Move to nested elements
            current_elements = element.elements.as_ref();
        }

        // Now current_elements should be the nested elements of the backbone
        if let Some(elements) = current_elements {
            for (name, element) in elements {
                // Determine element type
                let element_type = if element.elements.is_some() {
                    "BackboneElement".to_string()
                } else {
                    element
                        .type_name
                        .as_ref()
                        .unwrap_or(&"Any".to_string())
                        .clone()
                };

                result.push(ElementInfo {
                    name: name.clone(),
                    element_type,
                    documentation: element.short.clone(),
                });
            }
        }

        result
    }

    /// Check if an element at the given path is a backbone element (has nested elements).
    ///
    /// # Arguments
    /// * `parent_type` - The parent type name (e.g., "Task", "Patient")
    /// * `element_path` - Dot-separated path to the element (e.g., "input", "contact.telecom")
    ///
    /// # Returns
    /// True if the element at the path has nested elements (is a backbone element)
    pub fn is_backbone_element(&self, parent_type: &str, element_path: &str) -> bool {
        let Some(schema) = self.get_schema(parent_type) else {
            return false;
        };

        let path_parts: Vec<&str> = element_path.split('.').collect();
        let mut current_elements = schema.elements.as_ref();

        for (i, part) in path_parts.iter().enumerate() {
            let Some(elements) = current_elements else {
                return false;
            };

            let Some(element) = elements.get(*part) else {
                return false;
            };

            // If this is the last part, check if it has nested elements
            if i == path_parts.len() - 1 {
                return element.elements.is_some();
            }

            // Move to nested elements
            current_elements = element.elements.as_ref();
        }

        false
    }

    /// Helper to get backbone element's nested elements by parent type and path.
    /// Used internally by ModelProvider methods to navigate into inline backbone elements.
    pub fn get_backbone_elements_by_path(
        &self,
        parent_type: &str,
        element_path: &str,
    ) -> Option<&std::collections::HashMap<String, crate::types::FhirSchemaElement>> {
        let schema = self.get_schema(parent_type)?;
        let mut current_elements = schema.elements.as_ref()?;

        // Navigate through the path (handles nested backbones like "a.b.c")
        for part in element_path.split('.') {
            let element = current_elements.get(part)?;
            current_elements = element.elements.as_ref()?;
        }

        Some(current_elements)
    }

    /// Create new provider with schemas and FHIR version
    pub fn new(schemas: HashMap<String, FhirSchema>, fhir_version: ModelFhirVersion) -> Self {
        let type_mapping: HashMap<String, String> = TYPE_MAPPING
            .iter()
            .map(|(fhir_type, fhirpath_type)| (fhir_type.to_string(), fhirpath_type.to_string()))
            .collect();

        // Build reverse mapping for FHIRPath to FHIR types
        let reverse_type_mapping: HashMap<String, String> = TYPE_MAPPING
            .iter()
            .map(|(fhir_type, fhirpath_type)| (fhirpath_type.to_string(), fhir_type.to_string()))
            .collect();

        // Build URL to name mapping for O(1) lookup
        let url_to_name: HashMap<String, String> = schemas
            .iter()
            .map(|(name, schema)| (schema.url.clone(), name.clone()))
            .collect();

        Self {
            schemas,
            type_mapping,
            fhir_version,
            url_to_name,
            reverse_type_mapping,
        }
    }

    /// Update schemas (for dynamic loading)
    pub fn update_schemas(&mut self, schemas: HashMap<String, FhirSchema>) {
        // Rebuild URL to name mapping
        self.url_to_name = schemas
            .iter()
            .map(|(name, schema)| (schema.url.clone(), name.clone()))
            .collect();

        self.schemas = schemas;
    }

    /// Get all schemas
    pub fn schemas(&self) -> &HashMap<String, FhirSchema> {
        &self.schemas
    }

    /// Get a specific schema by URL or name
    pub fn get_schema_by_url(&self, url: &str) -> Option<&FhirSchema> {
        self.schemas.get(url)
    }

    /// Check if a schema exists by URL (supports both name and URL lookup)
    pub fn has_schema(&self, url_or_name: &str) -> bool {
        // Check by name first (direct key lookup)
        self.schemas.contains_key(url_or_name) ||
        // Then check by URL (O(1) lookup)
        self.url_to_name.contains_key(url_or_name)
    }

    /// Get schema by URL or name
    pub fn get_schema_by_url_or_name(&self, url_or_name: &str) -> Option<&FhirSchema> {
        // Try direct name lookup first
        if let Some(schema) = self.schemas.get(url_or_name) {
            return Some(schema);
        }

        // Try URL lookup with O(1) mapping
        if let Some(name) = self.url_to_name.get(url_or_name) {
            return self.schemas.get(name);
        }

        None
    }

    /// Map FHIR type to FHIRPath type using TYPE_MAPPING
    fn map_fhir_type(&self, fhir_type: &str) -> String {
        self.type_mapping
            .get(fhir_type)
            .cloned()
            .unwrap_or_else(|| fhir_type.to_string())
    }

    /// Get schema for a type name
    fn get_schema(&self, type_name: &str) -> Option<&FhirSchema> {
        self.schemas.get(type_name)
    }

    /// Check if one type is derived from another using schema hierarchy ONLY
    fn is_type_derived_from(&self, derived_type: &str, base_type: &str) -> bool {
        if derived_type == base_type {
            return true;
        }

        // Check schema hierarchy - use ONLY schema data, no hardcoding!
        if let Some(schema) = self.get_schema(derived_type)
            && let Some(base_type_name) = &schema.base
        {
            if base_type_name == base_type {
                return true;
            }
            // Recursive check up the hierarchy
            return self.is_type_derived_from(base_type_name, base_type);
        }

        false
    }
}

#[async_trait]
impl ModelProvider for FhirSchemaModelProvider {
    /// Core type lookup using schema map
    async fn get_type(&self, type_name: &str) -> ModelResult<Option<TypeInfo>> {
        if let Some(schema) = self.get_schema(type_name) {
            // Use the mapping if available, otherwise default to "Any" for complex types
            let mapped_type = if let Some(mapped) = self.type_mapping.get(&schema.name) {
                mapped.clone()
            } else if schema.kind == "resource" || schema.kind == "complex-type" {
                "Any".to_string()
            } else {
                self.map_fhir_type(&schema.name)
            };
            Ok(Some(TypeInfo {
                type_name: mapped_type,
                singleton: Some(true),
                is_empty: Some(false),
                namespace: Some("FHIR".to_string()),
                name: Some(schema.name.clone()),
            }))
        } else {
            // Check if it's a primitive type in our mapping (FHIR -> FHIRPath)
            if let Some(mapped) = self.type_mapping.get(type_name) {
                Ok(Some(TypeInfo {
                    type_name: mapped.clone(),
                    singleton: Some(true),
                    is_empty: Some(false),
                    namespace: Some("System".to_string()),
                    name: Some(type_name.to_string()),
                }))
            }
            // Check if it's a FHIRPath type (FHIRPath -> FHIR)
            else if self.reverse_type_mapping.contains_key(type_name) {
                Ok(Some(TypeInfo {
                    type_name: type_name.to_string(),
                    singleton: Some(true),
                    is_empty: Some(false),
                    namespace: Some("System".to_string()),
                    name: Some(type_name.to_string()),
                }))
            } else {
                Ok(None)
            }
        }
    }

    /// Get element type from complex type using schema information
    /// Handles choice navigation using FHIR schema patterns
    async fn get_element_type(
        &self,
        parent_type: &TypeInfo,
        property_name: &str,
    ) -> ModelResult<Option<TypeInfo>> {
        if let Some(type_name) = &parent_type.name {
            // Check if this is a backbone element path (e.g., "Task.input")
            let elements = if type_name.contains('.') {
                // Split into parent type and element path
                let parts: Vec<&str> = type_name.splitn(2, '.').collect();
                if parts.len() == 2 {
                    self.get_backbone_elements_by_path(parts[0], parts[1])
                } else {
                    None
                }
            } else {
                // Regular type lookup
                self.get_schema(type_name)
                    .and_then(|schema| schema.elements.as_ref())
            };

            let Some(elements) = elements else {
                return Ok(None);
            };
            // First try direct property name match
            if let Some(element) = elements.get(property_name) {
                // Check if this is a backbone element (has nested elements)
                if element.elements.is_some() {
                    // For backbone elements, use a path-based name so we can look up
                    // nested elements later. Format: "ParentType.elementPath"
                    // This works whether type_name is a simple type or already a path
                    let backbone_path = format!("{}.{}", type_name, property_name);
                    return Ok(Some(TypeInfo {
                        type_name: "Any".to_string(),
                        singleton: Some(element.max == Some(1)),
                        is_empty: Some(false),
                        namespace: Some("FHIR".to_string()),
                        name: Some(backbone_path),
                    }));
                }

                // Regular element with type_name
                if let Some(element_type_name) = &element.type_name {
                    let mapped_type = self.map_fhir_type(element_type_name);
                    return Ok(Some(TypeInfo {
                        type_name: mapped_type,
                        singleton: Some(element.max == Some(1)),
                        is_empty: Some(false),
                        namespace: Some("FHIR".to_string()),
                        name: Some(element_type_name.clone()),
                    }));
                }
            }

            // Handle choice navigation (e.g., value[x] -> valueString, valueInteger, etc.)
            // Look for choice base property (property name without type suffix)
            for (element_name, element) in elements {
                if element_name.ends_with("[x]") {
                    let base_name = element_name.trim_end_matches("[x]");
                    if let Some(type_suffix) = property_name.strip_prefix(base_name) {
                        // Extract the type from the property name (e.g., "valueString" -> "String")
                        if !type_suffix.is_empty() {
                            // Convert first character to lowercase for schema lookup
                            let mut chars = type_suffix.chars();
                            if let Some(first_char) = chars.next() {
                                let schema_type =
                                    format!("{}{}", first_char.to_lowercase(), chars.as_str());

                                // Check if this type is valid for this choice element
                                if let Some(choices) = &element.choices
                                    && choices.contains(&schema_type)
                                {
                                    let mapped_type = self.map_fhir_type(&schema_type);
                                    return Ok(Some(TypeInfo {
                                        type_name: mapped_type,
                                        singleton: Some(element.max == Some(1)),
                                        is_empty: Some(false),
                                        namespace: if schema_type
                                            .chars()
                                            .next()
                                            .unwrap()
                                            .is_uppercase()
                                        {
                                            Some("FHIR".to_string())
                                        } else {
                                            Some("System".to_string())
                                        },
                                        name: Some(schema_type),
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// Schema-driven type compatibility check using FHIR schema hierarchy
    fn of_type(&self, type_info: &TypeInfo, target_type: &str) -> Option<TypeInfo> {
        // Direct type match
        if type_info.type_name == target_type {
            return Some(type_info.clone());
        }

        // Name match
        if let Some(ref name) = type_info.name {
            if name == target_type {
                return Some(type_info.clone());
            }

            // Check type hierarchy using schema information
            if self.is_type_derived_from(name, target_type) {
                return Some(type_info.clone());
            }
        }

        // Check if the FHIRPath type is derived from target
        if self.is_type_derived_from(&type_info.type_name, target_type) {
            return Some(type_info.clone());
        }

        None
    }

    /// Get element names from complex type using schema
    fn get_element_names(&self, parent_type: &TypeInfo) -> Vec<String> {
        if let Some(type_name) = &parent_type.name {
            // Check if this is a backbone element path (e.g., "Task.input")
            // Backbone paths contain dots and are used for inline nested elements
            if type_name.contains('.') {
                // Split into parent type and element path
                let parts: Vec<&str> = type_name.splitn(2, '.').collect();
                if parts.len() == 2 {
                    let parent = parts[0];
                    let element_path = parts[1];

                    // Navigate to the backbone element's nested elements
                    if let Some(elements) = self.get_backbone_elements_by_path(parent, element_path)
                    {
                        return elements.keys().cloned().collect();
                    }
                }
                return Vec::new();
            }

            // Regular type lookup
            if let Some(schema) = self.get_schema(type_name)
                && let Some(elements) = &schema.elements
            {
                return elements.keys().cloned().collect();
            }
        }
        Vec::new()
    }

    /// Returns a union type of all possible child element types
    async fn get_children_type(&self, parent_type: &TypeInfo) -> ModelResult<Option<TypeInfo>> {
        if parent_type.singleton.unwrap_or(true) {
            Ok(None)
        } else {
            Ok(Some(TypeInfo {
                type_name: parent_type.type_name.clone(),
                singleton: Some(true),
                is_empty: Some(false),
                namespace: parent_type.namespace.clone(),
                name: parent_type.name.clone(),
            }))
        }
    }

    /// Get detailed information about elements of a type for completion suggestions.
    /// Includes inherited elements from base types (Resource, DomainResource, etc.).
    async fn get_elements(&self, type_name: &str) -> ModelResult<Vec<ElementInfo>> {
        let mut element_infos = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        // Collect elements from the type hierarchy (child first, then parents)
        let mut current_type = Some(type_name.to_string());
        while let Some(ref type_to_check) = current_type {
            if let Some(schema) = self.get_schema(type_to_check) {
                if let Some(elements) = &schema.elements {
                    for (name, element) in elements {
                        // Don't add duplicates (child elements override parent)
                        if !seen_names.contains(name) {
                            seen_names.insert(name.clone());

                            // Determine element type:
                            // - If element has nested `elements`, it's a BackboneElement
                            // - Otherwise use `type_name` or default to "Any"
                            let element_type = if element.elements.is_some() {
                                "BackboneElement".to_string()
                            } else {
                                element
                                    .type_name
                                    .as_ref()
                                    .unwrap_or(&"Any".to_string())
                                    .clone()
                            };

                            element_infos.push(ElementInfo {
                                name: name.clone(),
                                element_type,
                                documentation: element.short.clone(),
                            });
                        }
                    }
                }

                // Move to parent type
                current_type = schema.base.as_ref().and_then(|base_url| {
                    // Extract type name from URL (e.g., "http://hl7.org/fhir/StructureDefinition/DomainResource" -> "DomainResource")
                    base_url.rsplit('/').next().map(|s| s.to_string())
                });
            } else {
                current_type = None;
            }
        }

        Ok(element_infos)
    }

    /// Get list of all resource types
    async fn get_resource_types(&self) -> ModelResult<Vec<String>> {
        Ok(self
            .schemas
            .keys()
            .filter(|name| {
                // Check if it's a resource type by schema kind or naming convention
                if let Some(schema) = self.schemas.get(*name) {
                    schema.kind == "resource"
                        || name
                            .chars()
                            .next()
                            .map(|c| c.is_uppercase())
                            .unwrap_or(false)
                } else {
                    false
                }
            })
            .cloned()
            .collect())
    }

    /// Get list of all complex types
    async fn get_complex_types(&self) -> ModelResult<Vec<String>> {
        Ok(self
            .schemas
            .keys()
            .filter(|name| {
                if let Some(schema) = self.schemas.get(*name) {
                    // Only include actual complex types and resources, not primitives
                    (schema.kind == "complex-type" || schema.kind == "resource")
                        && !self.type_mapping.contains_key(*name)
                } else {
                    false
                }
            })
            .cloned()
            .collect())
    }

    /// Get list of all primitive types
    async fn get_primitive_types(&self) -> ModelResult<Vec<String>> {
        // Only return actual primitive types, not complex types that happen to be in type_mapping
        let primitive_types: Vec<String> = self
            .type_mapping
            .keys()
            .filter(|&name| {
                // Exclude complex types that are in type_mapping (like Quantity)
                !matches!(
                    name.as_str(),
                    "Quantity"
                        | "SimpleQuantity"
                        | "Money"
                        | "Duration"
                        | "Age"
                        | "Distance"
                        | "Count"
                        | "Any"
                )
            })
            .cloned()
            .collect();
        Ok(primitive_types)
    }
}

/// Embedded schema provider using pre-bundled schemas for fastest startup
#[derive(Debug)]
pub struct EmbeddedSchemaProvider {
    inner: FhirSchemaModelProvider,
}

impl EmbeddedSchemaProvider {
    /// Create new embedded provider with bundled schemas for specified FHIR version
    pub fn new(fhir_version: ModelFhirVersion) -> Self {
        use crate::embedded::{FhirVersion, get_schemas};

        // Convert ModelFhirVersion to local FhirVersion
        let local_version = match fhir_version {
            ModelFhirVersion::R4 => FhirVersion::R4,
            ModelFhirVersion::R4B => FhirVersion::R4B,
            ModelFhirVersion::R5 => FhirVersion::R5,
            ModelFhirVersion::R6 => FhirVersion::R6,
            ModelFhirVersion::Custom { .. } => FhirVersion::R4, // Default to R4 for custom versions
        };

        let schemas = get_schemas(local_version).clone();
        let inner = FhirSchemaModelProvider::new(schemas, fhir_version);
        Self { inner }
    }

    /// Convenience method to create R4 provider
    pub fn r4() -> Self {
        Self::new(ModelFhirVersion::R4)
    }

    /// Convenience method to create R4B provider
    pub fn r4b() -> Self {
        Self::new(ModelFhirVersion::R4B)
    }

    /// Convenience method to create R5 provider
    pub fn r5() -> Self {
        Self::new(ModelFhirVersion::R5)
    }

    /// Convenience method to create R6 provider
    pub fn r6() -> Self {
        Self::new(ModelFhirVersion::R6)
    }

    /// Get the FHIR version of this provider
    pub fn version(&self) -> &ModelFhirVersion {
        &self.inner.fhir_version
    }

    /// Get the number of schemas in this provider
    pub fn schema_count(&self) -> usize {
        self.inner.schemas.len()
    }

    /// Get access to all schemas
    pub fn schemas(&self) -> &std::collections::HashMap<String, crate::types::FhirSchema> {
        &self.inner.schemas
    }

    /// Validate a resource against a specific profile URL (async)
    pub async fn validate_resource_against_profile(
        &self,
        resource: &serde_json::Value,
        profile_url: &str,
    ) -> Result<crate::types::ValidationResult, Box<crate::types::ValidationError>> {
        use crate::validation::FhirSchemaValidator;

        // Create validator without FHIRPath evaluator (structural validation only)
        let validator = FhirSchemaValidator::new(self.inner.schemas.clone(), None);

        // Find schema by URL
        if let Some(schema) = self.inner.schemas.values().find(|s| s.url == profile_url) {
            Ok(validator
                .validate(resource, vec![schema.name.clone()])
                .await)
        } else {
            Err(Box::new(crate::types::ValidationError {
                error_type: "schema-not-found".to_string(),
                path: vec![],
                message: Some(format!("Profile not found: {profile_url}")),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            }))
        }
    }

    /// Validate a resource against its resource type
    pub async fn validate_resource_against_resource_type(
        &self,
        resource: &serde_json::Value,
        resource_type: &str,
    ) -> Result<crate::types::ValidationResult, Box<crate::types::ValidationError>> {
        use crate::validation::FhirSchemaValidator;

        // Create validator without FHIRPath evaluator (structural validation only)
        let validator = FhirSchemaValidator::new(self.inner.schemas.clone(), None);

        // Check if resource type exists
        if self.inner.schemas.contains_key(resource_type) {
            Ok(validator
                .validate(resource, vec![resource_type.to_string()])
                .await)
        } else {
            Err(Box::new(crate::types::ValidationError {
                error_type: "schema-not-found".to_string(),
                path: vec![],
                message: Some(format!("Resource type not found: {resource_type}")),
                value: None,
                expected: None,
                got: None,
                schema_path: None,
                constraint_key: None,
                constraint_expression: None,
                constraint_severity: None,
            }))
        }
    }

    /// Check if a resource type exists
    pub async fn resource_type_exists(&self, resource_type: &str) -> Result<bool, String> {
        Ok(self.inner.schemas.contains_key(resource_type))
    }

    /// Refresh resource types (no-op for embedded provider)
    pub async fn refresh_resource_types(&self) -> Result<(), String> {
        Ok(())
    }

    /// Navigate with data - simplified implementation
    pub async fn navigate_with_data(
        &self,
        resource_type: &str,
        property: &str,
        data: &serde_json::Value,
    ) -> Result<NavigationResult, String> {
        // Get the resource type
        if let Ok(Some(parent_type)) = self.inner.get_type(resource_type).await {
            // Get the element type
            if let Ok(Some(element_type)) =
                self.inner.get_element_type(&parent_type, property).await
            {
                // For choice types, try to determine the actual type from data
                if property.contains("value")
                    && data.is_object()
                    && let Some(obj) = data.as_object()
                {
                    // Look for a property that starts with the choice base
                    for key in obj.keys() {
                        if key.starts_with("value") && key != "value" {
                            // Extract the type (e.g., "valueString" -> "String")
                            let type_suffix = &key[5..]; // Remove "value"
                            return Ok(NavigationResult {
                                success: true,
                                result_type: Some(TypeInfo {
                                    type_name: type_suffix.to_string(),
                                    singleton: Some(true),
                                    namespace: Some("System".to_string()),
                                    name: Some(type_suffix.to_string()),
                                    is_empty: Some(false),
                                }),
                            });
                        }
                    }
                }

                Ok(NavigationResult {
                    success: true,
                    result_type: Some(element_type),
                })
            } else {
                Ok(NavigationResult {
                    success: false,
                    result_type: None,
                })
            }
        } else {
            Ok(NavigationResult {
                success: false,
                result_type: None,
            })
        }
    }

    /// Get FHIR version
    pub async fn get_fhir_version(&self) -> Result<ModelFhirVersion, String> {
        Ok(self.inner.fhir_version.clone())
    }

    /// Get children type (convert collection to singleton)
    pub async fn get_children_type(
        &self,
        type_info: &TypeInfo,
    ) -> Result<Option<TypeInfo>, String> {
        // If the type is a singleton, it has no children
        if type_info.singleton == Some(true) {
            return Ok(None);
        }

        // Return a singleton version of the same type
        Ok(Some(TypeInfo {
            type_name: type_info.type_name.clone(),
            singleton: Some(true),
            namespace: type_info.namespace.clone(),
            name: type_info.name.clone(),
            is_empty: type_info.is_empty,
        }))
    }
}

#[async_trait]
impl ModelProvider for EmbeddedSchemaProvider {
    async fn get_type(&self, type_name: &str) -> ModelResult<Option<TypeInfo>> {
        self.inner.get_type(type_name).await
    }

    async fn get_element_type(
        &self,
        parent_type: &TypeInfo,
        property_name: &str,
    ) -> ModelResult<Option<TypeInfo>> {
        self.inner
            .get_element_type(parent_type, property_name)
            .await
    }

    fn of_type(&self, type_info: &TypeInfo, target_type: &str) -> Option<TypeInfo> {
        self.inner.of_type(type_info, target_type)
    }

    fn get_element_names(&self, parent_type: &TypeInfo) -> Vec<String> {
        self.inner.get_element_names(parent_type)
    }

    async fn get_children_type(&self, parent_type: &TypeInfo) -> ModelResult<Option<TypeInfo>> {
        self.inner.get_children_type(parent_type).await
    }

    async fn get_elements(&self, type_name: &str) -> ModelResult<Vec<ElementInfo>> {
        self.inner.get_elements(type_name).await
    }

    async fn get_resource_types(&self) -> ModelResult<Vec<String>> {
        self.inner.get_resource_types().await
    }

    async fn get_complex_types(&self) -> ModelResult<Vec<String>> {
        self.inner.get_complex_types().await
    }

    async fn get_primitive_types(&self) -> ModelResult<Vec<String>> {
        self.inner.get_primitive_types().await
    }
}

/// Dynamic schema provider that can load schemas at runtime
#[derive(Debug)]
pub struct DynamicSchemaProvider {
    inner: FhirSchemaModelProvider,
}

impl DynamicSchemaProvider {
    /// Create new dynamic provider with schemas and FHIR version
    pub fn new(schemas: HashMap<String, FhirSchema>, fhir_version: ModelFhirVersion) -> Self {
        let inner = FhirSchemaModelProvider::new(schemas, fhir_version);
        Self { inner }
    }

    /// Create new dynamic provider from StructureDefinitions.
    /// Translates each StructureDefinition to FhirSchema on the fly.
    pub fn from_structure_definitions(
        structure_definitions: Vec<crate::types::StructureDefinition>,
        fhir_version: ModelFhirVersion,
    ) -> Self {
        let mut schemas = HashMap::new();

        for sd in structure_definitions {
            match crate::converter::translate(sd.clone(), None) {
                Ok(schema) => {
                    schemas.insert(schema.name.clone(), schema);
                }
                Err(_e) => {
                    // Skip invalid StructureDefinitions silently
                }
            }
        }

        let inner = FhirSchemaModelProvider::new(schemas, fhir_version);
        Self { inner }
    }

    /// Update schemas dynamically
    pub fn update_schemas(&mut self, schemas: HashMap<String, FhirSchema>) {
        self.inner.update_schemas(schemas);
    }

    /// Update schemas from StructureDefinitions.
    /// Translates each StructureDefinition to FhirSchema and replaces current schemas.
    pub fn update_from_structure_definitions(
        &mut self,
        structure_definitions: Vec<crate::types::StructureDefinition>,
    ) {
        let mut schemas = HashMap::new();

        for sd in structure_definitions {
            match crate::converter::translate(sd.clone(), None) {
                Ok(schema) => {
                    schemas.insert(schema.name.clone(), schema);
                }
                Err(_e) => {
                    // Skip invalid StructureDefinitions silently
                }
            }
        }

        self.inner.update_schemas(schemas);
    }

    /// Get the number of loaded schemas
    pub fn schema_count(&self) -> usize {
        self.inner.schemas.len()
    }

    pub fn schemas(&self) -> &HashMap<String, FhirSchema> {
        &self.inner.schemas
    }
}

#[async_trait]
impl ModelProvider for DynamicSchemaProvider {
    async fn get_type(&self, type_name: &str) -> ModelResult<Option<TypeInfo>> {
        self.inner.get_type(type_name).await
    }

    async fn get_element_type(
        &self,
        parent_type: &TypeInfo,
        property_name: &str,
    ) -> ModelResult<Option<TypeInfo>> {
        self.inner
            .get_element_type(parent_type, property_name)
            .await
    }

    fn of_type(&self, type_info: &TypeInfo, target_type: &str) -> Option<TypeInfo> {
        self.inner.of_type(type_info, target_type)
    }

    fn get_element_names(&self, parent_type: &TypeInfo) -> Vec<String> {
        self.inner.get_element_names(parent_type)
    }

    async fn get_children_type(&self, parent_type: &TypeInfo) -> ModelResult<Option<TypeInfo>> {
        self.inner.get_children_type(parent_type).await
    }

    async fn get_elements(&self, type_name: &str) -> ModelResult<Vec<ElementInfo>> {
        self.inner.get_elements(type_name).await
    }

    async fn get_resource_types(&self) -> ModelResult<Vec<String>> {
        self.inner.get_resource_types().await
    }

    async fn get_complex_types(&self) -> ModelResult<Vec<String>> {
        self.inner.get_complex_types().await
    }

    async fn get_primitive_types(&self) -> ModelResult<Vec<String>> {
        self.inner.get_primitive_types().await
    }
}
