// Schema indexing functionality - placeholder for future implementation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaIndex {
    pub url_to_type: HashMap<String, String>,
    pub type_to_url: HashMap<String, String>,
    pub profile_urls: Vec<String>,
    pub extension_urls: Vec<String>,
}

impl SchemaIndex {
    pub fn new() -> Self {
        Self {
            url_to_type: HashMap::new(),
            type_to_url: HashMap::new(),
            profile_urls: Vec::new(),
            extension_urls: Vec::new(),
        }
    }

    pub fn add_schema_mapping(&mut self, url: &str, resource_type: &str) {
        self.url_to_type
            .insert(url.to_string(), resource_type.to_string());
        self.type_to_url
            .insert(resource_type.to_string(), url.to_string());
    }

    pub fn get_type_by_url(&self, url: &str) -> Option<&String> {
        self.url_to_type.get(url)
    }

    pub fn get_url_by_type(&self, resource_type: &str) -> Option<&String> {
        self.type_to_url.get(resource_type)
    }

    pub fn add_profile_url(&mut self, profile_url: &str) {
        if !self.profile_urls.contains(&profile_url.to_string()) {
            self.profile_urls.push(profile_url.to_string());
        }
    }

    pub fn add_extension_url(&mut self, extension_url: &str) {
        if !self.extension_urls.contains(&extension_url.to_string()) {
            self.extension_urls.push(extension_url.to_string());
        }
    }
}

impl Default for SchemaIndex {
    fn default() -> Self {
        Self::new()
    }
}
