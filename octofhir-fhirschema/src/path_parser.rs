use crate::types::{PathComponent, StructureDefinitionElement};
use serde_json::json;

pub fn parse_path(element: &StructureDefinitionElement) -> Vec<PathComponent> {
    let path_parts: Vec<&str> = element.path.split('.').collect();
    // Skip the first part (resource type)
    let relevant_parts = if path_parts.len() > 1 {
        &path_parts[1..]
    } else {
        return vec![];
    };

    let mut path: Vec<PathComponent> = relevant_parts
        .iter()
        .map(|part| PathComponent {
            el: part.to_string(),
            slicing: None,
            slice_name: None,
            slice: None,
        })
        .collect();

    if path.is_empty() {
        return path;
    }

    // Add slicing/sliceName info to the last component
    let last_index = path.len() - 1;
    let mut path_item = path[last_index].clone();

    if let Some(slicing) = &element.slicing {
        let mut slicing_obj = json!({});

        if let Some(discriminator) = &slicing.discriminator {
            slicing_obj["discriminator"] = json!(discriminator);
        }
        if let Some(rules) = &slicing.rules {
            slicing_obj["rules"] = json!(rules);
        }
        if let Some(ordered) = slicing.ordered {
            slicing_obj["ordered"] = json!(ordered);
        }

        if let Some(min) = element.min {
            slicing_obj["min"] = json!(min);
        }
        if let Some(max) = &element.max {
            if max != "*" {
                if let Ok(max_val) = max.parse::<i32>() {
                    slicing_obj["max"] = json!(max_val);
                }
            }
        }

        path_item.slicing = Some(slicing_obj);
    }

    if let Some(slice_name) = &element.slice_name {
        let mut slice_obj = json!({});

        if let Some(min) = element.min {
            slice_obj["min"] = json!(min);
        }
        if let Some(max) = &element.max {
            if max != "*" {
                if let Ok(max_val) = max.parse::<i32>() {
                    slice_obj["max"] = json!(max_val);
                }
            }
        }

        path_item.slice = Some(slice_obj);
        path_item.slice_name = Some(slice_name.clone());
    }

    path[last_index] = path_item;

    path
}

pub fn get_common_path(path1: &[PathComponent], path2: &[PathComponent]) -> Vec<PathComponent> {
    let mut common = Vec::new();
    let min_length = path1.len().min(path2.len());

    for i in 0..min_length {
        if path1[i].el == path2[i].el {
            // Only keep the element name in common path, not slice info
            common.push(PathComponent {
                el: path1[i].el.clone(),
                slicing: None,
                slice_name: None,
                slice: None,
            });
        } else {
            break;
        }
    }

    common
}

pub fn enrich_path(prev_path: &[PathComponent], new_path: &[PathComponent]) -> Vec<PathComponent> {
    let mut enriched = Vec::new();

    for (i, new_component) in new_path.iter().enumerate() {
        if i < prev_path.len() && prev_path[i].el == new_component.el {
            // Merge slicing info from previous path, but prefer new slice name
            let mut merged = new_component.clone();

            // Only preserve slicing if not present in new path
            if merged.slicing.is_none() && prev_path[i].slicing.is_some() {
                merged.slicing = prev_path[i].slicing.clone();
            }

            enriched.push(merged);
        } else {
            enriched.push(new_component.clone());
        }
    }

    enriched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{StructureDefinitionDiscriminator, StructureDefinitionSlicing};

    #[test]
    fn test_parse_path_simple() {
        let element = StructureDefinitionElement {
            path: "Patient.name".to_string(),
            ..Default::default()
        };

        let result = parse_path(&element);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].el, "name");
    }

    #[test]
    fn test_parse_path_with_slicing() {
        let element = StructureDefinitionElement {
            path: "Patient.identifier".to_string(),
            slicing: Some(StructureDefinitionSlicing {
                discriminator: Some(vec![StructureDefinitionDiscriminator {
                    type_name: "pattern".to_string(),
                    path: "system".to_string(),
                }]),
                rules: Some("open".to_string()),
                ordered: Some(false),
            }),
            min: Some(1),
            max: Some("*".to_string()),
            ..Default::default()
        };

        let result = parse_path(&element);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].el, "identifier");
        assert!(result[0].slicing.is_some());
    }

    #[test]
    fn test_get_common_path() {
        let path1 = vec![
            PathComponent {
                el: "contact".to_string(),
                slicing: None,
                slice_name: None,
                slice: None,
            },
            PathComponent {
                el: "name".to_string(),
                slicing: None,
                slice_name: None,
                slice: None,
            },
        ];

        let path2 = vec![
            PathComponent {
                el: "contact".to_string(),
                slicing: None,
                slice_name: None,
                slice: None,
            },
            PathComponent {
                el: "telecom".to_string(),
                slicing: None,
                slice_name: None,
                slice: None,
            },
        ];

        let result = get_common_path(&path1, &path2);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].el, "contact");
    }
}
