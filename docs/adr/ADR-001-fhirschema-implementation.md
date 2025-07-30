# ADR-001: FHIRSchema Implementation Architecture

## Status
Proposed

## Context
FHIRSchema is a project designed to simplify the implementation and validation of FHIR (Fast Healthcare Interoperability Resources) resources across different programming languages. It provides a more developer-friendly representation of FHIR StructureDefinitions, inspired by JSON Schema design principles.

### Current State
- FHIR validation implementations are complex and require esoteric knowledge
- Few implementations exist across different programming languages
- Most implementers perform similar transformations from StructureDefinition to nested data structures
- Snapshots leak implementation details into the standard
- Developers need simple metadata sources for code generation and FHIRPath

### FHIRSchema Benefits
- Simple and intuitive structure
- Explicit arrays handling
- Clear operational semantics
- Human and machine-readable format
- Differential validation approach
- Better support for code generation and tooling

## Decision
We will implement a comprehensive set of Rust crates for working with FHIRSchema, starting with a converter from FHIR StructureDefinition to FHIRSchema format.

## Architecture

### Core Crates

#### 1. `fhirschema-core`
**Purpose**: Core data structures and types for FHIRSchema
**Responsibilities**:
- Define FHIRSchema data structures (Schema, Element, Constraint, etc.)
- Implement serialization/deserialization (serde support)
- Provide basic validation for schema structure
- Handle FHIRSchema extensions

**Key Components**:
- `Schema` struct with url, type, name, derivation, base properties
- `Element` struct with shape, cardinality, choice types, constraints
- `Constraint` struct with FHIRPath expressions
- `Slicing` and `Slice` structs for array slicing
- `Binding` struct for terminology bindings
- Extension support (`any`, `additionalProperties`)

#### 2. `fhirschema-converter`
**Purpose**: Convert FHIR StructureDefinition to FHIRSchema
**Responsibilities**:
- Parse FHIR StructureDefinition resources
- Transform to FHIRSchema format
- Handle differential vs snapshot conversion
- Resolve references and inheritance
- Generate proper element hierarchies

**Key Components**:
- `StructureDefinitionConverter` main conversion engine
- `ElementConverter` for individual element transformation
- `ConstraintConverter` for FHIRPath constraint conversion
- `SlicingConverter` for slicing transformation
- Reference resolution utilities

#### 3. `fhirschema-validator`
**Purpose**: Validate FHIR resources against FHIRSchema
**Responsibilities**:
- Implement schemata resolution algorithm
- Perform data element validation
- Handle primitive datatype validation
- Execute FHIRPath constraint validation using fhirpath-rs
- Support slicing validation

**Key Components**:
- `Validator` main validation engine
- `SchemataResolver` for schema collection and following
- `ElementValidator` for individual element validation
- `ConstraintValidator` integrating with fhirpath-rs for FHIRPath execution
- `PrimitiveValidator` for FHIR primitive types
- `FHIRPathContext` for FHIR-specific variables (%resource, %rootResource, %context, %ucum)

#### 4. `fhirschema-repository`
**Purpose**: Schema repository and management
**Responsibilities**:
- Store and retrieve FHIRSchema definitions
- Handle schema versioning
- Resolve schema references
- Cache frequently used schemas

**Key Components**:
- `SchemaRepository` main repository interface
- `FileSystemRepository` file-based storage
- `MemoryRepository` in-memory storage
- `SchemaResolver` for reference resolution

#### 5. `fhirschema-codegen`
**Purpose**: Code generation from FHIRSchema
**Responsibilities**:
- Generate TypeScript interfaces and classes from FHIRSchema
- Generate Rust structs from FHIRSchema (secondary priority)
- Generate validation code
- Template-based generation for multiple language targets

**Key Components**:
- `CodeGenerator` main generation engine
- `TypeScriptGenerator` TypeScript-specific generator (primary focus)
- `RustGenerator` Rust-specific generator
- `TemplateEngine` for code templates
- Language-specific formatters and type mappers

### Supporting Crates

#### 6. `fhirschema-cli`
**Purpose**: Command-line interface for FHIRSchema tools
**Responsibilities**:
- Convert StructureDefinition to FHIRSchema
- Validate resources against schemas
- Generate code from schemas
- Schema repository management

#### 7. `fhirschema-server`
**Purpose**: HTTP server for FHIRSchema services
**Responsibilities**:
- REST API for validation services
- Schema repository HTTP interface
- Conversion services endpoint
- Automated IG registry integration with nightly processing
- S3-compatible storage integration (Garage)
- IG processing pipeline and job management
- Health checks and metrics

## Implementation Phases

### Phase 1: Core Foundation
- Implement `fhirschema-core` with basic data structures
- Create `fhirschema-converter` for StructureDefinition conversion
- Basic CLI for conversion operations

### Phase 2: Validation Engine
- Implement `fhirschema-validator` with schemata resolution
- Integrate `fhirschema-fhirpath` for constraint validation
- Add validation capabilities to CLI

### Phase 3: Repository and Management
- Implement `fhirschema-repository` for schema storage
- Add repository management to CLI
- Schema versioning and reference resolution

### Phase 4: Code Generation
- Implement `fhirschema-codegen` for Rust code generation
- Template system for flexible generation
- Integration with build systems

### Phase 5: Server and Services
- Implement `fhirschema-server` for HTTP services
- REST API for all operations
- Automated IG registry integration with nightly processing
- S3-compatible storage integration (Garage)
- Performance optimization and caching

## Technical Considerations

### Dependencies
- `serde` for serialization/deserialization
- `serde_yaml` for YAML support
- `serde_json` for JSON support
- `fhirpath-rs` from octofhir ecosystem for FHIRPath evaluation
- `ucum-rs` from octofhir ecosystem for unit conversion and validation
- `clap` for CLI interface
- `tokio` and `axum` for async server
- `anyhow` for error handling
- `thiserror` for custom error types
- `aws-sdk-s3` or `rusoto_s3` for S3-compatible storage integration
- `reqwest` for HTTP client (IG registry API calls)
- `tokio-cron-scheduler` for scheduled IG processing tasks

### Error Handling
- Use `thiserror` for domain-specific error types
- Provide detailed error messages with context
- Support error chaining for debugging
- Include location information for validation errors

### Performance
- Lazy loading of schemas
- Caching of resolved schemata
- Parallel validation where possible
- Memory-efficient data structures

### Testing Strategy
- Unit tests for all core functionality
- Integration tests with real FHIR resources
- Property-based testing for validation logic
- Performance benchmarks
- Compliance testing against FHIRSchema specification

## Consequences

### Positive
- Comprehensive Rust ecosystem for FHIRSchema
- Modular architecture allows incremental adoption
- Strong type safety and performance from Rust
- Clear separation of concerns
- Extensible design for future requirements

### Negative
- Large scope requires significant development effort
- Multiple crates increase maintenance overhead
- Dependency on external FHIRPath implementation
- Need to maintain compatibility with evolving FHIRSchema specification

### Risks
- FHIRSchema specification may change during development
- Performance requirements may require architecture changes
- Integration complexity with existing FHIR tooling

## Alternatives Considered

### Single Monolithic Crate
- Simpler to develop initially
- Less flexible for users who need only specific functionality
- Harder to maintain and test

### Minimal Implementation
- Faster to market
- May not meet all use cases
- Harder to extend later

## References
- [FHIRSchema Specification](https://fhir-schema.github.io/fhir-schema/intro.html)
- [FHIR R4 Specification](https://hl7.org/fhir/R4/)
- [FHIRPath Specification](https://build.fhir.org/ig/HL7/FHIRPath/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
