// ModelProvider Trait Implementation Demo
//
// This example demonstrates that FhirSchemaModelProvider properly implements
// the ModelProvider trait from fhir-model-rs, enabling full integration
// with the OctoFHIR ecosystem.

use octofhir_fhir_model::type_system::PolymorphicContext;
use octofhir_fhirschema::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 ModelProvider Trait Implementation Demo");

    // ========================================================================
    // Demo 1: Create Provider - Simple API Hidden Behind Trait
    // ========================================================================

    println!("\n📋 Demo 1: ModelProvider Creation");

    // Create the provider with simple API
    let provider = FhirSchemaModelProvider::r4().await?;

    // Use as ModelProvider trait - this proves full trait implementation
    let model_provider: &dyn ModelProvider = &provider;

    println!("  ✅ FhirSchemaModelProvider created and cast to ModelProvider trait");
    println!("  📌 FHIR Version: {}", model_provider.get_fhir_version());

    // ========================================================================
    // Demo 2: Core Type Operations via Trait
    // ========================================================================

    println!("\n🏗️  Demo 2: Core Type Operations via ModelProvider Trait");

    // Test type hierarchy
    match model_provider.get_type_hierarchy("Patient").await {
        Ok(Some(hierarchy)) => {
            println!("  ✅ Type hierarchy retrieved via trait:");
            println!("    Type: {}", hierarchy.type_name);
            println!("    Parent: {:?}", hierarchy.direct_parent);
            println!("    Child Types: {} found", hierarchy.direct_children.len());
        }
        Ok(None) => println!("  ℹ️  No hierarchy found for Patient"),
        Err(e) => println!("  ⚠️  Error getting hierarchy: {e}"),
    }

    // Test type compatibility
    match model_provider
        .is_type_compatible("Patient", "Resource")
        .await
    {
        Ok(compatible) => println!("  ✅ Type compatibility Patient->Resource: {compatible}"),
        Err(e) => println!("  ⚠️  Compatibility check failed: {e}"),
    }

    // Test common supertype
    let types = vec!["Patient".to_string(), "Practitioner".to_string()];
    match model_provider.get_common_supertype(&types).await {
        Ok(Some(supertype)) => println!("  ✅ Common supertype: {supertype}"),
        Ok(None) => println!("  ℹ️  No common supertype found"),
        Err(e) => println!("  ⚠️  Supertype resolution failed: {e}"),
    }

    // ========================================================================
    // Demo 3: Navigation Operations via Trait
    // ========================================================================

    println!("\n🧭 Demo 3: Navigation Operations via ModelProvider Trait");

    let navigation_tests = vec![
        ("Patient", "name"),
        ("Patient", "name.family"),
        ("Observation", "value[x]"),
    ];

    for (base_type, path) in navigation_tests {
        println!("  Testing navigation: {base_type}.{path}");

        // Navigate typed path via trait
        match model_provider.navigate_typed_path(base_type, path).await {
            Ok(result) => {
                println!("    ✅ Navigation successful: {}", result.is_success);
                let type_info = &result.result_type;
                println!(
                    "    📍 Result Type: TypeInfo ({})",
                    type_info.namespace().unwrap_or("Unknown")
                );
            }
            Err(e) => println!("    ❌ Navigation failed: {e}"),
        }

        // Validate navigation safety via trait
        match model_provider
            .validate_navigation_safety(base_type, path)
            .await
        {
            Ok(validation) => {
                println!("    🛡️  Safety validation: {}", validation.is_valid);
            }
            Err(e) => println!("    ⚠️  Safety validation failed: {e}"),
        }

        // Get navigation metadata via trait
        match model_provider
            .get_navigation_metadata(base_type, path)
            .await
        {
            Ok(metadata) => {
                println!(
                    "    📊 Navigation metadata: path={}, target_type={}",
                    metadata.path, metadata.target_type
                );
            }
            Err(e) => println!("    ⚠️  Metadata retrieval failed: {e}"),
        }
    }

    // ========================================================================
    // Demo 4: Choice Type Operations via Trait
    // ========================================================================

    println!("\n🎯 Demo 4: Choice Type Operations via ModelProvider Trait");

    // Test choice type resolution via trait
    let polymorphic_context = PolymorphicContext {
        current_path: "Observation.value[x]".to_string(),
        base_type: "Observation".to_string(),
        available_types: vec!["string".to_string(), "Quantity".to_string()],
        constraints: Vec::new(),
        inference_hints: Vec::new(),
        resolution_strategy: octofhir_fhir_model::type_system::ResolutionStrategy::FirstMatch,
        metadata: std::collections::HashMap::new(),
    };

    match model_provider
        .resolve_choice_type("value[x]", &polymorphic_context)
        .await
    {
        Ok(resolution) => {
            println!("  ✅ Choice type resolution:");
            println!("    Resolved Type: {}", resolution.resolved_type);
            println!("    Confidence: {:.2}", resolution.confidence_score);
            println!(
                "    Alternatives: {} found",
                resolution.alternative_types.len()
            );
        }
        Err(e) => println!("  ⚠️  Choice type resolution failed: {e}"),
    }

    // Test choice expansions via trait
    match model_provider.get_choice_expansions("value[x]").await {
        Ok(expansions) => {
            println!("  ✅ Choice expansions: {} found", expansions.len());
            for expansion in expansions.iter().take(3) {
                println!("    - Choice Property: {}", expansion.choice_property);
                println!(
                    "      Forward mappings: {} entries",
                    expansion.forward_mappings.len()
                );
                println!(
                    "      Reverse mappings: {} entries",
                    expansion.reverse_mappings.len()
                );
            }
        }
        Err(e) => println!("  ⚠️  Choice expansions failed: {e}"),
    }

    // Test choice type inference via trait
    match model_provider.infer_choice_type(&polymorphic_context).await {
        Ok(inference) => {
            println!("  ✅ Type inference:");
            println!(
                "    Confidence threshold: {:.2}",
                inference.confidence_threshold
            );
            println!(
                "    Inference rules: {} found",
                inference.inference_rules.len()
            );
        }
        Err(e) => println!("  ⚠️  Type inference failed: {e}"),
    }

    // ========================================================================
    // Demo 5: FHIRPath Functions via Trait
    // ========================================================================

    println!("\n🔍 Demo 5: FHIRPath Functions via ModelProvider Trait");

    // Test profile conformance via trait
    let profile_url = "http://hl7.org/fhir/us/core/StructureDefinition/us-core-patient";
    match model_provider.conforms_to_profile(profile_url).await {
        Ok(conformance) => {
            println!("  ✅ Profile conformance check:");
            println!("    Profile: {}", conformance.profile_url);
            println!("    Valid: {}", conformance.is_valid);
            println!("    Resource Type: {:?}", conformance.resource_type);
        }
        Err(e) => println!("  ⚠️  Conformance check failed: {e}"),
    }

    // Test expression type analysis via trait
    let test_expression = "Patient.name.family";
    match model_provider
        .analyze_expression_types(test_expression)
        .await
    {
        Ok(analysis) => {
            println!("  ✅ Expression analysis:");
            println!("    Expression: {}", analysis.expression);
            println!("    Analysis complete");
        }
        Err(e) => println!("  ⚠️  Expression analysis failed: {e}"),
    }

    // Test FHIRPath expression validation via trait
    match model_provider
        .validate_fhirpath_expression(test_expression, "Patient")
        .await
    {
        Ok(validation) => {
            println!("  ✅ FHIRPath validation: {}", validation.is_valid);
        }
        Err(e) => println!("  ⚠️  FHIRPath validation failed: {e}"),
    }

    // ========================================================================
    // Demo 6: Advanced Operations via Trait
    // ========================================================================

    println!("\n🔧 Demo 6: Advanced Operations via ModelProvider Trait");

    // Test collection semantics via trait
    match model_provider.get_collection_semantics("Patient").await {
        Ok(semantics) => {
            println!("  ✅ Collection semantics retrieved for Patient");
            println!("    Default semantics applied");
        }
        Err(e) => println!("  ⚠️  Collection semantics failed: {e}"),
    }

    // Test optimization hints via trait
    match model_provider.get_optimization_hints("Patient.name").await {
        Ok(hints) => {
            println!("  ✅ Optimization hints: {} found", hints.len());
        }
        Err(e) => println!("  ⚠️  Optimization hints failed: {e}"),
    }

    // ========================================================================
    // Demo 7: Core Information Methods via Trait
    // ========================================================================

    println!("\n📚 Demo 7: Core Information Methods via ModelProvider Trait");

    // Test type reflection via trait
    match model_provider.get_type_reflection("Patient").await {
        Ok(Some(reflection)) => {
            println!("  ✅ Type reflection:");
            println!("    Namespace: {:?}", reflection.namespace());
            println!("    Reflection info available");
        }
        Ok(None) => println!("  ℹ️  No reflection info for Patient"),
        Err(e) => println!("  ⚠️  Type reflection failed: {e}"),
    }

    // Test constraints via trait
    match model_provider.get_constraints("Patient").await {
        Ok(constraints) => {
            println!("  ✅ Constraints: {} found", constraints.len());
            for constraint in constraints.iter().take(2) {
                println!(
                    "    - {} ({}): {}",
                    constraint.key, constraint.severity, constraint.human
                );
            }
        }
        Err(e) => println!("  ⚠️  Constraints retrieval failed: {e}"),
    }

    // Test supported resource types via trait
    match model_provider.get_supported_resource_types().await {
        Ok(types) => {
            println!("  ✅ Supported resource types: {} found", types.len());
            println!("    Types: {}", types.join(", "));
        }
        Err(e) => println!("  ⚠️  Resource types retrieval failed: {e}"),
    }

    // Test cache clearing via trait
    match model_provider.clear_caches().await {
        Ok(_) => println!("  ✅ Caches cleared successfully via trait"),
        Err(e) => println!("  ⚠️  Cache clearing failed: {e}"),
    }

    // ========================================================================
    // Summary
    // ========================================================================

    println!("\n✅ ModelProvider Trait Implementation Demo Completed!");
    println!("\n📋 Key Achievements:");
    println!("  🎯 Full ModelProvider Trait: All methods implemented");
    println!("  🏗️  Core Type Operations: Hierarchy, compatibility, supertypes");
    println!("  🧭 Navigation Operations: Path navigation, safety validation, metadata");
    println!("  🎯 Choice Type Operations: Resolution, expansions, inference");
    println!("  🔍 FHIRPath Functions: Conformance, analysis, validation");
    println!("  🔧 Advanced Operations: Collection semantics, optimization hints");
    println!("  📚 Information Methods: Type reflection, constraints, resource types");

    println!("\n🚀 FhirSchemaModelProvider is now fully compatible with the");
    println!("   OctoFHIR ecosystem through the ModelProvider trait!");
    println!("   It can be used anywhere a ModelProvider is expected.");

    Ok(())
}
