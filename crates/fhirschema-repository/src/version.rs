//! Schema version management for FHIRSchema repository

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use crate::{RepositoryError, RepositoryResult};

/// Schema version representation supporting semantic versioning
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchemaVersion {
    /// Major version number
    pub major: u32,
    /// Minor version number
    pub minor: u32,
    /// Patch version number
    pub patch: u32,
    /// Pre-release identifier (e.g., "alpha", "beta", "rc.1")
    pub pre_release: Option<String>,
    /// Build metadata
    pub build: Option<String>,
}

impl SchemaVersion {
    /// Create a new version
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            pre_release: None,
            build: None,
        }
    }

    /// Create a new version with pre-release identifier
    pub fn new_pre_release(
        major: u32,
        minor: u32,
        patch: u32,
        pre_release: impl Into<String>,
    ) -> Self {
        Self {
            major,
            minor,
            patch,
            pre_release: Some(pre_release.into()),
            build: None,
        }
    }

    /// Create a new version with build metadata
    pub fn new_with_build(
        major: u32,
        minor: u32,
        patch: u32,
        build: impl Into<String>,
    ) -> Self {
        Self {
            major,
            minor,
            patch,
            pre_release: None,
            build: Some(build.into()),
        }
    }

    /// Check if this is a pre-release version
    pub fn is_pre_release(&self) -> bool {
        self.pre_release.is_some()
    }

    /// Check if this is a stable release
    pub fn is_stable(&self) -> bool {
        self.pre_release.is_none()
    }

    /// Get the next major version
    pub fn next_major(&self) -> Self {
        Self::new(self.major + 1, 0, 0)
    }

    /// Get the next minor version
    pub fn next_minor(&self) -> Self {
        Self::new(self.major, self.minor + 1, 0)
    }

    /// Get the next patch version
    pub fn next_patch(&self) -> Self {
        Self::new(self.major, self.minor, self.patch + 1)
    }

    /// Check if this version is compatible with another (same major version)
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major && self >= other
    }

    /// Check if this version represents a breaking change from another
    pub fn is_breaking_change_from(&self, other: &Self) -> bool {
        self.major > other.major
    }

    /// Parse version from string (supports semantic versioning format)
    pub fn parse(s: &str) -> RepositoryResult<Self> {
        s.parse().map_err(|e| RepositoryError::invalid_schema(format!("Invalid version format: {}", e)))
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;

        if let Some(ref pre) = self.pre_release {
            write!(f, "-{}", pre)?;
        }

        if let Some(ref build) = self.build {
            write!(f, "+{}", build)?;
        }

        Ok(())
    }
}

impl FromStr for SchemaVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        // Split on '+' to separate build metadata
        let (version_part, build) = if let Some(pos) = s.find('+') {
            let (v, b) = s.split_at(pos);
            (v, Some(b[1..].to_string()))
        } else {
            (s, None)
        };

        // Split on '-' to separate pre-release
        let (core_part, pre_release) = if let Some(pos) = version_part.find('-') {
            let (c, p) = version_part.split_at(pos);
            (c, Some(p[1..].to_string()))
        } else {
            (version_part, None)
        };

        // Parse core version (major.minor.patch)
        let parts: Vec<&str> = core_part.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("Version must have exactly 3 parts (major.minor.patch), got {}", parts.len()));
        }

        let major = parts[0].parse::<u32>()
            .map_err(|_| format!("Invalid major version: {}", parts[0]))?;
        let minor = parts[1].parse::<u32>()
            .map_err(|_| format!("Invalid minor version: {}", parts[1]))?;
        let patch = parts[2].parse::<u32>()
            .map_err(|_| format!("Invalid patch version: {}", parts[2]))?;

        Ok(SchemaVersion {
            major,
            minor,
            patch,
            pre_release,
            build,
        })
    }
}

impl PartialOrd for SchemaVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SchemaVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare major, minor, patch
        match (self.major.cmp(&other.major), self.minor.cmp(&other.minor), self.patch.cmp(&other.patch)) {
            (Ordering::Equal, Ordering::Equal, Ordering::Equal) => {
                // Same core version, compare pre-release
                match (&self.pre_release, &other.pre_release) {
                    (None, None) => Ordering::Equal,
                    (None, Some(_)) => Ordering::Greater, // Stable > pre-release
                    (Some(_), None) => Ordering::Less,    // Pre-release < stable
                    (Some(a), Some(b)) => a.cmp(b),       // Compare pre-release strings
                }
            }
            (Ordering::Equal, Ordering::Equal, patch_cmp) => patch_cmp,
            (Ordering::Equal, minor_cmp, _) => minor_cmp,
            (major_cmp, _, _) => major_cmp,
        }
    }
}

impl Default for SchemaVersion {
    fn default() -> Self {
        Self::new(0, 1, 0)
    }
}

/// Version manager for handling version operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManager {
    /// Current version
    current: SchemaVersion,
    /// Version history
    history: Vec<SchemaVersion>,
}

impl VersionManager {
    /// Create a new version manager with initial version
    pub fn new(initial_version: SchemaVersion) -> Self {
        Self {
            current: initial_version.clone(),
            history: vec![initial_version],
        }
    }

    /// Get the current version
    pub fn current(&self) -> &SchemaVersion {
        &self.current
    }

    /// Get all versions in history
    pub fn history(&self) -> &[SchemaVersion] {
        &self.history
    }

    /// Add a new version
    pub fn add_version(&mut self, version: SchemaVersion) -> RepositoryResult<()> {
        // Validate that new version is greater than current
        if version <= self.current {
            return Err(RepositoryError::version_conflict(
                format!("New version {} must be greater than current version {}", version, self.current)
            ));
        }

        // Check if version already exists
        if self.history.contains(&version) {
            return Err(RepositoryError::version_conflict(
                format!("Version {} already exists", version)
            ));
        }

        self.current = version.clone();
        self.history.push(version);
        self.history.sort();

        Ok(())
    }

    /// Get the latest stable version
    pub fn latest_stable(&self) -> Option<&SchemaVersion> {
        self.history.iter()
            .filter(|v| v.is_stable())
            .max()
    }

    /// Get all stable versions
    pub fn stable_versions(&self) -> Vec<&SchemaVersion> {
        let mut stable: Vec<_> = self.history.iter()
            .filter(|v| v.is_stable())
            .collect();
        stable.sort();
        stable
    }

    /// Get all pre-release versions
    pub fn pre_release_versions(&self) -> Vec<&SchemaVersion> {
        let mut pre_release: Vec<_> = self.history.iter()
            .filter(|v| v.is_pre_release())
            .collect();
        pre_release.sort();
        pre_release
    }

    /// Find versions compatible with a given version
    pub fn compatible_versions(&self, version: &SchemaVersion) -> Vec<&SchemaVersion> {
        self.history.iter()
            .filter(|v| v.is_compatible_with(version))
            .collect()
    }

    /// Check if a version exists
    pub fn has_version(&self, version: &SchemaVersion) -> bool {
        self.history.contains(version)
    }

    /// Remove a version from history
    pub fn remove_version(&mut self, version: &SchemaVersion) -> RepositoryResult<()> {
        if !self.has_version(version) {
            return Err(RepositoryError::version_not_found(
                "unknown", version.to_string()
            ));
        }

        // Cannot remove current version if it's the only one
        if self.history.len() == 1 && &self.current == version {
            return Err(RepositoryError::version_conflict(
                "Cannot remove the only version"
            ));
        }

        self.history.retain(|v| v != version);

        // If we removed the current version, set current to the latest
        if &self.current == version {
            self.current = self.history.iter().max().unwrap().clone();
        }

        Ok(())
    }

    /// Get version statistics
    pub fn statistics(&self) -> VersionStatistics {
        let stable_count = self.stable_versions().len();
        let pre_release_count = self.pre_release_versions().len();

        VersionStatistics {
            total_versions: self.history.len(),
            stable_versions: stable_count,
            pre_release_versions: pre_release_count,
            current_version: self.current.clone(),
            latest_stable: self.latest_stable().cloned(),
        }
    }
}

/// Version statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionStatistics {
    /// Total number of versions
    pub total_versions: usize,
    /// Number of stable versions
    pub stable_versions: usize,
    /// Number of pre-release versions
    pub pre_release_versions: usize,
    /// Current version
    pub current_version: SchemaVersion,
    /// Latest stable version
    pub latest_stable: Option<SchemaVersion>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        assert_eq!(SchemaVersion::parse("1.2.3").unwrap(), SchemaVersion::new(1, 2, 3));
        assert_eq!(
            SchemaVersion::parse("1.2.3-alpha").unwrap(),
            SchemaVersion::new_pre_release(1, 2, 3, "alpha")
        );
        assert_eq!(
            SchemaVersion::parse("1.2.3+build.1").unwrap(),
            SchemaVersion::new_with_build(1, 2, 3, "build.1")
        );
    }

    #[test]
    fn test_version_ordering() {
        let v1 = SchemaVersion::new(1, 0, 0);
        let v2 = SchemaVersion::new(1, 1, 0);
        let v3 = SchemaVersion::new(2, 0, 0);
        let v4 = SchemaVersion::new_pre_release(1, 1, 0, "alpha");

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v4 < v2); // Pre-release < stable
    }

    #[test]
    fn test_version_compatibility() {
        let v1 = SchemaVersion::new(1, 0, 0);
        let v2 = SchemaVersion::new(1, 1, 0);
        let v3 = SchemaVersion::new(2, 0, 0);

        assert!(v2.is_compatible_with(&v1));
        assert!(!v3.is_compatible_with(&v1));
        assert!(v3.is_breaking_change_from(&v1));
    }
}
