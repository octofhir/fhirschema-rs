// Advanced path navigation and type inference system for FHIR elements

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::ResolutionContext;
use crate::error::{FhirSchemaError, Result};
use crate::types::{ElementDefinition, TypeResolver};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FhirPath {
    pub full_path: String,
    pub segments: Vec<PathSegment>,
    pub resource_type: Option<String>,
    pub is_choice_element: bool,
    pub choice_suffix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathSegment {
    pub name: String,
    pub is_array: bool,
    pub array_index: Option<usize>,
    pub is_choice: bool,
    pub choice_type: Option<String>,
    pub slice_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PathNavigationResult {
    pub element_definition: ElementDefinition,
    pub resolved_type: String,
    pub path: FhirPath,
    pub is_valid: bool,
    pub validation_messages: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TypeInferenceResult {
    pub inferred_type: String,
    pub confidence: f64,
    pub reasoning: String,
    pub alternative_types: Vec<(String, f64)>,
}

/// Advanced path navigation system for traversing FHIR element paths
pub struct PathNavigator {
    // Type resolver for resolving element types
    type_resolver: Arc<TypeResolver>,

    // Cache for path navigation results
    navigation_cache: Arc<RwLock<HashMap<String, PathNavigationResult>>>,

    // Cache for type inference results
    inference_cache: Arc<RwLock<HashMap<String, TypeInferenceResult>>>,

    // Canonical manager for accessing FHIR definitions
    #[allow(dead_code)]
    canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
}

impl PathNavigator {
    pub async fn new(
        type_resolver: Arc<TypeResolver>,
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
    ) -> Result<Self> {
        Ok(Self {
            type_resolver,
            navigation_cache: Arc::new(RwLock::new(HashMap::new())),
            inference_cache: Arc::new(RwLock::new(HashMap::new())),
            canonical_manager,
        })
    }

    /// Navigate to a specific FHIR path and return element information
    pub async fn navigate_path(
        &self,
        path: &str,
        context: &ResolutionContext,
    ) -> Result<PathNavigationResult> {
        let cache_key = format!("nav:{}:{}", path, context.hash());

        // Check cache first
        {
            let cache = self.navigation_cache.read().await;
            if let Some(cached_result) = cache.get(&cache_key) {
                return Ok(cached_result.clone());
            }
        }

        // Parse the path
        let fhir_path = self.parse_fhir_path(path)?;

        // Navigate through the path
        let result = self.navigate_path_internal(&fhir_path, context).await?;

        // Cache the result
        {
            let mut cache = self.navigation_cache.write().await;
            cache.insert(cache_key, result.clone());
        }

        Ok(result)
    }

    /// Parse a FHIR path string into structured components
    pub fn parse_fhir_path(&self, path: &str) -> Result<FhirPath> {
        let mut segments = Vec::new();
        let parts: Vec<&str> = path.split('.').collect();

        if parts.is_empty() {
            return Err(FhirSchemaError::path_error("Empty path provided"));
        }

        let resource_type = Some(parts[0].to_string());
        let is_choice_element = path.contains("[x]") || self.detect_choice_element_pattern(path);
        let choice_suffix = self.extract_choice_suffix_from_path(path);

        for (index, part) in parts.iter().enumerate() {
            let segment = self.parse_path_segment(part, index == 0)?;
            segments.push(segment);
        }

        Ok(FhirPath {
            full_path: path.to_string(),
            segments,
            resource_type,
            is_choice_element,
            choice_suffix,
        })
    }

    /// Parse an individual path segment
    fn parse_path_segment(&self, segment: &str, _is_root: bool) -> Result<PathSegment> {
        // Handle array notation like "contact[0]"
        let (name, array_index) = if let Some(bracket_pos) = segment.find('[') {
            let name = segment[..bracket_pos].to_string();
            let bracket_content = &segment[bracket_pos + 1..segment.len() - 1];

            if bracket_content == "x" {
                // Choice element
                (name, None)
            } else if let Ok(index) = bracket_content.parse::<usize>() {
                // Array index
                (name, Some(index))
            } else {
                // Slice name
                (name, None)
            }
        } else {
            (segment.to_string(), None)
        };

        let is_choice = segment.contains("[x]");
        let choice_type = if is_choice {
            None // Will be resolved during navigation
        } else {
            self.detect_choice_type_from_segment(&name)
        };

        let slice_name = if segment.contains('[') && !is_choice && array_index.is_none() {
            Some(segment[segment.find('[').unwrap() + 1..segment.len() - 1].to_string())
        } else {
            None
        };

        Ok(PathSegment {
            name,
            is_array: array_index.is_some(),
            array_index,
            is_choice,
            choice_type,
            slice_name,
        })
    }

    /// Navigate through a parsed FHIR path
    async fn navigate_path_internal(
        &self,
        fhir_path: &FhirPath,
        context: &ResolutionContext,
    ) -> Result<PathNavigationResult> {
        let mut current_type = fhir_path
            .resource_type
            .clone()
            .ok_or_else(|| FhirSchemaError::path_error("No resource type found in path"))?;

        let mut validation_messages = Vec::new();
        let mut element_definition = ElementDefinition::new(&fhir_path.full_path);

        // Navigate through each segment
        for (index, segment) in fhir_path.segments.iter().enumerate() {
            if index == 0 {
                // Root resource type - validate it exists
                match self.validate_resource_type(&current_type).await {
                    Ok(_) => {}
                    Err(e) => {
                        validation_messages
                            .push(format!("Invalid resource type '{current_type}': {e}"));
                    }
                }
                continue;
            }

            // Navigate to the next element
            match self
                .navigate_to_element(&current_type, segment, context)
                .await
            {
                Ok((next_type, element_def)) => {
                    current_type = next_type;
                    element_definition = element_def;
                }
                Err(e) => {
                    validation_messages.push(format!(
                        "Failed to navigate to '{}' from type '{}': {}",
                        segment.name, current_type, e
                    ));
                }
            }
        }

        Ok(PathNavigationResult {
            element_definition,
            resolved_type: current_type,
            path: fhir_path.clone(),
            is_valid: validation_messages.is_empty(),
            validation_messages,
        })
    }

    /// Navigate to a specific element within a type
    async fn navigate_to_element(
        &self,
        parent_type: &str,
        segment: &PathSegment,
        context: &ResolutionContext,
    ) -> Result<(String, ElementDefinition)> {
        // Handle choice elements
        if segment.is_choice {
            return self
                .resolve_choice_element(parent_type, segment, context)
                .await;
        }

        // Handle regular elements
        let element_path = format!("{}.{}", parent_type, segment.name);

        // Try to get element definition from canonical manager
        match self.get_element_definition(&element_path, context).await {
            Ok(element_def) => {
                let resolved_type = self.determine_element_type(&element_def, context).await?;
                Ok((resolved_type, element_def))
            }
            Err(_) => {
                // Fallback to type inference
                let inferred_type = self
                    .infer_element_type(parent_type, &segment.name, context)
                    .await?;
                let fallback_element = ElementDefinition::new(&element_path);
                Ok((inferred_type, fallback_element))
            }
        }
    }

    /// Resolve choice element navigation
    async fn resolve_choice_element(
        &self,
        parent_type: &str,
        segment: &PathSegment,
        context: &ResolutionContext,
    ) -> Result<(String, ElementDefinition)> {
        let base_name = segment.name.replace("[x]", "");
        let choice_suffix = segment.choice_type.as_deref().unwrap_or("");

        // Use the type resolver to resolve the choice type
        let resolved_types = self
            .type_resolver
            .resolve_choice_type(&base_name, choice_suffix, context)
            .await?;

        if resolved_types.is_empty() {
            return Err(FhirSchemaError::type_resolution_error(&format!(
                "No types resolved for choice element '{}'",
                segment.name
            )));
        }

        // Take the first (most relevant) resolved type
        let resolved_type = &resolved_types[0];
        let element_def = ElementDefinition::new(&format!("{}.{}", parent_type, segment.name))
            .with_cardinality(0, Some(1));

        Ok((resolved_type.type_name.clone(), element_def))
    }

    /// Get element definition from canonical manager
    async fn get_element_definition(
        &self,
        element_path: &str,
        _context: &ResolutionContext,
    ) -> Result<ElementDefinition> {
        // This is a placeholder implementation
        // In a real implementation, this would query the canonical manager
        // for the StructureDefinition and extract the element definition

        Ok(ElementDefinition::new(element_path))
    }

    /// Determine the type of an element from its definition
    async fn determine_element_type(
        &self,
        element_def: &ElementDefinition,
        _context: &ResolutionContext,
    ) -> Result<String> {
        // Extract type from element definition
        if let Some(element_types) = &element_def.element_type {
            if let Some(first_type) = element_types.first() {
                return Ok(first_type.code.clone());
            }
        }

        // Fallback to string type
        Ok("string".to_string())
    }

    /// Infer element type using context and heuristics
    pub async fn infer_element_type(
        &self,
        parent_type: &str,
        element_name: &str,
        context: &ResolutionContext,
    ) -> Result<String> {
        let cache_key = format!("infer:{}:{}:{}", parent_type, element_name, context.hash());

        // Check cache first
        {
            let cache = self.inference_cache.read().await;
            if let Some(cached_result) = cache.get(&cache_key) {
                return Ok(cached_result.inferred_type.clone());
            }
        }

        // Perform type inference
        let inference_result = self
            .infer_type_internal(parent_type, element_name, context)
            .await?;

        // Cache the result
        {
            let mut cache = self.inference_cache.write().await;
            cache.insert(cache_key, inference_result.clone());
        }

        Ok(inference_result.inferred_type)
    }

    /// Internal type inference logic
    async fn infer_type_internal(
        &self,
        parent_type: &str,
        element_name: &str,
        context: &ResolutionContext,
    ) -> Result<TypeInferenceResult> {
        let mut alternatives = Vec::new();
        let mut confidence = 0.5; // Base confidence
        let reasoning;

        // Pattern-based inference
        let inferred_type = match element_name {
            // Common primitive patterns
            name if name.ends_with("Date") || name.ends_with("DateTime") => {
                confidence = 0.9;
                reasoning = "Element name suggests date/time type".to_string();
                alternatives.push(("date".to_string(), 0.7));
                "dateTime".to_string()
            }
            name if name.ends_with("Code") || name.ends_with("System") => {
                confidence = 0.8;
                reasoning = "Element name suggests code type".to_string();
                alternatives.push(("string".to_string(), 0.6));
                "code".to_string()
            }
            name if name.ends_with("Url") || name.ends_with("Uri") => {
                confidence = 0.9;
                reasoning = "Element name suggests URI type".to_string();
                alternatives.push(("string".to_string(), 0.5));
                "uri".to_string()
            }
            name if name.ends_with("Id") || name == "id" => {
                confidence = 0.9;
                reasoning = "Element name suggests ID type".to_string();
                "id".to_string()
            }
            name if name.ends_with("Count") || name.ends_with("Number") => {
                confidence = 0.8;
                reasoning = "Element name suggests integer type".to_string();
                alternatives.push(("decimal".to_string(), 0.6));
                "integer".to_string()
            }
            name if name.ends_with("Flag") || name.starts_with("is") || name.starts_with("has") => {
                confidence = 0.9;
                reasoning = "Element name suggests boolean type".to_string();
                "boolean".to_string()
            }
            // Complex type patterns
            name if name.ends_with("Reference") || name.ends_with("Ref") => {
                confidence = 0.8;
                reasoning = "Element name suggests Reference type".to_string();
                "Reference".to_string()
            }
            name if name.ends_with("Coding") => {
                confidence = 0.9;
                reasoning = "Element name suggests Coding type".to_string();
                alternatives.push(("CodeableConcept".to_string(), 0.7));
                "Coding".to_string()
            }
            name if name.ends_with("Concept") => {
                confidence = 0.8;
                reasoning = "Element name suggests CodeableConcept type".to_string();
                alternatives.push(("Coding".to_string(), 0.6));
                "CodeableConcept".to_string()
            }
            // Context-based inference
            _ => {
                reasoning = "Using context-based inference".to_string();
                self.infer_from_context(parent_type, element_name, context)
                    .await?
            }
        };

        Ok(TypeInferenceResult {
            inferred_type,
            confidence,
            reasoning,
            alternative_types: alternatives,
        })
    }

    /// Infer type from parent context
    async fn infer_from_context(
        &self,
        parent_type: &str,
        element_name: &str,
        _context: &ResolutionContext,
    ) -> Result<String> {
        // Context-specific inference rules
        match (parent_type, element_name) {
            ("Patient", "name") => Ok("HumanName".to_string()),
            ("Patient", "telecom") => Ok("ContactPoint".to_string()),
            ("Patient", "address") => Ok("Address".to_string()),
            ("Patient", "identifier") => Ok("Identifier".to_string()),
            ("Patient", "gender") => Ok("code".to_string()),
            ("Patient", "birthDate") => Ok("date".to_string()),
            ("Patient", "active") => Ok("boolean".to_string()),

            ("Observation", "value") => Ok("Quantity".to_string()), // Most common
            ("Observation", "component") => Ok("BackboneElement".to_string()),
            ("Observation", "code") => Ok("CodeableConcept".to_string()),
            ("Observation", "subject") => Ok("Reference".to_string()),

            (_, "extension") => Ok("Extension".to_string()),
            (_, "modifierExtension") => Ok("Extension".to_string()),
            (_, "meta") => Ok("Meta".to_string()),
            (_, "text") => Ok("Narrative".to_string()),

            // Fallback to string
            _ => Ok("string".to_string()),
        }
    }

    /// Validate that a resource type exists
    async fn validate_resource_type(&self, resource_type: &str) -> Result<()> {
        // Check against common FHIR resource types
        let common_resources = [
            "Patient",
            "Practitioner",
            "Organization",
            "Location",
            "Observation",
            "Condition",
            "Procedure",
            "MedicationRequest",
            "DiagnosticReport",
            "Encounter",
            "Bundle",
            "OperationOutcome",
            "CapabilityStatement",
            "StructureDefinition",
            "ValueSet",
            "CodeSystem",
            "ConceptMap",
        ];

        if common_resources.contains(&resource_type) {
            return Ok(());
        }

        // In a real implementation, this would query the canonical manager
        // to check if the resource type is valid
        Ok(())
    }

    /// Detect choice element patterns in path
    fn detect_choice_element_pattern(&self, path: &str) -> bool {
        // Common choice element patterns
        let choice_patterns = ["value", "onset", "effective", "multipleBirth", "deceased"];

        choice_patterns.iter().any(|pattern| {
            path.contains(&format!("{pattern}[x]")) || self.has_choice_suffix_pattern(path, pattern)
        })
    }

    /// Check if path has choice suffix pattern (e.g., valueString, onsetDateTime)
    fn has_choice_suffix_pattern(&self, path: &str, base: &str) -> bool {
        if let Some(pos) = path.find(base) {
            let after_base = &path[pos + base.len()..];
            // Check if the next part looks like a type suffix
            after_base
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Extract choice suffix from path
    fn extract_choice_suffix_from_path(&self, path: &str) -> Option<String> {
        let choice_patterns = ["value", "onset", "effective", "multipleBirth", "deceased"];

        for pattern in &choice_patterns {
            if let Some(pos) = path.find(pattern) {
                let after_pattern = &path[pos + pattern.len()..];
                if let Some(dot_pos) = after_pattern.find('.') {
                    let suffix = &after_pattern[..dot_pos];
                    if !suffix.is_empty() && suffix != "[x]" {
                        return Some(suffix.to_string());
                    }
                } else if !after_pattern.is_empty() && after_pattern != "[x]" {
                    return Some(after_pattern.to_string());
                }
            }
        }

        None
    }

    /// Detect choice type from segment name
    fn detect_choice_type_from_segment(&self, segment_name: &str) -> Option<String> {
        let choice_patterns = ["value", "onset", "effective", "multipleBirth", "deceased"];

        for pattern in &choice_patterns {
            if segment_name.starts_with(pattern) && segment_name.len() > pattern.len() {
                return Some(segment_name[pattern.len()..].to_string());
            }
        }

        None
    }

    /// Clear all caches
    pub async fn clear_caches(&self) {
        let mut nav_cache = self.navigation_cache.write().await;
        let mut inf_cache = self.inference_cache.write().await;

        nav_cache.clear();
        inf_cache.clear();
    }

    /// Get navigation statistics
    pub async fn get_navigation_stats(&self) -> (usize, usize) {
        let nav_cache = self.navigation_cache.read().await;
        let inf_cache = self.inference_cache.read().await;

        (nav_cache.len(), inf_cache.len())
    }

    /// Validate a path without full navigation
    pub async fn validate_path_syntax(&self, path: &str) -> Result<bool> {
        match self.parse_fhir_path(path) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get all possible paths from a resource type
    pub async fn get_possible_paths(
        &self,
        resource_type: &str,
        max_depth: usize,
    ) -> Result<Vec<String>> {
        let mut paths = Vec::new();
        let mut queue = VecDeque::new();

        queue.push_back((resource_type.to_string(), 0));

        while let Some((current_path, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            paths.push(current_path.clone());

            // In a real implementation, this would query the canonical manager
            // to get all child elements and add them to the queue
        }

        Ok(paths)
    }
}

impl PathNavigator {
    /// Create a new PathNavigator with provided canonical manager
    pub fn with_canonical_manager(
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
    ) -> Self {
        let type_resolver = Arc::new(TypeResolver::with_canonical_manager(
            canonical_manager.clone(),
        ));

        Self {
            type_resolver,
            navigation_cache: Arc::new(RwLock::new(HashMap::new())),
            inference_cache: Arc::new(RwLock::new(HashMap::new())),
            canonical_manager,
        }
    }
}
