# OctoFHIR FHIRSchema

A fresh Rust implementation of FHIRSchema conversion and validation.

## Overview

This project provides:
- **Converter**: Transforms FHIR StructureDefinitions into FHIRSchema format
- **Validator**: Validates FHIR resources against FHIRSchema definitions  
- **Schema Generator**: CLI tool for generating precompiled schemas

## Project Structure

- **`octofhir-fhirschema`** - Main library crate with core functionality
- **`octofhir-fhirschema-devtools`** - Development tools including schema generator

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
octofhir-fhirschema = "0.1.0"
```

## Usage

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
use octofhir_fhirschema::{validate, ValidationContext, FhirSchema};
use std::collections::HashMap;

// Create validation context with schemas
let mut schemas = HashMap::new();
schemas.insert("Patient".to_string(), patient_schema);
let context = ValidationContext { schemas };

// Validate resource
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

### Schema Generator CLI

Generate precompiled schemas for different FHIR versions:

```bash
# Generate R4 schemas
cargo run --bin schema-generator -- --version r4 --output ./schemas

# Generate R4B schemas  
cargo run --bin schema-generator -- --version r4b --output ./schemas

# Generate R5 schemas
cargo run --bin schema-generator -- --version r5 --output ./schemas

# Generate R6 schemas
cargo run --bin schema-generator -- --version r6 --output ./schemas
```

## Core Types

### FhirSchema
The target schema format with elements, constraints, and metadata.

### StructureDefinition  
Input FHIR StructureDefinition with differential elements.

### ValidationContext
Contains schemas for validation lookup.

### ValidationResult
Results of validation with errors and success flag.

## Features

- **Clean Architecture**: Simple, maintainable code without over-engineering
- **Type Safety**: Leverages Rust's type system for compile-time safety
- **Performance**: Efficient algorithms and minimal allocations
- **Multi-Version Support**: R4, R4B, R5, R6 FHIR versions
- **Binary Serialization**: Fast loading of precompiled schemas

## Development

### Running Tests

```bash
cargo test
```

### Building

```bash
cargo build --release
```

### Generating Documentation

```bash
cargo doc --open
```

## Testing

The implementation includes comprehensive tests:

- **Unit Tests**: Test individual components (converter, validator, path processing)
- **Integration Tests**: Test full conversion and validation workflows
- **Performance Tests**: Ensure efficient processing of large schemas

## API Reference

### Main Functions

- `translate(structure_definition, context)` - Convert StructureDefinition to FHIRSchema
- `validate(context, path, data)` - Validate FHIR resource against schemas

### Core Types

- `FhirSchema` - Target schema format
- `StructureDefinition` - Input FHIR structure definition  
- `ValidationContext` - Schema lookup context
- `ValidationResult` - Validation results with errors

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Run the test suite
5. Submit a pull request

## References

- [FHIR StructureDefinition Specification](http://hl7.org/fhir/structuredefinition.html)
- [FHIRSchema Format Documentation](https://github.com/fhir-schema/fhirschema)