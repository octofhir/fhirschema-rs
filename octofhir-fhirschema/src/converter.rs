use crate::action_calculator::calculate_actions;
use crate::choice_handler::{expand_choice_element, is_choice_element};
use crate::element_transformer::transform_element;
use crate::error::{FhirSchemaError, Result};
use crate::path_parser::{enrich_path, parse_path};
use crate::stack_processor::{apply_actions, apply_actions_lending};
use crate::types::{
    ConversionContext, FhirSchema, FhirSchemaConstraint, FhirSchemaElement, StructureDefinition,
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

    // Extract resource-level constraints from the root element (path = the type, e.g.
    // "Organization"): the snapshot's, else the differential's — a profile published without a
    // snapshot still declares its own invariants there.
    let root_of = |elements: &[StructureDefinitionElement]| {
        elements
            .iter()
            .find(|e| e.path == structure_definition.type_name)
            .cloned()
    };
    let root = structure_definition
        .snapshot
        .as_ref()
        .and_then(|snapshot| root_of(&snapshot.element))
        .or_else(|| {
            structure_definition
                .differential
                .as_ref()
                .and_then(|differential| root_of(&differential.element))
        });
    if let Some(root_element) = &root
        && let Some(constraints) = &root_element.constraint
        && !constraints.is_empty()
    {
        let mut constraint_map = HashMap::new();
        for constraint in constraints {
            constraint_map.insert(
                constraint.key.clone(),
                FhirSchemaConstraint {
                    expression: constraint.expression.clone(),
                    human: constraint.human.clone(),
                    severity: constraint.severity.clone(),
                },
            );
        }
        schema.constraint = Some(constraint_map);
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

/// Whether a slice, when its slicing root is not in the differential, also constrains the element
/// it slices. It does for a single-valued element: a named slice there is the element itself
/// (`Composition.date:IssueDate`, `FamilyMemberHistory.born[x]:BornAge`). It does not for a
/// repeating one, whose slice describes some of its items only: `extension` and
/// `modifierExtension`, and any element whose base cardinality in the snapshot repeats. A
/// choice's type slice (`value[x]:valueQuantity`) constrains its typed element, not the choice.
fn slice_constrains_element(
    element: &StructureDefinitionElement,
    structure_definition: &StructureDefinition,
) -> bool {
    let Some(slice_name) = element.slice_name.as_deref() else {
        return false;
    };
    let name = element.path.rsplit('.').next().unwrap_or_default();
    // `extension` is always repeating. A type slice of a choice (`value[x]:valueQuantity`) is
    // expanded to the choice (`value`) and the typed element (`valueQuantity`): it constrains
    // the typed element, and must not narrow the choice to its one type.
    let narrows_choice = slice_name
        .strip_prefix(name)
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()));
    if matches!(name, "extension" | "modifierExtension") || narrows_choice {
        return false;
    }
    let base_max = structure_definition
        .snapshot
        .as_ref()
        .and_then(|snapshot| {
            snapshot
                .element
                .iter()
                .find(|e| e.path == element.path && e.slice_name.is_none())
        })
        .and_then(|e| {
            e.base
                .as_ref()
                .map(|base| base.max.as_str())
                .or(e.max.as_deref())
        });
    !matches!(base_max, Some(max) if max == "*" || max.parse::<u32>().is_ok_and(|n| n > 1))
}

/// Sort elements by path hierarchy to ensure BackboneElement children are contiguous.
/// This fixes the bug where non-contiguous children (e.g., engine.type, matcher.*, engine.script)
/// cause the first child to be lost during stack-based processing.
fn sort_elements_by_path(
    elements: Vec<StructureDefinitionElement>,
) -> Vec<StructureDefinitionElement> {
    // Each element is keyed by the first appearance of every prefix of its id
    // (`Provenance`, `Provenance.agent:ProvenanceAuthor`, `….type`), so a subtree becomes
    // contiguous while siblings keep their order, and a slice's children stay with their
    // slice rather than with the element sliced (ids carry slice names; paths do not).
    let mut first: HashMap<String, usize> = HashMap::new();
    let keys: Vec<Vec<usize>> = elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let id = element.id.as_deref().unwrap_or(&element.path);
            let mut prefix = String::new();
            id.split('.')
                .map(|segment| {
                    if !prefix.is_empty() {
                        prefix.push('.');
                    }
                    prefix.push_str(segment);
                    *first.entry(prefix.clone()).or_insert(index)
                })
                .collect()
        })
        .collect();
    let mut order: Vec<usize> = (0..elements.len()).collect();
    order.sort_by(|a, b| keys[*a].cmp(&keys[*b]));
    let mut slots: Vec<Option<StructureDefinitionElement>> =
        elements.into_iter().map(Some).collect();
    order
        .into_iter()
        .filter_map(|index| slots[index].take())
        .collect()
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

    // Sort elements by path hierarchy to ensure BackboneElement children are contiguous
    let elements = sort_elements_by_path(elements);

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
        let lend = slice_constrains_element(&element, &structure_definition);
        stack = apply_actions_lending(stack, &actions, &transformed_element, lend)?;

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

    #[test]
    fn test_non_contiguous_backbone_children() {
        use crate::types::{StructureDefinitionDifferential, StructureDefinitionType};

        // This test reproduces the bug where non-contiguous BackboneElement children
        // (e.g., engine.type, then matcher.*, then engine.script) cause the first
        // child to be lost during stack-based processing.
        let structure_def = StructureDefinition {
            resource_type: "StructureDefinition".to_string(),
            url: "http://example.com/Test".to_string(),
            name: "Test".to_string(),
            status: "active".to_string(),
            kind: "logical".to_string(),
            type_name: "Test".to_string(),
            derivation: Some("specialization".to_string()),
            base_definition: Some(
                "http://hl7.org/fhir/StructureDefinition/DomainResource".to_string(),
            ),
            id: None,
            version: None,
            title: None,
            date: None,
            description: None,
            abstract_type: None,
            package_name: None,
            package_version: None,
            package_id: None,
            snapshot: None,
            differential: Some(StructureDefinitionDifferential {
                element: vec![
                    StructureDefinitionElement {
                        id: Some("Test".to_string()),
                        path: "Test".to_string(),
                        ..Default::default()
                    },
                    StructureDefinitionElement {
                        id: Some("Test.engine".to_string()),
                        path: "Test.engine".to_string(),
                        type_info: Some(vec![StructureDefinitionType {
                            code: "BackboneElement".to_string(),
                            profile: None,
                            target_profile: None,
                            extension: None,
                        }]),
                        ..Default::default()
                    },
                    StructureDefinitionElement {
                        id: Some("Test.engine.type".to_string()),
                        path: "Test.engine.type".to_string(),
                        type_info: Some(vec![StructureDefinitionType {
                            code: "code".to_string(),
                            profile: None,
                            target_profile: None,
                            extension: None,
                        }]),
                        ..Default::default()
                    },
                    // Non-contiguous: matcher elements come between engine children
                    StructureDefinitionElement {
                        id: Some("Test.matcher".to_string()),
                        path: "Test.matcher".to_string(),
                        type_info: Some(vec![StructureDefinitionType {
                            code: "BackboneElement".to_string(),
                            profile: None,
                            target_profile: None,
                            extension: None,
                        }]),
                        ..Default::default()
                    },
                    StructureDefinitionElement {
                        id: Some("Test.matcher.value".to_string()),
                        path: "Test.matcher.value".to_string(),
                        type_info: Some(vec![StructureDefinitionType {
                            code: "string".to_string(),
                            profile: None,
                            target_profile: None,
                            extension: None,
                        }]),
                        ..Default::default()
                    },
                    // engine.script comes after matcher - this was lost before the fix!
                    StructureDefinitionElement {
                        id: Some("Test.engine.script".to_string()),
                        path: "Test.engine.script".to_string(),
                        type_info: Some(vec![StructureDefinitionType {
                            code: "string".to_string(),
                            profile: None,
                            target_profile: None,
                            extension: None,
                        }]),
                        ..Default::default()
                    },
                ],
            }),
        };

        let result = translate(structure_def, None).unwrap();

        // Both engine children should be present
        let elements = result.elements.as_ref().expect("elements should exist");
        let engine = elements.get("engine").expect("engine should exist");
        let engine_elements = engine
            .elements
            .as_ref()
            .expect("engine.elements should exist");

        assert!(
            engine_elements.contains_key("type"),
            "engine.type should be present"
        );
        assert!(
            engine_elements.contains_key("script"),
            "engine.script should be present"
        );
    }
}
