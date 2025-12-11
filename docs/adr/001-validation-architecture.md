# ADR-001: Async-First Validation API

## Status

Accepted

## Context

The octofhir-fhirschema crate needs to provide a validation API that can:
1. Validate FHIR resources against schemas
2. Evaluate FHIRPath constraint expressions
3. Integrate with external services (terminology validation, profile resolution)

FHIRPath evaluation is inherently asynchronous because:
- Expressions may need to resolve external references
- Complex expressions benefit from non-blocking execution
- Integration with async FHIRPath evaluators is required

## Decision

**All validation methods are async functions.**

The validation API is designed async-first:

```rust
impl FhirSchemaValidator {
    pub async fn validate(&self, resource: &JsonValue, schema_urls: Vec<String>) -> ValidationResult;
    pub async fn validate_with_profiles(&self, resource: &JsonValue, profile_urls: Vec<String>) -> ValidationResult;
}
```

The `FhirPathEvaluator` trait from octofhir-fhir-model is async:

```rust
#[async_trait]
pub trait FhirPathEvaluator: Send + Sync {
    async fn validate_constraints(&self, resource: &JsonValue, constraints: &[FhirPathConstraint]) -> Result<ConstraintValidationResult>;
}
```

## Consequences

### Positive

1. **Non-blocking validation**: Long-running validations don't block the thread
2. **FHIRPath integration**: Native support for async FHIRPath evaluators
3. **Extensibility**: Easy to add async terminology validation in the future
4. **Consistency**: Single API pattern (no sync/async variants to maintain)

### Negative

1. **Runtime dependency**: Requires an async runtime (tokio recommended)
2. **API complexity**: Callers must use `.await` or be in async context
3. **Test complexity**: Tests must use `#[tokio::test]`

### Mitigations

1. For sync-only contexts, use `tokio::runtime::Runtime::block_on()`
2. Provide convenience methods in the builder that handle common patterns
3. Document async requirements clearly in examples

## Example Usage

```rust
// Async context
let result = validator.validate(&resource, vec!["Patient".to_string()]).await;

// Sync context (not recommended)
let rt = tokio::runtime::Runtime::new()?;
let result = rt.block_on(validator.validate(&resource, vec!["Patient".to_string()]));
```

## References

- FHIR Schema Specification: https://fhir-schema.github.io/
- FHIRPath Specification: http://hl7.org/fhirpath/
- Rust async book: https://rust-lang.github.io/async-book/
