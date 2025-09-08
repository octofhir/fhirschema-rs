use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Package fingerprint for cache invalidation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageFingerprint {
    /// Package identifier (e.g., "hl7.fhir.r4.core")
    pub package_id: String,
    /// Package version (e.g., "4.0.1")
    pub package_version: String,
    /// SHA-256 hash of package contents
    pub content_hash: String,
    /// Timestamp when fingerprint was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Metadata about the package
    pub metadata: HashMap<String, String>,
}

impl PackageFingerprint {
    /// Create a new fingerprint
    pub fn new(package_id: String, package_version: String, content_hash: String) -> Self {
        Self {
            package_id,
            package_version,
            content_hash,
            created_at: chrono::Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the fingerprint
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Check if this fingerprint matches another
    pub fn matches(&self, other: &PackageFingerprint) -> bool {
        self.package_id == other.package_id
            && self.package_version == other.package_version
            && self.content_hash == other.content_hash
    }

    /// Get a short representation of the fingerprint for logging
    pub fn short_hash(&self) -> &str {
        &self.content_hash[..8]
    }
}

/// Generate a fingerprint for a package based on its contents
pub fn generate_package_fingerprint(
    package_id: &str,
    package_version: &str,
    package_contents: &[u8],
) -> PackageFingerprint {
    // Create SHA-256 hash of package contents
    let mut hasher = Sha256::new();
    hasher.update(package_contents);
    let content_hash = format!("{:x}", hasher.finalize());

    PackageFingerprint::new(
        package_id.to_string(),
        package_version.to_string(),
        content_hash,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_fingerprint_creation() {
        let fingerprint = PackageFingerprint::new(
            "test.package".to_string(),
            "1.0.0".to_string(),
            "abcdef123456".to_string(),
        );

        assert_eq!(fingerprint.package_id, "test.package");
        assert_eq!(fingerprint.package_version, "1.0.0");
        assert_eq!(fingerprint.content_hash, "abcdef123456");
        assert_eq!(fingerprint.short_hash(), "abcdef12");
    }

    #[test]
    fn test_fingerprint_matching() {
        let fingerprint1 = PackageFingerprint::new(
            "test.package".to_string(),
            "1.0.0".to_string(),
            "hash123".to_string(),
        );

        let fingerprint2 = PackageFingerprint::new(
            "test.package".to_string(),
            "1.0.0".to_string(),
            "hash123".to_string(),
        );

        let fingerprint3 = PackageFingerprint::new(
            "test.package".to_string(),
            "1.0.0".to_string(),
            "hash456".to_string(),
        );

        assert!(fingerprint1.matches(&fingerprint2));
        assert!(!fingerprint1.matches(&fingerprint3));
    }

    #[test]
    fn test_content_fingerprinting() {
        let content1 = b"test content";
        let content2 = b"different content";

        let fp1 = generate_package_fingerprint("test", "1.0", content1);
        let fp2 = generate_package_fingerprint("test", "1.0", content2);

        assert_ne!(fp1.content_hash, fp2.content_hash);
        assert_eq!(fp1.package_id, fp2.package_id);
        assert_eq!(fp1.package_version, fp2.package_version);
    }
}
