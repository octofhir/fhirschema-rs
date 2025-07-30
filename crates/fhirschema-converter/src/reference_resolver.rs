//! Reference resolver for handling StructureDefinition references and dependencies.

use crate::{Result, Error};
use std::collections::HashMap;
use std::path::PathBuf;

/// Utilities for resolving StructureDefinition references.
pub struct ReferenceResolver {
    /// Cache of resolved references
    reference_cache: HashMap<String, String>,
    /// Base URLs for different FHIR versions
    base_urls: HashMap<String, String>,
    /// Local file paths for StructureDefinitions
    local_paths: HashMap<String, PathBuf>,
}

impl ReferenceResolver {
    /// Create a new reference resolver.
    pub fn new() -> Self {
        let mut base_urls = HashMap::new();
        base_urls.insert("4.0.1".to_string(), "http://hl7.org/fhir/R4".to_string());
        base_urls.insert("4.3.0".to_string(), "http://hl7.org/fhir/R4B".to_string());
        base_urls.insert("5.0.0".to_string(), "http://hl7.org/fhir/R5".to_string());

        Self {
            reference_cache: HashMap::new(),
            base_urls,
            local_paths: HashMap::new(),
        }
    }

    /// Resolve a StructureDefinition reference by canonical URL.
    pub fn resolve(&mut self, canonical_url: &str) -> Result<String> {
        // Check cache first
        if let Some(cached_result) = self.reference_cache.get(canonical_url) {
            return Ok(cached_result.clone());
        }

        // Parse the canonical URL
        let (base_url, resource_type, id, version) = self.parse_canonical_url(canonical_url)?;

        // Try to resolve locally first
        if let Some(local_content) = self.resolve_local(&base_url, &resource_type, &id, version.as_deref())? {
            self.reference_cache.insert(canonical_url.to_string(), local_content.clone());
            return Ok(local_content);
        }

        // Try to resolve from known FHIR base URLs
        if let Some(fhir_content) = self.resolve_fhir_base(&base_url, &resource_type, &id, version.as_deref())? {
            self.reference_cache.insert(canonical_url.to_string(), fhir_content.clone());
            return Ok(fhir_content);
        }

        Err(Error::Conversion(format!("Could not resolve reference: {}", canonical_url)))
    }

    /// Check if a reference is local or remote.
    pub fn is_local_reference(&self, reference: &str) -> bool {
        // Check if it's a relative path or file:// URL
        if reference.starts_with("file://") || reference.starts_with("./") || reference.starts_with("../") {
            return true;
        }

        // Check if it matches any registered local paths
        for local_url in self.local_paths.keys() {
            if reference.starts_with(local_url) {
                return true;
            }
        }

        // Check if it's a known FHIR base URL
        for base_url in self.base_urls.values() {
            if reference.starts_with(base_url) {
                return false; // It's a standard FHIR reference
            }
        }

        // Default to remote for http/https URLs
        reference.starts_with("http://") || reference.starts_with("https://")
    }

    /// Add a local path mapping for StructureDefinitions.
    pub fn add_local_path(&mut self, base_url: String, path: PathBuf) {
        self.local_paths.insert(base_url, path);
    }

    /// Parse a canonical URL into components.
    fn parse_canonical_url(&self, canonical_url: &str) -> Result<(String, String, String, Option<String>)> {
        // Handle version in URL (e.g., http://example.org/StructureDefinition/Patient|1.0.0)
        let (url_part, version) = if let Some(pipe_pos) = canonical_url.rfind('|') {
            let url = &canonical_url[..pipe_pos];
            let version = &canonical_url[pipe_pos + 1..];
            (url, Some(version.to_string()))
        } else {
            (canonical_url, None)
        };

        // Parse the URL structure
        if let Some(last_slash) = url_part.rfind('/') {
            let base_and_type = &url_part[..last_slash];
            let id = &url_part[last_slash + 1..];

            if let Some(second_last_slash) = base_and_type.rfind('/') {
                let base_url = &base_and_type[..second_last_slash];
                let resource_type = &base_and_type[second_last_slash + 1..];

                return Ok((
                    base_url.to_string(),
                    resource_type.to_string(),
                    id.to_string(),
                    version,
                ));
            }
        }

        Err(Error::Conversion(format!("Invalid canonical URL format: {}", canonical_url)))
    }

    /// Resolve reference from local file system.
    fn resolve_local(&self, base_url: &str, resource_type: &str, id: &str, _version: Option<&str>) -> Result<Option<String>> {
        if let Some(base_path) = self.local_paths.get(base_url) {
            let file_path = base_path.join(format!("{}-{}.json", resource_type, id));

            if file_path.exists() {
                match std::fs::read_to_string(&file_path) {
                    Ok(content) => return Ok(Some(content)),
                    Err(e) => {
                        eprintln!("Warning: Failed to read local file {}: {}", file_path.display(), e);
                    }
                }
            }
        }

        Ok(None)
    }

    /// Resolve reference from FHIR base URLs.
    fn resolve_fhir_base(&self, base_url: &str, resource_type: &str, id: &str, _version: Option<&str>) -> Result<Option<String>> {
        // Check if this is a known FHIR base URL
        for fhir_base in self.base_urls.values() {
            if base_url.starts_with(fhir_base) {
                // This would be where we'd make HTTP requests to fetch the resource
                // For now, we'll return a placeholder indicating it's a valid FHIR reference
                return Ok(Some(format!(
                    r#"{{"resourceType": "{}", "id": "{}", "url": "{}/{}/{}", "status": "active"}}"#,
                    resource_type, id, base_url, resource_type, id
                )));
            }
        }

        Ok(None)
    }

    /// Get all cached references.
    pub fn get_cached_references(&self) -> &HashMap<String, String> {
        &self.reference_cache
    }

    /// Clear the reference cache.
    pub fn clear_cache(&mut self) {
        self.reference_cache.clear();
    }

    /// Validate that a canonical URL is well-formed.
    pub fn validate_canonical_url(&self, canonical_url: &str) -> Result<()> {
        self.parse_canonical_url(canonical_url)?;
        Ok(())
    }

    /// Extract the resource ID from a canonical URL.
    pub fn extract_resource_id(&self, canonical_url: &str) -> Result<String> {
        let (_, _, id, _) = self.parse_canonical_url(canonical_url)?;
        Ok(id)
    }

    /// Extract the base URL from a canonical URL.
    pub fn extract_base_url(&self, canonical_url: &str) -> Result<String> {
        let (base_url, _, _, _) = self.parse_canonical_url(canonical_url)?;
        Ok(base_url)
    }

    /// Check if a reference points to a core FHIR resource.
    pub fn is_core_fhir_reference(&self, reference: &str) -> bool {
        for base_url in self.base_urls.values() {
            if reference.starts_with(base_url) {
                return true;
            }
        }
        false
    }
}

impl Default for ReferenceResolver {
    fn default() -> Self {
        Self::new()
    }
}
