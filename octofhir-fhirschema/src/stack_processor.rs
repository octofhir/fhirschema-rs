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

/// Puts `value` at `path` (`["coding", "code"]`) in a match object, creating and merging the
/// objects on the way.
fn insert_at_path(object: &mut serde_json::Map<String, Value>, path: &[&str], value: Value) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = object;
    for part in parents {
        let entry = current
            .entry((*part).to_string())
            .or_insert_with(|| json!({}));
        if !entry.is_object() {
            *entry = json!({});
        }
        current = entry.as_object_mut().expect("just made an object");
    }
    match (current.get_mut(*last), value) {
        (Some(Value::Object(existing)), Value::Object(incoming)) => {
            for (k, v) in incoming {
                existing.insert(k, v);
            }
        }
        (_, value) => {
            current.insert((*last).to_string(), value);
        }
    }
}

fn build_match_for_slice(slicing: &Value, slice_schema: &Value) -> Value {
    let discriminator = slicing.get("discriminator");
    if discriminator.is_none() {
        // An extension slice needs no slicing definition: it is sliced by
        // `url` (FHIR "Extension slicing"), the extension's canonical.
        return match slice_schema.get("url").and_then(Value::as_str) {
            Some(url) => json!({ "url": url }),
            None => json!({}),
        };
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
                    if let Some(pattern) = slice_schema.get("pattern")
                        && let Some(value) = pattern.get("value")
                        && let Some(obj) = match_obj.as_object_mut()
                        && let Some(value_obj) = value.as_object()
                    {
                        for (k, v) in value_obj {
                            obj.insert(k.clone(), v.clone());
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
                                if path_parts.len() == 1
                                    && path_parts[0] == field_name
                                    && let Some(match_map) = match_obj.as_object_mut()
                                {
                                    match_map.insert(field_name, pattern_value.clone());
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

                    if let Some(pattern_value) = value.get("value")
                        && let Some(obj) = match_obj.as_object_mut()
                    {
                        // `coding.code` and `coding.system` both go under `coding`: merge
                        // along the path instead of replacing what an earlier
                        // discriminator put there.
                        insert_at_path(obj, &path_parts, pattern_value.clone());
                    }
                }
            }
        }
    }

    match_obj
}

fn build_slice_node(slice_schema: Value, match_value: Value, slice_info: Option<&Value>) -> Value {
    // Process slice schema to handle circular references
    let mut processed_schema = slice_schema;

    // If schema has an empty extensions object, replace with "[Circular Reference]"
    // This happens when an Extension type has nested extension elements
    if let Some(schema_obj) = processed_schema.as_object_mut()
        && let Some(extensions) = schema_obj.get("extensions")
        && extensions.is_object()
        && extensions.as_object().is_some_and(|o| o.is_empty())
    {
        schema_obj.insert("extensions".to_string(), json!("[Circular Reference]"));
    }

    let mut node = json!({
        "match": match_value,
        "schema": processed_schema
    });

    if let Some(slice) = slice_info {
        if let Some(min) = slice.get("min")
            && min.as_i64().unwrap_or(0) != 0
        {
            node["min"] = min.clone();
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
        if let Some(slicing_obj) = merged_slicing.as_object_mut()
            && let Some(action_slicing_obj) = slicing_from_action.as_object()
        {
            for (k, v) in action_slicing_obj {
                slicing_obj.insert(k.clone(), v.clone());
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

        // A required slice requires the element it slices: without the element there is no item
        // to count against the slice's minimum.
        let slice_required = slice_node
            .get("min")
            .and_then(Value::as_i64)
            .is_some_and(|min| min > 0);
        parent["slicing"]["slices"][slice_name] = slice_node;
        if slice_required {
            parent["_required"] = json!(true);
        }
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

            if let Some(match_obj) = match_value
                && let Some(url) = match_obj.get("url")
            {
                extension["url"] = url.clone();
            }

            // Add slice properties (min, max)
            // Per FHIRSchema spec: include min only if >= 1 (required extension)
            // Always include max for cardinality constraint (default to "*" if not specified)
            if let Some(min) = slice.get("min") {
                let min_val = min.as_i64().unwrap_or(0);
                if min_val >= 1 {
                    extension["min"] = min.clone();
                }
            }

            // Add max from slice level, or from schema, or default to "*"
            if let Some(max) = slice.get("max") {
                extension["max"] = max.clone();
            } else if let Some(schema_obj) = schema.and_then(|s| s.as_object()) {
                if let Some(schema_max) = schema_obj.get("max") {
                    extension["max"] = schema_max.clone();
                } else {
                    // Default to "*" (unbounded) per FHIR spec
                    extension["max"] = json!("*");
                }
            } else {
                // No schema, default to "*"
                extension["max"] = json!("*");
            }

            // Add clean schema properties
            if let Some(schema_obj) = schema.and_then(|s| s.as_object()) {
                for (key, value) in schema_obj {
                    // Skip internal fields: min (handled above), slicing, elements, type
                    if !matches!(key.as_str(), "min" | "slicing" | "elements" | "type") {
                        // Handle circular reference: replace empty extensions object with "[Circular Reference]"
                        // This happens when an Extension type has nested extension elements
                        if key == "extensions" {
                            if value.is_object() && value.as_object().is_some_and(|o| o.is_empty())
                            {
                                extension[key] = json!("[Circular Reference]");
                            } else {
                                extension[key] = value.clone();
                            }
                        } else {
                            extension[key] = value.clone();
                        }
                    }
                }

                // Add min from schema if >= 1 and not already added from slice
                if let Some(schema_min) = schema_obj.get("min") {
                    let min_val = schema_min.as_i64().unwrap_or(0);
                    if min_val >= 1 && extension.get("min").is_none() {
                        extension["min"] = schema_min.clone();
                    }
                }
            }

            extensions.insert(slice_name.clone(), extension);
        }
    }

    extensions
}

/// Merge `child` into the `existing` definition of the same element: the child's properties win,
/// and their slices and child elements are combined.
fn merge_element(existing: &mut Value, child: Value) {
    let (Some(target), Value::Object(source)) = (existing.as_object_mut(), child) else {
        return;
    };
    for (key, value) in source {
        match (key.as_str(), target.get_mut(&key)) {
            ("elements", Some(current)) if current.is_object() && value.is_object() => {
                let current = current.as_object_mut().expect("object");
                for (name, element) in value.as_object().cloned().unwrap_or_default() {
                    match current.get_mut(&name) {
                        Some(known) if known.is_object() => merge_element(known, element),
                        _ => {
                            current.insert(name, element);
                        }
                    }
                }
            }
            ("slicing", Some(current)) if current.is_object() && value.is_object() => {
                let slices = value.get("slices").cloned();
                let current = current.as_object_mut().expect("object");
                for (name, field) in value.as_object().cloned().unwrap_or_default() {
                    if name != "slices" {
                        current.insert(name, field);
                    }
                }
                if let Some(Value::Object(slices)) = slices {
                    let known = current.entry("slices").or_insert_with(|| json!({}));
                    if let Some(known) = known.as_object_mut() {
                        known.extend(slices);
                    }
                }
            }
            _ => {
                target.insert(key, value);
            }
        }
    }
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

    // An element entered again (a choice, then one of its type slices) keeps what it already has.
    match parent["elements"].get_mut(element_name) {
        Some(existing) if existing.is_object() => merge_element(existing, clean_child),
        _ => parent["elements"][element_name] = clean_child,
    }

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
    apply_actions_lending(stack, actions, value, false)
}

/// [`apply_actions`] for an element that may be a slice listed without its slicing root.
///
/// The value belongs to the innermost element entered: an element entered on the way to a child
/// or to one of its slices is a container. With `lend_to_sliced`, an element entered only to
/// reach the slice that is `value` also takes `value`: a named slice of a single-valued element
/// (`Composition.date:IssueDate`, `born[x]:BornAge`) constrains the element itself. A repeating
/// element (`extension:sqlText`) must not take its slice's cardinality or profile.
pub fn apply_actions_lending(
    stack: Vec<Value>,
    actions: &[Action],
    value: &FhirSchemaElement,
    lend_to_sliced: bool,
) -> crate::error::Result<Vec<Value>> {
    let mut current_stack = stack;
    let value_json =
        serde_json::to_value(value).map_err(crate::error::FhirSchemaError::SerializationError)?;

    for (i, action) in actions.iter().enumerate() {
        let next_action = actions.get(i + 1);
        let enters_value_slice = matches!(action, Action::Enter { .. })
            && matches!(next_action, Some(Action::EnterSlice { .. }))
            && i + 2 == actions.len();

        let value_to_use = match next_action {
            Some(Action::EnterSlice { .. }) if lend_to_sliced && enters_value_slice => {
                value_json.clone()
            }
            Some(Action::Enter { .. } | Action::EnterSlice { .. }) => json!({}),
            _ => value_json.clone(),
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
    fn nested_discriminators_on_one_element_merge() {
        // US Core vital signs (from the R4 vitalsigns profile): category sliced by
        // coding.code and coding.system.
        let slicing = json!({
            "discriminator": [
                { "type": "value", "path": "coding.code" },
                { "type": "value", "path": "coding.system" }
            ]
        });
        let slice_schema = json!({
            "elements": { "coding": { "elements": {
                "system": { "pattern": { "type": "uri", "value": "http://terminology.hl7.org/CodeSystem/observation-category" } },
                "code": { "pattern": { "type": "code", "value": "vital-signs" } }
            } } }
        });
        assert_eq!(
            build_match_for_slice(&slicing, &slice_schema),
            json!({ "coding": {
                "code": "vital-signs",
                "system": "http://terminology.hl7.org/CodeSystem/observation-category"
            } })
        );
        // Three levels keep their own names.
        let slicing = json!({ "discriminator": [{ "type": "value", "path": "a.b.c" }] });
        let slice_schema = json!({ "elements": { "a": { "elements": { "b": { "elements": {
            "c": { "pattern": { "type": "code", "value": "x" } }
        } } } } } });
        assert_eq!(
            build_match_for_slice(&slicing, &slice_schema),
            json!({ "a": { "b": { "c": "x" } } })
        );
    }

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
