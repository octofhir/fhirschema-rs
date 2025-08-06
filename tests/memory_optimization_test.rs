use octofhir_fhirschema::converter::{
    ConversionContext, ConverterConfig, FhirSchemaConverter, StructureDefinition,
    StructureDefinitionConverter,
};
use std::time::Instant;

#[test]
fn test_memory_optimization_context_reuse() {
    let converter = FhirSchemaConverter::new();
    let config = ConverterConfig::default();
    let mut context = ConversionContext::new(&config);

    // Create multiple structure definitions to simulate batch processing
    let structure_defs: Vec<StructureDefinition> = (0..10)
        .map(|i| {
            let mut def = StructureDefinition::new(format!("TestResource{i}"), "resource");
            def.url = Some(
                url::Url::parse(&format!(
                    "http://example.com/StructureDefinition/TestResource{i}"
                ))
                .unwrap(),
            );
            def
        })
        .collect();

    let start = Instant::now();

    // Process all definitions using the same context (memory optimized approach)
    for def in &structure_defs {
        let _schema = converter
            .convert_with_context(def, &mut context)
            .expect("Conversion should succeed");
    }

    let duration = start.elapsed();
    println!("[DEBUG_LOG] Memory optimized conversion took: {duration:?}");

    // Verify context state is properly managed
    assert!(!context.has_errors(), "Context should not have errors");

    // The key improvement: we reused the same context instead of creating new ones
    // This eliminates the StructureDefinition cloning that was happening in begin_conversion
    println!(
        "[DEBUG_LOG] Successfully processed {} definitions with single context",
        structure_defs.len()
    );
}

#[test]
fn test_context_boolean_flag_optimization() {
    let converter = FhirSchemaConverter::new();
    let config = ConverterConfig::default();
    let mut context = ConversionContext::new(&config);

    // Verify the context uses boolean flag instead of cloning StructureDefinition
    assert!(
        !context.is_conversion_active,
        "Context should start inactive"
    );

    let def = StructureDefinition::new("TestResource", "resource");

    // Begin conversion should set the flag without cloning the entire StructureDefinition
    context
        .begin_conversion(&def)
        .expect("Begin conversion should succeed");
    assert!(
        context.is_conversion_active,
        "Context should be active after begin_conversion"
    );

    // Validate state should work with boolean flag
    context
        .validate_state()
        .expect("State validation should pass");

    // Create a dummy schema for end_conversion
    let schema = octofhir_fhirschema::FhirSchema::new("TestResource");

    // End conversion should reset the flag
    context
        .end_conversion(&schema)
        .expect("End conversion should succeed");
    assert!(
        !context.is_conversion_active,
        "Context should be inactive after end_conversion"
    );

    println!("[DEBUG_LOG] Context boolean flag optimization working correctly");
}
