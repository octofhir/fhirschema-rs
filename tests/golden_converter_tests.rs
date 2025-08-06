mod common;

use octofhir_fhirschema::*;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Golden test framework for StructureDefinition to FHIRSchema conversion
///
/// Test structure:
/// - tests/golden/input/*.json - Input StructureDefinitions
/// - tests/golden/expected/*.fhirschema.json - Expected FHIRSchema outputs
/// - tests/golden/actual/*.fhirschema.json - Actual outputs (gitignored)
fn golden_test_dir() -> PathBuf {
    PathBuf::from("tests/golden")
}

fn ensure_test_directories() {
    let dirs = vec![
        golden_test_dir().join("input"),
        golden_test_dir().join("expected"),
        golden_test_dir().join("actual"),
    ];

    for dir in dirs {
        fs::create_dir_all(&dir).expect("Failed to create test directory");
    }
}

fn load_json_file(path: &Path) -> Value {
    let content =
        fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read file: {path:?}"));
    serde_json::from_str(&content).unwrap_or_else(|_| panic!("Failed to parse JSON from: {path:?}"))
}

fn save_json_file(path: &Path, value: &Value) {
    let content = serde_json::to_string_pretty(value).expect("Failed to serialize JSON");
    fs::write(path, content).unwrap_or_else(|_| panic!("Failed to write file: {path:?}"));
}

fn run_golden_test(test_name: &str) {
    ensure_test_directories();

    let input_path = golden_test_dir()
        .join("input")
        .join(format!("{test_name}.json"));
    let expected_path = golden_test_dir()
        .join("expected")
        .join(format!("{test_name}.fhirschema.json"));
    let actual_path = golden_test_dir()
        .join("actual")
        .join(format!("{test_name}.fhirschema.json"));

    // Load input StructureDefinition
    let input_json = load_json_file(&input_path);
    let mut structure_def: StructureDefinition = serde_json::from_value(input_json.clone())
        .unwrap_or_else(|_| panic!("Failed to parse StructureDefinition from: {input_path:?}"));

    // Extract elements from snapshot/differential
    structure_def
        .extract_elements()
        .unwrap_or_else(|_| panic!("Failed to extract elements from: {test_name}"));

    // Convert to FHIRSchema
    let converter = FhirSchemaConverter::new();
    let schema = converter
        .convert(&structure_def)
        .unwrap_or_else(|_| panic!("Conversion failed for: {test_name}"));

    // Save actual output
    let actual_json = serde_json::to_value(&schema).expect("Failed to serialize FHIRSchema");
    save_json_file(&actual_path, &actual_json);

    // Compare with expected output
    if expected_path.exists() {
        let expected_json = load_json_file(&expected_path);

        // Deep comparison with better error messages
        compare_json_values(&expected_json, &actual_json, "")
            .unwrap_or_else(|_| panic!("Golden test failed for: {test_name}"));
    } else {
        // First run - create expected file
        println!("Creating new expected file for: {test_name}");
        save_json_file(&expected_path, &actual_json);
    }
}

fn compare_json_values(
    expected: &Value,
    actual: &Value,
    path: &str,
) -> std::result::Result<(), String> {
    match (expected, actual) {
        (Value::Object(exp_obj), Value::Object(act_obj)) => {
            // Check all expected keys exist in actual
            for (key, exp_value) in exp_obj {
                let current_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };

                match act_obj.get(key) {
                    Some(act_value) => {
                        compare_json_values(exp_value, act_value, &current_path)?;
                    }
                    None => {
                        return Err(format!("Missing key at path: {current_path}"));
                    }
                }
            }

            // Check for unexpected keys in actual
            for (key, _) in act_obj {
                if !exp_obj.contains_key(key) {
                    let current_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    return Err(format!("Unexpected key at path: {current_path}"));
                }
            }

            Ok(())
        }
        (Value::Array(exp_arr), Value::Array(act_arr)) => {
            if exp_arr.len() != act_arr.len() {
                return Err(format!(
                    "Array length mismatch at path: {} (expected: {}, actual: {})",
                    path,
                    exp_arr.len(),
                    act_arr.len()
                ));
            }

            for (index, (exp_item, act_item)) in exp_arr.iter().zip(act_arr.iter()).enumerate() {
                let current_path = format!("{path}[{index}]");
                compare_json_values(exp_item, act_item, &current_path)?;
            }

            Ok(())
        }
        _ => {
            if expected != actual {
                Err(format!(
                    "Value mismatch at path: {path}\nExpected: {expected:?}\nActual: {actual:?}"
                ))
            } else {
                Ok(())
            }
        }
    }
}

#[test]
fn test_patient_structure_definition() {
    run_golden_test("patient");
}

#[test]
fn test_observation_structure_definition() {
    run_golden_test("observation");
}

#[test]
fn test_medication_request_structure_definition() {
    run_golden_test("medication-request");
}

#[test]
fn test_bundle_structure_definition() {
    run_golden_test("bundle");
}

#[test]
fn test_extension_structure_definition() {
    run_golden_test("extension");
}

// Test for complex slicing scenarios
#[test]
fn test_questionnaire_response_structure_definition() {
    run_golden_test("questionnaire-response");
}

// Test for resources with many choice types
#[test]
fn test_element_definition_structure_definition() {
    run_golden_test("element-definition");
}

// Utility function to update all golden test expected files
// Run with: cargo test update_all_golden_tests -- --ignored
#[test]
#[ignore]
fn update_all_golden_tests() {
    ensure_test_directories();

    let input_dir = golden_test_dir().join("input");
    if let Ok(entries) = fs::read_dir(&input_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(test_name) = path.file_stem().and_then(|s| s.to_str()) {
                    println!("Updating golden test: {test_name}");

                    // Force update by removing expected file
                    let expected_path = golden_test_dir()
                        .join("expected")
                        .join(format!("{test_name}.fhirschema.json"));
                    let _ = fs::remove_file(&expected_path);

                    // Run the test to generate new expected file
                    run_golden_test(test_name);
                }
            }
        }
    }
}
