use crate::action_calculator::calculate_actions;
use crate::choice_handler::{expand_choice_element, is_choice_element};
use crate::element_transformer::transform_element;
use crate::error::{FhirSchemaError, Result};
use crate::path_parser::{enrich_path, parse_path};
use crate::stack_processor::apply_actions;
use crate::types::{
    ConversionContext, FhirSchema, FhirSchemaElement, StructureDefinition,
    StructureDefinitionElement,
};
use serde_json::{Value, json};
use std::collections::HashMap;

fn build_resource_header(
    structure_definition: &StructureDefinition,
    context: Option<&ConversionContext>,
) -> FhirSchema {
    let mut schema = FhirSchema {
        name: structure_definition.name.clone(),
        type_name: structure_definition.type_name.clone(),
        url: structure_definition.url.clone(),
        version: structure_definition.version.clone(),
        description: structure_definition.description.clone(),
        package_name: structure_definition.package_name.clone(),
        package_version: structure_definition.package_version.clone(),
        package_id: structure_definition.package_id.clone(),
        kind: structure_definition.kind.clone(),
        derivation: structure_definition.derivation.clone(),
        base: None,
        abstract_type: structure_definition.abstract_type,
        class: determine_class(structure_definition),
        package_meta: context.and_then(|c| c.package_meta.clone()),
        elements: None,
        required: None,
        excluded: None,
        extensions: None,
        constraint: None,
        primitive_type: None,
        choices: None,
    };

    // Set base if present (and not Element itself)
    if let Some(base_definition) = &structure_definition.base_definition
        && structure_definition.type_name != "Element"
    {
        schema.base = Some(base_definition.clone());
    }

    schema
}

fn determine_class(structure_definition: &StructureDefinition) -> String {
    if structure_definition.kind == "resource"
        && structure_definition.derivation.as_deref() == Some("constraint")
    {
        return "profile".to_string();
    }
    if structure_definition.type_name == "Extension" {
        return "extension".to_string();
    }
    structure_definition.kind.clone()
}

fn get_differential(structure_definition: &StructureDefinition) -> Vec<StructureDefinitionElement> {
    structure_definition
        .differential
        .as_ref()
        .map(|d| {
            d.element
                .iter()
                .filter(|e| e.path.contains('.'))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn sort_elements_by_index(mut elements: HashMap<String, Value>) -> HashMap<String, Value> {
    // Get all entries and sort by index
    let mut entries: Vec<(String, Value)> = elements.drain().collect();
    entries.sort_by(|a, b| {
        let index_a =
            a.1.get("index")
                .and_then(|i| i.as_u64())
                .unwrap_or(u64::MAX);
        let index_b =
            b.1.get("index")
                .and_then(|i| i.as_u64())
                .unwrap_or(u64::MAX);
        index_a.cmp(&index_b)
    });

    // Rebuild object in sorted order
    let mut result = HashMap::new();
    for (key, mut value) in entries {
        // Recursively sort nested elements
        if let Some(nested_elements) = value.get("elements").cloned()
            && let Ok(nested_map) =
                serde_json::from_value::<HashMap<String, Value>>(nested_elements)
        {
            let sorted_nested = sort_elements_by_index(nested_map);
            value["elements"] = serde_json::to_value(sorted_nested).unwrap_or(json!({}));
        }
        result.insert(key, value);
    }

    result
}

fn normalize_schema(mut schema: Value) -> Value {
    match schema {
        Value::Array(ref mut arr) => {
            // Don't sort arrays - preserve their original order
            for item in arr.iter_mut() {
                *item = normalize_schema(item.clone());
            }
            schema
        }
        Value::Object(ref mut obj) => {
            // Handle circular references in extensions fields
            // Replace empty extensions objects with "[Circular Reference]"
            if let Some(extensions) = obj.get("extensions")
                && extensions.is_object()
                && extensions.as_object().is_some_and(|o| o.is_empty())
            {
                obj.insert("extensions".to_string(), json!("[Circular Reference]"));
            }

            // Process all values recursively first
            for value in obj.values_mut() {
                *value = normalize_schema(value.clone());
            }

            // Sort elements by index if this is an elements object
            if let Some(elements_val) = obj.get("elements").cloned()
                && let Ok(elements_map) =
                    serde_json::from_value::<HashMap<String, Value>>(elements_val)
            {
                let sorted_elements = sort_elements_by_index(elements_map);
                obj.insert(
                    "elements".to_string(),
                    serde_json::to_value(sorted_elements).unwrap_or(json!({})),
                );
            }

            // Sort required array
            if let Some(Value::Array(required)) = obj.get_mut("required") {
                required.sort_by(|a, b| {
                    let a_str = a.as_str().unwrap_or("");
                    let b_str = b.as_str().unwrap_or("");
                    a_str.cmp(b_str)
                });
            }

            schema
        }
        _ => schema,
    }
}

pub fn translate(
    structure_definition: StructureDefinition,
    context: Option<ConversionContext>,
) -> Result<FhirSchema> {
    // Handle primitive types - they don't have differential elements
    if structure_definition.kind == "primitive-type" {
        let header = build_resource_header(&structure_definition, context.as_ref());
        return Ok(header);
    }

    let header = build_resource_header(&structure_definition, context.as_ref());
    let elements = get_differential(&structure_definition);

    // Initialize stack with header
    let header_json = serde_json::to_value(&header).map_err(FhirSchemaError::SerializationError)?;
    let mut stack = vec![header_json];
    let mut prev_path = Vec::new();
    let mut element_queue = elements;
    let mut index = 0;

    // Process elements in original order
    element_queue.reverse();
    while let Some(element) = element_queue.pop() {
        // Handle choice elements
        if is_choice_element(&element) {
            let expanded = expand_choice_element(&element)?;

            // Add expanded elements back to queue in reverse order
            for expanded_elem in expanded.into_iter().rev() {
                element_queue.push(expanded_elem);
            }
            index += 1; // Increment index for the original choice element
            continue;
        }

        // Parse and enrich path
        let parsed_path = parse_path(&element);
        let enriched_path = enrich_path(&prev_path, &parsed_path);

        // Calculate actions
        let actions = calculate_actions(&prev_path, &enriched_path);

        // Transform element
        let mut transformed_element = transform_element(&element, &structure_definition)?;
        transformed_element.index = Some(index);
        index += 1;

        // Apply actions
        stack = apply_actions(stack, &actions, &transformed_element)?;

        prev_path = enriched_path;
    }

    // Final cleanup - process remaining exits back to root
    let final_actions = calculate_actions(&prev_path, &[]);
    let dummy_element = FhirSchemaElement {
        index: Some(index),
        ..Default::default()
    };
    stack = apply_actions(stack, &final_actions, &dummy_element)?;

    // Should have exactly one element on stack - the completed schema
    if stack.len() != 1 {
        return Err(FhirSchemaError::conversion_error(format!(
            "Invalid stack state: expected 1 element, got {}",
            stack.len()
        )));
    }

    // Normalize and convert back to FhirSchema
    let normalized = normalize_schema(stack.into_iter().next().unwrap());
    let final_schema: FhirSchema =
        serde_json::from_value(normalized).map_err(FhirSchemaError::SerializationError)?;

    Ok(final_schema)
}

// Export all modules for testing
pub use crate::action_calculator::calculate_actions as calculate_actions_export;
pub use crate::choice_handler::{
    expand_choice_element as expand_choice_element_export,
    is_choice_element as is_choice_element_export,
};
pub use crate::element_transformer::transform_element as transform_element_export;
pub use crate::path_parser::{
    enrich_path as enrich_path_export, get_common_path, parse_path as parse_path_export,
};
pub use crate::stack_processor::apply_actions as apply_actions_export;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_class() {
        let mut structure_def = StructureDefinition {
            resource_type: "StructureDefinition".to_string(),
            url: "http://example.com".to_string(),
            name: "TestProfile".to_string(),
            status: "active".to_string(),
            kind: "resource".to_string(),
            type_name: "Patient".to_string(),
            derivation: Some("constraint".to_string()),
            id: None,
            version: None,
            title: None,
            date: None,
            description: None,
            abstract_type: None,
            base_definition: None,
            package_name: None,
            package_version: None,
            package_id: None,
            snapshot: None,
            differential: None,
        };

        assert_eq!(determine_class(&structure_def), "profile");

        structure_def.type_name = "Extension".to_string();
        structure_def.derivation = None; // Reset derivation
        assert_eq!(determine_class(&structure_def), "extension");

        structure_def.type_name = "string".to_string();
        structure_def.kind = "primitive-type".to_string();
        assert_eq!(determine_class(&structure_def), "primitive-type");
    }

    #[test]
    fn test_translate_primitive_type() {
        let structure_def = StructureDefinition {
            resource_type: "StructureDefinition".to_string(),
            url: "http://hl7.org/fhir/StructureDefinition/string".to_string(),
            name: "string".to_string(),
            status: "active".to_string(),
            kind: "primitive-type".to_string(),
            type_name: "string".to_string(),
            id: None,
            version: None,
            title: None,
            date: None,
            description: None,
            abstract_type: None,
            base_definition: None,
            derivation: None,
            package_name: None,
            package_version: None,
            package_id: None,
            snapshot: None,
            differential: None,
        };

        let result = translate(structure_def, None).unwrap();
        assert_eq!(result.name, "string");
        assert_eq!(result.type_name, "string");
        assert_eq!(result.kind, "primitive-type");
        assert_eq!(result.class, "primitive-type");
    }
}
