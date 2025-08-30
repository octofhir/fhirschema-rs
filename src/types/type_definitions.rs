/// Common FHIR type definitions for quick reference
pub struct FhirTypeDefinitions;

impl FhirTypeDefinitions {
    /// Get all FHIR R4 resource types
    pub fn resource_types() -> &'static [&'static str] {
        &[
            "Account", "ActivityDefinition", "AdverseEvent", "AllergyIntolerance",
            "Appointment", "AppointmentResponse", "AuditEvent", "Basic",
            "Binary", "BiologicallyDerivedProduct", "BodyStructure", "Bundle",
            "CapabilityStatement", "CarePlan", "CareTeam", "CatalogEntry",
            "ChargeItem", "ChargeItemDefinition", "Claim", "ClaimResponse",
            "ClinicalImpression", "CodeSystem", "Communication", "CommunicationRequest",
            "CompartmentDefinition", "Composition", "ConceptMap", "Condition",
            "Consent", "Contract", "Coverage", "CoverageEligibilityRequest",
            "CoverageEligibilityResponse", "DetectedIssue", "Device", "DeviceDefinition",
            "DeviceMetric", "DeviceRequest", "DeviceUseStatement", "DiagnosticReport",
            "DocumentManifest", "DocumentReference", "DomainResource", "EffectEvidenceSynthesis",
            "Encounter", "Endpoint", "EnrollmentRequest", "EnrollmentResponse",
            "EpisodeOfCare", "EventDefinition", "Evidence", "EvidenceVariable",
            "ExampleScenario", "ExplanationOfBenefit", "FamilyMemberHistory", "Flag",
            "Goal", "GraphDefinition", "Group", "GuidanceResponse",
            "HealthcareService", "ImagingStudy", "Immunization", "ImmunizationEvaluation",
            "ImmunizationRecommendation", "ImplementationGuide", "InsurancePlan", "Invoice",
            "Library", "Linkage", "List", "Location",
            "Measure", "MeasureReport", "Media", "Medication",
            "MedicationAdministration", "MedicationDispense", "MedicationKnowledge", "MedicationRequest",
            "MedicationStatement", "MedicinalProduct", "MedicinalProductAuthorization", "MedicinalProductContraindication",
            "MedicinalProductIndication", "MedicinalProductIngredient", "MedicinalProductInteraction", "MedicinalProductManufactured",
            "MedicinalProductPackaged", "MedicinalProductPharmaceutical", "MedicinalProductUndesirableEffect", "MessageDefinition",
            "MessageHeader", "MolecularSequence", "NamingSystem", "NutritionOrder",
            "Observation", "ObservationDefinition", "OperationDefinition", "OperationOutcome",
            "Organization", "OrganizationAffiliation", "Parameters", "Patient",
            "PaymentNotice", "PaymentReconciliation", "Person", "PlanDefinition",
            "Practitioner", "PractitionerRole", "Procedure", "Provenance",
            "Questionnaire", "QuestionnaireResponse", "RelatedPerson", "RequestGroup",
            "ResearchDefinition", "ResearchElementDefinition", "ResearchStudy", "ResearchSubject",
            "Resource", "RiskAssessment", "RiskEvidenceSynthesis", "Schedule",
            "SearchParameter", "ServiceRequest", "Slot", "Specimen",
            "SpecimenDefinition", "StructureDefinition", "StructureMap", "Subscription",
            "Substance", "SubstanceNucleicAcid", "SubstancePolymer", "SubstanceProtein",
            "SubstanceReferenceInformation", "SubstanceSourceMaterial", "SubstanceSpecification", "SupplyDelivery",
            "SupplyRequest", "Task", "TerminologyCapabilities", "TestReport",
            "TestScript", "ValueSet", "VerificationResult", "VisionPrescription"
        ]
    }
    
    /// Get all FHIR primitive types
    pub fn primitive_types() -> &'static [&'static str] {
        &[
            "base64Binary", "boolean", "canonical", "code", "date", "dateTime",
            "decimal", "id", "instant", "integer", "markdown", "oid",
            "positiveInt", "string", "time", "unsignedInt", "uri", "url", "uuid"
        ]
    }
    
    /// Get all FHIR complex types (data types)
    pub fn complex_types() -> &'static [&'static str] {
        &[
            "Address", "Age", "Annotation", "Attachment", "BackboneElement",
            "CodeableConcept", "Coding", "ContactDetail", "ContactPoint", "Contributor",
            "Count", "DataRequirement", "Distance", "Dosage", "Duration",
            "Element", "ElementDefinition", "Expression", "Extension", "HumanName",
            "Identifier", "MarketingStatus", "Meta", "Money", "Narrative",
            "ParameterDefinition", "Period", "Population", "ProdCharacteristic", "ProductShelfLife",
            "Quantity", "Range", "Ratio", "Reference", "RelatedArtifact",
            "SampledData", "Signature", "SimpleQuantity", "SubstanceAmount", "Timing",
            "TriggerDefinition", "UsageContext"
        ]
    }
    
    /// Get all FHIR special types
    pub fn special_types() -> &'static [&'static str] {
        &[
            "xhtml"
        ]
    }
    
    /// Check if a type is a FHIR resource type
    pub fn is_resource_type(type_name: &str) -> bool {
        Self::resource_types().contains(&type_name)
    }
    
    /// Check if a type is a FHIR primitive type
    pub fn is_primitive_type(type_name: &str) -> bool {
        Self::primitive_types().contains(&type_name)
    }
    
    /// Check if a type is a FHIR complex type
    pub fn is_complex_type(type_name: &str) -> bool {
        Self::complex_types().contains(&type_name)
    }
    
    /// Check if a type is a FHIR special type
    pub fn is_special_type(type_name: &str) -> bool {
        Self::special_types().contains(&type_name)
    }
    
    /// Get all FHIR types (resources + primitives + complex + special)
    pub fn all_types() -> Vec<&'static str> {
        let mut types = Vec::new();
        types.extend_from_slice(Self::resource_types());
        types.extend_from_slice(Self::primitive_types());
        types.extend_from_slice(Self::complex_types());
        types.extend_from_slice(Self::special_types());
        types
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_resource_type_classification() {
        assert!(FhirTypeDefinitions::is_resource_type("Patient"));
        assert!(FhirTypeDefinitions::is_resource_type("Observation"));
        assert!(!FhirTypeDefinitions::is_resource_type("string"));
        assert!(!FhirTypeDefinitions::is_resource_type("HumanName"));
    }
    
    #[test]
    fn test_primitive_type_classification() {
        assert!(FhirTypeDefinitions::is_primitive_type("string"));
        assert!(FhirTypeDefinitions::is_primitive_type("integer"));
        assert!(!FhirTypeDefinitions::is_primitive_type("Patient"));
        assert!(!FhirTypeDefinitions::is_primitive_type("HumanName"));
    }
    
    #[test]
    fn test_complex_type_classification() {
        assert!(FhirTypeDefinitions::is_complex_type("HumanName"));
        assert!(FhirTypeDefinitions::is_complex_type("Address"));
        assert!(!FhirTypeDefinitions::is_complex_type("Patient"));
        assert!(!FhirTypeDefinitions::is_complex_type("string"));
    }
    
    #[test]
    fn test_no_type_overlaps() {
        let resource_types: std::collections::HashSet<_> = FhirTypeDefinitions::resource_types().iter().collect();
        let primitive_types: std::collections::HashSet<_> = FhirTypeDefinitions::primitive_types().iter().collect();
        let complex_types: std::collections::HashSet<_> = FhirTypeDefinitions::complex_types().iter().collect();
        let special_types: std::collections::HashSet<_> = FhirTypeDefinitions::special_types().iter().collect();
        
        // Ensure no overlaps between type categories
        assert!(resource_types.is_disjoint(&primitive_types));
        assert!(resource_types.is_disjoint(&complex_types));
        assert!(resource_types.is_disjoint(&special_types));
        assert!(primitive_types.is_disjoint(&complex_types));
        assert!(primitive_types.is_disjoint(&special_types));
        assert!(complex_types.is_disjoint(&special_types));
    }
}