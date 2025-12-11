//! Terminology validation services for FHIR binding validation.
//!
//! This module provides the infrastructure for validating code values against
//! value sets and code systems. It uses a trait-based architecture similar to
//! the FHIRPath evaluator integration, allowing optional terminology validation.
//!
//! # Architecture
//!
//! The terminology validation follows the same optional feature pattern as FHIRPath:
//! - `TerminologyService` trait defines the interface
//! - `CachedTerminologyService` wraps any service with TTL-based caching
//! - Validators optionally accept a terminology service
//!
//! # Example
//!
//! ```ignore
//! use octofhir_fhirschema::terminology::{TerminologyService, CachedTerminologyService, CacheConfig};
//! use std::sync::Arc;
//!
//! // Create your terminology service implementation
//! let service: Arc<dyn TerminologyService> = Arc::new(MyTerminologyService::new());
//!
//! // Wrap with caching
//! let cached = CachedTerminologyService::new(service, CacheConfig::default());
//!
//! // Use with validator builder
//! let provider = ValidationProviderBuilder::new(FhirVersion::R4)
//!     .with_embedded_schemas()
//!     .with_terminology(Arc::new(cached))
//!     .build()?;
//! ```

use async_trait::async_trait;
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// Error codes for terminology/binding validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminologyErrorCode {
    /// VS1001: Value set not found
    ValueSetNotFound = 1001,
    /// VS1002: Code not in value set
    CodeNotInValueSet = 1002,
    /// VS1003: Invalid code system
    InvalidCodeSystem = 1003,
    /// VS1004: Required binding violation
    RequiredBindingViolation = 1004,
    /// VS1005: Extensible binding violation (warning)
    ExtensibleBindingViolation = 1005,
    /// VS1006: Code system mismatch
    CodeSystemMismatch = 1006,
    /// VS1007: Missing required code
    MissingRequiredCode = 1007,
    /// VS1008: Terminology service unavailable
    ServiceUnavailable = 1008,
    /// VS1009: Invalid code format
    InvalidCodeFormat = 1009,
}

impl std::fmt::Display for TerminologyErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VS{:04}", *self as u32)
    }
}

/// Errors that can occur during terminology validation
#[derive(Debug, Error)]
pub enum TerminologyError {
    /// Value set was not found
    #[error("Value set not found: {url}")]
    ValueSetNotFound { url: String },

    /// Code is not a member of the value set
    #[error("Code '{code}' (system: {system:?}) is not in value set '{value_set}'")]
    CodeNotInValueSet {
        code: String,
        system: Option<String>,
        value_set: String,
    },

    /// Invalid code system URL
    #[error("Invalid code system: {system}")]
    InvalidCodeSystem { system: String },

    /// Required binding was violated
    #[error("Required binding violation: {message}")]
    RequiredBindingViolation { message: String },

    /// Service is unavailable
    #[error("Terminology service unavailable: {message}")]
    ServiceUnavailable { message: String },

    /// Network or communication error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl TerminologyError {
    /// Get the error code for this error
    pub fn code(&self) -> TerminologyErrorCode {
        match self {
            TerminologyError::ValueSetNotFound { .. } => TerminologyErrorCode::ValueSetNotFound,
            TerminologyError::CodeNotInValueSet { .. } => TerminologyErrorCode::CodeNotInValueSet,
            TerminologyError::InvalidCodeSystem { .. } => TerminologyErrorCode::InvalidCodeSystem,
            TerminologyError::RequiredBindingViolation { .. } => {
                TerminologyErrorCode::RequiredBindingViolation
            }
            TerminologyError::ServiceUnavailable { .. } => TerminologyErrorCode::ServiceUnavailable,
            TerminologyError::NetworkError(_) => TerminologyErrorCode::ServiceUnavailable,
            TerminologyError::InternalError(_) => TerminologyErrorCode::ServiceUnavailable,
        }
    }
}

/// Result type for terminology operations
pub type TerminologyResult<T> = Result<T, TerminologyError>;

/// Result of validating a code against a value set
#[derive(Debug, Clone)]
pub struct CodeValidationResult {
    /// Whether the code is valid
    pub valid: bool,
    /// Display text for the code (if found)
    pub display: Option<String>,
    /// Warning message (for extensible bindings)
    pub warning: Option<String>,
}

impl CodeValidationResult {
    /// Create a valid result
    pub fn valid() -> Self {
        Self {
            valid: true,
            display: None,
            warning: None,
        }
    }

    /// Create a valid result with display
    pub fn valid_with_display(display: String) -> Self {
        Self {
            valid: true,
            display: Some(display),
            warning: None,
        }
    }

    /// Create an invalid result
    pub fn invalid() -> Self {
        Self {
            valid: false,
            display: None,
            warning: None,
        }
    }

    /// Create a valid result with a warning
    pub fn valid_with_warning(warning: String) -> Self {
        Self {
            valid: true,
            display: None,
            warning: Some(warning),
        }
    }
}

/// Trait for terminology validation services.
///
/// Implementations can connect to FHIR terminology servers, local databases,
/// or in-memory value sets. The trait is async to support network-based services.
///
/// # Example Implementation
///
/// ```ignore
/// struct MyTerminologyService {
///     client: reqwest::Client,
///     base_url: String,
/// }
///
/// #[async_trait]
/// impl TerminologyService for MyTerminologyService {
///     async fn validate_code(
///         &self,
///         value_set_url: &str,
///         code: &str,
///         system: Option<&str>,
///     ) -> TerminologyResult<CodeValidationResult> {
///         // Call $validate-code operation on FHIR server
///         let url = format!("{}/ValueSet/$validate-code", self.base_url);
///         // ... HTTP request logic
///         Ok(CodeValidationResult::valid())
///     }
/// }
/// ```
#[async_trait]
pub trait TerminologyService: Send + Sync {
    /// Validate a code against a value set.
    ///
    /// # Arguments
    ///
    /// * `value_set_url` - Canonical URL of the value set
    /// * `code` - The code value to validate
    /// * `system` - Optional code system URL (for CodeableConcept.coding.system)
    ///
    /// # Returns
    ///
    /// * `Ok(CodeValidationResult)` - Validation result with validity and optional display
    /// * `Err(TerminologyError)` - If validation could not be performed
    async fn validate_code(
        &self,
        value_set_url: &str,
        code: &str,
        system: Option<&str>,
    ) -> TerminologyResult<CodeValidationResult>;

    /// Check if a value set exists and is available.
    ///
    /// This is optional - implementations may return true by default and
    /// handle missing value sets in `validate_code`.
    async fn value_set_exists(&self, value_set_url: &str) -> TerminologyResult<bool> {
        // Default implementation assumes value set exists
        let _ = value_set_url;
        Ok(true)
    }

    /// Get the display text for a code.
    ///
    /// This is optional - implementations may return None if display lookup
    /// is not supported.
    async fn get_display(&self, system: &str, code: &str) -> TerminologyResult<Option<String>> {
        // Default implementation doesn't provide display
        let _ = (system, code);
        Ok(None)
    }
}

/// Configuration for the terminology cache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Time-to-live for cached validation results
    pub ttl: Duration,
    /// Maximum number of entries in the cache
    pub max_size: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(3600), // 1 hour
            max_size: 10_000,
        }
    }
}

impl CacheConfig {
    /// Create a new cache configuration
    pub fn new(ttl: Duration, max_size: u64) -> Self {
        Self { ttl, max_size }
    }

    /// Create configuration for short-lived caches (5 minutes)
    pub fn short_lived() -> Self {
        Self {
            ttl: Duration::from_secs(300),
            max_size: 1_000,
        }
    }

    /// Create configuration for long-lived caches (24 hours)
    pub fn long_lived() -> Self {
        Self {
            ttl: Duration::from_secs(86400),
            max_size: 50_000,
        }
    }
}

/// Cache key for terminology lookups
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    value_set_url: String,
    code: String,
    system: Option<String>,
}

/// A cached wrapper around a TerminologyService.
///
/// Reduces calls to the underlying service by caching validation results.
/// Uses moka's async cache with TTL-based eviction.
///
/// # Example
///
/// ```ignore
/// let inner_service = Arc::new(MyTerminologyService::new());
/// let cached = CachedTerminologyService::new(inner_service, CacheConfig::default());
///
/// // First call hits the underlying service
/// let result = cached.validate_code("http://example.org/vs", "ABC", None).await?;
///
/// // Second call with same parameters returns cached result
/// let result = cached.validate_code("http://example.org/vs", "ABC", None).await?;
/// ```
pub struct CachedTerminologyService {
    inner: Arc<dyn TerminologyService>,
    cache: Cache<CacheKey, CodeValidationResult>,
}

impl CachedTerminologyService {
    /// Create a new cached terminology service.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying terminology service to wrap
    /// * `config` - Cache configuration (TTL, max size)
    pub fn new(inner: Arc<dyn TerminologyService>, config: CacheConfig) -> Self {
        let cache = Cache::builder()
            .time_to_live(config.ttl)
            .max_capacity(config.max_size)
            .build();

        Self { inner, cache }
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.cache.entry_count(),
            weighted_size: self.cache.weighted_size(),
        }
    }

    /// Clear all cached entries
    pub fn clear_cache(&self) {
        self.cache.invalidate_all();
    }
}

/// Statistics about the cache
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of entries in the cache
    pub entry_count: u64,
    /// Weighted size of the cache
    pub weighted_size: u64,
}

#[async_trait]
impl TerminologyService for CachedTerminologyService {
    async fn validate_code(
        &self,
        value_set_url: &str,
        code: &str,
        system: Option<&str>,
    ) -> TerminologyResult<CodeValidationResult> {
        let key = CacheKey {
            value_set_url: value_set_url.to_string(),
            code: code.to_string(),
            system: system.map(|s| s.to_string()),
        };

        // Try to get from cache
        if let Some(result) = self.cache.get(&key).await {
            return Ok(result);
        }

        // Cache miss - call underlying service
        let result = self
            .inner
            .validate_code(value_set_url, code, system)
            .await?;

        // Cache the result
        self.cache.insert(key, result.clone()).await;

        Ok(result)
    }

    async fn value_set_exists(&self, value_set_url: &str) -> TerminologyResult<bool> {
        // Don't cache existence checks - they're usually fast and we want fresh data
        self.inner.value_set_exists(value_set_url).await
    }

    async fn get_display(&self, system: &str, code: &str) -> TerminologyResult<Option<String>> {
        // Could add separate cache for display lookups if needed
        self.inner.get_display(system, code).await
    }
}

/// FHIR binding strength levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingStrength {
    /// Code MUST be from the value set
    Required,
    /// Code SHOULD be from the value set, but others allowed with text
    Extensible,
    /// Code SHOULD be from the value set for interoperability
    Preferred,
    /// Value set is just an example
    Example,
}

impl BindingStrength {
    /// Parse binding strength from string
    pub fn parse_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "required" => Some(BindingStrength::Required),
            "extensible" => Some(BindingStrength::Extensible),
            "preferred" => Some(BindingStrength::Preferred),
            "example" => Some(BindingStrength::Example),
            _ => None,
        }
    }

    /// Whether validation failure at this strength is an error
    pub fn is_error_on_failure(&self) -> bool {
        matches!(self, BindingStrength::Required)
    }

    /// Whether validation failure at this strength should produce a warning
    pub fn is_warning_on_failure(&self) -> bool {
        matches!(
            self,
            BindingStrength::Extensible | BindingStrength::Preferred
        )
    }
}

/// Type alias for code map: (code, system) -> display
type CodeMap = std::collections::HashMap<(String, Option<String>), Option<String>>;

/// A simple in-memory terminology service for testing.
///
/// This service maintains an in-memory map of value sets to valid codes.
/// Useful for unit tests and simple scenarios.
#[derive(Debug, Default)]
pub struct InMemoryTerminologyService {
    /// Map of value_set_url -> CodeMap
    value_sets: std::collections::HashMap<String, CodeMap>,
}

impl InMemoryTerminologyService {
    /// Create a new empty service
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a code to a value set
    pub fn add_code(
        &mut self,
        value_set_url: &str,
        code: &str,
        system: Option<&str>,
        display: Option<&str>,
    ) {
        let vs = self
            .value_sets
            .entry(value_set_url.to_string())
            .or_default();
        vs.insert(
            (code.to_string(), system.map(|s| s.to_string())),
            display.map(|d| d.to_string()),
        );
    }

    /// Add multiple codes to a value set
    pub fn add_codes(&mut self, value_set_url: &str, codes: &[(&str, Option<&str>)]) {
        for (code, system) in codes {
            self.add_code(value_set_url, code, *system, None);
        }
    }
}

#[async_trait]
impl TerminologyService for InMemoryTerminologyService {
    async fn validate_code(
        &self,
        value_set_url: &str,
        code: &str,
        system: Option<&str>,
    ) -> TerminologyResult<CodeValidationResult> {
        let Some(vs) = self.value_sets.get(value_set_url) else {
            return Err(TerminologyError::ValueSetNotFound {
                url: value_set_url.to_string(),
            });
        };

        let key = (code.to_string(), system.map(|s| s.to_string()));

        if let Some(display) = vs.get(&key) {
            Ok(match display {
                Some(d) => CodeValidationResult::valid_with_display(d.clone()),
                None => CodeValidationResult::valid(),
            })
        } else {
            // Try loose matching: if the input has no system, try to find the code with any system
            if system.is_none() {
                // Search for the code with any system
                for ((c, _sys), display) in vs.iter() {
                    if c == code {
                        return Ok(match display {
                            Some(d) => CodeValidationResult::valid_with_display(d.clone()),
                            None => CodeValidationResult::valid(),
                        });
                    }
                }
            }

            // Also try without system for loose matching
            let key_no_system = (code.to_string(), None);
            if vs.contains_key(&key_no_system) {
                Ok(CodeValidationResult::valid())
            } else {
                Ok(CodeValidationResult::invalid())
            }
        }
    }

    async fn value_set_exists(&self, value_set_url: &str) -> TerminologyResult<bool> {
        Ok(self.value_sets.contains_key(value_set_url))
    }

    async fn get_display(&self, system: &str, code: &str) -> TerminologyResult<Option<String>> {
        // Search all value sets for the code
        for vs in self.value_sets.values() {
            let key = (code.to_string(), Some(system.to_string()));
            if let Some(Some(display)) = vs.get(&key) {
                return Ok(Some(display.clone()));
            }
        }
        Ok(None)
    }
}

// ============================================================================
// Adapter for fhir-model-rs TerminologyProvider
// ============================================================================

use octofhir_fhir_model::TerminologyProvider;

/// Adapter to use a `TerminologyProvider` (from fhir-model-rs) as a `TerminologyService`.
///
/// This allows using the shared `CachedTerminologyProvider` or `DefaultTerminologyProvider`
/// from fhir-model-rs with fhirschema's validation infrastructure.
///
/// # Example
///
/// ```ignore
/// use octofhir_fhir_model::terminology::{DefaultTerminologyProvider, CachedTerminologyProvider};
/// use octofhir_fhirschema::terminology::TerminologyProviderAdapter;
/// use std::sync::Arc;
///
/// // Create a terminology provider from fhir-model-rs
/// let provider = DefaultTerminologyProvider::new()?;
///
/// // Wrap it in the adapter to use with fhirschema
/// let adapter = TerminologyProviderAdapter::new(Arc::new(provider));
///
/// // Use with validator builder
/// let validator = ValidationProviderBuilder::new(FhirVersion::R4)
///     .with_embedded_schemas()
///     .with_terminology(Arc::new(adapter))
///     .build()?;
/// ```
#[derive(Debug)]
pub struct TerminologyProviderAdapter {
    inner: Arc<dyn TerminologyProvider>,
}

impl TerminologyProviderAdapter {
    /// Create a new adapter wrapping a `TerminologyProvider`.
    ///
    /// # Arguments
    ///
    /// * `provider` - The terminology provider to adapt
    pub fn new(provider: Arc<dyn TerminologyProvider>) -> Self {
        Self { inner: provider }
    }
}

#[async_trait]
impl TerminologyService for TerminologyProviderAdapter {
    async fn validate_code(
        &self,
        value_set_url: &str,
        code: &str,
        system: Option<&str>,
    ) -> TerminologyResult<CodeValidationResult> {
        // Use validate_code_vs from TerminologyProvider which validates against a ValueSet
        let result = self
            .inner
            .validate_code_vs(value_set_url, system, code, None)
            .await
            .map_err(|e| TerminologyError::ServiceUnavailable {
                message: e.to_string(),
            })?;

        if result.result {
            Ok(match result.display {
                Some(display) => CodeValidationResult::valid_with_display(display),
                None => CodeValidationResult::valid(),
            })
        } else {
            Ok(match result.message {
                Some(msg) => CodeValidationResult {
                    valid: false,
                    display: None,
                    warning: Some(msg),
                },
                None => CodeValidationResult::invalid(),
            })
        }
    }

    async fn value_set_exists(&self, value_set_url: &str) -> TerminologyResult<bool> {
        // Try to expand the value set - if it fails, the value set doesn't exist or is unavailable
        let result = self.inner.expand_valueset(value_set_url, None).await;
        match result {
            Ok(_) => Ok(true),
            Err(_) => Ok(false), // Treat errors as "not found" for simplicity
        }
    }

    async fn get_display(&self, system: &str, code: &str) -> TerminologyResult<Option<String>> {
        // Use lookup_code from TerminologyProvider
        let result = self
            .inner
            .lookup_code(system, code, None, None)
            .await
            .map_err(|e| TerminologyError::ServiceUnavailable {
                message: e.to_string(),
            })?;

        Ok(result.display)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_service() {
        let mut service = InMemoryTerminologyService::new();
        service.add_code(
            "http://example.org/vs/gender",
            "male",
            Some("http://hl7.org/fhir/administrative-gender"),
            Some("Male"),
        );
        service.add_code(
            "http://example.org/vs/gender",
            "female",
            Some("http://hl7.org/fhir/administrative-gender"),
            Some("Female"),
        );

        let result = service
            .validate_code(
                "http://example.org/vs/gender",
                "male",
                Some("http://hl7.org/fhir/administrative-gender"),
            )
            .await
            .unwrap();
        assert!(result.valid);
        assert_eq!(result.display, Some("Male".to_string()));

        let result = service
            .validate_code(
                "http://example.org/vs/gender",
                "unknown",
                Some("http://hl7.org/fhir/administrative-gender"),
            )
            .await
            .unwrap();
        assert!(!result.valid);
    }

    #[tokio::test]
    async fn test_cached_service() {
        let mut inner = InMemoryTerminologyService::new();
        inner.add_code("http://example.org/vs", "ABC", None, None);

        let cached = CachedTerminologyService::new(
            Arc::new(inner),
            CacheConfig::new(Duration::from_secs(60), 100),
        );

        // First call
        let result = cached
            .validate_code("http://example.org/vs", "ABC", None)
            .await
            .unwrap();
        assert!(result.valid);

        // Second call should hit cache
        let result = cached
            .validate_code("http://example.org/vs", "ABC", None)
            .await
            .unwrap();
        assert!(result.valid);

        // Sync the cache to ensure stats are updated (moka is eventually consistent)
        cached.cache.run_pending_tasks().await;

        let stats = cached.cache_stats();
        assert_eq!(stats.entry_count, 1);
    }

    #[test]
    fn test_binding_strength() {
        assert_eq!(
            BindingStrength::parse_str("required"),
            Some(BindingStrength::Required)
        );
        assert_eq!(
            BindingStrength::parse_str("EXTENSIBLE"),
            Some(BindingStrength::Extensible)
        );
        assert!(BindingStrength::Required.is_error_on_failure());
        assert!(!BindingStrength::Preferred.is_error_on_failure());
        assert!(BindingStrength::Extensible.is_warning_on_failure());
    }

    #[test]
    fn test_error_codes() {
        let err = TerminologyError::ValueSetNotFound {
            url: "http://example.org/vs".to_string(),
        };
        assert_eq!(err.code(), TerminologyErrorCode::ValueSetNotFound);
        assert_eq!(format!("{}", err.code()), "VS1001");
    }

    #[tokio::test]
    async fn test_terminology_provider_adapter() {
        use octofhir_fhir_model::terminology::NoOpTerminologyProvider;

        // Create adapter wrapping NoOpTerminologyProvider
        let noop_provider = Arc::new(NoOpTerminologyProvider);
        let adapter = TerminologyProviderAdapter::new(noop_provider);

        // Test validate_code - NoOp always returns valid
        let result = adapter
            .validate_code(
                "http://example.org/vs",
                "test-code",
                Some("http://example.org/system"),
            )
            .await
            .unwrap();
        assert!(result.valid);

        // Test value_set_exists - NoOp returns empty expansion but succeeds
        let exists = adapter
            .value_set_exists("http://example.org/vs")
            .await
            .unwrap();
        assert!(exists); // NoOp returns success for expand_valueset

        // Test get_display - NoOp returns mock display
        let display = adapter
            .get_display("http://example.org/system", "test-code")
            .await
            .unwrap();
        assert!(display.is_some()); // NoOp returns "Mock display for {code}"
    }
}
