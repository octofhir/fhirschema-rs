use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirSchemaConfig {
    pub fhir_version: FhirVersion,
    pub enable_validation: bool,
    pub cache_config: CacheConfig,
    pub performance_config: PerformanceConfig,
    pub package_sources: Vec<PackageSource>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FhirVersion {
    #[serde(rename = "4.0.1")]
    R4,
    #[serde(rename = "4.3.0")]
    R4B,
    #[serde(rename = "5.0.0")]
    R5,
    #[serde(rename = "6.0.0-ballot3")]
    R6,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub l1_size: usize,
    pub l2_size: usize,
    pub enable_disk_cache: bool,
    pub disk_cache_path: Option<String>,
    pub ttl: Duration,
    pub compression_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_concurrent_conversions: usize,
    pub max_concurrent_validations: usize,
    pub worker_pool_size: usize,
    pub enable_metrics: bool,
    pub conversion_timeout: Duration,
    pub validation_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSource {
    pub id: String,
    pub url: String,
    pub version: Option<String>,
    pub enabled: bool,
    pub metadata: HashMap<String, String>,
}

impl Default for FhirSchemaConfig {
    fn default() -> Self {
        Self {
            fhir_version: FhirVersion::R4,
            enable_validation: true,
            cache_config: CacheConfig::default(),
            performance_config: PerformanceConfig::default(),
            package_sources: vec![PackageSource {
                id: "hl7.fhir.r4.core".to_string(),
                url: "https://packages.fhir.org/hl7.fhir.r4.core".to_string(),
                version: Some("4.0.1".to_string()),
                enabled: true,
                metadata: HashMap::new(),
            }],
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            l1_size: 1000,
            l2_size: 5000,
            enable_disk_cache: false,
            disk_cache_path: None,
            ttl: Duration::from_secs(3600), // 1 hour
            compression_enabled: true,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_concurrent_conversions: num_cpus::get(),
            max_concurrent_validations: num_cpus::get() * 2,
            worker_pool_size: num_cpus::get(),
            enable_metrics: false,
            conversion_timeout: Duration::from_secs(30),
            validation_timeout: Duration::from_secs(10),
        }
    }
}

impl FhirSchemaConfig {
    pub fn for_version(version: FhirVersion) -> Self {
        let package_sources = match version {
            FhirVersion::R4 => vec![PackageSource {
                id: "hl7.fhir.r4.core".to_string(),
                url: "https://packages.fhir.org/hl7.fhir.r4.core".to_string(),
                version: Some("4.0.1".to_string()),
                enabled: true,
                metadata: HashMap::new(),
            }],
            FhirVersion::R4B => vec![PackageSource {
                id: "hl7.fhir.r4b.core".to_string(),
                url: "https://packages.fhir.org/hl7.fhir.r4b.core".to_string(),
                version: Some("4.3.0".to_string()),
                enabled: true,
                metadata: HashMap::new(),
            }],
            FhirVersion::R5 => vec![PackageSource {
                id: "hl7.fhir.r5.core".to_string(),
                url: "https://packages.fhir.org/hl7.fhir.r5.core".to_string(),
                version: Some("5.0.0".to_string()),
                enabled: true,
                metadata: HashMap::new(),
            }],
            FhirVersion::R6 => vec![PackageSource {
                id: "hl7.fhir.r6.core".to_string(),
                url: "https://packages.fhir.org/hl7.fhir.r6.core".to_string(),
                version: Some("6.0.0-ballot3".to_string()),
                enabled: true,
                metadata: HashMap::new(),
            }],
        };

        Self {
            fhir_version: version,
            package_sources,
            ..Default::default()
        }
    }

    pub fn with_validation(mut self, enabled: bool) -> Self {
        self.enable_validation = enabled;
        self
    }

    pub fn with_cache_config(mut self, cache_config: CacheConfig) -> Self {
        self.cache_config = cache_config;
        self
    }

    pub fn with_performance_config(mut self, performance_config: PerformanceConfig) -> Self {
        self.performance_config = performance_config;
        self
    }

    pub fn add_package_source(mut self, source: PackageSource) -> Self {
        self.package_sources.push(source);
        self
    }
}

impl CacheConfig {
    pub fn aggressive() -> Self {
        Self {
            l1_size: 5000,
            l2_size: 20000,
            enable_disk_cache: true,
            disk_cache_path: Some("~/.octofhir/fhirschema/cache".to_string()),
            ttl: Duration::from_secs(86400), // 24 hours
            compression_enabled: true,
        }
    }

    pub fn memory_only() -> Self {
        Self {
            l1_size: 2000,
            l2_size: 10000,
            enable_disk_cache: false,
            disk_cache_path: None,
            ttl: Duration::from_secs(3600),
            compression_enabled: true,
        }
    }

    pub fn minimal() -> Self {
        Self {
            l1_size: 100,
            l2_size: 500,
            enable_disk_cache: false,
            disk_cache_path: None,
            ttl: Duration::from_secs(600), // 10 minutes
            compression_enabled: false,
        }
    }
}

impl std::fmt::Display for FhirVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FhirVersion::R4 => write!(f, "4.0.1"),
            FhirVersion::R4B => write!(f, "4.3.0"),
            FhirVersion::R5 => write!(f, "5.0.0"),
            FhirVersion::R6 => write!(f, "6.0.0-ballot3"),
        }
    }
}

impl FhirVersion {
    /// Get all supported FHIR versions
    pub fn all() -> &'static [FhirVersion] {
        &[
            FhirVersion::R4,
            FhirVersion::R4B,
            FhirVersion::R5,
            FhirVersion::R6,
        ]
    }

    /// Get the package name for this FHIR version
    pub fn package_name(&self) -> &'static str {
        match self {
            FhirVersion::R4 => "hl7.fhir.r4.core",
            FhirVersion::R4B => "hl7.fhir.r4b.core",
            FhirVersion::R5 => "hl7.fhir.r5.core",
            FhirVersion::R6 => "hl7.fhir.r6.core",
        }
    }

    /// Get the package version for this FHIR version
    pub fn package_version(&self) -> &'static str {
        match self {
            FhirVersion::R4 => "4.0.1",
            FhirVersion::R4B => "4.3.0",
            FhirVersion::R5 => "5.0.0",
            FhirVersion::R6 => "6.0.0-ballot3",
        }
    }

    /// Get a short identifier for this version (e.g., "r4", "r4b")
    pub fn short_name(&self) -> &'static str {
        match self {
            FhirVersion::R4 => "r4",
            FhirVersion::R4B => "r4b",
            FhirVersion::R5 => "r5",
            FhirVersion::R6 => "r6",
        }
    }
}
