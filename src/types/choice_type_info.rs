use serde::{Deserialize, Serialize};

/// Resolved choice type information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedChoiceType {
    pub base_path: String,     // "value[x]"
    pub expanded_path: String, // "valueString"
    pub actual_type: String,   // "string"
    pub is_primitive: bool,
    pub type_info: Option<TypeMetadata>,
}

impl ResolvedChoiceType {
    pub fn new(base_path: &str, type_code: &str) -> Self {
        let base_without_choice = base_path.trim_end_matches("[x]");
        let capitalized_type = capitalize_first(type_code);
        let expanded_path = format!("{base_without_choice}{capitalized_type}");

        Self {
            base_path: base_path.to_string(),
            expanded_path,
            actual_type: type_code.to_string(),
            is_primitive: is_primitive_type(type_code),
            type_info: None,
        }
    }

    pub fn with_type_info(mut self, type_info: TypeMetadata) -> Self {
        self.type_info = Some(type_info);
        self
    }
}

/// Choice type pattern analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceTypePattern {
    pub base_path: String,
    pub expansions: Vec<String>,
    pub detected_types: Vec<String>,
}

/// Choice type validation result
#[derive(Debug, Clone)]
pub struct ChoiceValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub inconsistencies: Vec<ChoiceInconsistency>,
}

#[derive(Debug, Clone)]
pub struct ChoiceInconsistency {
    pub issue_type: String,
    pub paths: Vec<String>,
    pub description: String,
}

impl ChoiceValidationResult {
    pub fn new() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            inconsistencies: Vec::new(),
        }
    }

    pub fn valid() -> Self {
        Self::new()
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.is_valid = false;
    }

    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    pub fn add_inconsistency(&mut self, inconsistency: ChoiceInconsistency) {
        let is_error = inconsistency.issue_type == "error";
        self.inconsistencies.push(inconsistency);
        if is_error {
            self.is_valid = false;
        }
    }
}

impl Default for ChoiceValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Type metadata for enhanced choice resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeMetadata {
    pub is_collection: bool,
    pub cardinality: Option<Cardinality>,
    pub constraints: Vec<String>,
    pub binding: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cardinality {
    pub min: i32,
    pub max: Option<i32>, // None for unbounded (*)
}

impl Cardinality {
    pub fn new(min: i32, max: Option<i32>) -> Self {
        Self { min, max }
    }

    pub fn is_required(&self) -> bool {
        self.min > 0
    }

    pub fn is_unbounded(&self) -> bool {
        self.max.is_none()
    }
}

// Helper functions
fn capitalize_first(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }

    let mut chars: Vec<char> = s.chars().collect();
    chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
    chars.into_iter().collect()
}

fn is_primitive_type(type_code: &str) -> bool {
    matches!(
        type_code,
        "boolean"
            | "integer"
            | "string"
            | "decimal"
            | "uri"
            | "url"
            | "canonical"
            | "base64Binary"
            | "instant"
            | "date"
            | "dateTime"
            | "time"
            | "code"
            | "oid"
            | "id"
            | "markdown"
            | "unsignedInt"
            | "positiveInt"
            | "uuid"
    )
}
