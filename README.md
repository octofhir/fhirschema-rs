# OctoFHIR FHIRSchema

[![Crates.io](https://img.shields.io/crates/v/octofhir-fhirschema.svg)](https://crates.io/crates/octofhir-fhirschema)
[![Documentation](https://docs.rs/octofhir-fhirschema/badge.svg)](https://docs.rs/octofhir-fhirschema)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

A high-performance Rust library for working with FHIRSchema that provides both library functionality and an optional CLI. This library converts FHIR StructureDefinitions into FHIRSchema format and validates FHIR resources against FHIRSchema definitions.

## Features

- 🚀 **High Performance**: Async/await throughout with efficient memory usage and parallel processing
- 🔄 **Full Conversion**: Convert FHIR StructureDefinitions to FHIRSchema format with parallel batch processing
- ✅ **Validation**: Validate FHIR resources against FHIRSchema definitions
- 📦 **Package Integration**: Seamless integration with `octofhir-canonical-manager`
- 🛠️ **CLI Tool**: Optional command-line interface for standalone usage
- 🌐 **Server Mode**: Optional HTTP server for schema management and validation
- 🧪 **Golden Test Compatible**: 100% compatibility with reference TypeScript implementation
- 💾 **Storage & Caching**: In-memory LRU caching and optional disk-based persistence with compression
- 🔍 **Search & Discovery**: Search StructureDefinitions across FHIR packages

## Quick Start

### Library Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
octofhir-fhirschema = "0.1"
octofhir-canonical-manager = "0.1"
```

#### Convert StructureDefinition to FHIRSchema

```rust
use octofhir_fhirschema::*;
use octofhir_canonical_manager::CanonicalManager;

#[tokio::main]
async fn main() -> Result<()> {
    // Load a StructureDefinition
    let mut structure_def: StructureDefinition = 
        serde_json::from_str(&std::fs::read_to_string("patient.json")?)?;
    
    // Extract elements from snapshot/differential
    structure_def.extract_elements()?;
    
    // Convert to FHIRSchema
    let converter = FhirSchemaConverter::new();
    let schema = converter.convert(&structure_def)?;
    
    // Save the result
    let output = serde_json::to_string_pretty(&schema)?;
    std::fs::write("patient.fhirschema.json", output)?;
    
    Ok(())
}
```

#### Validate FHIR Schema

```rust
use octofhir_fhirschema::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load a FHIRSchema
    let schema: FhirSchema = 
        serde_json::from_str(&std::fs::read_to_string("patient.fhirschema.json")?)?;
    
    // Validate the schema
    let validator = BasicSchemaValidator;
    let issues = validator.validate_schema(&schema)?;
    
    if issues.is_empty() {
        println!("✅ Schema is valid!");
    } else {
        println!("❌ Schema has {} validation issues", issues.len());
        for issue in issues {
            println!("  - {}: {}", issue.code, issue.message);
        }
    }
    
    Ok(())
}
```

### CLI Usage

Install the CLI tool:

```bash
cargo install octofhir-fhirschema --features cli
```

#### Convert StructureDefinition to FHIRSchema

```bash
# Convert a single file
octofhir-fhirschema convert-structure-definition \
  --input patient-structuredefinition.json \
  --output patient.fhirschema.json

# Download and convert FHIR packages
octofhir-fhirschema download \
  --package hl7.fhir.r4.core \
  --version 4.0.1 \
  --output ./schemas/ \
  --resource-types Patient,Observation
```

#### Validate FHIRSchema

```bash
# Validate a schema file
octofhir-fhirschema validate --schema patient.fhirschema.json

# Get detailed JSON output
octofhir-fhirschema validate --schema patient.fhirschema.json --format json
```

#### Schema Information

```bash
# Show schema details
octofhir-fhirschema info --schema patient.fhirschema.json
```

#### Package Management

```bash
# List installed FHIR packages
octofhir-fhirschema list

# Search for StructureDefinitions
octofhir-fhirschema search --query Patient

# Search in specific packages
octofhir-fhirschema search --query "us.core"
```

## Architecture

The library is organized into several key modules:

- **`converter`**: Converts FHIR StructureDefinitions to FHIRSchema format
- **`validation`**: Validates FHIR resources against FHIRSchema definitions  
- **`types`**: Core FHIRSchema data structures and types
- **`storage`**: Schema storage and caching functionality
- **`error`**: Comprehensive error handling

## Integration with OctoFHIR Ecosystem

This library integrates seamlessly with the broader OctoFHIR ecosystem:

- **`octofhir-canonical-manager`**: For FHIR package management and canonical URL resolution
- **Storage compatibility**: Shares configuration and storage patterns with other OctoFHIR tools

## Performance

Based on golden test benchmarks:

- **Conversion Speed**: 7 complex schemas converted in 0.01 seconds
- **Memory Efficiency**: Maximum 77MB memory usage during conversion
- **Compatibility**: 100% compatibility with reference TypeScript implementation (7/7 golden tests passing)

## Features

### Core Features

- [x] StructureDefinition to FHIRSchema conversion
- [x] FHIRSchema validation engine
- [x] Element transformation and constraint handling
- [x] Choice type expansion and slicing support
- [x] Reference validation and primitive extensions
- [x] FHIRPath constraint evaluation

### CLI Features

- [x] Convert StructureDefinition files to FHIRSchema
- [x] Validate FHIRSchema files
- [x] Display schema information and statistics
- [x] Download and convert FHIR packages
- [x] Search StructureDefinitions across packages
- [x] Package management integration

### Storage Features

- [x] In-memory schema storage with concurrent access
- [x] LRU caching for frequently accessed schemas
- [x] Optional disk-based persistence
- [x] Schema metadata and dependency tracking

## Development

### Building

```bash
# Build library only
cargo build

# Build with CLI support
cargo build --features cli

# Build with all features
cargo build --features all
```

### Testing

```bash
# Run all tests
cargo test

# Run golden tests specifically
cargo test golden

# Run with output
cargo test -- --nocapture
```

### Benchmarking

```bash
# Run performance benchmarks
cargo bench
```

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## References

- [FHIRSchema Specification](https://github.com/atomic-ehr/fhirschema/blob/main/spec/fhirschema-specification.md)
- [OctoFHIR Canonical Manager](https://crates.io/crates/octofhir-canonical-manager)
- [FHIR R4 Specification](http://hl7.org/fhir/R4/)

Made with ❤️ by OctoFHIR Team 🐙🦀
