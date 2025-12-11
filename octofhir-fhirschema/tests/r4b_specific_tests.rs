//! R4B-Specific Resource Tests
//!
//! This module tests R4B-specific resources and features:
//! - SubscriptionTopic (new in R4B)
//! - SubscriptionStatus (new in R4B)
//! - Pharmaceutical product restructuring
//! - Cross-version differences

use octofhir_fhirschema::{EmbeddedSchemaProvider, ModelFhirVersion};

// ============================================================================
// SubscriptionTopic Tests (New in R4B)
// ============================================================================

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_topic_exists_in_r4b() {
    let provider = EmbeddedSchemaProvider::r4b();

    let schema = provider
        .schemas()
        .get("SubscriptionTopic")
        .expect("SubscriptionTopic should exist in R4B");

    assert_eq!(schema.name, "SubscriptionTopic");
    assert_eq!(schema.type_name, "SubscriptionTopic");
    assert_eq!(schema.kind, "resource");
}

#[tokio::test]
async fn test_subscription_topic_not_in_r4() {
    let provider = EmbeddedSchemaProvider::r4();

    let schema = provider.schemas().get("SubscriptionTopic");
    assert!(schema.is_none(), "SubscriptionTopic should NOT exist in R4");
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_topic_has_url_element() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionTopic").unwrap();

    let elements = schema
        .elements
        .as_ref()
        .expect("SubscriptionTopic should have elements");

    assert!(
        elements.contains_key("url"),
        "SubscriptionTopic should have 'url' element"
    );

    let url_element = &elements["url"];
    assert_eq!(url_element.type_name.as_deref(), Some("uri"));
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_topic_has_status_element() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionTopic").unwrap();

    let elements = schema.elements.as_ref().unwrap();

    assert!(
        elements.contains_key("status"),
        "SubscriptionTopic should have 'status' element"
    );

    let status_element = &elements["status"];
    assert_eq!(status_element.type_name.as_deref(), Some("code"));
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_topic_has_resource_trigger() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionTopic").unwrap();

    let elements = schema.elements.as_ref().unwrap();

    assert!(
        elements.contains_key("resourceTrigger"),
        "SubscriptionTopic should have 'resourceTrigger' element"
    );

    let resource_trigger = &elements["resourceTrigger"];
    assert_eq!(
        resource_trigger.array,
        Some(true),
        "resourceTrigger should be an array"
    );
    assert_eq!(
        resource_trigger.type_name.as_deref(),
        Some("BackboneElement")
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_topic_resource_trigger_has_resource() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionTopic").unwrap();

    let elements = schema.elements.as_ref().unwrap();
    let resource_trigger = &elements["resourceTrigger"];

    let nested_elements = resource_trigger
        .elements
        .as_ref()
        .expect("resourceTrigger should have nested elements");

    assert!(
        nested_elements.contains_key("resource"),
        "resourceTrigger should have 'resource' element"
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_topic_resource_trigger_has_supported_interaction() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionTopic").unwrap();

    let elements = schema.elements.as_ref().unwrap();
    let resource_trigger = &elements["resourceTrigger"];

    let nested_elements = resource_trigger.elements.as_ref().unwrap();

    assert!(
        nested_elements.contains_key("supportedInteraction"),
        "resourceTrigger should have 'supportedInteraction' element"
    );

    let supported_interaction = &nested_elements["supportedInteraction"];
    assert_eq!(
        supported_interaction.array,
        Some(true),
        "supportedInteraction should be an array"
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_topic_has_can_filter_by() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionTopic").unwrap();

    let elements = schema.elements.as_ref().unwrap();

    assert!(
        elements.contains_key("canFilterBy"),
        "SubscriptionTopic should have 'canFilterBy' element"
    );

    let can_filter_by = &elements["canFilterBy"];
    assert_eq!(
        can_filter_by.array,
        Some(true),
        "canFilterBy should be an array"
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_topic_can_filter_by_has_resource() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionTopic").unwrap();

    let elements = schema.elements.as_ref().unwrap();
    let can_filter_by = &elements["canFilterBy"];

    let nested_elements = can_filter_by
        .elements
        .as_ref()
        .expect("canFilterBy should have nested elements");

    assert!(
        nested_elements.contains_key("resource"),
        "canFilterBy should have 'resource' element"
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_topic_can_filter_by_has_filter_parameter() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionTopic").unwrap();

    let elements = schema.elements.as_ref().unwrap();
    let can_filter_by = &elements["canFilterBy"];

    let nested_elements = can_filter_by.elements.as_ref().unwrap();

    assert!(
        nested_elements.contains_key("filterParameter"),
        "canFilterBy should have 'filterParameter' element"
    );
}

// ============================================================================
// SubscriptionStatus Tests (New in R4B)
// ============================================================================

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_status_exists_in_r4b() {
    let provider = EmbeddedSchemaProvider::r4b();

    let schema = provider
        .schemas()
        .get("SubscriptionStatus")
        .expect("SubscriptionStatus should exist in R4B");

    assert_eq!(schema.name, "SubscriptionStatus");
    assert_eq!(schema.type_name, "SubscriptionStatus");
    assert_eq!(schema.kind, "resource");
}

#[tokio::test]
async fn test_subscription_status_not_in_r4() {
    let provider = EmbeddedSchemaProvider::r4();

    let schema = provider.schemas().get("SubscriptionStatus");
    assert!(
        schema.is_none(),
        "SubscriptionStatus should NOT exist in R4"
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_status_has_status_element() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionStatus").unwrap();

    let elements = schema.elements.as_ref().unwrap();

    assert!(
        elements.contains_key("status"),
        "SubscriptionStatus should have 'status' element"
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_status_has_type_element() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionStatus").unwrap();

    let elements = schema.elements.as_ref().unwrap();

    assert!(
        elements.contains_key("type"),
        "SubscriptionStatus should have 'type' element"
    );

    let type_element = &elements["type"];
    assert_eq!(type_element.type_name.as_deref(), Some("code"));
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_status_has_events_since_subscription_start() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionStatus").unwrap();

    let elements = schema.elements.as_ref().unwrap();

    assert!(
        elements.contains_key("eventsSinceSubscriptionStart"),
        "SubscriptionStatus should have 'eventsSinceSubscriptionStart' element"
    );

    let events_element = &elements["eventsSinceSubscriptionStart"];
    assert_eq!(events_element.type_name.as_deref(), Some("string"));
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_status_has_notification_event() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionStatus").unwrap();

    let elements = schema.elements.as_ref().unwrap();

    assert!(
        elements.contains_key("notificationEvent"),
        "SubscriptionStatus should have 'notificationEvent' element"
    );

    let notification_event = &elements["notificationEvent"];
    assert_eq!(
        notification_event.array,
        Some(true),
        "notificationEvent should be an array"
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_status_notification_event_has_event_number() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionStatus").unwrap();

    let elements = schema.elements.as_ref().unwrap();
    let notification_event = &elements["notificationEvent"];

    let nested_elements = notification_event
        .elements
        .as_ref()
        .expect("notificationEvent should have nested elements");

    assert!(
        nested_elements.contains_key("eventNumber"),
        "notificationEvent should have 'eventNumber' element"
    );

    let event_number = &nested_elements["eventNumber"];
    assert_eq!(event_number.type_name.as_deref(), Some("string"));
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_status_notification_event_has_timestamp() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionStatus").unwrap();

    let elements = schema.elements.as_ref().unwrap();
    let notification_event = &elements["notificationEvent"];

    let nested_elements = notification_event.elements.as_ref().unwrap();

    assert!(
        nested_elements.contains_key("timestamp"),
        "notificationEvent should have 'timestamp' element"
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_status_notification_event_has_focus() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionStatus").unwrap();

    let elements = schema.elements.as_ref().unwrap();
    let notification_event = &elements["notificationEvent"];

    let nested_elements = notification_event.elements.as_ref().unwrap();

    assert!(
        nested_elements.contains_key("focus"),
        "notificationEvent should have 'focus' element"
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_subscription_status_has_subscription() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider.schemas().get("SubscriptionStatus").unwrap();

    let elements = schema.elements.as_ref().unwrap();

    assert!(
        elements.contains_key("subscription"),
        "SubscriptionStatus should have 'subscription' element"
    );

    let subscription = &elements["subscription"];
    assert_eq!(subscription.type_name.as_deref(), Some("Reference"));
}

// ============================================================================
// Pharmaceutical Products Tests (R4 vs R4B Restructuring)
// ============================================================================

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_medicinal_product_removed_in_r4b() {
    let r4_provider = EmbeddedSchemaProvider::r4();
    let r4b_provider = EmbeddedSchemaProvider::r4b();

    // Old R4 resources that were removed/consolidated in R4B
    let deprecated_resources = vec![
        "MedicinalProduct",
        "MedicinalProductAuthorization",
        "MedicinalProductContraindication",
        "MedicinalProductIndication",
        "MedicinalProductIngredient",
        "MedicinalProductInteraction",
        "MedicinalProductManufactured",
        "MedicinalProductPackaged",
        "MedicinalProductPharmaceutical",
        "MedicinalProductUndesirableEffect",
    ];

    for resource in &deprecated_resources {
        // Should exist in R4
        assert!(
            r4_provider.schemas().get(*resource).is_some(),
            "{} should exist in R4",
            resource
        );

        // Should NOT exist in R4B
        assert!(
            r4b_provider.schemas().get(*resource).is_none(),
            "{} should NOT exist in R4B (deprecated)",
            resource
        );
    }
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_new_pharmaceutical_resources_in_r4b() {
    let r4_provider = EmbeddedSchemaProvider::r4();
    let r4b_provider = EmbeddedSchemaProvider::r4b();

    // New R4B consolidated resources
    let new_resources = vec![
        "MedicinalProductDefinition",
        "AdministrableProductDefinition",
        "ManufacturedItemDefinition",
        "Ingredient",
        "ClinicalUseDefinition",
        "RegulatedAuthorization",
        "PackagedProductDefinition",
    ];

    for resource in &new_resources {
        // Should exist in R4B
        assert!(
            r4b_provider.schemas().get(*resource).is_some(),
            "{} should exist in R4B",
            resource
        );

        // May or may not exist in R4 (some are new, some replaced)
        // Just log for informational purposes
        if r4_provider.schemas().get(*resource).is_some() {
            println!("{} also exists in R4", resource);
        } else {
            println!("{} is new in R4B", resource);
        }
    }
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_medicinal_product_definition_structure() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider
        .schemas()
        .get("MedicinalProductDefinition")
        .expect("MedicinalProductDefinition should exist in R4B");

    let elements = schema.elements.as_ref().unwrap();

    // Verify key consolidation elements
    let expected_elements = vec![
        "identifier",
        "type",
        "domain",
        "version",
        "status",
        "statusDate",
        "description",
        "combinedPharmaceuticalDoseForm",
        "indication",
        "legalStatusOfSupply",
        "name",
    ];

    for element_name in expected_elements {
        assert!(
            elements.contains_key(element_name),
            "MedicinalProductDefinition should have '{}' element",
            element_name
        );
    }
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_ingredient_exists_in_r4b() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider
        .schemas()
        .get("Ingredient")
        .expect("Ingredient should exist in R4B");

    assert_eq!(schema.name, "Ingredient");
    assert_eq!(schema.type_name, "Ingredient");
    assert_eq!(schema.kind, "resource");
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_clinical_use_definition_exists_in_r4b() {
    let provider = EmbeddedSchemaProvider::r4b();
    let schema = provider
        .schemas()
        .get("ClinicalUseDefinition")
        .expect("ClinicalUseDefinition should exist in R4B");

    assert_eq!(schema.name, "ClinicalUseDefinition");
    assert_eq!(schema.type_name, "ClinicalUseDefinition");
    assert_eq!(schema.kind, "resource");
}

// ============================================================================
// Cross-Version Comparison Tests
// ============================================================================

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_r4b_has_fewer_schemas_than_r4() {
    let r4_provider = EmbeddedSchemaProvider::r4();
    let r4b_provider = EmbeddedSchemaProvider::r4b();

    let r4_count = r4_provider.schema_count();
    let r4b_count = r4b_provider.schema_count();

    println!("R4 schema count: {}", r4_count);
    println!("R4B schema count: {}", r4b_count);

    // R4 has 573 schemas, R4B has 568 schemas (consolidation of pharmaceutical products)
    assert_eq!(r4_count, 573, "R4 should have 573 schemas");
    assert_eq!(r4b_count, 568, "R4B should have 568 schemas");
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_patient_exists_in_both_r4_and_r4b() {
    let r4_provider = EmbeddedSchemaProvider::r4();
    let r4b_provider = EmbeddedSchemaProvider::r4b();

    let r4_patient = r4_provider.schemas().get("Patient");
    let r4b_patient = r4b_provider.schemas().get("Patient");

    assert!(r4_patient.is_some(), "Patient should exist in R4");
    assert!(r4b_patient.is_some(), "Patient should exist in R4B");

    // Both should have name element as an array
    let r4_elements = r4_patient.unwrap().elements.as_ref().unwrap();
    let r4b_elements = r4b_patient.unwrap().elements.as_ref().unwrap();

    let r4_name = &r4_elements["name"];
    let r4b_name = &r4b_elements["name"];

    assert_eq!(
        r4_name.array,
        Some(true),
        "R4 Patient.name should be an array"
    );
    assert_eq!(
        r4b_name.array,
        Some(true),
        "R4B Patient.name should be an array"
    );
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_observation_exists_in_both_r4_and_r4b() {
    let r4_provider = EmbeddedSchemaProvider::r4();
    let r4b_provider = EmbeddedSchemaProvider::r4b();

    let r4_observation = r4_provider.schemas().get("Observation");
    let r4b_observation = r4b_provider.schemas().get("Observation");

    assert!(r4_observation.is_some(), "Observation should exist in R4");
    assert!(r4b_observation.is_some(), "Observation should exist in R4B");
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_r4b_version_correctly_identified() {
    let provider = EmbeddedSchemaProvider::r4b();

    assert_eq!(*provider.version(), ModelFhirVersion::R4B);
}

#[tokio::test]
#[ignore = "R4B schemas not fully generated yet"]
async fn test_r4_and_r4b_have_common_core_resources() {
    let r4_provider = EmbeddedSchemaProvider::r4();
    let r4b_provider = EmbeddedSchemaProvider::r4b();

    // Core resources that should exist in both versions
    let core_resources = vec![
        "Patient",
        "Observation",
        "Practitioner",
        "Organization",
        "Medication",
        "Condition",
        "Procedure",
        "Encounter",
        "DiagnosticReport",
        "AllergyIntolerance",
    ];

    for resource in core_resources {
        assert!(
            r4_provider.schemas().get(resource).is_some(),
            "{} should exist in R4",
            resource
        );
        assert!(
            r4b_provider.schemas().get(resource).is_some(),
            "{} should exist in R4B",
            resource
        );
    }
}
