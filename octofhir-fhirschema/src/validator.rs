use crate::types::{FhirSchema, ValidationContext, ValidationError, ValidationResult};
use serde_json::Value;
use std::collections::HashMap;

pub fn validate(
    _ctx: &ValidationContext,
    _path: Vec<Value>, // Can be string or number
    _data: &Value,
) -> ValidationResult {
    // TODO: Implement validation logic
    ValidationResult {
        errors: vec![],
        valid: true,
    }
}

pub fn validate_schemas(
    ctx: &ValidationContext,
    schemas: &[&FhirSchema],
    data: &Value,
) -> ValidationResult {
    // TODO: Implement schema validation logic
    let mut errors = Vec::new();

    // For now, just validate against the first schema
    if let Some(schema) = schemas.first() {
        // Basic implementation for tests
        let result = validate_schema_with_data(ctx, schema, data, &[], data);
        errors.extend(result.errors);
    }

    let valid = errors.is_empty();
    ValidationResult { errors, valid }
}

fn validate_schema_with_data(
    ctx: &ValidationContext,
    schema: &FhirSchema,
    data: &Value,
    path: &[Value],
    root_data: &Value,
) -> ValidationResult {
    let mut errors = Vec::new();

    // Get merged elements if this schema has both type and elements
    let merged_elements = if let Some(elements) = &schema.elements {
        if !schema.type_name.is_empty() {
            if let Some(type_schema) = ctx.schemas.get(&schema.type_name) {
                if type_schema.kind != "primitive-type" {
                    if let Some(type_elements) = &type_schema.elements {
                        // Merge type elements with schema elements (schema elements override)
                        let mut merged = type_elements.clone();
                        for (k, v) in elements {
                            merged.insert(k.clone(), v.clone());
                        }
                        merged
                    } else {
                        elements.clone()
                    }
                } else {
                    elements.clone()
                }
            } else {
                elements.clone()
            }
        } else {
            elements.clone()
        }
    } else {
        HashMap::new()
    };

    // Type validation
    if !schema.type_name.is_empty() {
        let mut type_errors = Vec::new();
        let is_valid = validate_type_with_data(
            ctx,
            &schema.type_name,
            data,
            path,
            schema,
            &mut type_errors,
            root_data,
        );

        if !is_valid {
            if !type_errors.is_empty() {
                errors.extend(type_errors);
            } else {
                // Generic type error
                errors.push(ValidationError {
                    error_type: "type".to_string(),
                    path: path.to_vec(),
                    message: Some(format!("Expected type {}", schema.type_name)),
                    value: Some(data.clone()),
                    expected: None,
                    got: None,
                    schema_path: Some({
                        let mut schema_path = path
                            .iter()
                            .filter_map(|p| p.as_str())
                            .map(|s| Value::String(s.to_string()))
                            .collect::<Vec<_>>();
                        schema_path.extend(vec![
                            Value::String("type".to_string()),
                            Value::String(schema.type_name.clone()),
                            Value::String("type".to_string()),
                        ]);
                        schema_path
                    }),
                });
            }
        }
    }

    // Elements validation
    let has_elements = !merged_elements.is_empty();
    if has_elements && data.is_object() {
        let data_obj = data.as_object().unwrap();

        // Check for unknown elements
        for key in data_obj.keys() {
            if !merged_elements.contains_key(key) && !key.starts_with('_') {
                errors.push(ValidationError {
                    error_type: "element/unknown".to_string(),
                    path: {
                        let mut new_path = path.to_vec();
                        new_path.push(Value::String(key.clone()));
                        new_path
                    },
                    message: None,
                    value: None,
                    expected: None,
                    got: None,
                    schema_path: None,
                });
            }
        }

        // Validate known elements
        for (key, element_schema) in &merged_elements {
            if let Some(element_value) = data_obj.get(key) {
                let mut element_path = path.to_vec();
                element_path.push(Value::String(key.clone()));
                let element_result = validate_element_with_data(
                    ctx,
                    element_schema,
                    element_value,
                    &element_path,
                    root_data,
                );
                errors.extend(element_result.errors);
            }
        }
    } else if has_elements && data.is_array() {
        // When schema expects elements but data is array, it's a type error
        errors.push(ValidationError {
            error_type: "type/array".to_string(),
            message: Some("Expected not array".to_string()),
            path: path.to_vec(),
            value: Some(data.clone()),
            expected: None,
            got: None,
            schema_path: None,
        });
    } else if !schema.type_name.is_empty() && data.is_object() {
        // If we have a primitive type but got an object, also check for unknown elements
        let primitive_types = ["string", "code", "url", "boolean", "number", "integer"];
        if primitive_types.contains(&schema.type_name.as_str()) {
            let data_obj = data.as_object().unwrap();
            for key in data_obj.keys() {
                errors.push(ValidationError {
                    error_type: "element/unknown".to_string(),
                    path: {
                        let mut new_path = path.to_vec();
                        new_path.push(Value::String(key.clone()));
                        new_path
                    },
                    message: None,
                    value: None,
                    expected: None,
                    got: None,
                    schema_path: None,
                });
            }
        }
    }

    // Required fields validation
    if let Some(required) = &schema.required {
        let empty_map = serde_json::Map::new();
        let data_obj = data.as_object().unwrap_or(&empty_map);
        for required_field in required {
            if !data_obj.contains_key(required_field)
                && !data_obj.contains_key(&format!("_{required_field}"))
            {
                errors.push(ValidationError {
                    error_type: "require".to_string(),
                    path: {
                        let mut new_path = path.to_vec();
                        new_path.push(Value::String(required_field.clone()));
                        new_path
                    },
                    message: None,
                    value: None,
                    expected: None,
                    got: None,
                    schema_path: None,
                });
            }
        }
    }

    // Choices validation
    if let Some(choices) = &schema.choices {
        let empty_map = serde_json::Map::new();
        let data_obj = data.as_object().unwrap_or(&empty_map);
        for (choice_name, choice_options) in choices {
            let present_choices: Vec<&String> = choice_options
                .iter()
                .filter(|choice| data_obj.contains_key(*choice))
                .collect();

            if present_choices.len() > 1 {
                let mut choice_values = serde_json::Map::new();
                for choice in &present_choices {
                    if let Some(value) = data_obj.get(*choice) {
                        choice_values.insert((*choice).clone(), value.clone());
                    }
                }

                errors.push(ValidationError {
                    error_type: "choices/multiple".to_string(),
                    path: {
                        let mut new_path = path.to_vec();
                        new_path.push(Value::String(choice_name.clone()));
                        new_path
                    },
                    message: Some("Only one choice element is allowed".to_string()),
                    value: Some(Value::Object(choice_values)),
                    expected: None,
                    got: None,
                    schema_path: None,
                });
            }

            // Also check for excluded choices
            if let Some(elements) = &schema.elements {
                for key in data_obj.keys() {
                    if let Some(element_schema) = elements.get(key) {
                        if element_schema.choice_of.as_ref() == Some(choice_name)
                            && !choice_options.contains(key)
                        {
                            errors.push(ValidationError {
                                error_type: "choice/excluded".to_string(),
                                message: Some(format!(
                                    "Choice element {} is not allowed, only {}",
                                    choice_name,
                                    choice_options.join(", ")
                                )),
                                path: {
                                    let mut new_path = path.to_vec();
                                    new_path.push(Value::String(choice_name.clone()));
                                    new_path
                                },
                                value: None,
                                expected: None,
                                got: None,
                                schema_path: Some(vec![Value::String("choices".to_string())]),
                            });
                        }
                    }
                }
            }
        }
    }

    let valid = errors.is_empty();
    ValidationResult { errors, valid }
}

fn validate_element_with_data(
    ctx: &ValidationContext,
    element_schema: &crate::types::FhirSchemaElement,
    data: &Value,
    path: &[Value],
    root_data: &Value,
) -> ValidationResult {
    let mut errors = Vec::new();

    // Array validation
    if element_schema.array.unwrap_or(false) && !data.is_array() {
        errors.push(ValidationError {
            error_type: "type/array".to_string(),
            message: Some("Expected array".to_string()),
            path: path.to_vec(),
            value: Some(data.clone()),
            expected: None,
            got: None,
            schema_path: {
                let mut schema_path = path.to_vec();
                schema_path.push(Value::String("array".to_string()));
                Some(schema_path)
            },
        });
        return ValidationResult {
            errors,
            valid: false,
        };
    }

    if !element_schema.array.unwrap_or(false) && data.is_array() {
        errors.push(ValidationError {
            error_type: "type/array".to_string(),
            message: Some("Expected not array".to_string()),
            path: path.to_vec(),
            value: Some(data.clone()),
            expected: None,
            got: None,
            schema_path: None,
        });
        return ValidationResult {
            errors,
            valid: false,
        };
    }

    // Handle array elements
    if element_schema.array.unwrap_or(false) && data.is_array() {
        let array = data.as_array().unwrap();

        // Cardinality validation
        if let Some(min) = element_schema.min {
            if (array.len() as i32) < min {
                errors.push(ValidationError {
                    error_type: "min".to_string(),
                    message: Some(format!("expected min={} got {}", min, array.len())),
                    value: Some(Value::Number((array.len() as i32).into())),
                    expected: Some(Value::Number(min.into())),
                    got: None,
                    path: path.to_vec(),
                    schema_path: None,
                });
            }
        }

        if let Some(max) = element_schema.max {
            if (array.len() as i32) > max {
                errors.push(ValidationError {
                    error_type: "max".to_string(),
                    message: Some(format!("expected max={} got {}", max, array.len())),
                    value: Some(Value::Number((array.len() as i32).into())),
                    expected: Some(Value::Number(max.into())),
                    got: None,
                    path: path.to_vec(),
                    schema_path: None,
                });
            }
        }

        // Validate each array element
        for (index, item) in array.iter().enumerate() {
            let mut element_path = path.to_vec();
            element_path.push(Value::Number(index.into()));

            // Create element schema without array flag
            let mut item_schema = element_schema.clone();
            item_schema.array = Some(false);

            let result =
                validate_element_with_data(ctx, &item_schema, item, &element_path, root_data);
            errors.extend(result.errors);
        }
    } else {
        // Non-array validation
        // Convert element schema to FhirSchema for validation
        let temp_schema = FhirSchema {
            url: "temp".to_string(),
            version: None,
            name: "temp".to_string(),
            type_name: element_schema.type_name.clone().unwrap_or_default(),
            kind: "temp".to_string(),
            derivation: None,
            base: None,
            abstract_type: None,
            class: "temp".to_string(),
            description: None,
            package_name: None,
            package_version: None,
            package_id: None,
            package_meta: None,
            elements: element_schema.elements.clone(),
            required: element_schema.required.clone(),
            excluded: element_schema.excluded.clone(),
            extensions: element_schema.extensions.clone(),
            constraint: element_schema.constraint.clone(),
            primitive_type: None,
            choices: None,
        };

        let result = validate_schema_with_data(ctx, &temp_schema, data, path, root_data);
        errors.extend(result.errors);

        // Pattern validation
        if let Some(pattern) = &element_schema.pattern {
            if let Some(pattern_string) = &pattern.string {
                if let Some(data_string) = data.as_str() {
                    if data_string != pattern_string {
                        errors.push(ValidationError {
                            error_type: "pattern".to_string(),
                            expected: Some(Value::String(pattern_string.clone())),
                            got: Some(data.clone()),
                            path: path.to_vec(),
                            message: None,
                            value: None,
                            schema_path: {
                                let mut schema_path = path.to_vec();
                                schema_path.push(Value::String("pattern".to_string()));
                                Some(schema_path)
                            },
                        });
                    }
                }
            }
        }
    }

    let valid = errors.is_empty();
    ValidationResult { errors, valid }
}

fn validate_type_with_data(
    ctx: &ValidationContext,
    type_name: &str,
    data: &Value,
    path: &[Value],
    parent_schema: &FhirSchema,
    errors: &mut Vec<ValidationError>,
    root_data: &Value,
) -> bool {
    // Handle null values with primitive extensions
    if data.is_null() {
        // For array elements, check if there's a corresponding primitive extension
        if path.len() >= 2 {
            if let Some(Value::Number(index)) = path.last() {
                if let Some(Value::String(field_name)) = path.get(path.len() - 2) {
                    let parent = get_parent_data_safe(root_data, path);
                    if let Some(parent_obj) = parent.and_then(|p| p.as_object()) {
                        let primitive_ext_key = format!("_{field_name}");
                        if let Some(Value::Array(primitive_ext)) =
                            parent_obj.get(&primitive_ext_key)
                        {
                            let index_usize = index.as_u64().unwrap_or(0) as usize;
                            if index_usize < primitive_ext.len()
                                && !primitive_ext[index_usize].is_null()
                            {
                                return true; // Allow null if primitive extension exists at same index
                            }
                        }
                    }
                }
            }
        } else if !path.is_empty() {
            // Non-array case
            if let Some(Value::String(element_name)) = path.last() {
                let parent = get_parent_data_safe(root_data, path);
                if let Some(parent_obj) = parent.and_then(|p| p.as_object()) {
                    let primitive_ext_key = format!("_{element_name}");
                    if parent_obj.contains_key(&primitive_ext_key) {
                        return true; // Allow null if primitive extension exists
                    }
                }
            }
        }
        return false;
    }

    // Primitive type validation first
    match type_name {
        "string" | "code" | "url" => data.is_string(),
        "boolean" => data.is_boolean(),
        "number" | "integer" => data.is_number(),
        _ => {
            // Check if it's a referenced type
            if let Some(type_schema) = ctx.schemas.get(type_name) {
                if type_schema.kind != "primitive-type" {
                    // If parent schema has elements, don't validate type's elements
                    // They will be validated by the merged elements
                    if parent_schema.elements.is_some() {
                        // Just validate the type without elements
                        let mut type_schema_without_elements = type_schema.clone();
                        type_schema_without_elements.elements = None;
                        let result = validate_schema_with_data(
                            ctx,
                            &type_schema_without_elements,
                            data,
                            path,
                            root_data,
                        );

                        for error in result.errors {
                            let mut new_error = error;
                            if let Some(ref mut schema_path) = new_error.schema_path {
                                // Build correct schema path for nested type references
                                let element_path: Vec<&str> =
                                    path.iter().filter_map(|p| p.as_str()).collect();
                                if let Some(parent_element) = element_path.last() {
                                    // Check if the error schema-path already starts with the parent element
                                    if schema_path.first().and_then(|s| s.as_str())
                                        == Some(parent_element)
                                    {
                                        // Remove the duplicate parent element
                                        let mut new_schema_path = element_path
                                            .iter()
                                            .take(element_path.len() - 1)
                                            .map(|s| Value::String(s.to_string()))
                                            .collect::<Vec<_>>();
                                        new_schema_path
                                            .push(Value::String(parent_element.to_string()));
                                        new_schema_path.push(Value::String("type".to_string()));
                                        new_schema_path.push(Value::String(type_name.to_string()));
                                        new_schema_path.extend(schema_path.iter().skip(1).cloned());
                                        new_error.schema_path = Some(new_schema_path);
                                    } else {
                                        let mut new_schema_path = element_path
                                            .iter()
                                            .take(element_path.len() - 1)
                                            .map(|s| Value::String(s.to_string()))
                                            .collect::<Vec<_>>();
                                        new_schema_path
                                            .push(Value::String(parent_element.to_string()));
                                        new_schema_path.push(Value::String("type".to_string()));
                                        new_schema_path.push(Value::String(type_name.to_string()));
                                        new_schema_path.extend(schema_path.iter().cloned());
                                        new_error.schema_path = Some(new_schema_path);
                                    }
                                }
                            }
                            errors.push(new_error);
                        }
                        return result.valid;
                    } else {
                        let result =
                            validate_schema_with_data(ctx, type_schema, data, path, root_data);
                        for error in result.errors {
                            let mut new_error = error;
                            if let Some(ref mut schema_path) = new_error.schema_path {
                                // Build correct schema path for nested type references
                                let element_path: Vec<&str> =
                                    path.iter().filter_map(|p| p.as_str()).collect();
                                if let Some(parent_element) = element_path.last() {
                                    // Similar logic as above
                                    if schema_path.first().and_then(|s| s.as_str())
                                        == Some(parent_element)
                                    {
                                        let mut new_schema_path = element_path
                                            .iter()
                                            .take(element_path.len() - 1)
                                            .map(|s| Value::String(s.to_string()))
                                            .collect::<Vec<_>>();
                                        new_schema_path
                                            .push(Value::String(parent_element.to_string()));
                                        new_schema_path.push(Value::String("type".to_string()));
                                        new_schema_path.push(Value::String(type_name.to_string()));
                                        new_schema_path.extend(schema_path.iter().skip(1).cloned());
                                        new_error.schema_path = Some(new_schema_path);
                                    } else {
                                        let mut new_schema_path = element_path
                                            .iter()
                                            .take(element_path.len() - 1)
                                            .map(|s| Value::String(s.to_string()))
                                            .collect::<Vec<_>>();
                                        new_schema_path
                                            .push(Value::String(parent_element.to_string()));
                                        new_schema_path.push(Value::String("type".to_string()));
                                        new_schema_path.push(Value::String(type_name.to_string()));
                                        new_schema_path.extend(schema_path.iter().cloned());
                                        new_error.schema_path = Some(new_schema_path);
                                    }
                                }
                            }
                            errors.push(new_error);
                        }
                        return result.valid;
                    }
                }
            }
            true // Unknown types pass for now
        }
    }
}

fn get_parent_data_safe<'a>(root_data: &'a Value, path: &[Value]) -> Option<&'a Value> {
    // Handle array elements - need to go up two levels
    if path.len() >= 2 && path[path.len() - 1].is_number() {
        let mut current = root_data;
        for path_component in path.iter().take(path.len() - 2) {
            if let Some(key) = path_component.as_str() {
                current = current.get(key)?;
            } else if let Some(index) = path_component.as_u64() {
                current = current.get(index as usize)?;
            } else {
                return None;
            }
        }
        return Some(current);
    }

    let mut current = root_data;
    for path_component in path.iter().take(path.len() - 1) {
        if let Some(key) = path_component.as_str() {
            current = current.get(key)?;
        } else if let Some(index) = path_component.as_u64() {
            current = current.get(index as usize)?;
        } else {
            return None;
        }
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_simple() {
        let ctx = ValidationContext {
            schemas: HashMap::new(),
        };

        let result = validate(&ctx, vec![], &json!({}));
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }
}
