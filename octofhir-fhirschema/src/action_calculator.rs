use crate::path_parser::get_common_path;
use crate::types::{Action, PathComponent};

fn slice_changed(prev_item: &PathComponent, new_item: &PathComponent) -> bool {
    match (&prev_item.slice_name, &new_item.slice_name) {
        (Some(prev_slice), Some(new_slice)) => prev_slice != new_slice,
        _ => false,
    }
}

fn exit_slice_action(item: &PathComponent) -> Action {
    let slice_name = item.slice_name.clone().unwrap_or_default();
    Action::ExitSlice {
        slice_name,
        slicing: item.slicing.clone(),
        slice: item.slice.clone(),
    }
}

fn calculate_exits(
    prev_length: usize,
    common_length: usize,
    prev_path: &[PathComponent],
    new_path: &[PathComponent],
) -> Vec<Action> {
    let mut exits = Vec::new();

    // Exit from the deepest to the common path
    for i in (common_length..prev_length).rev() {
        let prev_item = &prev_path[i];

        // Add exit-slice if needed
        if prev_item.slice_name.is_some() {
            exits.push(exit_slice_action(prev_item));
        }

        // Add exit
        exits.push(Action::Exit {
            el: prev_item.el.clone(),
        });
    }

    // Check for slice change at common boundary
    if common_length > 0 && common_length <= prev_path.len() && common_length <= new_path.len() {
        let prev_item = &prev_path[common_length - 1];
        let new_item = &new_path[common_length - 1];

        if slice_changed(prev_item, new_item) {
            exits.push(exit_slice_action(prev_item));
        }
    }

    exits
}

fn calculate_enters(
    common_length: usize,
    new_length: usize,
    prev_path: &[PathComponent],
    new_path: &[PathComponent],
) -> Vec<Action> {
    let mut enters = Vec::new();

    // Check for slice change at common boundary
    if common_length > 0 && common_length <= new_length {
        let prev_item = if common_length <= prev_path.len() {
            Some(&prev_path[common_length - 1])
        } else {
            None
        };
        let new_item = &new_path[common_length - 1];

        if let Some(new_slice_name) = &new_item.slice_name {
            let should_enter = match prev_item {
                Some(prev) => match &prev.slice_name {
                    Some(prev_slice) => prev_slice != new_slice_name,
                    None => true,
                },
                None => true,
            };

            if should_enter {
                enters.push(Action::EnterSlice {
                    slice_name: new_slice_name.clone(),
                });
            }
        }

        // If we're at the same level, return early
        if common_length == new_length {
            return enters;
        }
    }

    // Add enters for new path components
    for new_item in new_path.iter().take(new_length).skip(common_length) {
        enters.push(Action::Enter {
            el: new_item.el.clone(),
        });

        if let Some(slice_name) = &new_item.slice_name {
            enters.push(Action::EnterSlice {
                slice_name: slice_name.clone(),
            });
        }
    }

    enters
}

pub fn calculate_actions(prev_path: &[PathComponent], new_path: &[PathComponent]) -> Vec<Action> {
    let prev_length = prev_path.len();
    let new_length = new_path.len();
    let common_path = get_common_path(prev_path, new_path);
    let common_length = common_path.len();

    let exits = calculate_exits(prev_length, common_length, prev_path, new_path);
    let enters = calculate_enters(common_length, new_length, prev_path, new_path);

    [exits, enters].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_actions_simple() {
        let prev_path = vec![PathComponent {
            el: "contact".to_string(),
            slicing: None,
            slice_name: None,
            slice: None,
        }];

        let new_path = vec![
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

        let actions = calculate_actions(&prev_path, &new_path);

        // Should have one Enter action for "name"
        assert_eq!(actions.len(), 1);
        if let Action::Enter { el } = &actions[0] {
            assert_eq!(el, "name");
        } else {
            panic!("Expected Enter action");
        }
    }

    #[test]
    fn test_calculate_actions_with_exit() {
        let prev_path = vec![
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

        let new_path = vec![
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

        let actions = calculate_actions(&prev_path, &new_path);

        // Should have Exit "name" and Enter "telecom"
        assert_eq!(actions.len(), 2);
        if let Action::Exit { el } = &actions[0] {
            assert_eq!(el, "name");
        } else {
            panic!("Expected Exit action");
        }
        if let Action::Enter { el } = &actions[1] {
            assert_eq!(el, "telecom");
        } else {
            panic!("Expected Enter action");
        }
    }

    #[test]
    fn test_calculate_actions_slice_change() {
        let prev_path = vec![PathComponent {
            el: "identifier".to_string(),
            slicing: None,
            slice_name: Some("system".to_string()),
            slice: None,
        }];

        let new_path = vec![PathComponent {
            el: "identifier".to_string(),
            slicing: None,
            slice_name: Some("type".to_string()),
            slice: None,
        }];

        let actions = calculate_actions(&prev_path, &new_path);

        // Should have ExitSlice and EnterSlice actions
        assert!(actions.len() >= 2);

        // Check that we have the expected slice actions
        let has_exit_slice = actions
            .iter()
            .any(|a| matches!(a, Action::ExitSlice { slice_name, .. } if slice_name == "system"));
        let has_enter_slice = actions
            .iter()
            .any(|a| matches!(a, Action::EnterSlice { slice_name } if slice_name == "type"));

        assert!(has_exit_slice, "Should have ExitSlice for 'system'");
        assert!(has_enter_slice, "Should have EnterSlice for 'type'");
    }
}
