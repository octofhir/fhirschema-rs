use crate::types::{Action, FhirSchemaElement};
use serde_json::{Value, json};
use std::collections::HashMap;

fn pop_and_update<F>(stack: Vec<Value>, update_fn: F) -> Vec<Value>
where
    F: FnOnce(&mut Value, Value) -> crate::error::Result<()>,
{
    let mut new_stack = stack;
    if new_stack.len() < 2 {
        return new_stack;
    }

    let child = new_stack.pop().unwrap();
    let parent_index = new_stack.len() - 1;

    let child_backup = child.clone();
    if update_fn(&mut new_stack[parent_index], child).is_err() {
        // If update fails, put child back
        new_stack.push(child_backup);
    }

    new_stack
}

fn build_match_for_slice(slicing: &Value, slice_schema: &Value) -> Value {
    let discriminator = slicing.get("discriminator");
    if discriminator.is_none() {
        return json!({});
    }

    let mut match_obj = json!({});

    if let Some(discriminators) = discriminator.and_then(|d| d.as_array()) {
        for discriminator in discriminators {
            let disc_type = discriminator.get("type").and_then(|t| t.as_str());
            let path = discriminator.get("path").and_then(|p| p.as_str());

            if !matches!(disc_type, Some("pattern") | Some("value") | None) {
                continue;
            }

            if let Some(path_str) = path {
                let path_str = path_str.trim();

                if path_str == "$this" {
                    // Merge pattern value from slice schema
                    if let Some(pattern) = slice_schema.get("pattern") {
                        if let Some(value) = pattern.get("value") {
                            if let Some(obj) = match_obj.as_object_mut() {
                                if let Some(value_obj) = value.as_object() {
                                    for (k, v) in value_obj {
                                        obj.insert(k.clone(), v.clone());
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Build path to pattern in nested elements
                    let path_parts: Vec<&str> =
                        path_str.split('.').filter(|p| !p.is_empty()).collect();

                    // Look for pattern fields in slice schema
                    if let Some(slice_obj) = slice_schema.as_object() {
                        for (pattern_key, pattern_value) in slice_obj {
                            if pattern_key.starts_with("pattern") {
                                let field_name = pattern_key.replace("pattern", "").to_lowercase();
                                if path_parts.len() == 1 && path_parts[0] == field_name {
                                    if let Some(match_map) = match_obj.as_object_mut() {
                                        match_map.insert(field_name, pattern_value.clone());
                                    }
                                }
                            }
                        }
                    }

                    // Also check nested elements
                    let mut current_path = vec!["elements"];
                    for (i, part) in path_parts.iter().enumerate() {
                        current_path.push(part);
                        if i < path_parts.len() - 1 {
                            current_path.push("elements");
                        }
                    }
                    current_path.push("pattern");

                    // Get value from slice schema
                    let mut value = slice_schema;
                    for segment in &current_path {
                        if let Some(v) = value.get(segment) {
                            value = v;
                        } else {
                            break;
                        }
                    }

                    if let Some(pattern_value) = value.get("value") {
                        // For simple case, just set the last part directly
                        if path_parts.len() == 1 {
                            if let Some(obj) = match_obj.as_object_mut() {
                                obj.insert(path_parts[0].to_string(), pattern_value.clone());
                            }
                        } else if path_parts.len() > 1 {
                            // For nested paths, build a nested object
                            let mut nested_value = pattern_value.clone();
                            for _path_part in path_parts.iter().rev().skip(1) {
                                let mut temp_obj = serde_json::Map::new();
                                temp_obj
                                    .insert(path_parts.last().unwrap().to_string(), nested_value);
                                nested_value = json!(temp_obj);
                            }
                            if let Some(obj) = match_obj.as_object_mut() {
                                obj.insert(path_parts[0].to_string(), nested_value);
                            }
                        }
                    }
                }
            }
        }
    }

    match_obj
}

fn build_slice_node(slice_schema: Value, match_value: Value, slice_info: Option<&Value>) -> Value {
    let mut node = json!({
        "match": match_value,
        "schema": slice_schema
    });

    if let Some(slice) = slice_info {
        if let Some(min) = slice.get("min") {
            if min.as_i64().unwrap_or(0) != 0 {
                node["min"] = min.clone();
            }
        }

        if let Some(max) = slice.get("max") {
            node["max"] = max.clone();
        }
    }

    node
}

fn build_slice(
    action: &Action,
    parent: &mut Value,
    slice_schema: Value,
) -> crate::error::Result<()> {
    if let Action::ExitSlice {
        slice_name,
        slicing,
        slice,
    } = action
    {
        let slicing_info = parent.get("slicing").cloned().unwrap_or_else(|| json!({}));
        let slicing_from_action = slicing.clone().unwrap_or_else(|| json!({}));

        // Merge slicing info
        let mut merged_slicing = slicing_info;
        if let Some(slicing_obj) = merged_slicing.as_object_mut() {
            if let Some(action_slicing_obj) = slicing_from_action.as_object() {
                for (k, v) in action_slicing_obj {
                    slicing_obj.insert(k.clone(), v.clone());
                }
            }
        }

        let match_value = build_match_for_slice(&merged_slicing, &slice_schema);
        let slice_node = build_slice_node(slice_schema, match_value, slice.as_ref());

        // Initialize slicing if needed
        if parent.get("slicing").is_none() {
            parent["slicing"] = json!({});
        }

        // Set the merged slicing
        parent["slicing"] = merged_slicing;

        // Initialize slices if needed
        if parent["slicing"]["slices"].is_null() {
            parent["slicing"]["slices"] = json!({});
        }

        parent["slicing"]["slices"][slice_name] = slice_node;
    }

    Ok(())
}

fn slicing_to_extensions(slicing_element: &Value) -> HashMap<String, Value> {
    let mut extensions = HashMap::new();

    if let Some(slices) = slicing_element
        .get("slicing")
        .and_then(|s| s.get("slices"))
        .and_then(|s| s.as_object())
    {
        for (slice_name, slice) in slices {
            let match_value = slice.get("match");
            let schema = slice.get("schema");

            let mut extension = json!({});

            if let Some(match_obj) = match_value {
                if let Some(url) = match_obj.get("url") {
                    extension["url"] = url.clone();
                }
            }

            // Add slice properties (min, max) - skip min if 0
            if let Some(min) = slice.get("min") {
                if min.as_i64().unwrap_or(0) != 0 {
                    extension["min"] = min.clone();
                }
            }
            if let Some(max) = slice.get("max") {
                extension["max"] = max.clone();
            }

            // Add clean schema properties
            if let Some(schema_obj) = schema.and_then(|s| s.as_object()) {
                for (key, value) in schema_obj {
                    if !matches!(key.as_str(), "slicing" | "elements" | "type") {
                        extension[key] = value.clone();
                    }
                }

                // Add min from schema if not 0 and not already added from sliceProps
                if let Some(schema_min) = schema_obj.get("min") {
                    if schema_min.as_i64().unwrap_or(0) != 0 && extension.get("min").is_none() {
                        extension["min"] = schema_min.clone();
                    }
                }
            }

            extensions.insert(slice_name.clone(), extension);
        }
    }

    extensions
}

fn add_element(element_name: &str, parent: &mut Value, child: Value) -> crate::error::Result<()> {
    // Special handling for extension elements
    if element_name == "extension" {
        let extensions = slicing_to_extensions(&child);
        parent["extensions"] = json!(extensions);
    }

    // Initialize elements if needed
    if parent.get("elements").is_none() {
        parent["elements"] = json!({});
    }

    // Determine actual element name (handle choiceOf)
    let actual_element_name = child
        .get("choiceOf")
        .and_then(|c| c.as_str())
        .unwrap_or(element_name)
        .to_string();

    // Remove internal _required flag before adding
    let mut clean_child = child.clone();
    let required_flag = clean_child
        .get("_required")
        .and_then(|r| r.as_bool())
        .unwrap_or(false);
    if let Some(obj) = clean_child.as_object_mut() {
        obj.remove("_required");
    }

    parent["elements"][element_name] = clean_child;

    // Add to required array if needed
    if required_flag {
        if parent.get("required").is_none() {
            parent["required"] = json!([]);
        }

        if let Some(required_array) = parent["required"].as_array_mut() {
            let required_name = json!(actual_element_name);
            if !required_array.contains(&required_name) {
                required_array.push(required_name);
            }
        }
    }

    Ok(())
}

pub fn apply_actions(
    stack: Vec<Value>,
    actions: &[Action],
    value: &FhirSchemaElement,
) -> crate::error::Result<Vec<Value>> {
    let mut current_stack = stack;
    let value_json =
        serde_json::to_value(value).map_err(crate::error::FhirSchemaError::SerializationError)?;

    for (i, action) in actions.iter().enumerate() {
        let next_action = actions.get(i + 1);

        // If next action is enter, use empty object instead of value
        let value_to_use = if matches!(next_action, Some(Action::Enter { .. })) {
            json!({})
        } else {
            value_json.clone()
        };

        match action {
            Action::Enter { .. } => {
                current_stack.push(value_to_use);
            }
            Action::EnterSlice { .. } => {
                current_stack.push(value_to_use);
            }
            Action::Exit { el } => {
                let element_name = el.clone();
                current_stack = pop_and_update(current_stack, |parent, child| {
                    add_element(&element_name, parent, child)
                });
            }
            Action::ExitSlice { .. } => {
                current_stack = pop_and_update(current_stack, |parent, child| {
                    build_slice(action, parent, child)
                });
            }
        }
    }

    Ok(current_stack)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_match_for_slice() {
        let slicing = json!({
            "discriminator": [{
                "type": "pattern",
                "path": "$this"
            }]
        });

        let slice_schema = json!({
            "pattern": {
                "value": {
                    "system": "http://example.com"
                }
            }
        });

        let result = build_match_for_slice(&slicing, &slice_schema);
        assert_eq!(result["system"], "http://example.com");
    }

    #[test]
    fn test_apply_actions_simple() {
        let stack = vec![json!({"name": "Test"})];
        let actions = vec![Action::Enter {
            el: "contact".to_string(),
        }];
        let value = FhirSchemaElement {
            type_name: Some("ContactPoint".to_string()),
            ..Default::default()
        };

        let result = apply_actions(stack, &actions, &value).unwrap();
        assert_eq!(result.len(), 2);
    }
}
