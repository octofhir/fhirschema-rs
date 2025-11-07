use crate::error::Result;
use crate::types::{
    FhirSchemaBinding, FhirSchemaConstraint, FhirSchemaElement, FhirSchemaPattern,
    StructureDefinition, StructureDefinitionElement, StructureDefinitionExtension,
};
use std::collections::HashMap;

const BINDING_NAME_EXT: &str =
    "http://hl7.org/fhir/StructureDefinition/elementdefinition-bindingName";
const DEFAULT_TYPE_EXT: &str =
    "http://hl7.org/fhir/StructureDefinition/elementdefinition-defaulttype";
const FHIR_TYPE_EXT: &str = "http://hl7.org/fhir/StructureDefinition/structuredefinition-fhir-type";

fn get_extension<'a>(
    extensions: &'a Option<Vec<StructureDefinitionExtension>>,
    url: &str,
) -> Option<&'a StructureDefinitionExtension> {
    extensions.as_ref()?.iter().find(|ext| ext.url == url)
}

fn pattern_type_normalize(type_name: &str) -> String {
    let type_map = HashMap::from([
        ("Instant", "instant"),
        ("Time", "time"),
        ("Date", "date"),
        ("DateTime", "dateTime"),
        ("Decimal", "decimal"),
        ("Boolean", "boolean"),
        ("Integer", "integer"),
        ("String", "string"),
        ("Uri", "uri"),
        ("Base64Binary", "base64Binary"),
        ("Code", "code"),
        ("Id", "id"),
        ("Oid", "oid"),
        ("UnsignedInt", "unsignedInt"),
        ("PositiveInt", "positiveInt"),
        ("Markdown", "markdown"),
        ("Url", "url"),
        ("Canonical", "canonical"),
        ("Uuid", "uuid"),
    ]);

    type_map.get(type_name).unwrap_or(&type_name).to_string()
}

fn process_patterns(
    element: &mut FhirSchemaElement,
    pattern_fields: &HashMap<String, serde_json::Value>,
) {
    for (key, value) in pattern_fields {
        if key.starts_with("pattern") {
            let type_name = pattern_type_normalize(&key.replace("pattern", ""));
            element.pattern = Some(FhirSchemaPattern {
                type_name: type_name.clone(),
                value: value.clone(),
                string: None,
            });

            // If pattern has type but element doesn't, use pattern type
            if element.type_name.is_none() {
                element.type_name = Some(type_name);
            }
        } else if key.starts_with("fixed") {
            let type_name = pattern_type_normalize(&key.replace("fixed", ""));
            element.pattern = Some(FhirSchemaPattern {
                type_name: type_name.clone(),
                value: value.clone(),
                string: None,
            });

            // If pattern has type but element doesn't, use pattern type
            if element.type_name.is_none() {
                element.type_name = Some(type_name);
            }
        }
    }
}

fn build_reference_targets(types: &[crate::types::StructureDefinitionType]) -> Option<Vec<String>> {
    let mut refers = Vec::new();

    for type_def in types {
        if let Some(target_profile) = &type_def.target_profile {
            refers.extend(target_profile.clone());
        }
    }

    if refers.is_empty() {
        None
    } else {
        refers.sort();
        refers.dedup();
        Some(refers)
    }
}

fn preprocess_element(element: &StructureDefinitionElement) -> StructureDefinitionElement {
    let mut processed = element.clone();

    if let Some(type_info) = &element.type_info
        && !type_info.is_empty()
    {
        let first_type = &type_info[0];
        if first_type.code == "Reference" {
            let refers = build_reference_targets(type_info);
            processed.refers = refers;

            // Simplify type to just Reference
            processed.type_info = Some(vec![crate::types::StructureDefinitionType {
                code: "Reference".to_string(),
                profile: None,
                target_profile: None,
                extension: None,
            }]);
        }
    }

    processed
}

fn build_element_binding(
    element: &FhirSchemaElement,
    definition_element: &StructureDefinitionElement,
    structure_definition: &StructureDefinition,
) -> Result<FhirSchemaElement> {
    let mut result = element.clone();

    let normalize_binding =
        |binding: &crate::types::StructureDefinitionBinding| -> FhirSchemaBinding {
            let mut result = FhirSchemaBinding {
                strength: binding.strength.clone(),
                value_set: binding.value_set.clone(),
                binding_name: None,
            };

            if let Some(binding_name_ext) = get_extension(&binding.extension, BINDING_NAME_EXT) {
                result.binding_name = binding_name_ext.value_string.clone();
            }

            result
        };

    // Skip binding for choice parent elements
    if element.choices.is_some() {
        result.binding = None;
        return Ok(result);
    }

    // For choice elements, get binding from parent declaration
    if let Some(choice_of) = &element.choice_of {
        if let Some(snapshot) = &structure_definition.snapshot {
            let decl_path = format!(
                "{}.{}[x]",
                structure_definition.id.as_deref().unwrap_or(""),
                choice_of
            );
            if let Some(decl) = snapshot.element.iter().find(|e| e.path == decl_path)
                && let Some(binding) = &decl.binding
            {
                result.binding = Some(normalize_binding(binding));
            }
        }
        return Ok(result);
    }

    // Normal binding
    if let Some(binding) = &definition_element.binding
        && binding.value_set.is_some()
    {
        result.binding = Some(normalize_binding(binding));
    }

    Ok(result)
}

fn build_element_constraints(
    element: &FhirSchemaElement,
    definition_element: &StructureDefinitionElement,
) -> FhirSchemaElement {
    let mut result = element.clone();

    if let Some(constraints) = &definition_element.constraint
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
        result.constraint = Some(constraint_map);
    }

    result
}

fn build_element_type(
    element: &FhirSchemaElement,
    definition_element: &StructureDefinitionElement,
    structure_definition: &StructureDefinition,
) -> FhirSchemaElement {
    let mut result = element.clone();

    if let Some(type_info) = &definition_element.type_info
        && !type_info.is_empty()
    {
        // Check for type in extension
        if let Some(ext) = &type_info[0].extension {
            for extension in ext {
                if extension.url == FHIR_TYPE_EXT
                    && let Some(value_url) = &extension.value_url
                {
                    result.type_name = Some(value_url.clone());
                    return result;
                }
            }
        }

        // Normal type
        let type_code = type_info[0].code.clone();
        result.type_name = Some(type_code);

        // Add defaultType for logical models
        if structure_definition.kind == "logical"
            && let Some(default_type_ext) =
                get_extension(&definition_element.extension, DEFAULT_TYPE_EXT)
            && let Some(value_url) = &default_type_ext.value_url
        {
            result.default_type = Some(value_url.clone());
        }
    }

    result
}

fn build_element_extension(
    element: &FhirSchemaElement,
    definition_element: &StructureDefinitionElement,
) -> FhirSchemaElement {
    let mut result = element.clone();

    if let Some(type_info) = &definition_element.type_info
        && !type_info.is_empty()
    {
        let first_type = &type_info[0];
        if first_type.code == "Extension"
            && let Some(profile) = &first_type.profile
            && !profile.is_empty()
        {
            result.url = Some(profile[0].clone());

            // Set cardinality for extensions
            if let Some(min) = definition_element.min {
                result.min = Some(min);
            }
            if let Some(max) = &definition_element.max
                && max != "*"
                && let Ok(max_val) = max.parse::<i32>()
            {
                result.max = Some(max_val);
            }
        }
    }

    result
}

fn build_element_cardinality(
    element: &FhirSchemaElement,
    definition_element: &StructureDefinitionElement,
) -> FhirSchemaElement {
    let mut result = element.clone();

    // Extension elements have cardinality handled elsewhere
    if result.url.is_some() {
        return result;
    }

    let is_array = definition_element
        .max
        .as_ref()
        .map(|m| m == "*")
        .unwrap_or(false)
        || definition_element.min.map(|m| m >= 2).unwrap_or(false)
        || definition_element
            .max
            .as_ref()
            .and_then(|m| m.parse::<i32>().ok())
            .map(|m| m >= 2)
            .unwrap_or(false);

    let is_required = definition_element.min.map(|m| m == 1).unwrap_or(false);

    // Clear min/max from result initially
    result.min = None;
    result.max = None;

    if is_array {
        result.array = Some(true);
        if let Some(min) = definition_element.min
            && min > 0
        {
            result.min = Some(min);
        }
        if let Some(max) = &definition_element.max
            && max != "*"
            && let Ok(max_val) = max.parse::<i32>()
        {
            result.max = Some(max_val);
        }
    }

    if is_required {
        result.required_flag = Some(true);
    }

    result
}

fn content_reference_to_element_reference(
    reference: &str,
    structure_definition: &StructureDefinition,
) -> Vec<String> {
    // Remove the # prefix and split
    let path_parts: Vec<&str> = reference.trim_start_matches('#').split('.').collect();
    let mut result = vec![structure_definition.url.clone()];

    for part in path_parts.iter().skip(1) {
        result.push("elements".to_string());
        result.push(part.to_string());
    }

    result
}

fn build_element_content_reference(
    element: &FhirSchemaElement,
    definition_element: &StructureDefinitionElement,
    structure_definition: &StructureDefinition,
) -> FhirSchemaElement {
    let mut result = element.clone();

    if let Some(content_reference) = &definition_element.content_reference {
        result.element_reference = Some(content_reference_to_element_reference(
            content_reference,
            structure_definition,
        ));
    }

    result
}

fn clear_element(element: &FhirSchemaElement) -> FhirSchemaElement {
    element.clone()
}

pub fn is_array_element(element: &StructureDefinitionElement) -> bool {
    element.max.as_ref().map(|m| m == "*").unwrap_or(false)
        || element.min.map(|m| m >= 2).unwrap_or(false)
        || element
            .max
            .as_ref()
            .and_then(|m| m.parse::<i32>().ok())
            .map(|m| m >= 2)
            .unwrap_or(false)
}

pub fn is_required_element(element: &StructureDefinitionElement) -> bool {
    element.min.map(|m| m == 1).unwrap_or(false)
}

pub fn transform_element(
    element: &StructureDefinitionElement,
    structure_definition: &StructureDefinition,
) -> Result<FhirSchemaElement> {
    let preprocessed = preprocess_element(element);
    let mut transformed = FhirSchemaElement {
        type_name: None,
        default_type: None,
        array: None,
        min: None,
        max: None,
        refers: preprocessed.refers.clone(),
        element_reference: None,
        short: element.short.clone(),
        binding: None,
        pattern: None,
        constraint: None,
        elements: None,
        choice_of: element.choice_of.clone(),
        choices: element.choices.clone(),
        url: None,
        must_support: element.must_support,
        is_modifier: element.is_modifier,
        is_modifier_reason: element.is_modifier_reason.clone(),
        is_summary: element.is_summary,
        slicing: None,
        extensions: None,
        required: None,
        excluded: None,
        required_flag: None,
        index: None,
        order_meaning: None,
    };

    transformed = clear_element(&transformed);
    transformed = build_element_binding(&transformed, &preprocessed, structure_definition)?;
    transformed = build_element_constraints(&transformed, &preprocessed);
    transformed =
        build_element_content_reference(&transformed, &preprocessed, structure_definition);
    transformed = build_element_extension(&transformed, &preprocessed);
    transformed = build_element_cardinality(&transformed, &preprocessed);
    transformed = build_element_type(&transformed, &preprocessed, structure_definition);
    process_patterns(&mut transformed, &element.pattern_fields);

    Ok(transformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_type_normalize() {
        assert_eq!(pattern_type_normalize("String"), "string");
        assert_eq!(pattern_type_normalize("DateTime"), "dateTime");
        assert_eq!(pattern_type_normalize("Boolean"), "boolean");
        assert_eq!(pattern_type_normalize("Unknown"), "Unknown");
    }

    #[test]
    fn test_is_array_element() {
        let element = StructureDefinitionElement {
            path: "Patient.name".to_string(),
            max: Some("*".to_string()),
            ..Default::default()
        };
        assert!(is_array_element(&element));

        let element2 = StructureDefinitionElement {
            path: "Patient.name".to_string(),
            max: Some("1".to_string()),
            ..Default::default()
        };
        assert!(!is_array_element(&element2));
    }

    #[test]
    fn test_is_required_element() {
        let element = StructureDefinitionElement {
            path: "Patient.name".to_string(),
            min: Some(1),
            ..Default::default()
        };
        assert!(is_required_element(&element));

        let element2 = StructureDefinitionElement {
            path: "Patient.name".to_string(),
            min: Some(0),
            ..Default::default()
        };
        assert!(!is_required_element(&element2));
    }
}
