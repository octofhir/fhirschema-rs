Цу# ADR-001: OctoFHIR FHIRSchema Library Implementation

**Status:** Accepted  
**Date:** 2025-08-05  
**Author:** OctoFHIR Team  

## Context

We need to develop a high-performance Rust library for working with FHIRSchema that provides both library functionality and an optional CLI with the `octofhir` prefix. The library should be simple, clean, and performant while passing golden tests from the reference TypeScript implementation.

### Reference Analysis

Based on analysis of the reference repository (https://github.com/atomic-ehr/fhirschema):

1. **Core Components:**
   - Converter: Transforms FHIR StructureDefinitions into FHIRSchema format
   - Validator: Validates FHIR resources against FHIRSchema definitions
   - Types: Shared data structures for FHIRSchema representation

2. **Golden Tests:** The reference implementation includes comprehensive golden tests with expected outputs for various FHIR resources (Patient, Bundle, Questionnaire, primitives, complex types)

3. **Architecture:** Clean separation between converter logic, validation engine, and type definitions

### Available Dependencies

We have access to `octofhir-canonical-manager` which provides:
- Package management for FHIR Implementation Guides
- Fast canonical URL resolution
- Resource search capabilities
- Async/await support
- Configuration management

## Decision

We will implement `octofhir-fhirschema` as a Rust library with the following architecture:

### Core Architecture

```
octofhir-fhirschema/
├── src/
│   ├── lib.rs              # Library entry point
│   ├── converter/          # StructureDefinition to FHIRSchema conversion
│   │   ├── mod.rs
│   │   ├── translator.rs   # Main conversion logic
│   │   ├── element.rs      # Element transformation
│   │   └── types.rs        # Converter-specific types
│   ├── validator/          # FHIRSchema validation engine
│   │   ├── mod.rs
│   │   ├── engine.rs       # Main validation logic
│   │   ├── rules.rs        # Validation rules implementation
│   │   └── context.rs      # Validation context
│   ├── schema/             # FHIRSchema data structures
│   │   ├── mod.rs
│   │   ├── types.rs        # Core FHIRSchema types
│   │   ├── serde.rs        # Serialization support
│   │   └── cache.rs        # Schema caching functionality
│   ├── storage/            # Schema storage and persistence
│   │   ├── mod.rs
│   │   ├── memory.rs       # In-memory schema storage
│   │   ├── disk.rs         # Disk-based schema persistence
│   │   └── manager.rs      # Storage manager and strategies
│   ├── error.rs            # Error types
│   └── cli/                # Optional CLI (feature-gated)
│       ├── mod.rs
│       ├── commands.rs
│       └── main.rs
├── tests/
│   ├── golden/             # Golden tests from reference
│   ├── integration/        # Integration tests
│   └── unit/               # Unit tests
└── examples/               # Usage examples
```

### Key Design Decisions

1. **Library-First Approach:**
   - Core functionality in `lib.rs` without CLI dependencies
   - CLI as optional feature (`cli` feature flag)
   - Clean public API for embedding in applications

2. **Performance Focus:**
   - Async/await throughout for I/O operations
   - Efficient serialization with `serde`
   - Memory-efficient data structures
   - Streaming where applicable

3. **Integration with Canonical Manager:**
   - Use `octofhir-canonical-manager` for StructureDefinition loading
   - Leverage its package management capabilities
   - Reuse its configuration system

4. **Golden Test Compatibility:**
   - Port all golden tests from TypeScript reference
   - Ensure bit-for-bit compatibility with expected outputs
   - Comprehensive test coverage

5. **Schema Storage & Caching:**
   - In-memory storage for fast access during runtime
   - Optional disk persistence for schema reuse across sessions
   - LRU caching with configurable size limits
   - Integration with canonical manager storage patterns

6. **CLI Design:**
   - Prefix: `octofhir-fhirschema` or `octofhir-fs`
   - Commands: `convert`, `validate`, `schema`, `cache`
   - JSON/YAML output support
   - Integration with canonical manager for package loading

## Implementation Plan

### Phase 1: Foundation (Week 1)
- Set up project structure and build system
- Define core FHIRSchema types with serde support
- Implement basic error handling
- Set up CI/CD pipeline

### Phase 2: Converter Implementation (Week 2-3)
- Implement StructureDefinition to FHIRSchema translator
- Handle element transformation logic
- Support for choice types, slicing, constraints
- Integration with canonical manager for dependency resolution

### Phase 3: Validation Engine (Week 3-4)
- Implement FHIRSchema validation engine
- Support for all validation rule types
- Context management for validation
- Performance optimization

### Phase 4: Golden Tests & Compatibility (Week 4-5)
- Port all golden tests from reference implementation
- Ensure compatibility with expected outputs
- Fix any discrepancies
- Performance benchmarking

### Phase 5: CLI Implementation (Week 5-6)
- Implement CLI commands
- Integration with canonical manager
- Documentation and examples
- Final testing and optimization

## Technical Specifications

### Dependencies
```toml
[dependencies]
octofhir-canonical-manager = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"
thiserror = "1.0"
lru = "0.12"  # For LRU caching
dashmap = "5.5"  # For concurrent in-memory storage
clap = { version = "4.0", features = ["derive"], optional = true }

[features]
default = []
cli = ["clap"]
disk-storage = []  # Optional disk persistence
```

### Public API Design
```rust
// Core conversion API
pub async fn convert_structure_definition(
    sd: &StructureDefinition,
    manager: &CanonicalManager,
) -> Result<FhirSchema, ConversionError>;

// Batch conversion
pub async fn convert_multiple(
    sds: &[StructureDefinition],
    manager: &CanonicalManager,
) -> Result<Vec<FhirSchema>, ConversionError>;

// Validation API
pub async fn validate_resource(
    resource: &serde_json::Value,
    schema: &FhirSchema,
    context: &ValidationContext,
) -> Result<ValidationResult, ValidationError>;

// Schema loading
pub async fn load_schema_from_url(
    url: &str,
    manager: &CanonicalManager,
) -> Result<FhirSchema, LoadError>;

// Schema storage and caching
pub struct SchemaStorage {
    memory: MemoryStorage,
    disk: Option<DiskStorage>,
    cache: LruCache<String, FhirSchema>,
}

impl SchemaStorage {
    pub fn new() -> Self;
    pub fn with_disk_storage(path: PathBuf) -> Self;
    pub async fn store(&self, url: &str, schema: FhirSchema) -> Result<(), StorageError>;
    pub async fn load(&self, url: &str) -> Result<Option<FhirSchema>, StorageError>;
    pub async fn remove(&self, url: &str) -> Result<(), StorageError>;
    pub fn clear_cache(&mut self);
}
```

### CLI Commands
```bash
# Convert StructureDefinition to FHIRSchema
octofhir-fhirschema convert input.json -o output.json

# Validate resource against schema
octofhir-fhirschema validate resource.json --schema schema.json

# Load schema from canonical URL
octofhir-fhirschema schema load "http://hl7.org/fhir/StructureDefinition/Patient"

# Batch operations
octofhir-fhirschema convert --batch input-dir/ -o output-dir/

# Cache management
octofhir-fhirschema cache clear
octofhir-fhirschema cache list
octofhir-fhirschema cache save --url "http://example.com/schema" --file schema.json
```

## Consequences

### Positive
- Clean, performant Rust implementation
- Full compatibility with reference TypeScript implementation
- Integration with existing OctoFHIR ecosystem
- Comprehensive test coverage
- Both library and CLI usage patterns

### Negative
- Initial development time investment
- Need to maintain compatibility with reference implementation
- Complex dependency management for FHIR packages

### Risks
- Golden test compatibility challenges
- Performance requirements vs. compatibility trade-offs
- Keeping up with reference implementation changes

## Alternatives Considered

1. **Fork TypeScript Implementation:** Rejected due to performance requirements and ecosystem integration needs
2. **Minimal Implementation:** Rejected due to golden test requirements
3. **CLI-Only Tool:** Rejected due to library-first requirement

## Success Criteria

1. All golden tests pass with identical output to reference implementation
2. Performance benchmarks show significant improvement over TypeScript version
3. Clean, documented public API suitable for library usage
4. Comprehensive CLI tool for standalone usage
5. Full integration with octofhir-canonical-manager
6. Published to crates.io with proper documentation

## References

- [Reference TypeScript Implementation](https://github.com/atomic-ehr/fhirschema)
- [FHIRSchema Specification](https://github.com/atomic-ehr/fhirschema/blob/main/spec/fhirschema-specification.md)
- [OctoFHIR Canonical Manager](https://crates.io/crates/octofhir-canonical-manager)
