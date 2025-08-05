use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use url::Url;

use super::{ConverterConfig, StructureDefinition};
use crate::{FhirSchema, FhirSchemaError, Result};
use octofhir_canonical_manager::CanonicalManager;
use std::sync::Arc;

pub struct ConversionContext {
    pub config: ConverterConfig,
    pub current_structure_definition: Option<StructureDefinition>,
    pub resolved_profiles: HashMap<Url, StructureDefinition>,
    pub processed_elements: HashSet<String>,
    pub choice_type_expansions: HashMap<String, Vec<String>>,
    pub canonical_manager: Option<Arc<CanonicalManager>>,
    pub conversion_stats: ConversionStats,
    start_time: Option<Instant>,
}

#[derive(Debug, Clone, Default)]
pub struct ConversionStats {
    pub elements_processed: usize,
    pub choice_types_expanded: usize,
    pub slices_processed: usize,
    pub constraints_converted: usize,
    pub profiles_resolved: usize,
    pub conversion_duration: Option<Duration>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl ConversionContext {
    pub fn new(config: &ConverterConfig) -> Self {
        Self {
            config: config.clone(),
            current_structure_definition: None,
            resolved_profiles: HashMap::new(),
            processed_elements: HashSet::new(),
            choice_type_expansions: HashMap::new(),
            canonical_manager: None,
            conversion_stats: ConversionStats::default(),
            start_time: None,
        }
    }

    pub fn with_canonical_manager(
        config: &ConverterConfig,
        canonical_manager: Arc<CanonicalManager>,
    ) -> Self {
        Self {
            config: config.clone(),
            current_structure_definition: None,
            resolved_profiles: HashMap::new(),
            processed_elements: HashSet::new(),
            choice_type_expansions: HashMap::new(),
            canonical_manager: Some(canonical_manager),
            conversion_stats: ConversionStats::default(),
            start_time: None,
        }
    }

    pub fn begin_conversion(&mut self, structure_definition: &StructureDefinition) -> Result<()> {
        self.start_time = Some(Instant::now());
        self.current_structure_definition = Some(structure_definition.clone());
        self.processed_elements.clear();
        self.choice_type_expansions.clear();
        self.conversion_stats = ConversionStats::default();

        if let Some(url) = &structure_definition.url {
            self.add_info(format!(
                "Beginning conversion of StructureDefinition: {url}"
            ));
        }

        Ok(())
    }

    pub fn end_conversion(&mut self, _schema: &FhirSchema) -> Result<()> {
        if let Some(start_time) = self.start_time {
            self.conversion_stats.conversion_duration = Some(start_time.elapsed());
        }

        self.add_info(format!(
            "Conversion completed. Processed {} elements, expanded {} choice types, processed {} slices, converted {} constraints",
            self.conversion_stats.elements_processed,
            self.conversion_stats.choice_types_expanded,
            self.conversion_stats.slices_processed,
            self.conversion_stats.constraints_converted
        ));

        Ok(())
    }

    pub fn mark_element_processed(&mut self, path: &str) {
        self.processed_elements.insert(path.to_string());
        self.conversion_stats.elements_processed += 1;
    }

    pub fn is_element_processed(&self, path: &str) -> bool {
        self.processed_elements.contains(path)
    }

    pub fn add_choice_type_expansion(&mut self, base_path: String, expanded_paths: Vec<String>) {
        self.choice_type_expansions
            .insert(base_path, expanded_paths);
        self.conversion_stats.choice_types_expanded += 1;
    }

    pub fn get_choice_type_expansions(&self, base_path: &str) -> Option<&Vec<String>> {
        self.choice_type_expansions.get(base_path)
    }

    pub fn resolve_profile(&mut self, url: &Url) -> Result<Option<&StructureDefinition>> {
        if self.resolved_profiles.contains_key(url) {
            return Ok(self.resolved_profiles.get(url));
        }

        if !self.config.resolve_profiles {
            return Ok(None);
        }

        // Sync method - only warn about missing async functionality
        if self.canonical_manager.is_some() {
            self.add_warning(format!(
                "Profile resolution via canonical manager requires async conversion: {url}"
            ));
        } else {
            self.add_warning(format!(
                "Profile resolution requested but no canonical manager configured: {url}"
            ));
        }

        Ok(None)
    }

    pub async fn resolve_profile_async(
        &mut self,
        url: &Url,
    ) -> Result<Option<&StructureDefinition>> {
        if self.resolved_profiles.contains_key(url) {
            return Ok(self.resolved_profiles.get(url));
        }

        if !self.config.resolve_profiles {
            return Ok(None);
        }

        if let Some(canonical_manager) = &self.canonical_manager {
            match canonical_manager.resolve(url.as_ref()).await {
                Ok(resolved_resource) => {
                    // Try to deserialize the resolved resource as a StructureDefinition
                    match serde_json::from_value::<StructureDefinition>(
                        resolved_resource.resource.content,
                    ) {
                        Ok(mut structure_def) => {
                            // Extract elements if needed
                            if structure_def.extract_elements().is_ok() {
                                self.add_resolved_profile(url.clone(), structure_def);
                                return Ok(self.resolved_profiles.get(url));
                            } else {
                                self.add_warning(format!("Failed to extract elements from resolved StructureDefinition: {url}"));
                            }
                        }
                        Err(e) => {
                            self.add_warning(format!(
                                "Failed to parse resolved resource as StructureDefinition {url}: {e}"
                            ));
                        }
                    }
                }
                Err(e) => {
                    self.add_warning(format!(
                        "Failed to resolve profile via canonical manager {url}: {e}"
                    ));
                }
            }
        } else {
            self.add_warning(format!(
                "Profile resolution requested but no canonical manager configured: {url}"
            ));
        }

        Ok(None)
    }

    pub fn add_resolved_profile(&mut self, url: Url, structure_definition: StructureDefinition) {
        self.resolved_profiles.insert(url, structure_definition);
        self.conversion_stats.profiles_resolved += 1;
    }

    pub fn increment_slices_processed(&mut self) {
        self.conversion_stats.slices_processed += 1;
    }

    pub fn increment_constraints_converted(&mut self) {
        self.conversion_stats.constraints_converted += 1;
    }

    pub fn add_warning(&mut self, message: String) {
        self.conversion_stats.warnings.push(message);
    }

    pub fn add_error(&mut self, message: String) {
        self.conversion_stats.errors.push(message);
    }

    pub fn add_info(&mut self, _message: String) {
        // For now, just ignore info messages. Could be logged later.
    }

    pub fn get_stats(&self) -> &ConversionStats {
        &self.conversion_stats
    }

    pub fn has_errors(&self) -> bool {
        !self.conversion_stats.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.conversion_stats.warnings.is_empty()
    }

    pub fn validate_state(&self) -> Result<()> {
        if self.current_structure_definition.is_none() {
            return Err(FhirSchemaError::Conversion {
                message: "No current StructureDefinition in conversion context".to_string(),
            });
        }
        Ok(())
    }
}

impl ConversionStats {
    pub fn total_items_processed(&self) -> usize {
        self.elements_processed
            + self.choice_types_expanded
            + self.slices_processed
            + self.constraints_converted
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.total_items_processed();
        if total == 0 {
            return 1.0;
        }
        let successful = total - self.errors.len();
        successful as f64 / total as f64
    }
}
