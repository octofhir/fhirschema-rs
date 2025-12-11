//! Common test utilities for FHIR Schema validation tests.
//!
//! Provides fixture loading, test helpers, and validation utilities.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Try to load a fixture, returning None if it doesn't exist.
pub fn try_load_fixture(path: &str) -> Option<Value> {
    let fixtures_dir = get_fixtures_dir();
    let full_path = fixtures_dir.join(path);

    fs::read_to_string(&full_path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

/// Load all JSON fixtures from a directory.
///
/// # Arguments
/// * `dir` - Relative path to directory from fixtures (e.g., "r4/base/valid")
///
/// # Returns
/// Vec of (filename, parsed JSON) tuples
pub fn load_all_fixtures(dir: &str) -> Vec<(String, Value)> {
    let fixtures_dir = get_fixtures_dir();
    let full_path = fixtures_dir.join(dir);

    if !full_path.exists() {
        return Vec::new();
    }

    let mut fixtures = Vec::new();

    if let Ok(entries) = fs::read_dir(&full_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(value) = serde_json::from_str::<Value>(&content) {
                        let name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        fixtures.push((name, value));
                    }
                }
            }
        }
    }

    fixtures
}

/// Get the fixtures directory path.
fn get_fixtures_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir).join("tests").join("fixtures")
}

/// Validation test result for reporting.
#[derive(Debug, Clone)]
pub struct FixtureTestResult {
    pub fixture_name: String,
    pub expected_valid: bool,
    pub actual_valid: bool,
    pub errors: Vec<String>,
}

impl FixtureTestResult {
    pub fn passed(&self) -> bool {
        self.expected_valid == self.actual_valid
    }
}

/// Summary statistics for fixture test runs.
#[derive(Debug, Default)]
pub struct FixtureTestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<FixtureTestResult>,
}

impl FixtureTestSummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, result: FixtureTestResult) {
        self.total += 1;
        if result.passed() {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        self.results.push(result);
    }

    pub fn pass_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.passed as f64 / self.total as f64) * 100.0
        }
    }

    pub fn print_summary(&self) {
        println!("\n=== Fixture Test Summary ===");
        println!("Total: {}", self.total);
        println!("Passed: {}", self.passed);
        println!("Failed: {}", self.failed);
        println!("Pass Rate: {:.1}%", self.pass_rate());

        if self.failed > 0 {
            println!("\nFailed tests:");
            for result in &self.results {
                if !result.passed() {
                    println!(
                        "  - {}: expected={}, actual={}, errors={:?}",
                        result.fixture_name,
                        result.expected_valid,
                        result.actual_valid,
                        result.errors
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixtures_dir_exists() {
        let fixtures_dir = get_fixtures_dir();
        assert!(fixtures_dir.exists(), "Fixtures directory should exist");
    }

    #[test]
    fn test_load_all_fixtures_empty_dir() {
        // Should return empty vec for non-existent dir
        let fixtures = load_all_fixtures("nonexistent/path");
        assert!(fixtures.is_empty());
    }
}
