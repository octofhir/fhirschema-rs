# ADR-002: Module Organization

## Status

Accepted

## Context

The octofhir-fhirschema crate grew organically, resulting in:
- Large monolithic files (validation.rs ~2,800 lines, types.rs ~550 lines)
- Scattered provider implementations across multiple files
- Unclear module boundaries

This made the codebase harder to:
- Navigate and understand
- Maintain and extend
- Test in isolation

## Decision

**Reorganize the crate into clear module hierarchies.**

### New Structure

```
src/
├── types/
│   ├── mod.rs           # Re-exports
│   ├── schema.rs        # FhirSchema, FhirSchemaElement
│   ├── validation.rs    # ValidationResult, ValidationError
│   └── structure_definition.rs  # StructureDefinition types
├── provider/
│   ├── mod.rs           # Re-exports
│   ├── model_provider.rs     # FhirSchemaModelProvider
│   ├── validation_provider.rs # FhirSchemaValidationProvider
│   └── builder.rs       # ValidationProviderBuilder
├── validation/
│   └── mod.rs           # FhirSchemaValidator, validation logic
├── embedded.rs          # Precompiled schema loading
├── error.rs             # Error types
├── converter.rs         # StructureDefinition conversion
└── lib.rs               # Public API exports
```

### Key Changes

1. **types/** - All type definitions grouped by domain
2. **provider/** - All provider implementations with builder pattern
3. **validation/** - Validation engine (kept together due to interdependencies)

### Public API

All commonly used types are re-exported at the crate root:

```rust
// Users can import directly
use octofhir_fhirschema::{
    FhirSchema, ValidationResult, ValidationProviderBuilder,
    FhirSchemaValidator, FhirVersion
};
```

## Consequences

### Positive

1. **Clarity**: Clear module boundaries and responsibilities
2. **Discoverability**: Related code is grouped together
3. **Maintainability**: Easier to modify without affecting unrelated code
4. **Documentation**: Module-level docs provide better overview

### Negative

1. **Migration effort**: Breaking changes for code using internal paths
2. **Import changes**: Some paths changed (mitigated by crate root re-exports)

### Breaking Changes

- `crate::model_provider` → `crate::provider::model_provider`
- `crate::validation_provider` → `crate::provider::validation_provider`
- `crate::types` → `crate::types::schema`, `crate::types::validation`, etc.

These are mitigated by re-exports at `crate::` level.

## Implementation

1. Create new directory structure
2. Extract types into submodules
3. Move provider implementations
4. Add ValidationProviderBuilder
5. Update lib.rs exports
6. Add module documentation

## References

- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
- Rust Module System: https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html
