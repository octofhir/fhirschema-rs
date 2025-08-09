use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use url::Url;

/// Package specification that defines how to obtain and process a FHIR package
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageSpec {
    /// Package name (e.g., "hl7.fhir.r4.core")
    pub name: String,

    /// Package version (e.g., "4.0.1", "latest", "^4.0.0")
    pub version: String,

    /// Source for obtaining the package
    pub source: PackageSource,

    /// Optional dependencies that should be installed alongside this package
    pub dependencies: Vec<PackageSpec>,

    /// Conversion options specific to this package
    pub conversion_options: ConversionOptions,

    /// Optional metadata
    pub metadata: PackageMetadata,
}

/// Defines where and how to obtain a package
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PackageSource {
    /// Official FHIR package registry
    Registry {
        /// Registry URL (defaults to packages.fhir.org)
        url: Option<String>,
        /// Authentication if required
        auth: Option<RegistryAuth>,
    },

    /// Local file system path
    Local {
        path: std::path::PathBuf,
        /// Whether to watch for changes
        watch: bool,
    },

    /// Git repository
    Git {
        url: String,
        branch: Option<String>,
        tag: Option<String>,
        commit: Option<String>,
        /// Path within the repository
        path: Option<String>,
    },

    /// HTTP/HTTPS URL (direct download)
    Http { url: String, auth: Option<HttpAuth> },

    /// Custom source (for extensibility)
    Custom {
        source_type: String,
        config: HashMap<String, String>,
    },
}

/// Registry authentication options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryAuth {
    pub username: String,
    pub password: String,
    pub token: Option<String>,
}

/// HTTP authentication options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpAuth {
    pub auth_type: HttpAuthType,
    pub credentials: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HttpAuthType {
    Bearer,
    Basic,
    ApiKey { header_name: String },
}

/// Conversion options for transforming StructureDefinitions to FhirSchema
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversionOptions {
    /// Whether to expand choice types (e.g., value[x])
    pub expand_choice_types: bool,

    /// Include slicing information in the schema
    pub include_slicing: bool,

    /// Process constraints and invariants
    pub process_constraints: bool,

    /// Resolve profile dependencies
    pub resolve_profiles: bool,

    /// Cache conversion results
    pub cache_results: bool,

    /// Resource type filters (empty = convert all)
    pub resource_type_filter: Vec<String>,

    /// Profile type filters
    pub profile_type_filter: Vec<ProfileTypeFilter>,

    /// Custom conversion settings
    pub custom_settings: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProfileTypeFilter {
    Resource,
    Extension,
    Type,
    Logical,
    Profile,
}

/// Package metadata for additional information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PackageMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub keywords: Vec<String>,
    pub fhir_version: Option<String>,
    pub jurisdiction: Option<String>,
    pub custom: HashMap<String, serde_json::Value>,
}

/// Unique package identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageId {
    pub name: String,
    pub version: String,
}

/// Version specification with semantic versioning support
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VersionSpec {
    /// Exact version
    Exact(String),

    /// Latest available version
    Latest,

    /// Range specification (e.g., "^4.0.0", "~4.0.1", ">=4.0.0,<5.0.0")
    Range(String),

    /// Git reference
    GitRef {
        commit: Option<String>,
        tag: Option<String>,
        branch: Option<String>,
    },
}

/// Package installation preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallOptions {
    /// Skip dependency resolution
    pub skip_dependencies: bool,

    /// Force reinstallation even if already installed
    pub force: bool,

    /// Allow pre-release versions
    pub allow_prerelease: bool,

    /// Maximum parallel installations
    pub max_parallel: usize,

    /// Timeout for package downloads
    pub timeout_seconds: u64,

    /// Validate packages after installation
    pub validate: bool,

    /// Custom installation hooks
    pub hooks: InstallHooks,
}

/// Installation hooks for custom processing
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallHooks {
    pub pre_install: Vec<HookSpec>,
    pub post_install: Vec<HookSpec>,
    pub pre_conversion: Vec<HookSpec>,
    pub post_conversion: Vec<HookSpec>,
}

/// Hook specification for custom processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSpec {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

/// Package installation result
#[derive(Debug, Clone)]
pub struct PackageInstallResult {
    pub installed: Vec<InstalledPackage>,
    pub skipped: Vec<PackageId>,
    pub failed: Vec<PackageInstallError>,
    pub conversion_results: ConversionResults,
    pub duration: std::time::Duration,
}

/// Information about an installed package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub id: PackageId,
    pub spec: PackageSpec,
    pub install_time: chrono::DateTime<chrono::Utc>,
    pub file_path: Option<std::path::PathBuf>,
    pub checksum: Option<String>,
    pub schemas: Vec<Url>,
    pub dependencies: Vec<PackageId>,
    pub metadata: PackageMetadata,
}

/// Package installation error
#[derive(Debug, Clone)]
pub struct PackageInstallError {
    pub package_id: PackageId,
    pub error: String,
    pub category: ErrorCategory,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCategory {
    Download,
    Parsing,
    Conversion,
    Storage,
    Dependency,
    Validation,
    Authentication,
    Network,
    FileSystem,
}

/// Results from schema conversion process
#[derive(Debug, Clone, Default)]
pub struct ConversionResults {
    pub total_structure_definitions: usize,
    pub converted_schemas: usize,
    pub skipped: usize,
    pub failed: Vec<ConversionError>,
    pub conversion_time: std::time::Duration,
    pub performance_stats: ConversionStats,
}

/// Conversion error details
#[derive(Debug, Clone)]
pub struct ConversionError {
    pub structure_definition_url: String,
    pub error_message: String,
    pub error_type: ConversionErrorType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConversionErrorType {
    InvalidStructureDefinition,
    MissingDependencies,
    ConversionFailure,
    ValidationFailure,
}

/// Performance statistics for conversion process
#[derive(Debug, Clone, Default)]
pub struct ConversionStats {
    pub avg_conversion_time_ms: f64,
    pub max_conversion_time_ms: u128,
    pub min_conversion_time_ms: u128,
    pub memory_usage_mb: f64,
    pub parallel_efficiency: f64,
}

// Implementation blocks

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            expand_choice_types: true,
            include_slicing: true,
            process_constraints: true,
            resolve_profiles: true,
            cache_results: true,
            resource_type_filter: Vec::new(),
            profile_type_filter: Vec::new(),
            custom_settings: HashMap::new(),
        }
    }
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            skip_dependencies: false,
            force: false,
            allow_prerelease: false,
            max_parallel: 4,
            timeout_seconds: 300,
            validate: true,
            hooks: InstallHooks::default(),
        }
    }
}

impl PackageSpec {
    /// Create a simple package specification for a registry package
    pub fn registry(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            source: PackageSource::Registry {
                url: None,
                auth: None,
            },
            dependencies: Vec::new(),
            conversion_options: ConversionOptions::default(),
            metadata: PackageMetadata::default(),
        }
    }

    /// Create a package specification for a local directory
    pub fn local<P: Into<std::path::PathBuf>>(name: &str, path: P) -> Self {
        Self {
            name: name.to_string(),
            version: "local".to_string(),
            source: PackageSource::Local {
                path: path.into(),
                watch: false,
            },
            dependencies: Vec::new(),
            conversion_options: ConversionOptions::default(),
            metadata: PackageMetadata::default(),
        }
    }

    /// Create a package specification for a git repository
    pub fn git(name: &str, url: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "git".to_string(),
            source: PackageSource::Git {
                url: url.to_string(),
                branch: None,
                tag: None,
                commit: None,
                path: None,
            },
            dependencies: Vec::new(),
            conversion_options: ConversionOptions::default(),
            metadata: PackageMetadata::default(),
        }
    }

    /// Add a dependency to this package
    pub fn with_dependency(mut self, dependency: PackageSpec) -> Self {
        self.dependencies.push(dependency);
        self
    }

    /// Set conversion options
    pub fn with_conversion_options(mut self, options: ConversionOptions) -> Self {
        self.conversion_options = options;
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: PackageMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl PackageId {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
        }
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.version)
    }
}

impl fmt::Display for PackageInstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} (category: {:?})",
            self.package_id, self.error, self.category
        )
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Conversion error for {}: {} ({:?})",
            self.structure_definition_url, self.error_message, self.error_type
        )
    }
}

/// Builder pattern for package specifications
pub struct PackageSpecBuilder {
    spec: PackageSpec,
}

impl PackageSpecBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            spec: PackageSpec {
                name: name.to_string(),
                version: "latest".to_string(),
                source: PackageSource::Registry {
                    url: None,
                    auth: None,
                },
                dependencies: Vec::new(),
                conversion_options: ConversionOptions::default(),
                metadata: PackageMetadata::default(),
            },
        }
    }

    pub fn version(mut self, version: &str) -> Self {
        self.spec.version = version.to_string();
        self
    }

    pub fn registry_source(mut self, url: Option<String>, auth: Option<RegistryAuth>) -> Self {
        self.spec.source = PackageSource::Registry { url, auth };
        self
    }

    pub fn local_source<P: Into<std::path::PathBuf>>(mut self, path: P, watch: bool) -> Self {
        self.spec.source = PackageSource::Local {
            path: path.into(),
            watch,
        };
        self
    }

    pub fn git_source(mut self, url: &str) -> Self {
        self.spec.source = PackageSource::Git {
            url: url.to_string(),
            branch: None,
            tag: None,
            commit: None,
            path: None,
        };
        self
    }

    pub fn dependency(mut self, dep: PackageSpec) -> Self {
        self.spec.dependencies.push(dep);
        self
    }

    pub fn conversion_options(mut self, options: ConversionOptions) -> Self {
        self.spec.conversion_options = options;
        self
    }

    pub fn build(self) -> PackageSpec {
        self.spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_spec_creation() {
        let spec = PackageSpec::registry("hl7.fhir.r4.core", "4.0.1");
        assert_eq!(spec.name, "hl7.fhir.r4.core");
        assert_eq!(spec.version, "4.0.1");

        let id = PackageId::new("test", "1.0.0");
        assert_eq!(format!("{id}"), "test@1.0.0");
    }

    #[test]
    fn test_package_spec_builder() {
        let spec = PackageSpecBuilder::new("test.package")
            .version("1.0.0")
            .registry_source(None, None)
            .build();

        assert_eq!(spec.name, "test.package");
        assert_eq!(spec.version, "1.0.0");
    }

    #[test]
    fn test_conversion_options_defaults() {
        let options = ConversionOptions::default();
        assert!(options.expand_choice_types);
        assert!(options.include_slicing);
        assert!(options.process_constraints);
        assert!(options.resolve_profiles);
        assert!(options.cache_results);
    }
}
