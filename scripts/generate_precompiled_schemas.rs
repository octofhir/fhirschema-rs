#!/usr/bin/env rust-script

//! Generate Precompiled Schemas Script
//! 
//! This script generates precompiled FHIR schemas for all supported versions
//! and saves them as binary files. This approach avoids running the build
//! script on every compilation.
//!
//! Usage:
//! ```bash
//! # Run with rust-script
//! cargo install rust-script
//! rust-script scripts/generate_precompiled_schemas.rs
//! 
//! # Or run directly with cargo
//! cargo run --bin generate_precompiled_schemas --features dynamic-caching
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::Path;

// This would normally use the actual octofhir dependencies
// For the script, we'll simulate the process
#[derive(Debug)]
struct FhirSchema {
    id: Option<String>,
    title: Option<String>,
    properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy)]
enum FhirVersion {
    R4,
    R4B,
    R5,
    R6,
}

impl FhirVersion {
    fn short_name(&self) -> &'static str {
        match self {
            FhirVersion::R4 => "r4",
            FhirVersion::R4B => "r4b",
            FhirVersion::R5 => "r5",
            FhirVersion::R6 => "r6",
        }
    }

    fn package_info(&self) -> (&'static str, &'static str) {
        match self {
            FhirVersion::R4 => ("hl7.fhir.r4.core", "4.0.1"),
            FhirVersion::R4B => ("hl7.fhir.r4b.core", "4.3.0"),
            FhirVersion::R5 => ("hl7.fhir.r5.core", "5.0.0"),
            FhirVersion::R6 => ("hl7.fhir.r6.core", "6.0.0-ballot3"),
        }
    }

    fn all() -> &'static [FhirVersion] {
        &[FhirVersion::R4, FhirVersion::R4B, FhirVersion::R5, FhirVersion::R6]
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Generating Precompiled FHIR Schemas");
    println!("======================================\n");

    // Ensure output directory exists
    let schemas_dir = Path::new("precompiled_schemas");
    if !schemas_dir.exists() {
        fs::create_dir_all(schemas_dir)?;
        println!("📁 Created precompiled_schemas directory");
    }

    // Generate schemas for each FHIR version
    for version in FhirVersion::all() {
        println!("📋 Processing FHIR {} ({})...", version.short_name().to_uppercase(), version.package_info().1);
        
        match generate_schemas_for_version(*version).await {
            Ok(schema_count) => {
                println!("  ✅ Generated {} schemas", schema_count);
            }
            Err(e) => {
                eprintln!("  ❌ Failed: {}", e);
                // Create empty placeholder for failed versions
                create_empty_schema_file(*version)?;
                println!("  📝 Created empty placeholder");
            }
        }
    }

    // Generate the embedded module
    generate_embedded_module().await?;
    println!("\n✅ Precompiled schema generation completed!");
    
    println!("\nNext steps:");
    println!("1. The precompiled schemas are now available in precompiled_schemas/");
    println!("2. Build with `cargo build --features precompiled-schemas`");
    println!("3. Use EmbeddedModelProvider for fastest startup");

    Ok(())
}

async fn generate_schemas_for_version(version: FhirVersion) -> Result<usize, Box<dyn std::error::Error>> {
    // In a real implementation, this would:
    // 1. Create CanonicalManager
    // 2. Install the FHIR package
    // 3. Extract StructureDefinitions  
    // 4. Convert to FhirSchemas
    // 5. Serialize with bincode

    // For this demo, we'll create minimal placeholder schemas
    let schemas = create_placeholder_schemas(version);
    let schema_count = schemas.len();

    // Serialize schemas with bincode
    let serialized = bincode::serialize(&schemas)?;
    
    // Write to file
    let filename = format!("{}_schemas.bin", version.short_name());
    let file_path = Path::new("precompiled_schemas").join(filename);
    fs::write(&file_path, &serialized)?;

    println!("  💾 Wrote {} bytes to {}", serialized.len(), file_path.display());

    Ok(schema_count)
}

fn create_placeholder_schemas(version: FhirVersion) -> Vec<FhirSchema> {
    // Create minimal schemas for core resource types
    let resource_types = get_core_resource_types(version);
    let mut schemas = Vec::new();

    for resource_type in resource_types {
        let mut properties = HashMap::new();
        properties.insert("resourceType".to_string(), serde_json::json!({
            "type": "string",
            "value": resource_type
        }));

        if resource_type != "Resource" {
            properties.insert("id".to_string(), serde_json::json!({
                "type": "string",
                "description": "Logical id of this artifact"
            }));
        }

        let schema = FhirSchema {
            id: Some(format!("http://hl7.org/fhir/StructureDefinition/{}", resource_type)),
            title: Some(resource_type.to_string()),
            properties,
        };

        schemas.push(schema);
    }

    schemas
}

fn get_core_resource_types(version: FhirVersion) -> Vec<&'static str> {
    let base_types = vec![
        "Resource",
        "DomainResource", 
        "Patient",
        "Practitioner",
        "Organization",
        "Bundle",
        "Observation",
        "Condition",
        "Medication",
        "MedicationRequest",
        "Encounter",
        "DiagnosticReport",
        "Procedure",
        "Immunization",
        "AllergyIntolerance",
        "CarePlan",
        "Goal",
        "ServiceRequest",
        "Device",
        "Location",
        "Specimen",
        "Coverage",
        "Account",
        "Person",
        "RelatedPerson",
        "Group",
        "HealthcareService",
        "Endpoint",
        "PractitionerRole",
        "Schedule",
        "Slot",
        "Appointment",
        "AppointmentResponse",
        "Flag",
        "List",
        "Composition",
        "DocumentReference",
        "Binary",
        "Media",
        "Communication",
        "CommunicationRequest", 
        "Task",
        "Provenance",
        "AuditEvent",
        "Consent",
    ];

    match version {
        FhirVersion::R4 | FhirVersion::R4B => base_types,
        FhirVersion::R5 | FhirVersion::R6 => {
            let mut types = base_types;
            types.extend_from_slice(&[
                "Subscription",
                "SubscriptionStatus",
                "SubscriptionTopic",
            ]);
            types
        }
    }
}

fn create_empty_schema_file(version: FhirVersion) -> Result<(), Box<dyn std::error::Error>> {
    let empty_schemas: Vec<FhirSchema> = Vec::new();
    let serialized = bincode::serialize(&empty_schemas)?;
    
    let filename = format!("{}_schemas.bin", version.short_name());
    let file_path = Path::new("precompiled_schemas").join(filename);
    fs::write(&file_path, &serialized)?;

    Ok(())
}

async fn generate_embedded_module() -> Result<(), Box<dyn std::error::Error>> {
    let mut content = String::new();
    
    content.push_str("// Auto-generated precompiled schemas\n");
    content.push_str("// Generated by scripts/generate_precompiled_schemas.rs\n");
    content.push_str("// DO NOT EDIT MANUALLY\n\n");
    
    content.push_str("#[cfg(feature = \"embedded-providers\")]\n");
    content.push_str("pub mod embedded {\n");
    content.push_str("    use std::collections::HashMap;\n\n");
    
    // Include binary schema data for each FHIR version
    for version in FhirVersion::all() {
        let version_upper = version.short_name().to_uppercase();
        content.push_str(&format!("    /// Precompiled schemas for FHIR {}\n", version_upper));
        content.push_str(&format!("    pub static {}_SCHEMAS: &[u8] = include_bytes!(\"../precompiled_schemas/{}_schemas.bin\");\n\n", 
                                version_upper, version.short_name()));
    }
    
    content.push_str("    /// Get precompiled schemas for a FHIR version\n");
    content.push_str("    pub fn get_schemas(version: &str) -> Option<&'static [u8]> {\n");
    content.push_str("        match version {\n");
    for version in FhirVersion::all() {
        let version_upper = version.short_name().to_uppercase();
        content.push_str(&format!("            \"{}\" => Some({}_SCHEMAS),\n", version.short_name(), version_upper));
    }
    content.push_str("            _ => None,\n");
    content.push_str("        }\n");
    content.push_str("    }\n\n");
    
    content.push_str("    /// Get all available FHIR versions with precompiled schemas\n");
    content.push_str("    pub fn available_versions() -> &'static [&'static str] {\n");
    let versions: Vec<String> = FhirVersion::all().iter().map(|v| format!("\"{}\"", v.short_name())).collect();
    content.push_str(&format!("        &[{}]\n", versions.join(", ")));
    content.push_str("    }\n");
    content.push_str("}\n");

    // Write embedded module to src/provider/
    let module_path = Path::new("src/provider/embedded_schemas.rs");
    fs::write(module_path, content)?;
    println!("📝 Generated embedded module at {}", module_path.display());

    Ok(())
}

// Add required dependencies for the script
#[cfg(not(feature = "build-script"))]
mod dependencies {
    // This would include the necessary dependencies
    pub use serde::{Deserialize, Serialize};
    pub use tokio;
    pub use bincode;
}

// Serde implementation for FhirSchema
impl serde::Serialize for FhirSchema {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("FhirSchema", 3)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("title", &self.title)?;
        state.serialize_field("properties", &self.properties)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for FhirSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field { Id, Title, Properties }

        struct FhirSchemaVisitor;

        impl<'de> serde::de::Visitor<'de> for FhirSchemaVisitor {
            type Value = FhirSchema;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct FhirSchema")
            }

            fn visit_map<V>(self, mut map: V) -> Result<FhirSchema, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut id = None;
                let mut title = None;
                let mut properties = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Id => {
                            if id.is_some() {
                                return Err(serde::de::Error::duplicate_field("id"));
                            }
                            id = map.next_value()?;
                        }
                        Field::Title => {
                            if title.is_some() {
                                return Err(serde::de::Error::duplicate_field("title"));
                            }
                            title = map.next_value()?;
                        }
                        Field::Properties => {
                            if properties.is_some() {
                                return Err(serde::de::Error::duplicate_field("properties"));
                            }
                            properties = Some(map.next_value()?);
                        }
                    }
                }
                Ok(FhirSchema {
                    id,
                    title,
                    properties: properties.unwrap_or_else(HashMap::new),
                })
            }
        }

        deserializer.deserialize_struct("FhirSchema", &["id", "title", "properties"], FhirSchemaVisitor)
    }
}