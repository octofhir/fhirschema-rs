# octofhir-fhirschema Usage Guide

This guide covers common use cases for the octofhir-fhirschema crate.

## Quick Start

### Basic Validation

```rust
use octofhir_fhirschema::{ValidationProviderBuilder, FhirVersion};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a validation provider with embedded R4 schemas
    let provider = ValidationProviderBuilder::new(FhirVersion::R4)
        .with_embedded_schemas()
        .build()?;

    // Create a Patient resource to validate
    let patient = json!({
        "resourceType": "Patient",
        "id": "example",
        "name": [{
            "use": "official",
            "family": "Smith",
            "given": ["John"]
        }]
    });

    // Validate the resource
    let result = provider.validate(&patient, "Patient").await?;

    if result {
        println!("Patient is valid!");
    }

    Ok(())
}
```

### Direct Validator Usage

For more control over validation, use `FhirSchemaValidator` directly:

```rust
use octofhir_fhirschema::{FhirSchemaValidator, get_schemas, FhirVersion};
use serde_json::json;

#[tokio::main]
async fn main() {
    // Get embedded schemas
    let schemas = get_schemas(FhirVersion::R4).clone();

    // Create validator
    let validator = FhirSchemaValidator::new(schemas, None);

    // Validate a resource
    let patient = json!({
        "resourceType": "Patient",
        "id": "example"
    });

    let result = validator.validate(&patient, vec!["Patient".to_string()]).await;

    if result.valid {
        println!("Resource is valid!");
    } else {
        for error in &result.errors {
            println!("Error: {} - {:?}", error.error_type, error.message);
        }
    }
}
```

## Validation with FHIRPath Constraints

To enable FHIRPath constraint validation, you need to provide a FHIRPath evaluator:

```rust
use octofhir_fhirschema::{ValidationProviderBuilder, FhirVersion};
use std::sync::Arc;

// Assuming you have a FHIRPath evaluator implementation
let fhirpath_evaluator = Arc::new(my_fhirpath_evaluator);

let provider = ValidationProviderBuilder::new(FhirVersion::R4)
    .with_embedded_schemas()
    .with_fhirpath(fhirpath_evaluator)
    .build()?;
```

## Profile Validation

Validate against specific profiles:

```rust
use octofhir_fhirschema::{FhirSchemaValidator, get_schemas, FhirVersion};

let schemas = get_schemas(FhirVersion::R4).clone();
let validator = FhirSchemaValidator::new(schemas, None);

// Validate against a specific profile URL
let result = validator.validate(
    &resource,
    vec!["http://hl7.org/fhir/StructureDefinition/Patient".to_string()]
).await;

// Validate against multiple profiles
let result = validator.validate_with_profiles(
    &resource,
    vec![
        "http://hl7.org/fhir/StructureDefinition/Patient".to_string(),
        "http://example.org/fhir/StructureDefinition/MyPatientProfile".to_string(),
    ]
).await;
```

## Error Handling

Validation errors include detailed information:

```rust
let result = validator.validate(&resource, vec!["Patient".to_string()]).await;

if !result.valid {
    for error in &result.errors {
        println!("Error type: {}", error.error_type);
        println!("Path: {:?}", error.path);
        println!("Message: {:?}", error.message);

        // For constraint errors
        if let Some(key) = &error.constraint_key {
            println!("Constraint: {}", key);
        }
    }
}
```

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| FS1001 | UnknownElement | Element is not defined in the schema |
| FS1002 | UnknownSchema | Referenced schema was not found |
| FS1003 | ExpectedArray | Expected an array but got a single value |
| FS1004 | UnexpectedArray | Expected a single value but got an array |
| FS1005 | UnknownKeyword | Unknown keyword in schema |
| FS1006 | WrongType | Value has wrong type |
| FS1007 | SlicingUnmatched | Array item doesn't match any defined slice |
| FS1008 | SlicingAmbiguous | Array item matches multiple slices |
| FS1009 | SliceCardinality | Slice cardinality violation |
| FS1010 | ConstraintViolation | FHIRPath constraint failed |
| FS1011 | CardinalityViolation | Required element missing or max exceeded |

## Provider Types

### EmbeddedSchemaProvider

Uses pre-compiled schemas bundled with the crate:

```rust
use octofhir_fhirschema::EmbeddedSchemaProvider;

let provider = EmbeddedSchemaProvider::r4();
// or
let provider = EmbeddedSchemaProvider::r4b();
let provider = EmbeddedSchemaProvider::r5();
```

### DynamicSchemaProvider

Loads schemas at runtime:

```rust
use octofhir_fhirschema::DynamicSchemaProvider;

// From StructureDefinitions
let provider = DynamicSchemaProvider::from_structure_definitions(
    structure_definitions,
    FhirVersion::R4
)?;
```

### FhirSchemaModelProvider

Low-level provider for type information:

```rust
use octofhir_fhirschema::{FhirSchemaModelProvider, get_schemas, ModelFhirVersion};

let schemas = get_schemas(FhirVersion::R4).clone();
let provider = FhirSchemaModelProvider::new(schemas, ModelFhirVersion::R4);

// Get type information
let type_info = provider.get_type("Patient").await?;
```

## StructureDefinition Conversion

Convert FHIR StructureDefinitions to FhirSchema format:

```rust
use octofhir_fhirschema::translate;
use serde_json::Value;

// Load a StructureDefinition
let sd: Value = serde_json::from_str(sd_json)?;

// Convert to FhirSchema
let schema = translate(&sd)?;

println!("Converted: {} ({})", schema.name, schema.url);
```

## FHIR Version Support

The crate supports multiple FHIR versions:

- **R4** (4.0.1) - Full support with embedded schemas
- **R4B** (4.3.0) - Full support with embedded schemas
- **R5** (5.0.0) - Support with embedded schemas
- **R6** (6.0.0) - Preview support

```rust
use octofhir_fhirschema::FhirVersion;

let version = FhirVersion::R4;
let schemas = get_schemas(version);
```

## Best Practices

1. **Reuse providers**: Create providers once and reuse them for multiple validations
2. **Use embedded schemas**: For standard FHIR validation, embedded schemas are fastest
3. **Add FHIRPath support**: For complete validation including invariants
4. **Handle errors gracefully**: Check both `valid` flag and `errors` collection
5. **Prefer async**: All validation is async for performance
