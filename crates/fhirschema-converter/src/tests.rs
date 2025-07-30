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

    #[test]
    fn test_complex_profile_with_slicing() {
        let converter = StructureDefinitionConverter::new();

        let structure_def_json = json!({
            "resourceType": "StructureDefinition",
            "url": "http://example.org/StructureDefinition/patient-with-slicing",
            "name": "PatientWithSlicing",
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
                        "id": "Patient.identifier",
                        "path": "Patient.identifier",
                        "slicing": {
                            "discriminator": [
                                {
                                    "type": "value",
                                    "path": "system"
                                }
                            ],
                            "rules": "open",
                            "ordered": false,
                            "description": "Slice by identifier system"
                        },
                        "min": 1
                    },
                    {
                        "id": "Patient.identifier:ssn",
                        "path": "Patient.identifier",
                        "sliceName": "ssn",
                        "short": "Social Security Number",
                        "definition": "Patient's Social Security Number",
                        "min": 0,
                        "max": "1",
                        "fixedValue": {
                            "system": "http://hl7.org/fhir/sid/us-ssn"
                        }
                    },
                    {
                        "id": "Patient.identifier:mrn",
                        "path": "Patient.identifier",
                        "sliceName": "mrn",
                        "short": "Medical Record Number",
                        "definition": "Patient's Medical Record Number",
                        "min": 1,
                        "max": "1",
                        "fixedValue": {
                            "system": "http://example.org/mrn"
                        }
                    }
                ]
            }
        });

        let result = converter.convert(&structure_def_json.to_string());
        assert!(result.is_ok(), "Conversion should succeed: {:?}", result.err());

        let schema = result.unwrap();
        assert_eq!(schema.url, "http://example.org/StructureDefinition/patient-with-slicing");
        assert_eq!(schema.name, "PatientWithSlicing");

        // Check that elements were converted
        assert!(schema.elements.is_some());
        let elements = schema.elements.unwrap();

        // Check that the sliced element exists
        assert!(elements.contains_key("Patient.identifier"));
        let identifier_element = &elements["Patient.identifier"];

        // Check slicing information
        assert!(identifier_element.slicing.is_some());
        let slicing = identifier_element.slicing.as_ref().unwrap();
        assert!(slicing.discriminator.is_some());
        assert_eq!(slicing.rules, Some("open".to_string()));
        assert_eq!(slicing.ordered, Some(false));
    }

    #[test]
    fn test_profile_with_extensions() {
        let converter = StructureDefinitionConverter::new();

        let structure_def_json = json!({
            "resourceType": "StructureDefinition",
            "url": "http://example.org/StructureDefinition/patient-with-extensions",
            "name": "PatientWithExtensions",
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
                        "id": "Patient.extension",
                        "path": "Patient.extension",
                        "slicing": {
                            "discriminator": [
                                {
                                    "type": "value",
                                    "path": "url"
                                }
                            ],
                            "rules": "open"
                        }
                    },
                    {
                        "id": "Patient.extension:birthPlace",
                        "path": "Patient.extension",
                        "sliceName": "birthPlace",
                        "short": "Birth Place Extension",
                        "definition": "The place where the patient was born",
                        "min": 0,
                        "max": "1",
                        "type": [
                            {
                                "code": "Extension",
                                "profile": ["http://example.org/StructureDefinition/birth-place"]
                            }
                        ]
                    },
                    {
                        "id": "Patient.extension:ethnicity",
                        "path": "Patient.extension",
                        "sliceName": "ethnicity",
                        "short": "Ethnicity Extension",
                        "definition": "Patient's ethnicity information",
                        "min": 0,
                        "max": "*",
                        "type": [
                            {
                                "code": "Extension",
                                "profile": ["http://example.org/StructureDefinition/ethnicity"]
                            }
                        ]
                    }
                ]
            }
        });

        let result = converter.convert(&structure_def_json.to_string());
        assert!(result.is_ok(), "Conversion should succeed: {:?}", result.err());

        let schema = result.unwrap();
        assert_eq!(schema.url, "http://example.org/StructureDefinition/patient-with-extensions");
        assert_eq!(schema.name, "PatientWithExtensions");

        // Check that elements were converted
        assert!(schema.elements.is_some());
        let elements = schema.elements.unwrap();

        // Check that the extension element exists
        assert!(elements.contains_key("Patient.extension"));
        let extension_element = &elements["Patient.extension"];

        // Check slicing information for extensions
        assert!(extension_element.slicing.is_some());
        let slicing = extension_element.slicing.as_ref().unwrap();
        assert!(slicing.discriminator.is_some());
        let discriminators = slicing.discriminator.as_ref().unwrap();
        assert_eq!(discriminators.len(), 1);
        assert_eq!(discriminators[0].discriminator_type, "value");
        assert_eq!(discriminators[0].path, "url");
    }

    #[test]
    fn test_nested_slicing_scenario() {
        let converter = StructureDefinitionConverter::new();

        let structure_def_json = json!({
            "resourceType": "StructureDefinition",
            "url": "http://example.org/StructureDefinition/complex-observation",
            "name": "ComplexObservation",
            "status": "active",
            "kind": "resource",
            "type": "Observation",
            "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Observation",
            "derivation": "constraint",
            "differential": {
                "element": [
                    {
                        "id": "Observation",
                        "path": "Observation"
                    },
                    {
                        "id": "Observation.component",
                        "path": "Observation.component",
                        "slicing": {
                            "discriminator": [
                                {
                                    "type": "pattern",
                                    "path": "code"
                                }
                            ],
                            "rules": "closed",
                            "ordered": true,
                            "description": "Slice by component code"
                        },
                        "min": 2,
                        "max": "3"
                    },
                    {
                        "id": "Observation.component:systolic",
                        "path": "Observation.component",
                        "sliceName": "systolic",
                        "short": "Systolic Blood Pressure",
                        "min": 1,
                        "max": "1",
                        "constraint": [
                            {
                                "key": "sys-1",
                                "severity": "error",
                                "human": "Systolic value must be present",
                                "expression": "value.exists()"
                            }
                        ]
                    },
                    {
                        "id": "Observation.component:diastolic",
                        "path": "Observation.component",
                        "sliceName": "diastolic",
                        "short": "Diastolic Blood Pressure",
                        "min": 1,
                        "max": "1",
                        "constraint": [
                            {
                                "key": "dia-1",
                                "severity": "warning",
                                "human": "Diastolic should be lower than systolic",
                                "expression": "value < %resource.component.where(code.coding.code = 'systolic').value"
                            }
                        ]
                    }
                ]
            }
        });

        let result = converter.convert(&structure_def_json.to_string());
        assert!(result.is_ok(), "Conversion should succeed: {:?}", result.err());

        let schema = result.unwrap();
        assert_eq!(schema.url, "http://example.org/StructureDefinition/complex-observation");
        assert_eq!(schema.name, "ComplexObservation");

        // Check that elements were converted
        assert!(schema.elements.is_some());
        let elements = schema.elements.unwrap();

        // Check that the component element exists with slicing
        assert!(elements.contains_key("Observation.component"));
        let component_element = &elements["Observation.component"];

        // Check slicing information
        assert!(component_element.slicing.is_some());
        let slicing = component_element.slicing.as_ref().unwrap();
        assert!(slicing.discriminator.is_some());
        assert_eq!(slicing.rules, Some("closed".to_string()));
        assert_eq!(slicing.ordered, Some(true));

        // Check constraints on sliced elements
        if let Some(constraints) = &component_element.constraints {
            assert!(!constraints.is_empty());
        }
    }
}
