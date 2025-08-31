use std::sync::Arc;

use super::path_resolver::PathResolver;
use crate::package::registry::SchemaIndex;

/// Navigation helper for path traversal
pub struct PathNavigator {
    resolver: Arc<PathResolver>,
    schema_index: Arc<SchemaIndex>,
}

impl PathNavigator {
    pub fn new(resolver: Arc<PathResolver>, schema_index: Arc<SchemaIndex>) -> Self {
        Self {
            resolver,
            schema_index,
        }
    }

    /// Navigate to child elements
    pub async fn navigate_to_children(&self, base_type: &str, path: &str) -> Vec<NavigationOption> {
        let Some(resolution) = self.resolver.resolve_path(base_type, path).await else {
            return Vec::new();
        };

        // Get all possible child paths from the target type
        self.get_child_paths(&resolution.target_type).await
    }

    /// Validate path exists and is accessible
    pub async fn validate_path(&self, base_type: &str, path: &str) -> PathValidationResult {
        match self.resolver.resolve_path(base_type, path).await {
            Some(resolution) => PathValidationResult {
                is_valid: true,
                target_type: Some(resolution.target_type),
                errors: Vec::new(),
                suggestions: Vec::new(),
            },
            None => {
                let suggestions = self.generate_path_suggestions(base_type, path).await;
                PathValidationResult {
                    is_valid: false,
                    target_type: None,
                    errors: vec!["Path not found".to_string()],
                    suggestions,
                }
            }
        }
    }

    /// Navigate backwards to parent elements
    pub async fn navigate_to_parent(
        &self,
        base_type: &str,
        path: &str,
    ) -> Option<NavigationOption> {
        if !path.contains('.') {
            // Already at root level
            return None;
        }

        let parts: Vec<&str> = path.rsplitn(2, '.').collect();
        if parts.len() != 2 {
            return None;
        }

        let parent_path = parts[1];
        let resolution = self.resolver.resolve_path(base_type, parent_path).await?;

        Some(NavigationOption {
            path: parent_path.to_string(),
            target_type: resolution.target_type,
            description: resolution.element_info.definition,
            is_collection: resolution.is_collection,
        })
    }

    /// Get breadcrumb navigation for a path
    pub async fn get_breadcrumbs(&self, base_type: &str, path: &str) -> Vec<BreadcrumbItem> {
        let mut breadcrumbs = Vec::new();
        let segments: Vec<&str> = path.split('.').collect();

        for i in 0..segments.len() {
            let current_path = segments[..=i].join(".");

            if let Some(resolution) = self.resolver.resolve_path(base_type, &current_path).await {
                breadcrumbs.push(BreadcrumbItem {
                    segment: segments[i].to_string(),
                    full_path: current_path,
                    target_type: resolution.target_type,
                    is_choice_type: resolution.is_choice_type,
                });
            }
        }

        breadcrumbs
    }

    /// Get all paths at a specific depth level
    pub async fn get_paths_at_depth(&self, base_type: &str, depth: usize) -> Vec<NavigationOption> {
        let all_paths = self.resolver.get_available_paths(base_type).await;

        let mut depth_paths = Vec::new();
        for path in all_paths {
            if path.matches('.').count() == depth {
                if let Some(resolution) = self.resolver.resolve_path(base_type, &path).await {
                    depth_paths.push(NavigationOption {
                        path,
                        target_type: resolution.target_type,
                        description: resolution.element_info.definition,
                        is_collection: resolution.is_collection,
                    });
                }
            }
        }

        depth_paths.sort_by(|a, b| a.path.cmp(&b.path));
        depth_paths
    }

    // === PRIVATE IMPLEMENTATION ===

    async fn get_child_paths(&self, type_name: &str) -> Vec<NavigationOption> {
        let Some(schema) = self.schema_index.get_schema_by_type(type_name).await else {
            return Vec::new();
        };

        let mut options = Vec::new();

        // Get direct child elements (depth 1)
        for (element_path, element) in &schema.elements {
            // Skip nested paths (those containing dots beyond the first level)
            if element_path.matches('.').count() <= 1 {
                let target_type = element
                    .element_type
                    .as_ref()
                    .and_then(|types| types.first())
                    .map(|t| t.code.clone())
                    .unwrap_or_else(|| "unknown".to_string());

                let is_collection = element
                    .max
                    .as_ref()
                    .map(|max| max == "*" || max.parse::<u32>().unwrap_or(1) > 1)
                    .unwrap_or(false);

                options.push(NavigationOption {
                    path: element_path.clone(),
                    target_type,
                    description: element.definition.clone(),
                    is_collection,
                });
            }
        }

        options.sort_by(|a, b| a.path.cmp(&b.path));
        options
    }

    pub async fn generate_path_suggestions(
        &self,
        base_type: &str,
        invalid_path: &str,
    ) -> Vec<String> {
        let available_paths = self.resolver.get_available_paths(base_type).await;

        // Simple similarity matching (could be enhanced with fuzzy matching)
        let mut suggestions: Vec<_> = available_paths
            .into_iter()
            .map(|path| (path.clone(), self.calculate_similarity(&path, invalid_path)))
            .filter(|(_, similarity)| *similarity > 0.6)
            .collect();

        suggestions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        suggestions
            .into_iter()
            .map(|(path, _)| path)
            .take(5)
            .collect()
    }

    fn calculate_similarity(&self, a: &str, b: &str) -> f64 {
        // Simple Levenshtein-based similarity
        let a_chars = a.chars().count();
        let b_chars = b.chars().count();
        let max_len = a_chars.max(b_chars);
        if max_len == 0 {
            return 1.0;
        }

        1.0 - (levenshtein_distance(a, b) as f64 / max_len as f64)
    }
}

#[derive(Debug, Clone)]
pub struct NavigationOption {
    pub path: String,
    pub target_type: String,
    pub description: Option<String>,
    pub is_collection: bool,
}

#[derive(Debug, Clone)]
pub struct PathValidationResult {
    pub is_valid: bool,
    pub target_type: Option<String>,
    pub errors: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    pub segment: String,
    pub full_path: String,
    pub target_type: String,
    pub is_choice_type: bool,
}

// Helper function for edit distance calculation
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();
    let len1 = s1_chars.len();
    let len2 = s2_chars.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    // Initialize first row and column
    #[allow(clippy::needless_range_loop)]
    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                0
            } else {
                1
            };

            matrix[i][j] = std::cmp::min(
                std::cmp::min(
                    matrix[i - 1][j] + 1, // deletion
                    matrix[i][j - 1] + 1, // insertion
                ),
                matrix[i - 1][j - 1] + cost, // substitution
            );
        }
    }

    matrix[len1][len2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("a", ""), 1);
        assert_eq!(levenshtein_distance("", "a"), 1);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", "ab"), 1);
        assert_eq!(levenshtein_distance("abc", "axc"), 1);
        assert_eq!(levenshtein_distance("name.given", "name.family"), 6);
    }

    #[test]
    fn test_similarity_calculation() {
        let navigator = PathNavigator {
            resolver: Arc::new(PathResolver::new(Arc::new(SchemaIndex::new()))),
            schema_index: Arc::new(SchemaIndex::new()),
        };

        let similarity = navigator.calculate_similarity("name.given", "name.family");
        assert!(similarity > 0.4); // Should be moderately similar (shared "name." prefix)

        let similarity = navigator.calculate_similarity("completely", "different");
        assert!(similarity < 0.3); // Should be very different
    }
}
