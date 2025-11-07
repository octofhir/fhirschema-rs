# octofhir-fhirschema

FHIR schema conversion and validation library for Rust.

## Overview

This crate provides tools for converting FHIR StructureDefinitions to FHIRSchema format and validating FHIR resources against those schemas. It supports FHIR versions R4, R4B, R5, and R6.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
octofhir-fhirschema = "0.3.2"
```

## Quick Start

### Converting StructureDefinition to FHIRSchema

```rust
use octofhir_fhirschema::{translate, StructureDefinition};

// Load your StructureDefinition
let structure_definition: StructureDefinition = serde_json::from_str(json_string)?;

// Convert to FHIRSchema
let schema = translate(structure_definition, None)?;
println!("Converted schema: {}", schema.name);
```

### Validating FHIR Resources

```rust
use octofhir_fhirschema::{validate, create_validation_context, FhirVersion};

// Create validation context with embedded schemas
let context = create_validation_context(FhirVersion::R4)?;

// Validate a FHIR resource
let patient_data = serde_json::json!({
    "resourceType": "Patient",
    "id": "example",
    "active": true
});

let result = validate(&context, vec![], &patient_data);

if result.valid {
    println!("Resource is valid!");
} else {
    println!("Validation errors: {:?}", result.errors);
}
```

### Using Embedded Schemas

The crate includes precompiled schemas for faster startup:

```rust
use octofhir_fhirschema::{FhirVersion, get_schema, list_resources};

// List available resource types
let resources = list_resources(FhirVersion::R4);
println!("Available resources: {:?}", resources);

// Get a specific schema
if let Some(patient_schema) = get_schema(FhirVersion::R4, "Patient") {
    println!("Patient schema loaded");
}
```

## Features

- **Schema Conversion**: Convert FHIR StructureDefinitions to FHIRSchema format
- **Resource Validation**: Validate FHIR resources against schemas
- **Multi-Version Support**: R4, R4B, R5, and R6 FHIR versions
- **Embedded Schemas**: Precompiled schemas for fast startup
- **Type Safety**: Full Rust type safety and error handling
- **Performance**: Efficient validation with minimal allocations

## Core Types

- `FhirSchema` - Converted schema format
- `StructureDefinition` - Input FHIR structure definition
- `ValidationContext` - Container for schemas used in validation
- `ValidationResult` - Validation results with success flag and errors
- `FhirSchemaValidator` - Main validator implementation

## Provider Pattern

The crate supports different schema providers:

```rust
use octofhir_fhirschema::{EmbeddedSchemaProvider, DynamicSchemaProvider, FhirVersion};

// Use embedded schemas (fastest)
let provider = EmbeddedSchemaProvider::new(FhirVersion::R4)?;

// Use dynamic schemas (more flexible)
let provider = DynamicSchemaProvider::new();
```

## Error Handling

All operations return `Result` types with detailed error information:

```rust
use octofhir_fhirschema::{FhirSchemaError, Result};

match translate(structure_def, None) {
    Ok(schema) => println!("Success: {}", schema.name),
    Err(FhirSchemaError::JsonError(e)) => eprintln!("JSON error: {}", e),
    Err(FhirSchemaError::ValidationError(e)) => eprintln!("Validation error: {}", e),
    Err(e) => eprintln!("Other error: {}", e),
}
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Repository

This crate is part of the [OctoFHIR](https://github.com/octofhir/fhirschema-rs) project.