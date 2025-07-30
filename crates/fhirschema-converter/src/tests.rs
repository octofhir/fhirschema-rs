//! Tests for the FHIRSchema converter.

use crate::StructureDefinitionConverter;
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_structure_definition_conversion() {
        let converter = StructureDefinitionConverter::new();

        // Create a minimal StructureDefinition JSON
        let structure_def_json = json!({
            "resourceType": "StructureDefinition",
            "url": "http://example.org/StructureDefinition/test-patient",
            "name": "TestPatient",
            "status": "active",
            "kind": "resource",
            "type": "Patient",
            "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
            "derivation": "constraint",
            "differential": {
                "element": [
                    {
                        "id": "Patient",
                        "path": "Patient"
                    },
                    {
                        "id": "Patient.name",
                        "path": "Patient.name",
                        "short": "Patient name",
                        "definition": "The name of the patient",
                        "min": 1,
                        "max": "*",
                        "type": [
                            {
                                "code": "HumanName"
                            }
                        ]
                    }
                ]
            }
        });

        let result = converter.convert(&structure_def_json.to_string());
        assert!(result.is_ok(), "Conversion should succeed: {:?}", result.err());

        let schema = result.unwrap();
        assert_eq!(schema.url, "http://example.org/StructureDefinition/test-patient");
        assert_eq!(schema.name, "TestPatient");
        assert_eq!(schema.schema_type, "Patient");
        assert_eq!(schema.derivation, "constraint");
        assert_eq!(schema.base, Some("http://hl7.org/fhir/StructureDefinition/Patient".to_string()));

        // Check that elements were converted
        assert!(schema.elements.is_some());
        let elements = schema.elements.unwrap();
        assert!(elements.contains_key("Patient.name"));

        let name_element = &elements["Patient.name"];
        assert_eq!(name_element.short, Some("Patient name".to_string()));
        assert_eq!(name_element.definition, Some("The name of the patient".to_string()));
        assert_eq!(name_element.min, Some(1));
        assert_eq!(name_element.max, Some("*".to_string()));
        assert_eq!(name_element.element_type, Some("HumanName".to_string()));
    }

    #[test]
    fn test_invalid_json() {
        let converter = StructureDefinitionConverter::new();
        let result = converter.convert("invalid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_required_fields() {
        let converter = StructureDefinitionConverter::new();

        // Missing required fields
        let incomplete_json = json!({
            "resourceType": "StructureDefinition"
        });

        let result = converter.convert(&incomplete_json.to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_choice_type_conversion() {
        let converter = StructureDefinitionConverter::new();

        let structure_def_json = json!({
            "resourceType": "StructureDefinition",
            "url": "http://example.org/StructureDefinition/test-choice",
            "name": "TestChoice",
            "status": "active",
            "kind": "resource",
            "type": "Patient",
            "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
            "derivation": "constraint",
            "differential": {
                "element": [
                    {
                        "id": "Patient",
                        "path": "Patient"
                    },
                    {
                        "id": "Patient.deceased[x]",
                        "path": "Patient.deceased[x]",
                        "short": "Indicates if the individual is deceased or not",
                        "type": [
                            {
                                "code": "boolean"
                            },
                            {
                                "code": "dateTime"
                            }
                        ]
                    }
                ]
            }
        });

        let result = converter.convert(&structure_def_json.to_string());
        assert!(result.is_ok());

        let schema = result.unwrap();
        let elements = schema.elements.unwrap();
        let deceased_element = &elements["Patient.deceased[x]"];

        // Should be a choice type
        assert_eq!(deceased_element.choice_of, Some("type".to_string()));
        assert!(deceased_element.choices.is_some());

        let choices = deceased_element.choices.as_ref().unwrap();
        assert_eq!(choices.len(), 2);
    }
}
