use serde::{Deserialize, Serialize};

/// Property information for type reflection in bridge libraries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyInfo {
    /// Property/element name
    pub name: String,

    /// Element type (e.g., "string", "HumanName", "Reference")
    pub element_type: String,

    /// Cardinality constraints
    pub cardinality: BridgeCardinality,

    /// Whether this is a collection (0..* or 1..*)
    pub is_collection: bool,

    /// Whether this property is required (min > 0)
    pub is_required: bool,

    /// Whether this is a choice type (value[x])
    pub is_choice_type: bool,

    /// Human-readable definition/description
    pub definition: Option<String>,
}

/// Cardinality information for bridge support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeCardinality {
    pub min: u32,
    pub max: Option<u32>, // None for unbounded (*)
}

impl BridgeCardinality {
    pub fn new(min: u32, max: Option<u32>) -> Self {
        Self { min, max }
    }

    pub fn is_required(&self) -> bool {
        self.min > 0
    }

    pub fn is_unbounded(&self) -> bool {
        self.max.is_none()
    }

    pub fn is_collection(&self) -> bool {
        self.max.is_none() || self.max.unwrap_or(0) > 1
    }

    pub fn is_optional(&self) -> bool {
        self.min == 0
    }

    pub fn allows_multiple(&self) -> bool {
        self.max.is_none() || self.max.unwrap_or(1) > 1
    }
}

/// Constraint information for bridge libraries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConstraintInfo {
    /// Constraint key/identifier
    pub key: String,

    /// Severity level (error, warning, information)
    pub severity: String,

    /// Human-readable description
    pub human_description: String,

    /// FHIRPath expression
    pub fhirpath_expression: String,

    /// Source schema/profile name
    pub source: Option<String>,

    /// XPath equivalent (if available)
    pub xpath: Option<String>,

    /// Whether this constraint requires FHIRPath evaluation
    pub requires_fhirpath: bool,
}

/// Validation result for bridge support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeValidationResult {
    /// Whether validation passed
    pub is_valid: bool,

    /// List of validation errors
    pub errors: Vec<BridgeValidationError>,

    /// List of validation warnings
    pub warnings: Vec<BridgeValidationWarning>,

    /// Performance metrics
    pub metrics: Option<BridgeValidationMetrics>,
}

/// Validation error for bridge support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeValidationError {
    /// Error message
    pub message: String,

    /// Location where error occurred (FHIRPath or element path)
    pub location: Option<String>,

    /// Error code for programmatic handling
    pub error_code: String,

    /// Constraint that failed (if applicable)
    pub constraint_key: Option<String>,
}

/// Validation warning for bridge support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeValidationWarning {
    /// Warning message
    pub message: String,

    /// Location where warning occurred
    pub location: Option<String>,

    /// Warning code
    pub warning_code: String,
}

/// Validation metrics for bridge support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeValidationMetrics {
    /// Total validation time in milliseconds
    pub validation_time_ms: u64,

    /// Number of constraints evaluated
    pub constraints_evaluated: usize,

    /// Number of FHIRPath expressions evaluated
    pub fhirpath_evaluations: usize,

    /// Cache hit ratio for path resolution
    pub path_cache_hit_ratio: f64,
}

/// Registry metrics for bridge support
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BridgeRegistryMetrics {
    /// Total number of schemas loaded
    pub total_schemas: usize,

    /// Number of resource types
    pub resource_types: usize,

    /// Number of profiles
    pub profiles: usize,

    /// Number of extensions
    pub extensions: usize,

    /// Memory usage in bytes
    pub memory_usage_bytes: u64,

    /// Index rebuild time in milliseconds
    pub index_rebuild_time_ms: u64,

    /// Cache statistics
    pub cache_stats: BridgeCacheStats,
}

/// Cache statistics for bridge support
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BridgeCacheStats {
    /// Schema cache hits
    pub schema_cache_hits: u64,

    /// Schema cache misses
    pub schema_cache_misses: u64,

    /// Path resolution cache hits
    pub path_cache_hits: u64,

    /// Path resolution cache misses
    pub path_cache_misses: u64,

    /// Type cache hits
    pub type_cache_hits: u64,

    /// Type cache misses
    pub type_cache_misses: u64,
}

impl Default for BridgeValidationResult {
    fn default() -> Self {
        Self::valid()
    }
}

impl BridgeValidationResult {
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            metrics: None,
        }
    }

    pub fn invalid(errors: Vec<BridgeValidationError>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
            metrics: None,
        }
    }

    pub fn with_warnings(mut self, warnings: Vec<BridgeValidationWarning>) -> Self {
        self.warnings = warnings;
        self
    }

    pub fn with_metrics(mut self, metrics: BridgeValidationMetrics) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub fn add_error(&mut self, error: BridgeValidationError) {
        self.errors.push(error);
        self.is_valid = false;
    }

    pub fn add_warning(&mut self, warning: BridgeValidationWarning) {
        self.warnings.push(warning);
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }
}

impl BridgeValidationError {
    pub fn new(message: String, error_code: String) -> Self {
        Self {
            message,
            location: None,
            error_code,
            constraint_key: None,
        }
    }

    pub fn with_location(mut self, location: String) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_constraint(mut self, constraint_key: String) -> Self {
        self.constraint_key = Some(constraint_key);
        self
    }
}

impl BridgeValidationWarning {
    pub fn new(message: String, warning_code: String) -> Self {
        Self {
            message,
            location: None,
            warning_code,
        }
    }

    pub fn with_location(mut self, location: String) -> Self {
        self.location = Some(location);
        self
    }
}

impl BridgeCacheStats {
    pub fn schema_hit_ratio(&self) -> f64 {
        let total = self.schema_cache_hits + self.schema_cache_misses;
        if total == 0 {
            0.0
        } else {
            self.schema_cache_hits as f64 / total as f64
        }
    }

    pub fn path_hit_ratio(&self) -> f64 {
        let total = self.path_cache_hits + self.path_cache_misses;
        if total == 0 {
            0.0
        } else {
            self.path_cache_hits as f64 / total as f64
        }
    }

    pub fn type_hit_ratio(&self) -> f64 {
        let total = self.type_cache_hits + self.type_cache_misses;
        if total == 0 {
            0.0
        } else {
            self.type_cache_hits as f64 / total as f64
        }
    }

    pub fn overall_hit_ratio(&self) -> f64 {
        let total_hits = self.schema_cache_hits + self.path_cache_hits + self.type_cache_hits;
        let total_misses =
            self.schema_cache_misses + self.path_cache_misses + self.type_cache_misses;
        let total = total_hits + total_misses;

        if total == 0 {
            0.0
        } else {
            total_hits as f64 / total as f64
        }
    }
}
