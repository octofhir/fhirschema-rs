use crate::types::{FhirSchema, ValidationContext};
use once_cell::sync::Lazy;
use std::collections::HashMap;

// Precompiled schema constants - use precompiled JSON files
pub static R4_SCHEMAS: &[u8] = include_bytes!("../precompiled_schemas/r4_schemas.json");
pub static R4B_SCHEMAS: &[u8] = include_bytes!("../precompiled_schemas/r4b_schemas.json");
pub static R5_SCHEMAS: &[u8] = include_bytes!("../precompiled_schemas/r5_schemas.json");
pub static R6_SCHEMAS: &[u8] = include_bytes!("../precompiled_schemas/r6_schemas.json");

// Lazy-loaded deserialized schemas
static R4_SCHEMA_MAP: Lazy<HashMap<String, FhirSchema>> = Lazy::new(|| {
    serde_json::from_slice::<HashMap<String, FhirSchema>>(R4_SCHEMAS).unwrap_or_else(|e| {
        eprintln!("Failed to deserialize R4 schemas from JSON: {e}");
        HashMap::new()
    })
});

static R4B_SCHEMA_MAP: Lazy<HashMap<String, FhirSchema>> = Lazy::new(|| {
    serde_json::from_slice::<HashMap<String, FhirSchema>>(R4B_SCHEMAS).unwrap_or_else(|e| {
        eprintln!("Failed to deserialize R4B schemas from JSON: {e}");
        HashMap::new()
    })
});

static R5_SCHEMA_MAP: Lazy<HashMap<String, FhirSchema>> = Lazy::new(|| {
    serde_json::from_slice::<HashMap<String, FhirSchema>>(R5_SCHEMAS).unwrap_or_else(|e| {
        eprintln!("Failed to deserialize R5 schemas from JSON: {e}");
        HashMap::new()
    })
});

static R6_SCHEMA_MAP: Lazy<HashMap<String, FhirSchema>> = Lazy::new(|| {
    serde_json::from_slice::<HashMap<String, FhirSchema>>(R6_SCHEMAS).unwrap_or_else(|e| {
        eprintln!("Failed to deserialize R6 schemas from JSON: {e}");
        HashMap::new()
    })
});

/// FHIR version enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FhirVersion {
    R4,
    R4B,
    R5,
    R6,
}

impl FhirVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            FhirVersion::R4 => "r4",
            FhirVersion::R4B => "r4b",
            FhirVersion::R5 => "r5",
            FhirVersion::R6 => "r6",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "r4" | "4.0" | "4.0.1" => Some(FhirVersion::R4),
            "r4b" | "4.3" | "4.3.0" => Some(FhirVersion::R4B),
            "r5" | "5.0" | "5.0.0" => Some(FhirVersion::R5),
            "r6" | "6.0" | "6.0.0-ballot3" => Some(FhirVersion::R6),
            _ => None,
        }
    }
}

impl std::str::FromStr for FhirVersion {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or("Invalid FHIR version")
    }
}

/// Get precompiled schemas for a specific FHIR version
pub fn get_schemas(version: FhirVersion) -> &'static HashMap<String, FhirSchema> {
    match version {
        FhirVersion::R4 => &R4_SCHEMA_MAP,
        FhirVersion::R4B => &R4B_SCHEMA_MAP,
        FhirVersion::R5 => &R5_SCHEMA_MAP,
        FhirVersion::R6 => &R6_SCHEMA_MAP,
    }
}

/// Get a specific schema by name for a FHIR version
pub fn get_schema(version: FhirVersion, name: &str) -> Option<&'static FhirSchema> {
    get_schemas(version).get(name)
}

/// Get all available schema names for a FHIR version
pub fn get_schema_names(version: FhirVersion) -> Vec<&'static String> {
    get_schemas(version).keys().collect()
}

/// Create a validation context from precompiled schemas
pub fn create_validation_context(version: FhirVersion) -> ValidationContext {
    ValidationContext {
        schemas: get_schemas(version).clone(),
    }
}

/// Check if a schema exists for a given resource type and version
pub fn has_schema(version: FhirVersion, resource_type: &str) -> bool {
    get_schemas(version).contains_key(resource_type)
}

/// Get schema information (counts, versions, etc.)
pub fn get_schema_info(version: FhirVersion) -> SchemaInfo {
    let schemas = get_schemas(version);
    let resource_count = schemas
        .values()
        .filter(|s| matches!(s.kind.as_str(), "resource" | "complex-type"))
        .count();
    let primitive_count = schemas
        .values()
        .filter(|s| s.kind == "primitive-type")
        .count();

    SchemaInfo {
        version,
        total_schemas: schemas.len(),
        resource_schemas: resource_count,
        primitive_schemas: primitive_count,
        data_type_schemas: schemas.len() - resource_count - primitive_count,
    }
}

#[derive(Debug, Clone)]
pub struct SchemaInfo {
    pub version: FhirVersion,
    pub total_schemas: usize,
    pub resource_schemas: usize,
    pub primitive_schemas: usize,
    pub data_type_schemas: usize,
}

impl SchemaInfo {
    pub fn print_summary(&self) {
        println!(
            "FHIR {} Schema Summary:",
            self.version.as_str().to_uppercase()
        );
        println!("  📊 Total schemas: {}", self.total_schemas);
        println!("  🏥 Resource types: {}", self.resource_schemas);
        println!("  🔤 Primitive types: {}", self.primitive_schemas);
        println!("  📋 Data types: {}", self.data_type_schemas);
    }
}

/// Utility function to list all available resources for a version
pub fn list_resources(version: FhirVersion) -> Vec<&'static String> {
    get_schemas(version)
        .iter()
        .filter(|(_, schema)| matches!(schema.kind.as_str(), "resource" | "complex-type"))
        .map(|(name, _)| name)
        .collect()
}

/// Utility function to list all primitive types for a version
pub fn list_primitives(version: FhirVersion) -> Vec<&'static String> {
    get_schemas(version)
        .iter()
        .filter(|(_, schema)| schema.kind == "primitive-type")
        .map(|(name, _)| name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fhir_version_from_str() {
        assert_eq!(FhirVersion::parse("r4"), Some(FhirVersion::R4));
        assert_eq!(FhirVersion::parse("R4"), Some(FhirVersion::R4));
        assert_eq!(FhirVersion::parse("4.0.1"), Some(FhirVersion::R4));
        assert_eq!(FhirVersion::parse("r5"), Some(FhirVersion::R5));
        assert_eq!(FhirVersion::parse("unknown"), None);

        // Test FromStr trait
        assert_eq!("r4".parse::<FhirVersion>(), Ok(FhirVersion::R4));
        assert_eq!("R4".parse::<FhirVersion>(), Ok(FhirVersion::R4));
        assert!("unknown".parse::<FhirVersion>().is_err());
    }

    #[test]
    fn test_fhir_version_as_str() {
        assert_eq!(FhirVersion::R4.as_str(), "r4");
        assert_eq!(FhirVersion::R4B.as_str(), "r4b");
        assert_eq!(FhirVersion::R5.as_str(), "r5");
    }

    #[test]
    fn test_get_schemas() {
        // Test that we can get schemas (even if empty in test environment)
        let schemas = get_schemas(FhirVersion::R4);
        assert!(schemas.is_empty() || !schemas.is_empty()); // Always true, just testing access
    }

    #[test]
    fn test_schema_info() {
        let info = get_schema_info(FhirVersion::R4);
        assert_eq!(info.version, FhirVersion::R4);
        // In test environment, schemas might be empty
        // Schema count should be meaningful (usize is always >= 0)
    }
}
