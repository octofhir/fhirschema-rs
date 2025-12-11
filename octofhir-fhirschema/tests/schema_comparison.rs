// R4B Schema Comparison Framework
// Compares Rust-generated FHIR schemas against TypeScript reference implementation
// from @atomic-ehr/fhirschema package

use octofhir_fhirschema::{EmbeddedSchemaProvider, FhirSchema};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

// ============================================================================
// Data Types
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaDifference {
    MissingField {
        path: String,
        in_reference: bool, // true if field is in reference but not in our schema
    },
    TypeMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    ValueMismatch {
        path: String,
        expected: Value,
        actual: Value,
    },
    CardinalityDifference {
        path: String,
        expected_min: Option<i64>,
        actual_min: Option<i64>,
        expected_max: Option<String>,
        actual_max: Option<String>,
    },
    ArrayLengthMismatch {
        path: String,
        expected_len: usize,
        actual_len: usize,
    },
}

#[derive(Debug, Clone)]
pub struct SchemaComparisonResult {
    pub resource_name: String,
    pub matches: bool,
    pub differences: Vec<SchemaDifference>,
    pub similarity_score: f64, // 0.0 = completely different, 1.0 = identical
}

impl SchemaComparisonResult {
    pub fn new(resource_name: String) -> Self {
        Self {
            resource_name,
            matches: true,
            differences: Vec::new(),
            similarity_score: 1.0,
        }
    }

    pub fn add_difference(&mut self, diff: SchemaDifference) {
        self.differences.push(diff);
        self.matches = false;
        self.recalculate_similarity();
    }

    fn recalculate_similarity(&mut self) {
        // Exponential decay based on difference count
        // 0 differences = 1.0
        // 1 difference = ~0.95
        // 5 differences = ~0.78
        // 10 differences = ~0.61
        // 20 differences = ~0.37
        let diff_count = self.differences.len() as f64;
        self.similarity_score = (-0.05 * diff_count).exp();
    }
}

// ============================================================================
// TypeScript Reference Downloader
// ============================================================================

pub struct TypeScriptReferenceDownloader {
    cache_dir: PathBuf,
}

impl TypeScriptReferenceDownloader {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Downloads and generates TypeScript reference schemas
    /// Returns path to directory containing generated JSON schemas
    pub async fn download_and_generate(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        // Create cache directory if it doesn't exist
        fs::create_dir_all(&self.cache_dir)?;

        let output_dir = self.cache_dir.join("r4b_reference");

        // If schemas already exist and are recent (< 30 days), use cached version
        if output_dir.exists() {
            if let Ok(metadata) = fs::metadata(&output_dir) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        if elapsed.as_secs() < 30 * 24 * 60 * 60 {
                            println!(
                                "Using cached TypeScript reference schemas (age: {} days)",
                                elapsed.as_secs() / (24 * 60 * 60)
                            );
                            return Ok(output_dir);
                        }
                    }
                }
            }
        }

        println!("Downloading TypeScript reference schemas from @atomic-ehr/fhirschema...");

        // Check if Bun is available
        if !self.check_bun()? {
            return Err("Bun not found. Please install Bun from https://bun.sh to enable TypeScript comparison.".into());
        }

        // Create temporary project
        let temp_dir = TempDir::new()?;
        let project_dir = temp_dir.path();

        // Initialize project
        self.init_project(project_dir)?;

        // Install @atomic-ehr/fhirschema package
        self.install_fhirschema_package(project_dir)?;

        // Generate schemas using the package
        self.generate_schemas(project_dir, &output_dir).await?;

        println!("✅ TypeScript reference schemas generated successfully");
        Ok(output_dir)
    }

    fn check_bun(&self) -> Result<bool, Box<dyn std::error::Error>> {
        let output = Command::new("bun").arg("--version").output();
        Ok(output.is_ok())
    }

    fn init_project(&self, project_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Create package.json
        let package_json = json!({
            "name": "fhir-schema-reference",
            "version": "1.0.0",
            "type": "module",
            "dependencies": {}
        });

        fs::write(
            project_dir.join("package.json"),
            serde_json::to_string_pretty(&package_json)?,
        )?;

        Ok(())
    }

    fn install_fhirschema_package(
        &self,
        project_dir: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Installing @atomic-ehr/fhirschema package with Bun...");

        let output = Command::new("bun")
            .args(&["add", "@atomic-ehr/fhirschema"])
            .current_dir(project_dir)
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "bun add failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        Ok(())
    }

    async fn generate_schemas(
        &self,
        project_dir: &Path,
        output_dir: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Generating R4B schemas with TypeScript fhir-canonical-manager...");

        // Create package.json with @atomic-ehr dependencies
        let package_json = json!({
            "name": "fhir-schema-reference",
            "version": "1.0.0",
            "type": "module",
            "dependencies": {
                "@atomic-ehr/fhir-canonical-manager": "latest",
                "@atomic-ehr/fhirschema": "latest"
            }
        });

        fs::write(
            project_dir.join("package.json"),
            serde_json::to_string_pretty(&package_json)?,
        )?;

        // Create TypeScript generation script that uses @atomic-ehr/fhir-canonical-manager
        let gen_script = include_str!("../scripts/generate-ts.mjs");
        fs::write(project_dir.join("generate-ts.mjs"), gen_script)?;

        // Install dependencies
        println!("Installing dependencies with Bun...");
        let output = Command::new("bun")
            .args(&["install"])
            .current_dir(project_dir)
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "Bun install failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        // Create output directory
        fs::create_dir_all(output_dir)?;

        // Run generation script
        println!("Running TypeScript schema generation...");
        let output = Command::new("bun")
            .args(&["run", "generate-ts.mjs"])
            .arg(project_dir.to_str().ok_or("Invalid project path")?)
            .arg(output_dir.to_str().ok_or("Invalid output path")?)
            .current_dir(project_dir)
            .output()?;

        if !output.status.success() {
            eprintln!("❌ Schema generation failed!");
            eprintln!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
            return Err(format!(
                "Schema generation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }

        println!("✅ TypeScript generation output:");
        println!("{}", String::from_utf8_lossy(&output.stdout));
        Ok(())
    }
}

// ============================================================================
// Schema Comparator
// ============================================================================

#[derive(Debug, Clone)]
pub struct ComparisonConfig {
    /// Fields to ignore during comparison (e.g., package metadata)
    pub ignore_fields: Vec<String>,
    /// Whether to ignore field ordering in objects
    pub ignore_ordering: bool,
    /// Tolerance for numeric comparisons
    pub numeric_tolerance: f64,
}

impl Default for ComparisonConfig {
    fn default() -> Self {
        Self {
            ignore_fields: vec![
                "package_name".to_string(),
                "package_version".to_string(),
                "_order".to_string(),
                "abstract".to_string(), // Rust-specific metadata field, not in TypeScript
                "meaningWhenMissing".to_string(), // Documentation: meaning when element is missing
                "orderMeaning".to_string(), // Documentation: semantic meaning of element order
                "representation".to_string(), // XML serialization format (xmlAttr, xmlText, etc.)
                "description".to_string(), // Documentation field in various contexts
            ],
            ignore_ordering: true,
            numeric_tolerance: 0.0001,
        }
    }
}

pub struct SchemaComparator {
    config: ComparisonConfig,
}

impl SchemaComparator {
    pub fn new(config: ComparisonConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self {
            config: ComparisonConfig::default(),
        }
    }

    /// Compare two schemas and return detailed differences
    pub fn compare(
        &self,
        resource_name: &str,
        our_schema: &FhirSchema,
        reference_schema: &Value,
    ) -> SchemaComparisonResult {
        let mut result = SchemaComparisonResult::new(resource_name.to_string());

        // Convert our schema to JSON for comparison
        let our_value = serde_json::to_value(our_schema).unwrap();

        // Recursively compare
        self.compare_values("", &our_value, reference_schema, &mut result);

        result
    }

    fn compare_values(
        &self,
        path: &str,
        our_value: &Value,
        reference_value: &Value,
        result: &mut SchemaComparisonResult,
    ) {
        match (our_value, reference_value) {
            (Value::Object(our_obj), Value::Object(ref_obj)) => {
                self.compare_objects(path, our_obj, ref_obj, result);
            }
            (Value::Array(our_arr), Value::Array(ref_arr)) => {
                self.compare_arrays(path, our_arr, ref_arr, result);
            }
            (Value::String(our_str), Value::String(ref_str)) => {
                if our_str != ref_str {
                    result.add_difference(SchemaDifference::ValueMismatch {
                        path: path.to_string(),
                        expected: Value::String(ref_str.clone()),
                        actual: Value::String(our_str.clone()),
                    });
                }
            }
            (Value::Number(our_num), Value::Number(ref_num)) => {
                let our_f64 = our_num.as_f64().unwrap_or(0.0);
                let ref_f64 = ref_num.as_f64().unwrap_or(0.0);
                if (our_f64 - ref_f64).abs() > self.config.numeric_tolerance {
                    result.add_difference(SchemaDifference::ValueMismatch {
                        path: path.to_string(),
                        expected: Value::Number(ref_num.clone()),
                        actual: Value::Number(our_num.clone()),
                    });
                }
            }
            (Value::Bool(our_bool), Value::Bool(ref_bool)) => {
                if our_bool != ref_bool {
                    result.add_difference(SchemaDifference::ValueMismatch {
                        path: path.to_string(),
                        expected: Value::Bool(*ref_bool),
                        actual: Value::Bool(*our_bool),
                    });
                }
            }
            (Value::Null, Value::Null) => {
                // Both null, no difference
            }
            _ => {
                // Type mismatch
                result.add_difference(SchemaDifference::TypeMismatch {
                    path: path.to_string(),
                    expected: format!("{:?}", reference_value),
                    actual: format!("{:?}", our_value),
                });
            }
        }
    }

    fn compare_objects(
        &self,
        path: &str,
        our_obj: &serde_json::Map<String, Value>,
        ref_obj: &serde_json::Map<String, Value>,
        result: &mut SchemaComparisonResult,
    ) {
        // Check for missing fields in our schema
        for (key, ref_value) in ref_obj {
            if self.config.ignore_fields.contains(key) {
                continue;
            }

            let field_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", path, key)
            };

            match our_obj.get(key) {
                Some(our_value) => {
                    self.compare_values(&field_path, our_value, ref_value, result);
                }
                None => {
                    result.add_difference(SchemaDifference::MissingField {
                        path: field_path,
                        in_reference: true,
                    });
                }
            }
        }

        // Check for extra fields in our schema
        for (key, _our_value) in our_obj {
            if self.config.ignore_fields.contains(key) {
                continue;
            }

            let field_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", path, key)
            };

            if !ref_obj.contains_key(key) {
                result.add_difference(SchemaDifference::MissingField {
                    path: field_path,
                    in_reference: false,
                });
            }
        }
    }

    fn compare_arrays(
        &self,
        path: &str,
        our_arr: &[Value],
        ref_arr: &[Value],
        result: &mut SchemaComparisonResult,
    ) {
        if our_arr.len() != ref_arr.len() {
            result.add_difference(SchemaDifference::ArrayLengthMismatch {
                path: path.to_string(),
                expected_len: ref_arr.len(),
                actual_len: our_arr.len(),
            });
            // Continue comparing up to the shorter length
        }

        let min_len = our_arr.len().min(ref_arr.len());
        for i in 0..min_len {
            let item_path = format!("{}[{}]", path, i);
            self.compare_values(&item_path, &our_arr[i], &ref_arr[i], result);
        }
    }
}

// ============================================================================
// Diff Report Generator
// ============================================================================

pub struct DiffReportGenerator {
    output_dir: PathBuf,
}

impl DiffReportGenerator {
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }

    /// Generate HTML and JSON reports from comparison results
    pub fn generate_reports(
        &self,
        results: &[SchemaComparisonResult],
    ) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&self.output_dir)?;

        // Generate JSON report
        self.generate_json_report(results)?;

        // Generate HTML report
        self.generate_html_report(results)?;

        Ok(())
    }

    fn generate_json_report(
        &self,
        results: &[SchemaComparisonResult],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json_path = self.output_dir.join("comparison_report.json");

        // Calculate summary statistics
        let total = results.len();
        let matches = results.iter().filter(|r| r.matches).count();
        let avg_similarity = results.iter().map(|r| r.similarity_score).sum::<f64>() / total as f64;
        let total_differences: usize = results.iter().map(|r| r.differences.len()).sum();

        let report = json!({
            "summary": {
                "total_resources": total,
                "exact_matches": matches,
                "match_rate": (matches as f64 / total as f64) * 100.0,
                "average_similarity": avg_similarity,
                "total_differences": total_differences,
            },
            "resources": results.iter().map(|r| {
                json!({
                    "name": r.resource_name,
                    "matches": r.matches,
                    "similarity_score": r.similarity_score,
                    "difference_count": r.differences.len(),
                    "differences": r.differences.iter().map(|d| {
                        match d {
                            SchemaDifference::MissingField { path, in_reference } => {
                                json!({
                                    "type": "missing_field",
                                    "path": path,
                                    "in_reference": in_reference,
                                })
                            }
                            SchemaDifference::TypeMismatch { path, expected, actual } => {
                                json!({
                                    "type": "type_mismatch",
                                    "path": path,
                                    "expected": expected,
                                    "actual": actual,
                                })
                            }
                            SchemaDifference::ValueMismatch { path, expected, actual } => {
                                json!({
                                    "type": "value_mismatch",
                                    "path": path,
                                    "expected": expected,
                                    "actual": actual,
                                })
                            }
                            SchemaDifference::CardinalityDifference { path, expected_min, actual_min, expected_max, actual_max } => {
                                json!({
                                    "type": "cardinality_difference",
                                    "path": path,
                                    "expected_min": expected_min,
                                    "actual_min": actual_min,
                                    "expected_max": expected_max,
                                    "actual_max": actual_max,
                                })
                            }
                            SchemaDifference::ArrayLengthMismatch { path, expected_len, actual_len } => {
                                json!({
                                    "type": "array_length_mismatch",
                                    "path": path,
                                    "expected_len": expected_len,
                                    "actual_len": actual_len,
                                })
                            }
                        }
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        });

        fs::write(json_path, serde_json::to_string_pretty(&report)?)?;
        Ok(())
    }

    fn generate_html_report(
        &self,
        results: &[SchemaComparisonResult],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let html_path = self.output_dir.join("comparison_report.html");

        let total = results.len();
        let matches = results.iter().filter(|r| r.matches).count();
        let avg_similarity = results.iter().map(|r| r.similarity_score).sum::<f64>() / total as f64;

        let mut html = String::from(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>R4B Schema Comparison Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }
        .container { max-width: 1200px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        h1 { color: #333; }
        .summary { background: #e3f2fd; padding: 15px; border-radius: 4px; margin: 20px 0; }
        .summary-stat { display: inline-block; margin-right: 30px; }
        .summary-stat strong { color: #1976d2; }
        .resource { margin: 20px 0; padding: 15px; border: 1px solid #ddd; border-radius: 4px; }
        .resource.match { background: #e8f5e9; border-color: #4caf50; }
        .resource.mismatch { background: #ffebee; border-color: #f44336; }
        .resource-name { font-size: 18px; font-weight: bold; margin-bottom: 10px; }
        .similarity { color: #666; font-size: 14px; }
        .differences { margin-top: 10px; font-size: 14px; }
        .diff-item { margin: 5px 0; padding: 5px; background: #fff; border-left: 3px solid #ff9800; padding-left: 10px; }
        .diff-type { font-weight: bold; color: #ff9800; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🔍 R4B Schema Comparison Report</h1>
        <div class="summary">
            <div class="summary-stat">
                <strong>Total Resources:</strong> "#,
        );
        html.push_str(&total.to_string());
        html.push_str(
            r#"
            </div>
            <div class="summary-stat">
                <strong>Exact Matches:</strong> "#,
        );
        html.push_str(&matches.to_string());
        html.push_str(
            r#"
            </div>
            <div class="summary-stat">
                <strong>Match Rate:</strong> "#,
        );
        html.push_str(&format!("{:.1}%", (matches as f64 / total as f64) * 100.0));
        html.push_str(
            r#"
            </div>
            <div class="summary-stat">
                <strong>Avg Similarity:</strong> "#,
        );
        html.push_str(&format!("{:.1}%", avg_similarity * 100.0));
        html.push_str(
            r#"
            </div>
        </div>
        <h2>Resource Details</h2>
"#,
        );

        for result in results {
            let class = if result.matches { "match" } else { "mismatch" };
            html.push_str(&format!(
                r#"<div class="resource {}">
    <div class="resource-name">{}</div>
    <div class="similarity">Similarity: {:.1}% ({} differences)</div>
"#,
                class,
                result.resource_name,
                result.similarity_score * 100.0,
                result.differences.len()
            ));

            if !result.differences.is_empty() {
                html.push_str(r#"<div class="differences">"#);
                for (_i, diff) in result.differences.iter().enumerate().take(10) {
                    let diff_desc = match diff {
                        SchemaDifference::MissingField { path, in_reference } => {
                            if *in_reference {
                                format!("Missing field (in reference): {}", path)
                            } else {
                                format!("Extra field (not in reference): {}", path)
                            }
                        }
                        SchemaDifference::TypeMismatch {
                            path,
                            expected,
                            actual,
                        } => {
                            format!(
                                "Type mismatch at {}: expected {} but got {}",
                                path, expected, actual
                            )
                        }
                        SchemaDifference::ValueMismatch { path, .. } => {
                            format!("Value mismatch at {}", path)
                        }
                        SchemaDifference::CardinalityDifference { path, .. } => {
                            format!("Cardinality difference at {}", path)
                        }
                        SchemaDifference::ArrayLengthMismatch {
                            path,
                            expected_len,
                            actual_len,
                        } => {
                            format!(
                                "Array length mismatch at {}: expected {} but got {}",
                                path, expected_len, actual_len
                            )
                        }
                    };
                    html.push_str(&format!(r#"<div class="diff-item">{}</div>"#, diff_desc));
                }
                if result.differences.len() > 10 {
                    html.push_str(&format!(
                        r#"<div class="diff-item">... and {} more differences</div>"#,
                        result.differences.len() - 10
                    ));
                }
                html.push_str(r#"</div>"#);
            }

            html.push_str("</div>\n");
        }

        html.push_str(
            r#"
    </div>
</body>
</html>
"#,
        );

        fs::write(html_path, html)?;
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_config_default() {
        let config = ComparisonConfig::default();
        assert!(config.ignore_fields.contains(&"package_name".to_string()));
        assert!(config.ignore_ordering);
    }

    #[test]
    fn test_schema_comparison_result_similarity() {
        let mut result = SchemaComparisonResult::new("Test".to_string());
        assert_eq!(result.similarity_score, 1.0);

        result.add_difference(SchemaDifference::MissingField {
            path: "test".to_string(),
            in_reference: true,
        });
        assert!(result.similarity_score < 1.0);
        assert!(result.similarity_score > 0.9);
    }

    #[test]
    fn test_schema_comparator_identical_values() {
        let comparator = SchemaComparator::with_defaults();
        let mut result = SchemaComparisonResult::new("Test".to_string());

        let value1 = json!({"name": "Patient", "type": "resource"});
        let value2 = json!({"name": "Patient", "type": "resource"});

        comparator.compare_values("", &value1, &value2, &mut result);

        assert_eq!(result.differences.len(), 0);
        assert_eq!(result.similarity_score, 1.0);
    }

    #[test]
    fn test_schema_comparator_value_mismatch() {
        let comparator = SchemaComparator::with_defaults();
        let mut result = SchemaComparisonResult::new("Test".to_string());

        let value1 = json!({"name": "Patient"});
        let value2 = json!({"name": "Observation"});

        comparator.compare_values("", &value1, &value2, &mut result);

        assert_eq!(result.differences.len(), 1);
        assert!(matches!(
            result.differences[0],
            SchemaDifference::ValueMismatch { .. }
        ));
    }

    #[test]
    fn test_schema_comparator_missing_field() {
        let comparator = SchemaComparator::with_defaults();
        let mut result = SchemaComparisonResult::new("Test".to_string());

        let value1 = json!({"name": "Patient"});
        let value2 = json!({"name": "Patient", "type": "resource"});

        comparator.compare_values("", &value1, &value2, &mut result);

        assert_eq!(result.differences.len(), 1);
        assert!(matches!(
            result.differences[0],
            SchemaDifference::MissingField {
                in_reference: true,
                ..
            }
        ));
    }

    #[test]
    fn test_schema_comparator_array_length_mismatch() {
        let comparator = SchemaComparator::with_defaults();
        let mut result = SchemaComparisonResult::new("Test".to_string());

        let value1 = json!({"items": [1, 2, 3]});
        let value2 = json!({"items": [1, 2]});

        comparator.compare_values("", &value1, &value2, &mut result);

        assert!(
            result
                .differences
                .iter()
                .any(|d| matches!(d, SchemaDifference::ArrayLengthMismatch { .. }))
        );
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    /// Main R4B comparison test against TypeScript reference
    /// This test requires Node.js 18+ to be installed
    #[tokio::test]
    #[ignore] // Run with: cargo test --test schema_comparison -- --ignored
    async fn test_r4b_compare_against_typescript_reference() {
        println!("\n🔍 Starting R4B Schema Comparison against TypeScript Reference");
        println!("===============================================================\n");

        // Setup - use fixtures directory for permanent storage
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        fs::create_dir_all(&fixtures_dir).expect("Failed to create fixtures dir");

        let cache_dir = fixtures_dir.clone();
        let report_dir = fixtures_dir.join("comparison_reports");
        fs::create_dir_all(&report_dir).expect("Failed to create reports dir");

        // Download and generate TypeScript reference schemas
        println!("📦 Downloading TypeScript reference schemas...");
        let downloader = TypeScriptReferenceDownloader::new(cache_dir.clone());
        let reference_dir = match downloader.download_and_generate().await {
            Ok(dir) => dir,
            Err(e) => {
                eprintln!("\n⚠️  TypeScript comparison skipped: {}", e);
                eprintln!("    To enable comparison, install Bun from https://bun.sh");
                return;
            }
        };

        // Generate R4B schemas on the fly from StructureDefinitions
        println!("\n📚 Generating Rust R4B schemas on the fly...");
        let our_schemas = generate_r4b_schemas_from_canonical_manager()
            .await
            .expect("Failed to generate R4B schemas");

        println!("   Generated {} Rust schemas", our_schemas.len());

        // Load TypeScript reference schemas
        println!("\n📖 Loading TypeScript reference schemas...");
        let reference_schemas = load_reference_schemas(&reference_dir)
            .expect("Failed to load TypeScript reference schemas");

        println!("   Found {} TypeScript schemas", reference_schemas.len());

        // Compare schemas
        println!("\n🔬 Comparing schemas...");
        let comparator = SchemaComparator::with_defaults();
        let mut results = Vec::new();

        let mut compared_count = 0;
        let mut skipped_count = 0;

        for (name, our_schema) in our_schemas.iter() {
            // Match schemas by canonical URL to support both base resources and profiles
            // Find corresponding reference schema using URL
            if let Some(ref_schema) = reference_schemas.get(&our_schema.url) {
                let result = comparator.compare(name, our_schema, ref_schema);
                results.push(result);
                compared_count += 1;

                if compared_count % 50 == 0 {
                    println!("   Compared {} schemas...", compared_count);
                }
            } else {
                skipped_count += 1;
                // Optionally log which URLs were not found
                if skipped_count <= 5 {
                    println!("   ⚠️  No TypeScript schema found for: {}", our_schema.url);
                }
            }
        }

        println!("\n✅ Comparison complete!");
        println!("   Compared: {}", compared_count);
        println!("   Skipped (not in reference): {}", skipped_count);

        // Generate reports
        println!("\n📊 Generating comparison reports...");
        let report_generator = DiffReportGenerator::new(report_dir.clone());
        report_generator
            .generate_reports(&results)
            .expect("Failed to generate reports");

        // Calculate summary statistics
        let total = results.len();
        let exact_matches = results.iter().filter(|r| r.matches).count();
        let match_rate = (exact_matches as f64 / total as f64) * 100.0;
        let avg_similarity = results.iter().map(|r| r.similarity_score).sum::<f64>() / total as f64;
        let avg_similarity_pct = avg_similarity * 100.0;

        println!("\n📈 Results Summary");
        println!("==================");
        println!("Total resources compared: {}", total);
        println!("Exact matches: {} ({:.1}%)", exact_matches, match_rate);
        println!("Average similarity: {:.1}%", avg_similarity_pct);
        println!(
            "\n📄 Reports generated at:\n   - {}/comparison_report.html\n   - {}/comparison_report.json",
            report_dir.display(),
            report_dir.display()
        );

        // Print top 10 resources with most differences
        let mut sorted_results = results.clone();
        sorted_results.sort_by_key(|r| std::cmp::Reverse(r.differences.len()));

        println!("\n🔎 Top 10 Resources with Most Differences:");
        for (i, result) in sorted_results.iter().take(10).enumerate() {
            println!(
                "   {}. {} - {} differences (similarity: {:.1}%)",
                i + 1,
                result.resource_name,
                result.differences.len(),
                result.similarity_score * 100.0
            );
        }

        // ASSERTION: Require 90%+ average similarity
        assert!(
            avg_similarity_pct >= 90.0,
            "\n❌ Average similarity {:.1}% is below the required 90% threshold!\n   \
             This indicates significant differences between Rust and TypeScript implementations.\n   \
             Review the detailed report at: {}/comparison_report.html",
            avg_similarity_pct,
            report_dir.display()
        );

        println!(
            "\n✅ Test passed! Average similarity {:.1}% meets the 90% threshold.",
            avg_similarity_pct
        );
    }

    /// Test a small sample of resources for quick validation
    #[tokio::test]
    #[ignore = "R4B schemas not fully generated yet"]
    async fn test_r4b_sample_schema_comparison() {
        println!("\n🔬 Testing sample R4B schema comparisons");

        let provider = EmbeddedSchemaProvider::r4b();
        let schemas = provider.schemas();

        // Test that we have expected resources
        let test_resources = vec![
            "Patient",
            "Observation",
            "SubscriptionTopic",
            "SubscriptionStatus",
            "MedicinalProductDefinition",
        ];

        for resource_name in test_resources {
            assert!(
                schemas.contains_key(resource_name),
                "Expected R4B resource '{}' not found in embedded schemas",
                resource_name
            );
        }

        println!("   ✅ All sample resources found");
        println!("   Total embedded schemas: {}", schemas.len());
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate R4B schemas on the fly from StructureDefinitions using canonical manager
async fn generate_r4b_schemas_from_canonical_manager()
-> Result<HashMap<String, FhirSchema>, Box<dyn std::error::Error>> {
    use octofhir_canonical_manager::{CanonicalManager, FcmConfig};
    use octofhir_fhirschema::{StructureDefinition, translate};

    // Initialize canonical manager
    let config = FcmConfig::load().await?;
    let canonical_manager = CanonicalManager::new(config).await?;

    // Install R4B package
    canonical_manager
        .install_package("hl7.fhir.r4b.core", "4.3.0")
        .await?;

    // Search for all StructureDefinitions
    let mut all_structure_definitions = Vec::new();
    let mut offset = 0;
    const BATCH_SIZE: usize = 1000;

    loop {
        let search_result = canonical_manager
            .search()
            .await
            .resource_type("StructureDefinition")
            .package("hl7.fhir.r4b.core")
            .limit(BATCH_SIZE)
            .offset(offset)
            .execute()
            .await?;

        let batch_size = search_result.resources.len();
        if batch_size == 0 {
            break;
        }

        all_structure_definitions.extend(search_result.resources);
        offset += BATCH_SIZE;

        if batch_size < BATCH_SIZE {
            break;
        }
    }

    // Convert StructureDefinitions to FhirSchemas
    let mut schemas = HashMap::new();
    for resolved_resource in all_structure_definitions {
        let structure_def_json = &resolved_resource.resource.content;
        let type_name = structure_def_json
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown");

        match serde_json::from_value::<StructureDefinition>(structure_def_json.clone()) {
            Ok(structure_def) => match translate(structure_def, None) {
                Ok(schema) => {
                    schemas.insert(type_name.to_string(), schema);
                }
                Err(e) => {
                    eprintln!("⚠️  Failed to convert {type_name}: {e}");
                }
            },
            Err(e) => {
                eprintln!("⚠️  Failed to parse StructureDefinition for {type_name}: {e}");
            }
        }
    }

    Ok(schemas)
}

/// Load all TypeScript reference schemas from a directory
/// Uses canonical URL as key to support both base resources and profiles
fn load_reference_schemas(
    dir: &Path,
) -> Result<std::collections::HashMap<String, Value>, Box<dyn std::error::Error>> {
    let mut schemas = std::collections::HashMap::new();

    if !dir.exists() {
        return Err(format!("Reference schema directory not found: {}", dir.display()).into());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)?;
            let schema: Value = serde_json::from_str(&content)?;

            // Use canonical URL as key - unique for both base resources and profiles
            // This prevents collisions when multiple profiles exist for the same type
            if let Some(url) = schema.get("url").and_then(|u| u.as_str()) {
                schemas.insert(url.to_string(), schema);
            }
        }
    }

    Ok(schemas)
}
