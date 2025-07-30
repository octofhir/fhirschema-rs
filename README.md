# FHIRSchema Implementation in Rust

[![CI](https://github.com/octofhir/fhirschema/workflows/CI/badge.svg)](https://github.com/octofhir/fhirschema/actions)
[![codecov](https://codecov.io/gh/octofhir/fhirschema/branch/main/graph/badge.svg)](https://codecov.io/gh/octofhir/fhirschema)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

A comprehensive Rust implementation of [FHIRSchema](https://fhir-schema.github.io/fhir-schema/intro.html), providing tools for converting FHIR StructureDefinitions to FHIRSchema format, validating FHIR resources, and generating code from schemas.

## Overview

FHIRSchema is a project designed to simplify the implementation and validation of FHIR (Fast Healthcare Interoperability Resources) resources across different programming languages. It provides a more developer-friendly representation of FHIR StructureDefinitions, inspired by JSON Schema design principles.

### Key Benefits

- **Simple and intuitive structure** - Easier to understand than traditional StructureDefinitions
- **Explicit arrays handling** - Clear operational semantics for array validation
- **Human and machine-readable format** - YAML and JSON support
- **Differential validation approach** - No dependency on snapshots
- **Better tooling support** - Optimized for code generation and FHIRPath operations

## Architecture

This implementation consists of several modular crates:

### Core Crates

- **`fhirschema-core`** - Core data structures and types for FHIRSchema
- **`fhirschema-converter`** - Convert FHIR StructureDefinition to FHIRSchema format
- **`fhirschema-validator`** - Validate FHIR resources against FHIRSchema (planned)
- **`fhirschema-repository`** - Schema storage and management (planned)
- **`fhirschema-codegen`** - Code generation from schemas (planned)

### Supporting Crates

- **`fhirschema-cli`** - Command-line interface for FHIRSchema tools
- **`fhirschema-server`** - HTTP server for FHIRSchema services (planned)

## Quick Start

### Prerequisites

- Rust 1.70.0 or later
- Cargo

### Installation

```bash
git clone https://github.com/octofhir/fhirschema.git
cd fhirschema
cargo build
```

### Usage

#### Convert StructureDefinition to FHIRSchema

```bash
cargo run --bin fhirschema convert --input patient.json --output patient-schema.yaml
```

#### Validate a schema file

```bash
cargo run --bin fhirschema validate --schema patient-schema.yaml --resource patient-instance.json
```

## Development

### Building

```bash
# Build all crates
cargo build

# Build with all features
cargo build --all-features

# Build specific crate
cargo build -p fhirschema-core
```

### Testing

```bash
# Run all tests
cargo test

# Run tests with coverage
cargo llvm-cov --all-features --workspace
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linting
cargo clippy --all-targets --all-features -- -D warnings

# Generate documentation
cargo doc --no-deps --document-private-items --all-features
```

## Project Status

This project is currently in **Phase 1: Core Foundation** development. See the [Implementation Plan](IMPLEMENTATION_PLAN.md) for detailed progress and roadmap.

### Current Status

- ✅ Project setup and workspace configuration
- 🚧 Core data structures implementation
- 📋 StructureDefinition converter (planned)
- 📋 Validation engine (planned)
- 📋 Code generation (planned)

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Guidelines

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use [Rust Performance Book](https://nnethercote.github.io/perf-book/) for optimization
- Apply [Rust Coding Guidelines](https://rust-lang.github.io/rust-clippy/master/index.html)
- Maintain high test coverage (>90%)
- Document all public APIs with examples

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## References

- [FHIRSchema Specification](https://fhir-schema.github.io/fhir-schema/intro.html)
- [FHIR R4 Specification](https://hl7.org/fhir/R4/)
- [FHIRPath Specification](https://build.fhir.org/ig/HL7/FHIRPath/)
- [Architecture Decision Record](docs/adr/ADR-001-fhirschema-implementation.md)

## Acknowledgments

This project is part of the [octofhir](https://github.com/octofhir) ecosystem and integrates with:

- [fhirpath-rs](https://github.com/octofhir/fhirpath-rs) - FHIRPath engine for constraint evaluation
- [ucum-rs](https://github.com/octofhir/ucum-rs) - UCUM unit conversion and validation
