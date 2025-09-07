// Type hierarchy builder for complex FHIR type relationships

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::ResolutionContext;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct TypeHierarchy {
    pub type_name: String,
    pub parent_type: Option<String>,
    pub child_types: Vec<String>,
    pub interfaces: Vec<String>,
    pub is_abstract: bool,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct TypeRelationship {
    pub source_type: String,
    pub target_type: String,
    pub relationship_type: RelationshipType,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationshipType {
    Inheritance, // is-a relationship
    Composition, // has-a relationship
    Interface,   // implements interface
    Reference,   // references another type
    Extension,   // extends via FHIR extension
}

/// Advanced type hierarchy builder for complex FHIR type relationships
// Debug impl manually provided below due to CanonicalManager not implementing Debug
pub struct TypeHierarchyBuilder {
    // Canonical manager for accessing FHIR definitions
    #[allow(dead_code)]
    canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,

    // Cache for built hierarchies
    hierarchy_cache: Arc<RwLock<HashMap<String, Vec<String>>>>,

    // Type relationship cache
    relationship_cache: Arc<RwLock<HashMap<String, Vec<TypeRelationship>>>>,

    // Built-in FHIR type hierarchy
    fhir_type_hierarchy: HashMap<String, TypeHierarchy>,
}

impl TypeHierarchyBuilder {
    pub async fn new(
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
    ) -> Result<Self> {
        let mut builder = Self {
            canonical_manager,
            hierarchy_cache: Arc::new(RwLock::new(HashMap::new())),
            relationship_cache: Arc::new(RwLock::new(HashMap::new())),
            fhir_type_hierarchy: HashMap::new(),
        };

        builder.build_fhir_core_hierarchy().await?;
        Ok(builder)
    }

    /// Build the core FHIR type hierarchy
    async fn build_fhir_core_hierarchy(&mut self) -> Result<()> {
        // Base types
        self.add_type_hierarchy(TypeHierarchy {
            type_name: "Element".to_string(),
            parent_type: None,
            child_types: vec!["BackboneElement".to_string(), "DataType".to_string()],
            interfaces: Vec::new(),
            is_abstract: true,
            depth: 0,
        });

        self.add_type_hierarchy(TypeHierarchy {
            type_name: "BackboneElement".to_string(),
            parent_type: Some("Element".to_string()),
            child_types: Vec::new(),
            interfaces: Vec::new(),
            is_abstract: false,
            depth: 1,
        });

        self.add_type_hierarchy(TypeHierarchy {
            type_name: "DataType".to_string(),
            parent_type: Some("Element".to_string()),
            child_types: vec!["PrimitiveType".to_string(), "ComplexType".to_string()],
            interfaces: Vec::new(),
            is_abstract: true,
            depth: 1,
        });

        // Primitive types
        self.add_type_hierarchy(TypeHierarchy {
            type_name: "PrimitiveType".to_string(),
            parent_type: Some("DataType".to_string()),
            child_types: vec![
                "boolean".to_string(),
                "integer".to_string(),
                "string".to_string(),
                "decimal".to_string(),
                "uri".to_string(),
                "url".to_string(),
                "canonical".to_string(),
                "base64Binary".to_string(),
                "instant".to_string(),
                "date".to_string(),
                "dateTime".to_string(),
                "time".to_string(),
                "code".to_string(),
                "oid".to_string(),
                "id".to_string(),
                "markdown".to_string(),
                "unsignedInt".to_string(),
                "positiveInt".to_string(),
                "uuid".to_string(),
            ],
            interfaces: Vec::new(),
            is_abstract: true,
            depth: 2,
        });

        // Complex types
        self.add_type_hierarchy(TypeHierarchy {
            type_name: "ComplexType".to_string(),
            parent_type: Some("DataType".to_string()),
            child_types: vec![
                "Quantity".to_string(),
                "CodeableConcept".to_string(),
                "Coding".to_string(),
                "Reference".to_string(),
                "Period".to_string(),
                "Range".to_string(),
                "Ratio".to_string(),
                "Address".to_string(),
                "HumanName".to_string(),
                "ContactPoint".to_string(),
                "Identifier".to_string(),
                "Attachment".to_string(),
                "Meta".to_string(),
                "Narrative".to_string(),
                "Extension".to_string(),
            ],
            interfaces: Vec::new(),
            is_abstract: true,
            depth: 2,
        });

        // Resource hierarchy
        self.add_type_hierarchy(TypeHierarchy {
            type_name: "Resource".to_string(),
            parent_type: None,
            child_types: vec![
                "DomainResource".to_string(),
                "Bundle".to_string(),
                "Parameters".to_string(),
                "OperationOutcome".to_string(),
            ],
            interfaces: Vec::new(),
            is_abstract: true,
            depth: 0,
        });

        self.add_type_hierarchy(TypeHierarchy {
            type_name: "DomainResource".to_string(),
            parent_type: Some("Resource".to_string()),
            child_types: vec![
                "Patient".to_string(),
                "Practitioner".to_string(),
                "Organization".to_string(),
                "Observation".to_string(),
                "Condition".to_string(),
                "Procedure".to_string(),
                "MedicationRequest".to_string(),
                "DiagnosticReport".to_string(),
                "Encounter".to_string(),
                "Location".to_string(),
            ],
            interfaces: Vec::new(),
            is_abstract: true,
            depth: 1,
        });

        // Add specific primitive types
        for primitive in &[
            "boolean",
            "integer",
            "string",
            "decimal",
            "uri",
            "url",
            "canonical",
            "base64Binary",
            "instant",
            "date",
            "dateTime",
            "time",
            "code",
            "oid",
            "id",
            "markdown",
            "unsignedInt",
            "positiveInt",
            "uuid",
        ] {
            self.add_type_hierarchy(TypeHierarchy {
                type_name: primitive.to_string(),
                parent_type: Some("PrimitiveType".to_string()),
                child_types: Vec::new(),
                interfaces: Vec::new(),
                is_abstract: false,
                depth: 3,
            });
        }

        // Add specific complex types
        for complex in &[
            "Quantity",
            "CodeableConcept",
            "Coding",
            "Reference",
            "Period",
            "Range",
            "Ratio",
            "Address",
            "HumanName",
            "ContactPoint",
            "Identifier",
            "Attachment",
            "Meta",
            "Narrative",
            "Extension",
        ] {
            self.add_type_hierarchy(TypeHierarchy {
                type_name: complex.to_string(),
                parent_type: Some("ComplexType".to_string()),
                child_types: Vec::new(),
                interfaces: Vec::new(),
                is_abstract: false,
                depth: 3,
            });
        }

        // Add specific resource types
        for resource in &[
            "Patient",
            "Practitioner",
            "Organization",
            "Observation",
            "Condition",
            "Procedure",
            "MedicationRequest",
            "DiagnosticReport",
            "Encounter",
            "Location",
        ] {
            self.add_type_hierarchy(TypeHierarchy {
                type_name: resource.to_string(),
                parent_type: Some("DomainResource".to_string()),
                child_types: Vec::new(),
                interfaces: Vec::new(),
                is_abstract: false,
                depth: 2,
            });
        }

        Ok(())
    }

    /// Add a type hierarchy to the internal structure
    fn add_type_hierarchy(&mut self, hierarchy: TypeHierarchy) {
        self.fhir_type_hierarchy
            .insert(hierarchy.type_name.clone(), hierarchy);
    }

    /// Build complete hierarchy for a given type
    pub async fn build_hierarchy(
        &self,
        type_name: &str,
        _context: &ResolutionContext,
    ) -> Result<Vec<String>> {
        let cache_key = format!("hierarchy:{type_name}");

        // Check cache first
        {
            let cache = self.hierarchy_cache.read().await;
            if let Some(cached_hierarchy) = cache.get(&cache_key) {
                return Ok(cached_hierarchy.clone());
            }
        }

        // Build hierarchy
        let hierarchy = self.build_hierarchy_internal(type_name).await?;

        // Cache the result
        {
            let mut cache = self.hierarchy_cache.write().await;
            cache.insert(cache_key, hierarchy.clone());
        }

        Ok(hierarchy)
    }

    /// Internal hierarchy building logic
    async fn build_hierarchy_internal(&self, type_name: &str) -> Result<Vec<String>> {
        let mut hierarchy = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(type_name.to_string());

        while let Some(current_type) = queue.pop_front() {
            if visited.contains(&current_type) {
                continue;
            }

            visited.insert(current_type.clone());
            hierarchy.push(current_type.clone());

            // Add parent types
            if let Some(parent) = self.get_parent_type(&current_type) {
                if !visited.contains(&parent) {
                    queue.push_back(parent);
                }
            }

            // Add interface types
            for interface in self.get_interface_types(&current_type) {
                if !visited.contains(&interface) {
                    queue.push_back(interface);
                }
            }
        }

        // Sort by hierarchy depth (most specific first)
        hierarchy.sort_by(|a, b| {
            let depth_a = self.get_type_depth(a);
            let depth_b = self.get_type_depth(b);
            depth_b.cmp(&depth_a)
        });

        Ok(hierarchy)
    }

    /// Get parent type for a given type
    fn get_parent_type(&self, type_name: &str) -> Option<String> {
        self.fhir_type_hierarchy
            .get(type_name)
            .and_then(|h| h.parent_type.clone())
    }

    /// Get interface types for a given type
    fn get_interface_types(&self, type_name: &str) -> Vec<String> {
        self.fhir_type_hierarchy
            .get(type_name)
            .map(|h| h.interfaces.clone())
            .unwrap_or_default()
    }

    /// Get type depth in hierarchy
    fn get_type_depth(&self, type_name: &str) -> usize {
        self.fhir_type_hierarchy
            .get(type_name)
            .map(|h| h.depth)
            .unwrap_or(0)
    }

    /// Check if one type is a subtype of another
    pub async fn is_subtype(
        &self,
        child_type: &str,
        parent_type: &str,
        context: &ResolutionContext,
    ) -> Result<bool> {
        if child_type == parent_type {
            return Ok(true);
        }

        let hierarchy = self.build_hierarchy(child_type, context).await?;
        Ok(hierarchy.contains(&parent_type.to_string()))
    }

    /// Get all subtypes of a given type
    pub async fn get_subtypes(&self, parent_type: &str) -> Result<Vec<String>> {
        self.get_subtypes_recursive(parent_type, &mut std::collections::HashSet::new())
            .await
    }

    /// Internal recursive implementation for get_subtypes with cycle detection
    fn get_subtypes_recursive<'a>(
        &'a self,
        parent_type: &'a str,
        visited: &'a mut std::collections::HashSet<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<String>>> + Send + 'a>> {
        Box::pin(async move {
            if visited.contains(parent_type) {
                return Ok(Vec::new()); // Prevent infinite recursion
            }

            visited.insert(parent_type.to_string());
            let mut subtypes = Vec::new();

            for (type_name, hierarchy) in &self.fhir_type_hierarchy {
                if let Some(parent) = &hierarchy.parent_type {
                    if parent == parent_type {
                        subtypes.push(type_name.clone());
                        // Recursively get subtypes
                        let nested_subtypes =
                            self.get_subtypes_recursive(type_name, visited).await?;
                        subtypes.extend(nested_subtypes);
                    }
                }
            }

            visited.remove(parent_type);
            subtypes.sort();
            subtypes.dedup();
            Ok(subtypes)
        })
    }

    /// Get the most specific common ancestor of two types
    pub async fn get_common_ancestor(
        &self,
        type1: &str,
        type2: &str,
        context: &ResolutionContext,
    ) -> Result<Option<String>> {
        if type1 == type2 {
            return Ok(Some(type1.to_string()));
        }

        let hierarchy1 = self.build_hierarchy(type1, context).await?;
        let hierarchy2 = self.build_hierarchy(type2, context).await?;

        // Find first common type in hierarchies
        for type_name in &hierarchy1 {
            if hierarchy2.contains(type_name) {
                return Ok(Some(type_name.clone()));
            }
        }

        Ok(None)
    }

    /// Build type relationships for analysis
    pub async fn build_type_relationships(
        &self,
        type_name: &str,
        _context: &ResolutionContext,
    ) -> Result<Vec<TypeRelationship>> {
        let cache_key = format!("relationships:{type_name}");

        // Check cache first
        {
            let cache = self.relationship_cache.read().await;
            if let Some(cached_relationships) = cache.get(&cache_key) {
                return Ok(cached_relationships.clone());
            }
        }

        let mut relationships = Vec::new();

        // Add inheritance relationships
        if let Some(parent) = self.get_parent_type(type_name) {
            relationships.push(TypeRelationship {
                source_type: type_name.to_string(),
                target_type: parent,
                relationship_type: RelationshipType::Inheritance,
                confidence: 1.0,
            });
        }

        // Add interface relationships
        for interface in self.get_interface_types(type_name) {
            relationships.push(TypeRelationship {
                source_type: type_name.to_string(),
                target_type: interface,
                relationship_type: RelationshipType::Interface,
                confidence: 1.0,
            });
        }

        // Add child relationships
        let subtypes = self.get_subtypes(type_name).await?;
        for subtype in subtypes {
            relationships.push(TypeRelationship {
                source_type: subtype,
                target_type: type_name.to_string(),
                relationship_type: RelationshipType::Inheritance,
                confidence: 1.0,
            });
        }

        // Cache the result
        {
            let mut cache = self.relationship_cache.write().await;
            cache.insert(cache_key, relationships.clone());
        }

        Ok(relationships)
    }

    /// Get type compatibility score between two types
    pub async fn get_compatibility_score(
        &self,
        source_type: &str,
        target_type: &str,
        context: &ResolutionContext,
    ) -> Result<f64> {
        if source_type == target_type {
            return Ok(1.0);
        }

        // Check if source is subtype of target
        if self.is_subtype(source_type, target_type, context).await? {
            let source_depth = self.get_type_depth(source_type) as f64;
            let target_depth = self.get_type_depth(target_type) as f64;

            // Higher score for closer types
            let depth_diff = (source_depth - target_depth).abs();
            return Ok(1.0 - (depth_diff * 0.1).min(0.8));
        }

        // Check for common ancestor
        if let Some(_common) = self
            .get_common_ancestor(source_type, target_type, context)
            .await?
        {
            return Ok(0.3); // Some compatibility through common ancestor
        }

        Ok(0.0) // No compatibility
    }

    /// Clear all caches
    pub async fn clear_caches(&self) {
        let mut hierarchy_cache = self.hierarchy_cache.write().await;
        let mut relationship_cache = self.relationship_cache.write().await;

        hierarchy_cache.clear();
        relationship_cache.clear();
    }

    /// Add custom type hierarchy (for extensions or profiles)
    pub async fn add_custom_type_hierarchy(&mut self, hierarchy: TypeHierarchy) {
        self.fhir_type_hierarchy
            .insert(hierarchy.type_name.clone(), hierarchy);

        // Clear caches as they may now be invalid
        self.clear_caches().await;
    }

    /// Get type hierarchy information
    pub fn get_type_info(&self, type_name: &str) -> Option<&TypeHierarchy> {
        self.fhir_type_hierarchy.get(type_name)
    }

    /// Get all known types
    pub fn get_all_types(&self) -> Vec<String> {
        self.fhir_type_hierarchy.keys().cloned().collect()
    }
}

impl TypeHierarchyBuilder {
    /// Create a new TypeHierarchyBuilder with provided canonical manager (sync version)
    pub fn new_sync(canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>) -> Self {
        Self {
            canonical_manager,
            hierarchy_cache: Arc::new(RwLock::new(HashMap::new())),
            relationship_cache: Arc::new(RwLock::new(HashMap::new())),
            fhir_type_hierarchy: HashMap::new(),
        }
    }

    /// Create a new TypeHierarchyBuilder with provided canonical manager (async version for compatibility)
    pub fn with_canonical_manager(
        canonical_manager: Arc<octofhir_canonical_manager::CanonicalManager>,
    ) -> Self {
        Self::new_sync(canonical_manager)
    }
}

impl std::fmt::Debug for TypeHierarchyBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeHierarchyBuilder")
            .field("canonical_manager", &"<CanonicalManager>")
            .finish()
    }
}
