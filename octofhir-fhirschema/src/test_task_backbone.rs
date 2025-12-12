#[test]
fn test_task_input_backbone() {
    use crate::{translate, StructureDefinition};
    
    let task_sd_json = std::fs::read_to_string(
        "/Users/alexanderstreltsov/work/octofhir/server-rs/.fhir/packages/hl7.fhir.r4.core#4.0.1/StructureDefinition-Task.json"
    ).expect("Failed to read Task StructureDefinition");
    
    let sd: StructureDefinition = serde_json::from_str(&task_sd_json).expect("Failed to parse Task SD");
    let schema = translate(sd, None).expect("Failed to convert Task SD");
    
    if let Some(elements) = &schema.elements {
        if let Some(input) = elements.get("input") {
            println!("Task.input type_name: {:?}", input.type_name);
            println!("Task.input has nested elements: {}", input.elements.is_some());
            if let Some(nested) = &input.elements {
                println!("Task.input nested element count: {}", nested.len());
                for (name, _) in nested {
                    println!("  - {}", name);
                }
            }
        } else {
            println!("Task.input not found!");
            println!("Available elements:");
            for name in elements.keys().take(20) {
                println!("  - {}", name);
            }
        }
    }
}
