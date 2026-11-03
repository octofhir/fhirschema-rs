use octofhir_fhirschema::{EmbeddedSchemaProvider, ModelFhirVersion, ModelProvider};

fn main() {
    for version in [
        ModelFhirVersion::R4,
        ModelFhirVersion::R4B,
        ModelFhirVersion::R5,
        ModelFhirVersion::R6,
    ] {
        let provider = EmbeddedSchemaProvider::new(version);
        assert!(provider.schema_count() > 0);
        let patient = futures::executor::block_on(provider.get_type("Patient"))
            .expect("local type lookup")
            .expect("embedded Patient schema");
        assert!(
            futures::executor::block_on(provider.get_element_type(&patient, "active"))
                .expect("local element lookup")
                .is_some()
        );
        assert!(provider.is_type_derived_from("Patient", "Resource"));
    }
}
