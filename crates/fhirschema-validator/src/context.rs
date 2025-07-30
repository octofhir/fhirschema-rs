//! FHIRPath context for FHIRSchema validation
//!
//! This module provides the FHIRPath execution context with FHIR-specific
//! variables (%context, %resource, %rootResource, %ucum) as required by
//! the FHIRSchema specification.

use serde_json::Value;
use std::collections::HashMap;

/// FHIRPath context for constraint evaluation
///
/// Provides the standard FHIR variables:
/// - %context: The current context node being validated
/// - %resource: The containing resource
/// - %rootResource: The root resource (same as %resource for top-level resources)
/// - %ucum: The UCUM URL for unit validation
#[derive(Debug, Clone)]
pub struct FHIRPathContext {
    /// The current context node (%context variable)
    context: Value,
    /// The containing resource (%resource variable)
    resource: Value,
    /// The root resource (%rootResource variable)
    root_resource: Value,
    /// Additional variables that can be set
    variables: HashMap<String, Value>,
    /// UCUM URL for unit validation
    ucum_url: String,
}

impl FHIRPathContext {
    /// Create a new FHIRPath context
    pub fn new(context: &Value, resource: &Value, root_resource: &Value) -> Self {
        Self {
            context: context.clone(),
            resource: resource.clone(),
            root_resource: root_resource.clone(),
            variables: HashMap::new(),
            ucum_url: "http://unitsofmeasure.org".to_string(),
        }
    }

    /// Create a context for a specific element path
    pub fn for_element(
        element_value: &Value,
        resource: &Value,
        root_resource: &Value,
    ) -> Self {
        Self::new(element_value, resource, root_resource)
    }

    /// Create a child context for nested validation
    pub fn child_context(&self, new_context: &Value) -> Self {
        Self {
            context: new_context.clone(),
            resource: self.resource.clone(),
            root_resource: self.root_resource.clone(),
            variables: self.variables.clone(),
            ucum_url: self.ucum_url.clone(),
        }
    }

    /// Get the context value (%context)
    pub fn context(&self) -> &Value {
        &self.context
    }

    /// Get the resource value (%resource)
    pub fn resource(&self) -> &Value {
        &self.resource
    }

    /// Get the root resource value (%rootResource)
    pub fn root_resource(&self) -> &Value {
        &self.root_resource
    }

    /// Get the UCUM URL (%ucum)
    pub fn ucum_url(&self) -> &str {
        &self.ucum_url
    }

    /// Set a custom UCUM URL
    pub fn set_ucum_url(&mut self, url: String) {
        self.ucum_url = url;
    }

    /// Set a custom variable
    pub fn set_variable(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }

    /// Get a custom variable
    pub fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }

    /// Get all variables as a map for FHIRPath evaluation
    pub fn get_all_variables(&self) -> HashMap<String, Value> {
        let mut all_vars = self.variables.clone();

        // Add standard FHIR variables
        all_vars.insert("context".to_string(), self.context.clone());
        all_vars.insert("resource".to_string(), self.resource.clone());
        all_vars.insert("rootResource".to_string(), self.root_resource.clone());
        all_vars.insert("ucum".to_string(), Value::String(self.ucum_url.clone()));

        all_vars
    }

    /// Check if the context represents a valid FHIR resource
    pub fn is_valid_resource(&self) -> bool {
        self.resource.is_object() &&
        self.resource.get("resourceType").is_some()
    }

    /// Get the resource type from the resource
    pub fn resource_type(&self) -> Option<&str> {
        self.resource
            .get("resourceType")
            .and_then(|rt| rt.as_str())
    }

    /// Get the resource ID if present
    pub fn resource_id(&self) -> Option<&str> {
        self.resource
            .get("id")
            .and_then(|id| id.as_str())
    }

    /// Create a context for array element validation
    pub fn for_array_element(&self, element: &Value, index: usize) -> Self {
        let mut child = self.child_context(element);
        child.set_variable("index".to_string(), Value::Number(index.into()));
        child
    }

    /// Create a context for object property validation
    pub fn for_object_property(&self, property_value: &Value, property_name: &str) -> Self {
        let mut child = self.child_context(property_value);
        child.set_variable("property".to_string(), Value::String(property_name.to_string()));
        child
    }

    /// Extract value at a FHIRPath expression from the current context
    pub fn extract_path_value(&self, path: &str) -> Option<Value> {
        // Simple path extraction - in a real implementation, this would use fhirpath-rs
        // For now, we'll do basic property access
        if path.starts_with('%') {
            // Handle variables
            match path {
                "%context" => Some(self.context.clone()),
                "%resource" => Some(self.resource.clone()),
                "%rootResource" => Some(self.root_resource.clone()),
                "%ucum" => Some(Value::String(self.ucum_url.clone())),
                _ => {
                    // Check custom variables
                    let var_name = &path[1..]; // Remove % prefix
                    self.get_variable(var_name).cloned()
                }
            }
        } else {
            // Simple property access on context
            self.extract_simple_path(&self.context, path)
        }
    }

    /// Simple path extraction for basic property access
    fn extract_simple_path(&self, value: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for part in parts {
            match current {
                Value::Object(obj) => {
                    current = obj.get(part)?;
                }
                Value::Array(arr) => {
                    // Handle array indexing if part is numeric
                    if let Ok(index) = part.parse::<usize>() {
                        current = arr.get(index)?;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(current.clone())
    }

    /// Check if the context has a specific property
    pub fn has_property(&self, property: &str) -> bool {
        match &self.context {
            Value::Object(obj) => obj.contains_key(property),
            _ => false,
        }
    }

    /// Get the type of the context value
    pub fn context_type(&self) -> &'static str {
        match &self.context {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(n) => {
                if n.is_f64() && n.as_f64().unwrap().fract() != 0.0 {
                    "decimal"
                } else {
                    "integer"
                }
            }
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    /// Create a minimal context for testing
    #[cfg(test)]
    pub fn minimal() -> Self {
        let empty_resource = serde_json::json!({
            "resourceType": "Patient",
            "id": "test"
        });

        Self::new(&empty_resource, &empty_resource, &empty_resource)
    }
}

impl Default for FHIRPathContext {
    fn default() -> Self {
        let empty_value = Value::Null;
        Self::new(&empty_value, &empty_value, &empty_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_resource() -> Value {
        json!({
            "resourceType": "Patient",
            "id": "test-patient",
            "name": [{
                "family": "Doe",
                "given": ["John"]
            }],
            "active": true
        })
    }

    #[test]
    fn test_context_creation() {
        let resource = create_test_resource();
        let context = FHIRPathContext::new(&resource, &resource, &resource);

        assert_eq!(context.context(), &resource);
        assert_eq!(context.resource(), &resource);
        assert_eq!(context.root_resource(), &resource);
        assert_eq!(context.ucum_url(), "http://unitsofmeasure.org");
    }

    #[test]
    fn test_for_element() {
        let resource = create_test_resource();
        let name_element = &resource["name"][0];
        let context = FHIRPathContext::for_element(name_element, &resource, &resource);

        assert_eq!(context.context(), name_element);
        assert_eq!(context.resource(), &resource);
    }

    #[test]
    fn test_child_context() {
        let resource = create_test_resource();
        let parent_context = FHIRPathContext::new(&resource, &resource, &resource);
        let name_element = &resource["name"][0];
        let child_context = parent_context.child_context(name_element);

        assert_eq!(child_context.context(), name_element);
        assert_eq!(child_context.resource(), &resource);
        assert_eq!(child_context.root_resource(), &resource);
    }

    #[test]
    fn test_variables() {
        let resource = create_test_resource();
        let mut context = FHIRPathContext::new(&resource, &resource, &resource);

        // Test setting and getting custom variables
        context.set_variable("custom".to_string(), json!("test_value"));
        assert_eq!(context.get_variable("custom"), Some(&json!("test_value")));

        // Test all variables include standard ones
        let all_vars = context.get_all_variables();
        assert!(all_vars.contains_key("context"));
        assert!(all_vars.contains_key("resource"));
        assert!(all_vars.contains_key("rootResource"));
        assert!(all_vars.contains_key("ucum"));
        assert!(all_vars.contains_key("custom"));
    }

    #[test]
    fn test_resource_validation() {
        let resource = create_test_resource();
        let context = FHIRPathContext::new(&resource, &resource, &resource);

        assert!(context.is_valid_resource());
        assert_eq!(context.resource_type(), Some("Patient"));
        assert_eq!(context.resource_id(), Some("test-patient"));
    }

    #[test]
    fn test_invalid_resource() {
        let invalid_resource = json!({"name": "not a resource"});
        let context = FHIRPathContext::new(&invalid_resource, &invalid_resource, &invalid_resource);

        assert!(!context.is_valid_resource());
        assert_eq!(context.resource_type(), None);
    }

    #[test]
    fn test_array_element_context() {
        let resource = create_test_resource();
        let context = FHIRPathContext::new(&resource, &resource, &resource);
        let name_element = &resource["name"][0];

        let array_context = context.for_array_element(name_element, 0);
        assert_eq!(array_context.context(), name_element);
        assert_eq!(array_context.get_variable("index"), Some(&json!(0)));
    }

    #[test]
    fn test_object_property_context() {
        let resource = create_test_resource();
        let context = FHIRPathContext::new(&resource, &resource, &resource);
        let active_value = &resource["active"];

        let property_context = context.for_object_property(active_value, "active");
        assert_eq!(property_context.context(), active_value);
        assert_eq!(property_context.get_variable("property"), Some(&json!("active")));
    }

    #[test]
    fn test_extract_path_value() {
        let resource = create_test_resource();
        let context = FHIRPathContext::new(&resource, &resource, &resource);

        // Test variable extraction
        assert_eq!(context.extract_path_value("%context"), Some(resource.clone()));
        assert_eq!(context.extract_path_value("%resource"), Some(resource.clone()));
        assert_eq!(context.extract_path_value("%ucum"), Some(json!("http://unitsofmeasure.org")));

        // Test simple property extraction
        assert_eq!(context.extract_path_value("resourceType"), Some(json!("Patient")));
        assert_eq!(context.extract_path_value("id"), Some(json!("test-patient")));
        assert_eq!(context.extract_path_value("active"), Some(json!(true)));
    }

    #[test]
    fn test_has_property() {
        let resource = create_test_resource();
        let context = FHIRPathContext::new(&resource, &resource, &resource);

        assert!(context.has_property("resourceType"));
        assert!(context.has_property("id"));
        assert!(context.has_property("name"));
        assert!(!context.has_property("nonexistent"));
    }

    #[test]
    fn test_context_type() {
        let resource = create_test_resource();
        let context = FHIRPathContext::new(&resource, &resource, &resource);
        assert_eq!(context.context_type(), "object");

        let string_context = FHIRPathContext::new(&json!("test"), &resource, &resource);
        assert_eq!(string_context.context_type(), "string");

        let number_context = FHIRPathContext::new(&json!(42), &resource, &resource);
        assert_eq!(number_context.context_type(), "integer");

        let decimal_context = FHIRPathContext::new(&json!(3.14), &resource, &resource);
        assert_eq!(decimal_context.context_type(), "decimal");
    }

    #[test]
    fn test_custom_ucum_url() {
        let resource = create_test_resource();
        let mut context = FHIRPathContext::new(&resource, &resource, &resource);

        context.set_ucum_url("http://custom.ucum.org".to_string());
        assert_eq!(context.ucum_url(), "http://custom.ucum.org");

        let all_vars = context.get_all_variables();
        assert_eq!(all_vars.get("ucum"), Some(&json!("http://custom.ucum.org")));
    }

    #[test]
    fn test_minimal_context() {
        let context = FHIRPathContext::minimal();
        assert!(context.is_valid_resource());
        assert_eq!(context.resource_type(), Some("Patient"));
    }
}
