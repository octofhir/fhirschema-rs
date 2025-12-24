//! Reference resolution services for FHIR reference validation.
//!
//! This module provides the infrastructure for validating reference existence
//! in external storage systems. It uses a trait-based architecture similar to
//! the TerminologyService pattern.
//!
//! # Architecture
//!
//! The reference validation follows the optional feature pattern:
//! - `ReferenceResolver` trait defines the interface
//! - Validators optionally accept a reference resolver
//! - When provided, validators check that referenced resources exist
//!
//! # Example
//!
//! ```ignore
//! use octofhir_fhirschema::reference::{ReferenceResolver, ReferenceResolutionResult};
//! use std::sync::Arc;
//!
//! // Create your reference resolver implementation
//! let resolver: Arc<dyn ReferenceResolver> = Arc::new(MyStorageResolver::new(storage));
//!
//! // Use with validator
//! let validator = FhirSchemaValidator::new(schemas, None)
//!     .with_reference_resolver(resolver);
//! ```

use async_trait::async_trait;
use thiserror::Error;

/// Error codes for reference validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceErrorCode {
    /// REF1001: Referenced resource does not exist
    NonExistentResource = 1001,
    /// REF1002: Contained reference not found
    ContainedNotFound = 1002,
    /// REF1003: Bundle entry reference not found
    BundleEntryNotFound = 1003,
    /// REF1004: Reference service unavailable
    ServiceUnavailable = 1004,
    /// REF1005: Invalid reference format
    InvalidReferenceFormat = 1005,
}

impl std::fmt::Display for ReferenceErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "REF{:04}", *self as u32)
    }
}

/// Errors that can occur during reference resolution
#[derive(Debug, Error)]
pub enum ReferenceError {
    /// Referenced resource does not exist
    #[error("Referenced resource {resource_type}/{id} does not exist")]
    NonExistentResource { resource_type: String, id: String },

    /// Contained reference not found in resource
    #[error("Contained reference #{id} not found in resource")]
    ContainedNotFound { id: String },

    /// Bundle entry reference not found
    #[error("Bundle entry reference {full_url} not found")]
    BundleEntryNotFound { full_url: String },

    /// Service is unavailable
    #[error("Reference resolution service unavailable: {message}")]
    ServiceUnavailable { message: String },

    /// Invalid reference format
    #[error("Invalid reference format: {reference}")]
    InvalidReferenceFormat { reference: String },
}

impl ReferenceError {
    /// Get the error code for this error
    pub fn code(&self) -> ReferenceErrorCode {
        match self {
            ReferenceError::NonExistentResource { .. } => ReferenceErrorCode::NonExistentResource,
            ReferenceError::ContainedNotFound { .. } => ReferenceErrorCode::ContainedNotFound,
            ReferenceError::BundleEntryNotFound { .. } => ReferenceErrorCode::BundleEntryNotFound,
            ReferenceError::ServiceUnavailable { .. } => ReferenceErrorCode::ServiceUnavailable,
            ReferenceError::InvalidReferenceFormat { .. } => {
                ReferenceErrorCode::InvalidReferenceFormat
            }
        }
    }
}

/// Result type for reference operations
pub type ReferenceResult<T> = Result<T, ReferenceError>;

/// Result of resolving a reference
#[derive(Debug, Clone)]
pub struct ReferenceResolutionResult {
    /// Whether the referenced resource exists
    pub exists: bool,
    /// Resource type of the referenced resource (if resolved)
    pub resource_type: Option<String>,
    /// ID of the referenced resource
    pub id: Option<String>,
}

impl ReferenceResolutionResult {
    /// Create a result indicating the resource was found
    pub fn found(resource_type: String, id: String) -> Self {
        Self {
            exists: true,
            resource_type: Some(resource_type),
            id: Some(id),
        }
    }

    /// Create a result indicating the resource was not found
    pub fn not_found() -> Self {
        Self {
            exists: false,
            resource_type: None,
            id: None,
        }
    }

    /// Create a result for skipped validation (e.g., external references)
    pub fn skipped() -> Self {
        Self {
            exists: true, // Assume exists when skipped
            resource_type: None,
            id: None,
        }
    }
}

/// Trait for reference resolution services.
///
/// Implementations check whether referenced resources exist in the storage.
/// This can be backed by database storage, in-memory storage, or other sources.
///
/// # Example Implementation
///
/// ```ignore
/// struct StorageReferenceResolver {
///     storage: Arc<dyn FhirStorage>,
/// }
///
/// #[async_trait]
/// impl ReferenceResolver for StorageReferenceResolver {
///     async fn resource_exists(
///         &self,
///         resource_type: &str,
///         id: &str,
///     ) -> ReferenceResult<bool> {
///         match self.storage.read(resource_type, id).await {
///             Ok(Some(_)) => Ok(true),
///             Ok(None) => Ok(false),
///             Err(e) => Err(ReferenceError::ServiceUnavailable { message: e.to_string() }),
///         }
///     }
///
///     async fn resolve_reference(
///         &self,
///         reference: &str,
///     ) -> ReferenceResult<ReferenceResolutionResult> {
///         // Parse reference and check existence
///         let (resource_type, id) = parse_reference(reference)?;
///         let exists = self.resource_exists(&resource_type, &id).await?;
///         if exists {
///             Ok(ReferenceResolutionResult::found(resource_type, id))
///         } else {
///             Ok(ReferenceResolutionResult::not_found())
///         }
///     }
/// }
/// ```
#[async_trait]
pub trait ReferenceResolver: Send + Sync {
    /// Check if a resource exists by type and ID.
    ///
    /// # Arguments
    ///
    /// * `resource_type` - The FHIR resource type (e.g., "Patient")
    /// * `id` - The resource ID
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Resource exists
    /// * `Ok(false)` - Resource does not exist
    /// * `Err(_)` - Could not determine existence
    async fn resource_exists(&self, resource_type: &str, id: &str) -> ReferenceResult<bool>;

    /// Resolve a reference string and check existence.
    ///
    /// Handles relative references like "Patient/123" and absolute URLs.
    /// Returns whether the referenced resource exists.
    ///
    /// # Arguments
    ///
    /// * `reference` - The reference string (e.g., "Patient/123")
    ///
    /// # Returns
    ///
    /// * `Ok(ReferenceResolutionResult)` - Resolution result with existence info
    /// * `Err(_)` - Could not resolve the reference
    async fn resolve_reference(
        &self,
        reference: &str,
    ) -> ReferenceResult<ReferenceResolutionResult>;
}

/// A no-op reference resolver that always returns true (resource exists).
///
/// Useful for testing or when reference validation is disabled.
#[derive(Debug, Default, Clone)]
pub struct NoOpReferenceResolver;

impl NoOpReferenceResolver {
    /// Create a new no-op resolver
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ReferenceResolver for NoOpReferenceResolver {
    async fn resource_exists(&self, _resource_type: &str, _id: &str) -> ReferenceResult<bool> {
        Ok(true)
    }

    async fn resolve_reference(
        &self,
        _reference: &str,
    ) -> ReferenceResult<ReferenceResolutionResult> {
        Ok(ReferenceResolutionResult::skipped())
    }
}

/// Context for resolving references within a Bundle.
///
/// Maintains a map of fullUrl to resources for resolving internal Bundle references.
#[derive(Debug, Clone, Default)]
pub struct BundleContext {
    /// Map of fullUrl -> (resourceType, id)
    entries: std::collections::HashMap<String, (String, String)>,
}

impl BundleContext {
    /// Create a new empty Bundle context
    pub fn new() -> Self {
        Self::default()
    }

    /// Build context from a Bundle resource
    pub fn from_bundle(bundle: &serde_json::Value) -> Self {
        let mut ctx = Self::new();

        if let Some(entries) = bundle.get("entry").and_then(|e| e.as_array()) {
            for entry in entries {
                if let Some(full_url) = entry.get("fullUrl").and_then(|u| u.as_str())
                    && let Some(resource) = entry.get("resource")
                {
                    let resource_type = resource
                        .get("resourceType")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    let id = resource
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();

                    ctx.entries
                        .insert(full_url.to_string(), (resource_type, id));
                }
            }
        }

        ctx
    }

    /// Check if a reference exists in the Bundle
    pub fn contains(&self, reference: &str) -> bool {
        self.entries.contains_key(reference)
    }

    /// Get resource info by reference
    pub fn get(&self, reference: &str) -> Option<&(String, String)> {
        self.entries.get(reference)
    }

    /// Number of entries in the context
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if context is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Context for resolving contained references within a resource.
#[derive(Debug, Clone, Default)]
pub struct ContainedContext {
    /// Map of contained id -> resourceType
    contained: std::collections::HashMap<String, String>,
}

impl ContainedContext {
    /// Create a new empty Contained context
    pub fn new() -> Self {
        Self::default()
    }

    /// Build context from a resource's contained array
    pub fn from_resource(resource: &serde_json::Value) -> Self {
        let mut ctx = Self::new();

        if let Some(contained) = resource.get("contained").and_then(|c| c.as_array()) {
            for item in contained {
                if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                    let resource_type = item
                        .get("resourceType")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();

                    ctx.contained.insert(id.to_string(), resource_type);
                }
            }
        }

        ctx
    }

    /// Check if a contained reference exists (without the # prefix)
    pub fn contains(&self, id: &str) -> bool {
        self.contained.contains_key(id)
    }

    /// Get resource type by contained id (without the # prefix)
    pub fn get_type(&self, id: &str) -> Option<&String> {
        self.contained.get(id)
    }

    /// Number of contained resources
    pub fn len(&self) -> usize {
        self.contained.len()
    }

    /// Check if context is empty
    pub fn is_empty(&self) -> bool {
        self.contained.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_error_codes() {
        let err = ReferenceError::NonExistentResource {
            resource_type: "Patient".to_string(),
            id: "123".to_string(),
        };
        assert_eq!(err.code(), ReferenceErrorCode::NonExistentResource);
        assert_eq!(format!("{}", err.code()), "REF1001");

        let err = ReferenceError::ContainedNotFound {
            id: "contained1".to_string(),
        };
        assert_eq!(err.code(), ReferenceErrorCode::ContainedNotFound);
        assert_eq!(format!("{}", err.code()), "REF1002");
    }

    #[test]
    fn test_resolution_result() {
        let found = ReferenceResolutionResult::found("Patient".to_string(), "123".to_string());
        assert!(found.exists);
        assert_eq!(found.resource_type, Some("Patient".to_string()));
        assert_eq!(found.id, Some("123".to_string()));

        let not_found = ReferenceResolutionResult::not_found();
        assert!(!not_found.exists);
        assert!(not_found.resource_type.is_none());

        let skipped = ReferenceResolutionResult::skipped();
        assert!(skipped.exists);
    }

    #[tokio::test]
    async fn test_noop_resolver() {
        let resolver = NoOpReferenceResolver::new();

        let exists = resolver.resource_exists("Patient", "123").await.unwrap();
        assert!(exists);

        let result = resolver.resolve_reference("Patient/123").await.unwrap();
        assert!(result.exists);
    }

    #[test]
    fn test_bundle_context() {
        let bundle = json!({
            "resourceType": "Bundle",
            "type": "transaction",
            "entry": [
                {
                    "fullUrl": "urn:uuid:123",
                    "resource": {
                        "resourceType": "Patient",
                        "id": "temp-1"
                    }
                },
                {
                    "fullUrl": "http://example.org/fhir/Organization/1",
                    "resource": {
                        "resourceType": "Organization",
                        "id": "1"
                    }
                }
            ]
        });

        let ctx = BundleContext::from_bundle(&bundle);
        assert_eq!(ctx.len(), 2);
        assert!(ctx.contains("urn:uuid:123"));
        assert!(ctx.contains("http://example.org/fhir/Organization/1"));
        assert!(!ctx.contains("Patient/123"));

        let (rt, id) = ctx.get("urn:uuid:123").unwrap();
        assert_eq!(rt, "Patient");
        assert_eq!(id, "temp-1");
    }

    #[test]
    fn test_contained_context() {
        let resource = json!({
            "resourceType": "Patient",
            "id": "1",
            "contained": [
                {
                    "resourceType": "Organization",
                    "id": "org1"
                },
                {
                    "resourceType": "Practitioner",
                    "id": "prac1"
                }
            ]
        });

        let ctx = ContainedContext::from_resource(&resource);
        assert_eq!(ctx.len(), 2);
        assert!(ctx.contains("org1"));
        assert!(ctx.contains("prac1"));
        assert!(!ctx.contains("other"));

        assert_eq!(ctx.get_type("org1"), Some(&"Organization".to_string()));
    }
}
