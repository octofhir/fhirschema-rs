use crate::types::{StructureDefinitionElement, StructureDefinitionType};
use std::collections::HashSet;

pub fn is_choice_element(element: &StructureDefinitionElement) -> bool {
    // Check if path ends with [x]
    if element.path.ends_with("[x]") {
        return true;
    }

    // Check if multiple types with different codes
    if let Some(type_info) = &element.type_info
        && type_info.len() > 1
    {
        let unique_codes: HashSet<&String> = type_info.iter().map(|t| &t.code).collect();
        return unique_codes.len() > 1;
    }

    false
}

fn capitalize(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }

    let mut chars: Vec<char> = s.chars().collect();
    chars[0] = chars[0].to_uppercase().next().unwrap();
    chars.into_iter().collect()
}

/// `elementdefinition-type-must-support` on a type of a choice: which of its types mustSupport
/// asks for.
const TYPE_MUST_SUPPORT: &str =
    "http://hl7.org/fhir/StructureDefinition/elementdefinition-type-must-support";

fn type_must_support(type_def: &StructureDefinitionType) -> bool {
    type_def.extension.as_ref().is_some_and(|extensions| {
        extensions.iter().any(|extension| {
            extension.url == TYPE_MUST_SUPPORT && extension.value_boolean == Some(true)
        })
    })
}

fn canonical_to_name(url: &str) -> String {
    let parts: Vec<&str> = url.split('/').collect();
    parts.last().map_or("", |v| v).to_string()
}

pub fn expand_choice_element(
    element: &StructureDefinitionElement,
) -> crate::error::Result<Vec<StructureDefinitionElement>> {
    let base_path = element.path.replace("[x]", "");
    let field_name = base_path.split('.').next_back().unwrap_or("").to_string();

    let type_info = match &element.type_info {
        Some(types) if !types.is_empty() => types,
        // A profile constraining the choice element itself (cardinality, mustSupport,
        // invariants) without narrowing its types: the element stays, with the base's types.
        _ => {
            let mut parent_element = element.clone();
            parent_element.path = base_path;
            return Ok(vec![parent_element]);
        }
    };

    let mut expanded = Vec::new();

    // Create the parent choice element
    let choices: Vec<String> = type_info
        .iter()
        .map(|t| format!("{}{}", field_name, capitalize(&canonical_to_name(&t.code))))
        .collect();

    let mut parent_element = element.clone();
    parent_element.path = base_path.clone();
    parent_element.choices = Some(choices.clone());
    parent_element.type_info = None; // Remove type from parent
    parent_element.binding = None; // Remove binding from parent
    expanded.push(parent_element);

    // With types marked, mustSupport covers those alone; without, it covers the whole choice.
    let types_marked = type_info.iter().any(type_must_support);

    // Create typed elements
    for type_def in type_info {
        let type_name = capitalize(&canonical_to_name(&type_def.code));

        let mut typed_element = element.clone();
        typed_element.path = format!("{base_path}{type_name}");
        typed_element.type_info = Some(vec![type_def.clone()]);
        typed_element.choice_of = Some(field_name.clone());
        typed_element.choices = None;
        if types_marked {
            typed_element.must_support = Some(type_must_support(type_def));
        }

        // Remove binding if it exists, it will be handled specially
        if element.binding.is_some() {
            typed_element.binding = None;
        }

        expanded.push(typed_element);
    }

    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::StructureDefinitionExtension;

    #[test]
    fn test_is_choice_element_with_x_suffix() {
        let element = StructureDefinitionElement {
            path: "Patient.value[x]".to_string(),
            ..Default::default()
        };
        assert!(is_choice_element(&element));
    }

    #[test]
    fn test_is_choice_element_with_multiple_types() {
        let element = StructureDefinitionElement {
            path: "Patient.value".to_string(),
            type_info: Some(vec![
                StructureDefinitionType {
                    code: "string".to_string(),
                    profile: None,
                    target_profile: None,
                    extension: None,
                },
                StructureDefinitionType {
                    code: "integer".to_string(),
                    profile: None,
                    target_profile: None,
                    extension: None,
                },
            ]),
            ..Default::default()
        };
        assert!(is_choice_element(&element));
    }

    #[test]
    fn test_is_choice_element_single_type() {
        let element = StructureDefinitionElement {
            path: "Patient.name".to_string(),
            type_info: Some(vec![StructureDefinitionType {
                code: "string".to_string(),
                profile: None,
                target_profile: None,
                extension: None,
            }]),
            ..Default::default()
        };
        assert!(!is_choice_element(&element));
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("string"), "String");
        assert_eq!(capitalize("dateTime"), "DateTime");
        assert_eq!(capitalize(""), "");
    }

    #[test]
    fn test_canonical_to_name() {
        assert_eq!(
            canonical_to_name("http://hl7.org/fhir/StructureDefinition/string"),
            "string"
        );
        assert_eq!(canonical_to_name("dateTime"), "dateTime");
    }

    fn typed(code: &str, must_support: Option<bool>) -> StructureDefinitionType {
        StructureDefinitionType {
            code: code.to_string(),
            profile: None,
            target_profile: None,
            extension: must_support.map(|value| {
                vec![StructureDefinitionExtension {
                    url: TYPE_MUST_SUPPORT.to_string(),
                    value_string: None,
                    value_canonical: None,
                    value_url: None,
                    value_boolean: Some(value),
                }]
            }),
        }
    }

    #[test]
    fn must_support_covers_the_marked_types_of_a_choice() {
        // US Core 6.1.0 smoking status: value[x] is mustSupport, CodeableConcept alone is marked.
        let element = StructureDefinitionElement {
            path: "Observation.value[x]".to_string(),
            must_support: Some(true),
            type_info: Some(vec![
                typed("CodeableConcept", Some(true)),
                typed("Quantity", None),
                typed("string", Some(false)),
            ]),
            ..Default::default()
        };
        let expanded = expand_choice_element(&element).unwrap();
        assert_eq!(expanded[0].must_support, Some(true));
        assert_eq!(expanded[1].must_support, Some(true));
        assert_eq!(expanded[2].must_support, Some(false));
        assert_eq!(expanded[3].must_support, Some(false));
    }

    #[test]
    fn must_support_without_marks_covers_every_type() {
        let element = StructureDefinitionElement {
            path: "Observation.value[x]".to_string(),
            must_support: Some(true),
            type_info: Some(vec![typed("Quantity", None), typed("string", None)]),
            ..Default::default()
        };
        let expanded = expand_choice_element(&element).unwrap();
        assert!(expanded.iter().all(|e| e.must_support == Some(true)));
    }

    #[test]
    fn test_expand_choice_element() {
        let element = StructureDefinitionElement {
            path: "Patient.value[x]".to_string(),
            type_info: Some(vec![
                StructureDefinitionType {
                    code: "string".to_string(),
                    profile: None,
                    target_profile: None,
                    extension: None,
                },
                StructureDefinitionType {
                    code: "integer".to_string(),
                    profile: None,
                    target_profile: None,
                    extension: None,
                },
            ]),
            ..Default::default()
        };

        let result = expand_choice_element(&element).unwrap();

        // Should have 3 elements: parent + 2 typed elements
        assert_eq!(result.len(), 3);

        // First element should be the parent choice element
        assert_eq!(result[0].path, "Patient.value");
        assert_eq!(
            result[0].choices,
            Some(vec!["valueString".to_string(), "valueInteger".to_string()])
        );
        assert!(result[0].type_info.is_none());

        // Second element should be string typed
        assert_eq!(result[1].path, "Patient.valueString");
        assert_eq!(result[1].choice_of, Some("value".to_string()));
        assert!(result[1].type_info.is_some());
        assert_eq!(result[1].type_info.as_ref().unwrap()[0].code, "string");

        // Third element should be integer typed
        assert_eq!(result[2].path, "Patient.valueInteger");
        assert_eq!(result[2].choice_of, Some("value".to_string()));
        assert!(result[2].type_info.is_some());
        assert_eq!(result[2].type_info.as_ref().unwrap()[0].code, "integer");
    }
}
