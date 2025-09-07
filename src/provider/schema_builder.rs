use std::path::Path;
use std::sync::Arc;

use crate::conversion::ConversionEngine;
use crate::core::{FhirVersion, PerformanceConfig};
use crate::error::{FhirSchemaError, Result};
use crate::types::FhirSchema;
use crate::utils::performance::Timer;

/// Schema builder for creating precompiled model.bin files
#[derive(Debug)]
pub struct SchemaBuilder {
    engine: ConversionEngine,
    fhir_version: FhirVersion,
}

impl SchemaBuilder {
    /// Create a new schema builder for the specified FHIR version
    pub async fn new(fhir_version: FhirVersion) -> Result<Self> {
        // Create canonical manager for the specified FHIR version
        let config = octofhir_canonical_manager::FcmConfig::default();

        let canonical_manager = Arc::new(
            octofhir_canonical_manager::CanonicalManager::new(config)
                .await
                .map_err(|e| {
                    FhirSchemaError::conversion_failed("CanonicalManager", &e.to_string())
                })?,
        );

        let performance_config = PerformanceConfig {
            max_concurrent_conversions: 8,
            max_concurrent_validations: 16,
            worker_pool_size: 4,
            enable_metrics: false,
            conversion_timeout: std::time::Duration::from_secs(30),
            validation_timeout: std::time::Duration::from_secs(10),
        };

        let engine = ConversionEngine::new(canonical_manager, &performance_config).await?;

        Ok(Self {
            engine,
            fhir_version,
        })
    }

    /// Build schemas from FHIR package and save to binary file
    pub async fn build_and_save_schemas<P: AsRef<Path>>(
        &self,
        package_name: &str,
        package_version: &str,
        output_path: P,
    ) -> Result<SchemaBuilderResult> {
        let timer = Timer::new();

        tracing::info!(
            "Building schemas for FHIR {} package {}@{}",
            self.fhir_version,
            package_name,
            package_version
        );

        // Load StructureDefinitions from the package
        let structure_definitions = self
            .load_package_structure_definitions(package_name, package_version)
            .await?;

        tracing::info!(
            "Loaded {} StructureDefinitions",
            structure_definitions.len()
        );

        // Convert StructureDefinitions to FhirSchemas
        let conversion_results = self.engine.convert_batch(structure_definitions).await?;

        // Extract successful conversions
        let mut schemas = Vec::new();
        let mut conversion_errors = Vec::new();

        for result in conversion_results {
            if result.is_success() {
                if let Some(schema) = result.schema {
                    schemas.push(schema);
                }
            } else {
                conversion_errors.extend(result.errors);
            }
        }

        tracing::info!(
            "Successfully converted {} schemas with {} errors",
            schemas.len(),
            conversion_errors.len()
        );

        // Save to binary file
        self.save_schemas_binary(&schemas, &output_path).await?;

        let build_result = SchemaBuilderResult {
            fhir_version: self.fhir_version,
            package_name: package_name.to_string(),
            package_version: package_version.to_string(),
            schema_count: schemas.len(),
            build_duration_ms: timer.elapsed_ms() as u64,
            conversion_errors: conversion_errors.len(),
            output_path: output_path.as_ref().to_path_buf(),
            file_size_bytes: self.get_file_size(&output_path).await.unwrap_or(0),
        };

        tracing::info!(
            "Schema build completed in {}ms: {} schemas saved to {}",
            build_result.build_duration_ms,
            build_result.schema_count,
            output_path.as_ref().display()
        );

        Ok(build_result)
    }

    /// Load StructureDefinitions from a FHIR package
    async fn load_package_structure_definitions(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> Result<Vec<serde_json::Value>> {
        tracing::info!(
            "Loading StructureDefinitions from package {}@{}",
            package_name,
            package_version
        );

        // Use the canonical manager to load actual StructureDefinitions from FHIR packages
        let config = octofhir_canonical_manager::config::FcmConfig::load()
            .await
            .map_err(|e| FhirSchemaError::PackageLoadError {
                package: package_name.to_string(),
                version: package_version.to_string(),
                message: format!("Failed to load canonical manager config: {e}"),
            })?;

        let manager = octofhir_canonical_manager::CanonicalManager::new(config)
            .await
            .map_err(|e| FhirSchemaError::PackageLoadError {
                package: package_name.to_string(),
                version: package_version.to_string(),
                message: format!("Failed to create canonical manager: {e}"),
            })?;

        // Install the package if not already installed
        tracing::info!("Installing package {}@{}", package_name, package_version);
        manager
            .install_package(package_name, package_version)
            .await
            .map_err(|e| FhirSchemaError::PackageLoadError {
                package: package_name.to_string(),
                version: package_version.to_string(),
                message: format!("Package installation failed: {e}"),
            })?;

        // Search for ALL StructureDefinitions across all installed packages, not just the target package
        tracing::info!("Searching for ALL StructureDefinitions across all installed packages for FHIR version {}", self.fhir_version);
        let search_query = octofhir_canonical_manager::search::SearchQuery {
            text: None,
            resource_types: vec!["StructureDefinition".to_string()],
            packages: vec![],  // Search across ALL packages, not just target package
            limit: Some(1000), // Set explicit large limit to get all StructureDefinitions
            ..Default::default()
        };

        let search_results = manager
            .search_engine()
            .search(&search_query)
            .await
            .map_err(|e| FhirSchemaError::PackageLoadError {
                package: package_name.to_string(),
                version: package_version.to_string(),
                message: format!("Failed to search for StructureDefinitions: {e}"),
            })?;

        let mut structure_definitions = Vec::new();

        tracing::info!(
            "Processing {} search results",
            search_results.resources.len()
        );

        for resource_match in search_results.resources {
            let resource = &resource_match.resource.content;
            // Verify it's actually a StructureDefinition
            if resource.get("resourceType").and_then(|rt| rt.as_str())
                == Some("StructureDefinition")
            {
                // Filter for actual resource types (not profiles or extensions)
                if let (Some(kind), Some(type_name)) = (
                    resource.get("kind").and_then(|k| k.as_str()),
                    resource.get("type").and_then(|t| t.as_str()),
                ) {
                    // Include FHIR resource StructureDefinitions based on comprehensive criteria
                    let abstract_field = resource
                        .get("abstract")
                        .and_then(|a| a.as_bool())
                        .unwrap_or(false);
                    let url = resource.get("url").and_then(|u| u.as_str()).unwrap_or("");
                    let derivation = resource
                        .get("derivation")
                        .and_then(|d| d.as_str())
                        .unwrap_or("");
                    let base_definition = resource
                        .get("baseDefinition")
                        .and_then(|b| b.as_str())
                        .unwrap_or("");

                    // Include if:
                    // 1. Kind is "resource"
                    // 2. URL starts with http://hl7.org/fhir/StructureDefinition/ (official FHIR resources)
                    // 3. NOT a constraint/profile (derivation != "constraint")
                    // 4. Base definition indicates it's a proper FHIR resource
                    let is_fhir_resource = kind == "resource"
                        && url.starts_with("http://hl7.org/fhir/StructureDefinition/")
                        && derivation != "constraint"
                        && (base_definition.contains("Resource")
                            || base_definition.contains("Element")
                            || abstract_field);

                    if is_fhir_resource {
                        tracing::info!("✅ Including FHIR resource: {} (kind: {}, type: {}, abstract: {}, derivation: {}, base: {})", 
                            resource.get("name").and_then(|n| n.as_str()).unwrap_or("unknown"),
                            kind, type_name, abstract_field, derivation,
                            base_definition.split('/').next_back().unwrap_or("unknown"));
                        structure_definitions.push(resource.clone());
                    } else {
                        tracing::debug!(
                            "❌ Skipping non-resource: {} (kind: {}, derivation: {}, url: {})",
                            resource
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("unknown"),
                            kind,
                            derivation,
                            url.chars().take(40).collect::<String>()
                        );
                    }
                } else {
                    tracing::debug!(
                        "Skipping StructureDefinition without kind/type fields: {}",
                        resource
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                    );
                }
            }
        }

        if structure_definitions.is_empty() {
            return Err(FhirSchemaError::PackageLoadError {
                package: package_name.to_string(),
                version: package_version.to_string(),
                message: "No FHIR resource StructureDefinitions found across all installed packages. Check package installation and canonical manager setup.".to_string(),
            });
        }

        tracing::info!(
            "🎉 Successfully loaded {} FHIR resource StructureDefinitions from canonical manager",
            structure_definitions.len()
        );

        Ok(structure_definitions)
    }

    /// Create a minimal StructureDefinition for a resource type
    fn create_minimal_structure_definition(
        &self,
        resource_type: &str,
        package_name: &str,
        package_version: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "resourceType": "StructureDefinition",
            "id": format!("{}-{}", package_name, resource_type.to_lowercase()),
            "url": format!("http://hl7.org/fhir/StructureDefinition/{}", resource_type),
            "name": resource_type,
            "title": resource_type,
            "status": "active",
            "kind": "resource",
            "abstract": resource_type == "Resource" || resource_type == "DomainResource",
            "type": resource_type,
            "baseDefinition": self.get_base_definition(resource_type),
            "derivation": "specialization",
            "version": package_version,
            "publisher": "HL7",
            "description": format!("Base definition for {} resource", resource_type),
            "fhirVersion": self.fhir_version.to_string(),
            "differential": {
                "element": self.create_minimal_elements(resource_type)
            }
        })
    }

    /// Get base definition URL for a resource type
    fn get_base_definition(&self, resource_type: &str) -> &'static str {
        match resource_type {
            "Resource" => "http://hl7.org/fhir/StructureDefinition/Element",
            "DomainResource" => "http://hl7.org/fhir/StructureDefinition/Resource",
            _ => "http://hl7.org/fhir/StructureDefinition/DomainResource",
        }
    }

    /// Create minimal elements for a resource type
    fn create_minimal_elements(&self, resource_type: &str) -> Vec<serde_json::Value> {
        let mut elements = vec![serde_json::json!({
            "id": resource_type,
            "path": resource_type,
            "short": format!("{} resource", resource_type),
            "definition": format!("Base definition for {} resource type", resource_type),
            "min": 0,
            "max": "*"
        })];

        // Add common elements for all resources
        if resource_type != "Resource" {
            elements.extend(vec![
                serde_json::json!({
                    "id": format!("{}.id", resource_type),
                    "path": format!("{}.id", resource_type),
                    "short": "Logical id of this artifact",
                    "definition": "The logical id of the resource",
                    "min": 0,
                    "max": "1",
                    "type": [{"code": "id"}]
                }),
                serde_json::json!({
                    "id": format!("{}.meta", resource_type),
                    "path": format!("{}.meta", resource_type),
                    "short": "Metadata about the resource",
                    "definition": "The metadata about the resource",
                    "min": 0,
                    "max": "1",
                    "type": [{"code": "Meta"}]
                }),
                serde_json::json!({
                    "id": format!("{}.implicitRules", resource_type),
                    "path": format!("{}.implicitRules", resource_type),
                    "short": "A set of rules under which this content was created",
                    "definition": "A reference to a set of rules that were followed when the resource was constructed",
                    "min": 0,
                    "max": "1",
                    "type": [{"code": "uri"}]
                }),
                serde_json::json!({
                    "id": format!("{}.language", resource_type),
                    "path": format!("{}.language", resource_type),
                    "short": "Language of the resource content",
                    "definition": "The base language in which the resource is written",
                    "min": 0,
                    "max": "1",
                    "type": [{"code": "code"}]
                })
            ]);
        }

        // Add domain resource elements
        if resource_type != "Resource" && resource_type != "DomainResource" {
            elements.extend(vec![
                serde_json::json!({
                    "id": format!("{}.text", resource_type),
                    "path": format!("{}.text", resource_type),
                    "short": "Text summary of the resource",
                    "definition": "A human-readable narrative that contains a summary of the resource",
                    "min": 0,
                    "max": "1",
                    "type": [{"code": "Narrative"}]
                }),
                serde_json::json!({
                    "id": format!("{}.contained", resource_type),
                    "path": format!("{}.contained", resource_type),
                    "short": "Contained, inline Resources",
                    "definition": "These resources do not have an independent existence",
                    "min": 0,
                    "max": "*",
                    "type": [{"code": "Resource"}]
                })
            ]);
        }

        elements
    }

    /// Save schemas to binary file
    async fn save_schemas_binary<P: AsRef<Path>>(
        &self,
        schemas: &[FhirSchema],
        output_path: P,
    ) -> Result<()> {
        let path = output_path.as_ref();

        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| FhirSchemaError::io_error(&e.to_string()))?;
        }

        // Serialize schemas to JSON (more compatible than bincode)
        let json_data = serde_json::to_vec(schemas)
            .map_err(|e| FhirSchemaError::serialization_error(&e.to_string()))?;

        // Write to file
        tokio::fs::write(path, json_data)
            .await
            .map_err(|e| FhirSchemaError::io_error(&e.to_string()))?;

        tracing::debug!("Saved {} schemas to {}", schemas.len(), path.display());
        Ok(())
    }

    /// Get file size
    async fn get_file_size<P: AsRef<Path>>(&self, path: P) -> Result<u64> {
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|e| FhirSchemaError::io_error(&e.to_string()))?;
        Ok(metadata.len())
    }

    /// Build all core FHIR versions
    pub async fn build_all_versions<P: AsRef<Path>>(
        output_dir: P,
    ) -> Result<Vec<SchemaBuilderResult>> {
        let output_dir = output_dir.as_ref();
        let mut results = Vec::new();

        for version in &[
            FhirVersion::R4,
            FhirVersion::R4B,
            FhirVersion::R5,
            FhirVersion::R6,
        ] {
            let builder = Self::new(*version).await?;

            let package_name = match version {
                FhirVersion::R4 => "hl7.fhir.r4.core",
                FhirVersion::R4B => "hl7.fhir.r4b.core",
                FhirVersion::R5 => "hl7.fhir.r5.core",
                FhirVersion::R6 => "hl7.fhir.r6.core",
            };

            let output_file = output_dir.join(format!("{}_schemas.bin", version.short_name()));

            let result = builder
                .build_and_save_schemas(package_name, version.package_version(), output_file)
                .await?;

            results.push(result);
        }

        Ok(results)
    }
}

/// Result of schema building operation
#[derive(Debug, Clone)]
pub struct SchemaBuilderResult {
    pub fhir_version: FhirVersion,
    pub package_name: String,
    pub package_version: String,
    pub schema_count: usize,
    pub build_duration_ms: u64,
    pub conversion_errors: usize,
    pub output_path: std::path::PathBuf,
    pub file_size_bytes: u64,
}

impl SchemaBuilderResult {
    /// Get human-readable file size
    pub fn file_size_human(&self) -> String {
        let size = self.file_size_bytes as f64;
        if size < 1024.0 {
            format!("{} bytes", self.file_size_bytes)
        } else if size < 1024.0 * 1024.0 {
            format!("{:.2} KB", size / 1024.0)
        } else if size < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.2} MB", size / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", size / (1024.0 * 1024.0 * 1024.0))
        }
    }

    /// Print build summary
    pub fn print_summary(&self) {
        println!(
            "✅ Built schemas for FHIR {} ({}@{})",
            self.fhir_version, self.package_name, self.package_version
        );
        println!(
            "   📊 {} schemas in {}ms",
            self.schema_count, self.build_duration_ms
        );
        println!(
            "   📁 {} ({})",
            self.output_path.display(),
            self.file_size_human()
        );
        if self.conversion_errors > 0 {
            println!("   ⚠️  {} conversion errors", self.conversion_errors);
        }
    }
}
