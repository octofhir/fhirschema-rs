# FHIRSchema Implementation Plan

## Overview

This document provides a comprehensive plan for implementing a set of Rust crates for working with FHIRSchema, based on thorough analysis of the FHIRSchema specification and following the guidelines for ADR-driven development.

## Analysis Summary

### FHIRSchema Specification Analysis

The FHIRSchema specification has been thoroughly analyzed from the `specs` folder, covering:

- **Core Concepts**: FHIRSchema provides a developer-friendly representation of FHIR StructureDefinitions, inspired by JSON Schema
- **Schema Structure**: Schemas have required properties (url, type, name, derivation) and optional properties (base, elements, constraints, extensions)
- **Element System**: Elements support shape (array/scalar), cardinality, choice types, nested elements, constraints, slicing, and bindings
- **Validation Algorithm**: Uses schemata resolution with collect/follow operations for differential validation
- **Constraint System**: FHIRPath expressions with FHIR-specific variables (%context, %resource, %rootResource, %ucum)
- **Slicing Support**: Array slicing with discriminators, ordering, and re-slicing capabilities
- **Extensions**: Optional FHIR-incompatible extensions (any, additionalProperties) for specialized use cases

### Key Benefits of FHIRSchema

- Simplifies FHIR validation implementation across programming languages
- Provides explicit array handling and clear operational semantics
- Enables better tooling for code generation and FHIRPath operations
- Reduces complexity compared to traditional StructureDefinition processing
- Supports differential validation without snapshot dependencies

## Architecture Decision

**ADR-001: FHIRSchema Implementation Architecture** documents the decision to implement a comprehensive Rust ecosystem for FHIRSchema operations.

### Proposed Architecture

#### Core Crates (5)
1. **fhirschema-core** - Core data structures and types
2. **fhirschema-converter** - StructureDefinition to FHIRSchema conversion
3. **fhirschema-validator** - Resource validation against FHIRSchema (uses octofhir/fhirpath-rs)
4. **fhirschema-repository** - Schema storage and management
5. **fhirschema-codegen** - Code generation from schemas (TypeScript-first, then Rust)

#### Supporting Crates (2)
6. **fhirschema-cli** - Command-line interface
7. **fhirschema-server** - HTTP server for services

#### External Dependencies
- **octofhir/fhirpath-rs** - FHIRPath engine for constraint evaluation
- **octofhir/ucum-rs** - UCUM unit conversion and validation

## Implementation Phases

### Phase 1: Core Foundation (4-6 weeks) - **PRIORITY: HIGH**
**Primary Focus**: StructureDefinition to FHIRSchema converter as requested

**Key Deliverables**:
- Core data structures (Schema, Element, Constraint, etc.)
- StructureDefinition converter with comprehensive transformation capabilities
- Basic CLI for conversion operations
- Foundation for all subsequent phases

**Tasks**:
- Project setup and workspace configuration
- Implement fhirschema-core data structures
- Implement fhirschema-converter core engine
- Create basic CLI interface
- Testing and documentation

### Phase 2: Validation Engine (6-8 weeks) - **PRIORITY: HIGH**
**Focus**: Complete validation capabilities with octofhir ecosystem integration

**Key Deliverables**:
- Schemata resolution algorithm implementation
- FHIRPath constraint validation using octofhir/fhirpath-rs
- Primitive datatype validation with octofhir/ucum-rs integration
- Slicing validation support

### Phase 3: Repository and Management (4-5 weeks) - **PRIORITY: MEDIUM**
**Focus**: Schema storage and management

**Key Deliverables**:
- Schema repository with multiple storage backends (Memory, FileSystem, S3-compatible)
- S3-compatible storage integration (Garage) for distributed schema storage
- Reference resolution and dependency management
- Version management and compatibility checking

### Phase 4: Code Generation (5-6 weeks) - **PRIORITY: MEDIUM**
**Focus**: TypeScript-first code generation capabilities

**Key Deliverables**:
- Template-based code generation engine
- TypeScript interface and class generation from FHIRSchema (primary focus)
- Rust struct generation from FHIRSchema (secondary)
- Build system integration for TypeScript/Node.js and Rust/Cargo

### Phase 5: Server and Services (6-8 weeks) - **PRIORITY: MEDIUM**
**Focus**: HTTP services, automated IG processing, and S3-compatible storage

**Key Deliverables**:
- REST API for all FHIRSchema operations
- Automated IG registry integration with nightly processing
- S3-compatible storage integration (Garage)
- IG processing pipeline and job management
- Async processing and job management
- Monitoring and observability

## Task Files

Detailed task files have been created for each phase:

- `tasks/phase-1-core-foundation.md` - **START HERE** (includes StructureDefinition converter)
- `tasks/phase-2-validation-engine.md`
- `tasks/phase-3-repository-management.md`
- `tasks/phase-4-code-generation.md`
- `tasks/phase-5-server-services.md`

Each task file includes:
- Detailed subtasks with checkboxes for progress tracking
- Clear acceptance criteria
- Estimated durations and priorities
- Risk mitigation strategies
- Dependencies and deliverables

## Getting Started

### Immediate Next Steps

1. **Review and approve ADR-001** - Ensure architectural decisions align with project goals
2. **Begin Phase 1 implementation** - Start with Task 1.1 (Project Setup)
3. **Focus on StructureDefinition converter** - This is the highest priority deliverable
4. **Set up development environment** - Follow Rust best practices and guidelines
5. **Create initial tests** - Use examples from FHIRSchema specification

### Development Guidelines

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use [Rust Performance Book](https://nnethercote.github.io/perf-book/) for optimization
- Apply [Rust Coding Guidelines](https://rust-lang.github.io/rust-clippy/master/index.html)
- Maintain high test coverage (>90%)
- Document all public APIs with examples

### Success Metrics

- Successfully convert FHIR StructureDefinitions to FHIRSchema format
- Generated schemas validate correctly against specification
- Performance meets production requirements
- Code quality passes all linting and formatting checks
- Documentation enables easy adoption by other developers

## Risk Management

### Key Risks and Mitigations

1. **Specification Complexity** - Start with simple cases, incrementally add complexity
2. **Performance Requirements** - Implement benchmarking early, optimize critical paths
3. **octofhir/fhirpath-rs Integration** - Work with octofhir ecosystem, contribute fixes if needed
4. **TypeScript Code Generation Quality** - Follow TypeScript best practices, extensive testing
5. **Specification Evolution** - Design flexible architecture, maintain compatibility layers

## Conclusion

This implementation plan provides a comprehensive roadmap for creating a robust FHIRSchema ecosystem in Rust. The phased approach ensures that the most critical functionality (StructureDefinition conversion) is delivered first, while the modular architecture allows for incremental adoption and future extensions.

The detailed task files provide specific guidance for implementation, and the ADR documents the architectural decisions for future reference. Following this plan will result in a production-ready FHIRSchema implementation that significantly simplifies FHIR validation and tooling development.

---

**Next Action**: Begin implementation with Phase 1, Task 1.1 - Project Setup and Workspace Configuration
