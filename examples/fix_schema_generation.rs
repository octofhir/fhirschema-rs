use octofhir_fhirschema::types::{FhirSchema, FhirSchemaProperty};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Fixing Schema Generation Issue");
    println!("==================================\n");

    // The issue: Current precompiled schemas only include resource types,
    // but not complex data types like HumanName, Period, Address, etc.
    // This causes property validation to fail for paths like Patient.name.given

    println!("📋 Creating missing complex type schemas...");

    let mut schemas = Vec::new();

    // Add critical FHIR complex types that are missing
    let complex_types = [
        ("HumanName", create_human_name_schema()),
        ("Address", create_address_schema()),
        ("Period", create_period_schema()),
        ("ContactPoint", create_contact_point_schema()),
        ("Coding", create_coding_schema()),
        ("CodeableConcept", create_codeable_concept_schema()),
        ("Identifier", create_identifier_schema()),
        ("Reference", create_reference_schema()),
        ("Quantity", create_quantity_schema()),
        ("Meta", create_meta_schema()),
        ("Attachment", create_attachment_schema()),
        ("Range", create_range_schema()),
        ("Ratio", create_ratio_schema()),
        ("SampledData", create_sampled_data_schema()),
        ("Signature", create_signature_schema()),
        ("Timing", create_timing_schema()),
        ("Annotation", create_annotation_schema()),
        ("Dosage", create_dosage_schema()),
    ];

    for (type_name, schema) in complex_types {
        schemas.push(schema);
        println!("✅ Created schema for: {type_name}");
    }

    // Serialize as JSON (matching the current format)
    let json_data = serde_json::to_string_pretty(&schemas)?;

    // Write to a new file for inspection
    std::fs::write("generated_complex_types.json", &json_data)?;

    println!("\n📊 Generated {} complex type schemas", schemas.len());
    println!("💾 Saved to: generated_complex_types.json");

    println!("\n🔍 Schema breakdown:");
    for schema in &schemas {
        if let Some(title) = &schema.title {
            println!("- {}: {} properties", title, schema.properties.len());
        }
    }

    println!("\n📝 Next steps:");
    println!("1. The precompiled schema generation script needs to include these complex types");
    println!("2. Update get_core_resource_types() to also call get_complex_types()");
    println!("3. Regenerate precompiled schemas with: cargo run --bin schema_builder");

    Ok(())
}

fn create_human_name_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("HumanName")
        .with_description("A human's name with the ability to identify parts and usage")
        .with_id("http://hl7.org/fhir/StructureDefinition/HumanName");

    // Add properties
    schema.add_property(
        "use",
        FhirSchemaProperty::string()
            .with_description("usual | official | temp | nickname | anonymous | old | maiden"),
    );
    schema.add_property(
        "text",
        FhirSchemaProperty::string().with_description("Text representation of the full name"),
    );
    schema.add_property(
        "family",
        FhirSchemaProperty::string().with_description("Family name (often called 'Surname')"),
    );
    schema.add_property(
        "given",
        FhirSchemaProperty::array(FhirSchemaProperty::string())
            .with_description("Given names (not always 'first'). Includes middle names"),
    );
    schema.add_property(
        "prefix",
        FhirSchemaProperty::array(FhirSchemaProperty::string())
            .with_description("Parts that come before the name"),
    );
    schema.add_property(
        "suffix",
        FhirSchemaProperty::array(FhirSchemaProperty::string())
            .with_description("Parts that come after the name"),
    );
    schema.add_property(
        "period",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Period")
            .with_description("Time period when name was/is in use"),
    );

    schema
}

fn create_address_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Address")
        .with_description("An address expressed using postal conventions (as opposed to GPS or other location definition formats)")
        .with_id("http://hl7.org/fhir/StructureDefinition/Address");

    schema.add_property(
        "use",
        FhirSchemaProperty::string().with_description("home | work | temp | old | billing"),
    );
    schema.add_property(
        "type",
        FhirSchemaProperty::string().with_description("postal | physical | both"),
    );
    schema.add_property(
        "text",
        FhirSchemaProperty::string().with_description("Text representation of the address"),
    );
    schema.add_property(
        "line",
        FhirSchemaProperty::array(FhirSchemaProperty::string())
            .with_description("Street name, number, direction & P.O. Box etc."),
    );
    schema.add_property(
        "city",
        FhirSchemaProperty::string().with_description("Name of city, town etc."),
    );
    schema.add_property(
        "district",
        FhirSchemaProperty::string().with_description("District name (aka county)"),
    );
    schema.add_property(
        "state",
        FhirSchemaProperty::string().with_description("Sub-unit of country (abbreviations ok)"),
    );
    schema.add_property(
        "postalCode",
        FhirSchemaProperty::string().with_description("Postal code for area"),
    );
    schema.add_property(
        "country",
        FhirSchemaProperty::string()
            .with_description("Country (e.g. may be ISO 3166 2 or 3 letter code)"),
    );
    schema.add_property(
        "period",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Period")
            .with_description("Time period when address was/is in use"),
    );

    schema
}

fn create_period_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Period")
        .with_description("A time period defined by a start and end date and optionally time")
        .with_id("http://hl7.org/fhir/StructureDefinition/Period");

    schema.add_property(
        "start",
        FhirSchemaProperty::string()
            .with_format("date-time")
            .with_description("Starting time with inclusive boundary"),
    );
    schema.add_property(
        "end",
        FhirSchemaProperty::string()
            .with_format("date-time")
            .with_description("End time with inclusive boundary, if not ongoing"),
    );

    schema
}

fn create_contact_point_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("ContactPoint")
        .with_description("Details of a Technology mediated contact point")
        .with_id("http://hl7.org/fhir/StructureDefinition/ContactPoint");

    schema.add_property(
        "system",
        FhirSchemaProperty::string()
            .with_description("phone | fax | email | pager | url | sms | other"),
    );
    schema.add_property(
        "value",
        FhirSchemaProperty::string().with_description("The actual contact point details"),
    );
    schema.add_property(
        "use",
        FhirSchemaProperty::string().with_description("home | work | temp | old | mobile"),
    );
    schema.add_property(
        "rank",
        FhirSchemaProperty::integer()
            .with_description("Specify preferred order of use (1 = highest)"),
    );
    schema.add_property(
        "period",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Period")
            .with_description("Time period when the contact point was/is in use"),
    );

    schema
}

fn create_coding_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Coding")
        .with_description("A reference to a code defined by a terminology system")
        .with_id("http://hl7.org/fhir/StructureDefinition/Coding");

    schema.add_property(
        "system",
        FhirSchemaProperty::string()
            .with_format("uri")
            .with_description("Identity of the terminology system"),
    );
    schema.add_property(
        "version",
        FhirSchemaProperty::string().with_description("Version of the system - if relevant"),
    );
    schema.add_property(
        "code",
        FhirSchemaProperty::string().with_description("Symbol in syntax defined by the system"),
    );
    schema.add_property(
        "display",
        FhirSchemaProperty::string().with_description("Representation defined by the system"),
    );
    schema.add_property(
        "userSelected",
        FhirSchemaProperty::boolean()
            .with_description("If this coding was chosen directly by the user"),
    );

    schema
}

fn create_codeable_concept_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("CodeableConcept")
        .with_description(
            "A concept that may be defined by a formal reference to a terminology or ontology",
        )
        .with_id("http://hl7.org/fhir/StructureDefinition/CodeableConcept");

    schema.add_property(
        "coding",
        FhirSchemaProperty::array(FhirSchemaProperty::reference(
            "http://hl7.org/fhir/StructureDefinition/Coding",
        ))
        .with_description("Code defined by a terminology system"),
    );
    schema.add_property(
        "text",
        FhirSchemaProperty::string().with_description("Plain text representation of the concept"),
    );

    schema
}

fn create_identifier_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Identifier")
        .with_description("An identifier intended for computation")
        .with_id("http://hl7.org/fhir/StructureDefinition/Identifier");

    schema.add_property(
        "use",
        FhirSchemaProperty::string().with_description("usual | official | temp | secondary | old"),
    );
    schema.add_property(
        "type",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/CodeableConcept")
            .with_description("Description of identifier"),
    );
    schema.add_property(
        "system",
        FhirSchemaProperty::string()
            .with_format("uri")
            .with_description("The namespace for the identifier value"),
    );
    schema.add_property(
        "value",
        FhirSchemaProperty::string().with_description("The value that is unique"),
    );
    schema.add_property(
        "period",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Period")
            .with_description("Time period when id is/was valid for use"),
    );
    schema.add_property(
        "assigner",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Reference")
            .with_description("Organization that issued id"),
    );

    schema
}

fn create_reference_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Reference")
        .with_description("A reference from one resource to another")
        .with_id("http://hl7.org/fhir/StructureDefinition/Reference");

    schema.add_property(
        "reference",
        FhirSchemaProperty::string()
            .with_description("Literal reference, Relative, internal or absolute URL"),
    );
    schema.add_property(
        "type",
        FhirSchemaProperty::string()
            .with_format("uri")
            .with_description("Type the reference refers to"),
    );
    schema.add_property(
        "identifier",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Identifier")
            .with_description("Logical reference, when literal reference is not known"),
    );
    schema.add_property(
        "display",
        FhirSchemaProperty::string().with_description("Text alternative for the resource"),
    );

    schema
}

fn create_quantity_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Quantity")
        .with_description("A measured or measurable amount")
        .with_id("http://hl7.org/fhir/StructureDefinition/Quantity");

    schema.add_property(
        "value",
        FhirSchemaProperty::number().with_description("Numerical value (with implicit precision)"),
    );
    schema.add_property(
        "comparator",
        FhirSchemaProperty::string()
            .with_description("< | <= | >= | > - how to understand the value"),
    );
    schema.add_property(
        "unit",
        FhirSchemaProperty::string().with_description("Unit representation"),
    );
    schema.add_property(
        "system",
        FhirSchemaProperty::string()
            .with_format("uri")
            .with_description("System that defines coded unit form"),
    );
    schema.add_property(
        "code",
        FhirSchemaProperty::string().with_description("Coded form of the unit"),
    );

    schema
}

fn create_meta_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Meta")
        .with_description("Metadata about a resource")
        .with_id("http://hl7.org/fhir/StructureDefinition/Meta");

    schema.add_property(
        "versionId",
        FhirSchemaProperty::string().with_description("Version specific identifier"),
    );
    schema.add_property(
        "lastUpdated",
        FhirSchemaProperty::string()
            .with_format("date-time")
            .with_description("When the resource version last changed"),
    );
    schema.add_property(
        "source",
        FhirSchemaProperty::string()
            .with_format("uri")
            .with_description("Identifies where the resource comes from"),
    );
    schema.add_property(
        "profile",
        FhirSchemaProperty::array(FhirSchemaProperty::string().with_format("canonical"))
            .with_description("Profiles this resource claims to conform to"),
    );
    schema.add_property(
        "security",
        FhirSchemaProperty::array(FhirSchemaProperty::reference(
            "http://hl7.org/fhir/StructureDefinition/Coding",
        ))
        .with_description("Security Labels applied to this resource"),
    );
    schema.add_property(
        "tag",
        FhirSchemaProperty::array(FhirSchemaProperty::reference(
            "http://hl7.org/fhir/StructureDefinition/Coding",
        ))
        .with_description("Tags applied to this resource"),
    );

    schema
}

fn create_attachment_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Attachment")
        .with_description("Content in a format defined elsewhere")
        .with_id("http://hl7.org/fhir/StructureDefinition/Attachment");

    schema.add_property(
        "contentType",
        FhirSchemaProperty::string()
            .with_description("Mime type of the content, with charset etc."),
    );
    schema.add_property(
        "language",
        FhirSchemaProperty::string().with_description("Human language of the content"),
    );
    schema.add_property(
        "data",
        FhirSchemaProperty::string()
            .with_format("base64Binary")
            .with_description("Data inline, base64ed"),
    );
    schema.add_property(
        "url",
        FhirSchemaProperty::string()
            .with_format("url")
            .with_description("Uri where the data can be found"),
    );
    schema.add_property(
        "size",
        FhirSchemaProperty::integer().with_description("Number of bytes of content"),
    );
    schema.add_property(
        "hash",
        FhirSchemaProperty::string()
            .with_format("base64Binary")
            .with_description("Hash of the data"),
    );
    schema.add_property(
        "title",
        FhirSchemaProperty::string().with_description("Label to display in place of the data"),
    );
    schema.add_property(
        "creation",
        FhirSchemaProperty::string()
            .with_format("date-time")
            .with_description("Date attachment was first created"),
    );

    schema
}

fn create_range_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Range")
        .with_description("Set of values bounded by low and high")
        .with_id("http://hl7.org/fhir/StructureDefinition/Range");

    schema.add_property(
        "low",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Quantity")
            .with_description("Low limit"),
    );
    schema.add_property(
        "high",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Quantity")
            .with_description("High limit"),
    );

    schema
}

fn create_ratio_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Ratio")
        .with_description("A ratio of two Quantity values - a numerator and a denominator")
        .with_id("http://hl7.org/fhir/StructureDefinition/Ratio");

    schema.add_property(
        "numerator",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Quantity")
            .with_description("Numerator value"),
    );
    schema.add_property(
        "denominator",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Quantity")
            .with_description("Denominator value"),
    );

    schema
}

fn create_sampled_data_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("SampledData")
        .with_description("A series of measurements taken by a device")
        .with_id("http://hl7.org/fhir/StructureDefinition/SampledData");

    schema.add_property(
        "origin",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Quantity")
            .with_description("Zero value and units"),
    );
    schema.add_property(
        "period",
        FhirSchemaProperty::number().with_description("Number of milliseconds between samples"),
    );
    schema.add_property(
        "factor",
        FhirSchemaProperty::number()
            .with_description("Multiply data by this before adding to origin"),
    );
    schema.add_property(
        "lowerLimit",
        FhirSchemaProperty::number().with_description("Lower limit of detection"),
    );
    schema.add_property(
        "upperLimit",
        FhirSchemaProperty::number().with_description("Upper limit of detection"),
    );
    schema.add_property(
        "dimensions",
        FhirSchemaProperty::integer()
            .with_description("Number of sample points at each time point"),
    );
    schema.add_property(
        "data",
        FhirSchemaProperty::string()
            .with_description("Decimal values with spaces, or \"E\" | \"U\" | \"L\""),
    );

    schema
}

fn create_signature_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Signature")
        .with_description("A Signature - XML DigSig, JWS, Graphical image of signature, etc.")
        .with_id("http://hl7.org/fhir/StructureDefinition/Signature");

    schema.add_property(
        "type",
        FhirSchemaProperty::array(FhirSchemaProperty::reference(
            "http://hl7.org/fhir/StructureDefinition/Coding",
        ))
        .with_description("Indication of the reason the entity signed the object"),
    );
    schema.add_property(
        "when",
        FhirSchemaProperty::string()
            .with_format("date-time")
            .with_description("When the signature was created"),
    );
    schema.add_property(
        "who",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Reference")
            .with_description("Who signed"),
    );
    schema.add_property(
        "onBehalfOf",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Reference")
            .with_description("The party represented"),
    );
    schema.add_property(
        "targetFormat",
        FhirSchemaProperty::string()
            .with_description("The technical format of the signed resources"),
    );
    schema.add_property(
        "sigFormat",
        FhirSchemaProperty::string().with_description("The technical format of the signature"),
    );
    schema.add_property(
        "data",
        FhirSchemaProperty::string()
            .with_format("base64Binary")
            .with_description("The actual signature content"),
    );

    schema
}

fn create_timing_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Timing")
        .with_description("A timing schedule that specifies an event that may occur multiple times")
        .with_id("http://hl7.org/fhir/StructureDefinition/Timing");

    schema.add_property(
        "event",
        FhirSchemaProperty::array(FhirSchemaProperty::string().with_format("date-time"))
            .with_description("When the event occurs"),
    );
    schema.add_property(
        "repeat",
        FhirSchemaProperty::object().with_description("When the event is to occur"),
    );
    schema.add_property(
        "code",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/CodeableConcept")
            .with_description("BID | TID | QID | AM | PM | QD | QOD | +"),
    );

    schema
}

fn create_annotation_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Annotation")
        .with_description("Text node with attribution")
        .with_id("http://hl7.org/fhir/StructureDefinition/Annotation");

    schema.add_property(
        "authorReference",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Reference")
            .with_description("Individual responsible for the annotation"),
    );
    schema.add_property(
        "authorString",
        FhirSchemaProperty::string().with_description("Individual responsible for the annotation"),
    );
    schema.add_property(
        "time",
        FhirSchemaProperty::string()
            .with_format("date-time")
            .with_description("When the annotation was made"),
    );
    schema.add_property(
        "text",
        FhirSchemaProperty::string().with_description("The annotation - text content"),
    );

    schema
}

fn create_dosage_schema() -> FhirSchema {
    let mut schema = FhirSchema::new("object")
        .with_title("Dosage")
        .with_description("How the medication is/was taken or should be taken")
        .with_id("http://hl7.org/fhir/StructureDefinition/Dosage");

    schema.add_property(
        "sequence",
        FhirSchemaProperty::integer().with_description("The order of the dosage instructions"),
    );
    schema.add_property(
        "text",
        FhirSchemaProperty::string().with_description("Free text dosage instructions"),
    );
    schema.add_property(
        "additionalInstruction",
        FhirSchemaProperty::array(FhirSchemaProperty::reference(
            "http://hl7.org/fhir/StructureDefinition/CodeableConcept",
        ))
        .with_description("Supplemental instruction"),
    );
    schema.add_property(
        "patientInstruction",
        FhirSchemaProperty::string().with_description("Patient or consumer oriented instructions"),
    );
    schema.add_property(
        "timing",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/Timing")
            .with_description("When medication should be administered"),
    );
    schema.add_property(
        "asNeededBoolean",
        FhirSchemaProperty::boolean().with_description("Take \"as needed\""),
    );
    schema.add_property(
        "asNeededCodeableConcept",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/CodeableConcept")
            .with_description("Take \"as needed\" (for x)"),
    );
    schema.add_property(
        "site",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/CodeableConcept")
            .with_description("Body site to administer to"),
    );
    schema.add_property(
        "route",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/CodeableConcept")
            .with_description("How drug should enter body"),
    );
    schema.add_property(
        "method",
        FhirSchemaProperty::reference("http://hl7.org/fhir/StructureDefinition/CodeableConcept")
            .with_description("Technique for administering medication"),
    );

    schema
}
