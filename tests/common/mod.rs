use octofhir_fhirschema::*;
use url::Url;

pub fn create_test_schema() -> FhirSchema {
    FhirSchema::new("Patient")
        .with_url(Url::parse("http://example.com/Patient").unwrap())
        .with_name("TestPatient")
        .with_element("Patient.id", create_test_element("Patient.id"))
        .with_element("Patient.name", create_test_element("Patient.name"))
}

pub fn create_test_element(path: &str) -> Element {
    Element::new(path)
        .with_type(ElementType::new("string"))
        .with_cardinality(0, "1")
}

pub fn create_test_constraint() -> Constraint {
    Constraint::new(
        "test-constraint",
        "error",
        "Test constraint message",
        "true",
    )
}

pub fn create_test_slicing() -> Slicing {
    Slicing::new("open")
        .with_discriminator(Discriminator::new("value", "code"))
        .with_description("Test slicing")
}

pub fn create_test_binding() -> Binding {
    Binding::new("required").with_value_set(Url::parse("http://example.com/ValueSet/test").unwrap())
}
