use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub success: bool,
    pub schema: Option<crate::types::FhirSchema>,
    pub errors: Vec<crate::error::ValidationError>,
    pub warnings: Vec<String>,
    pub metadata: ConversionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionMetadata {
    pub start_time: Option<std::time::SystemTime>,
    pub duration_ms: Option<u64>,
    pub structure_definition_url: Option<String>,
    pub processed_elements: usize,
    pub applied_constraints: usize,
    pub resolved_types: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<crate::error::ValidationError>,
    pub warnings: Vec<String>,
    pub metadata: ValidationMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationMetadata {
    pub start_time: Option<std::time::SystemTime>,
    pub duration_ms: Option<u64>,
    pub validated_paths: Vec<String>,
    pub constraints_evaluated: usize,
    pub fhirpath_expressions_executed: usize,
}

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub resolved_types: Vec<ResolvedType>,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedType {
    pub type_name: String,
    pub is_primitive: bool,
    pub is_complex: bool,
    pub is_resource: bool,
    pub base_type: Option<String>,
    pub constraints: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ResolutionContext {
    pub base_path: String,
    pub resource_type: Option<String>,
    pub profile_urls: Vec<String>,
    pub discriminator_paths: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl ConversionResult {
    pub fn success(schema: crate::types::FhirSchema) -> Self {
        Self {
            success: true,
            schema: Some(schema),
            errors: Vec::new(),
            warnings: Vec::new(),
            metadata: ConversionMetadata::default(),
        }
    }

    pub fn failure(errors: Vec<crate::error::ValidationError>) -> Self {
        Self {
            success: false,
            schema: None,
            errors,
            warnings: Vec::new(),
            metadata: ConversionMetadata::default(),
        }
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }

    pub fn with_metadata(mut self, metadata: ConversionMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn is_success(&self) -> bool {
        self.success
    }
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            metadata: ValidationMetadata::default(),
        }
    }

    pub fn invalid(errors: Vec<crate::error::ValidationError>) -> Self {
        Self {
            valid: false,
            errors,
            warnings: Vec::new(),
            metadata: ValidationMetadata::default(),
        }
    }

    pub fn from_constraint_results(
        constraint_results: Vec<ConstraintResult>,
    ) -> crate::error::Result<Self> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for result in constraint_results {
            match result {
                ConstraintResult::Error(err) => errors.push(err),
                ConstraintResult::Warning(warn) => warnings.push(warn),
                ConstraintResult::Success => {}
            }
        }

        Ok(Self {
            valid: errors.is_empty(),
            errors,
            warnings,
            metadata: ValidationMetadata::default(),
        })
    }
}

#[derive(Debug)]
pub enum ConstraintResult {
    Success,
    Error(crate::error::ValidationError),
    Warning(String),
}

impl Default for ConversionMetadata {
    fn default() -> Self {
        Self {
            start_time: Some(std::time::SystemTime::now()),
            duration_ms: None,
            structure_definition_url: None,
            processed_elements: 0,
            applied_constraints: 0,
            resolved_types: 0,
        }
    }
}

impl Default for ValidationMetadata {
    fn default() -> Self {
        Self {
            start_time: Some(std::time::SystemTime::now()),
            duration_ms: None,
            validated_paths: Vec::new(),
            constraints_evaluated: 0,
            fhirpath_expressions_executed: 0,
        }
    }
}

impl ResolutionContext {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
            resource_type: None,
            profile_urls: Vec::new(),
            discriminator_paths: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_resource_type(mut self, resource_type: &str) -> Self {
        self.resource_type = Some(resource_type.to_string());
        self
    }

    pub fn with_profile(mut self, profile_url: &str) -> Self {
        self.profile_urls.push(profile_url.to_string());
        self
    }

    pub fn hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.base_path.hash(&mut hasher);
        self.resource_type.hash(&mut hasher);
        self.profile_urls.hash(&mut hasher);
        self.discriminator_paths.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }
}
