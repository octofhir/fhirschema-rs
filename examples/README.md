# OctoFHIR Schema Examples

This directory contains comprehensive examples demonstrating how to use the octofhir-fhirschema library in various scenarios.

## Examples Overview

### 1. `embedded_provider_usage.rs`
**Lightning-Fast FHIR Schema Provider**

Demonstrates the EmbeddedModelProvider with precompiled schemas for zero I/O startup:
- Support for all FHIR versions (R4, R4B, R5, R6)
- O(1) resource type lookups
- Type hierarchy queries
- Schema retrieval
- Performance characteristics

```bash
cargo run --example embedded_provider_usage --features embedded-providers
```

### 2. `composite_provider_usage.rs` 
**Production-Ready Provider with Intelligent Fallback**

Shows the CompositeModelProvider for production applications:
- Automatic fallback chain (Embedded → Dynamic → Traditional)
- Multi-version support
- Performance optimization
- Integration patterns
- Error handling

```bash
cargo run --example composite_provider_usage --features embedded-providers,dynamic-caching
```

### 3. `integration_with_fhir_libraries.rs`
**Integration Patterns for FHIR Applications**

Comprehensive integration examples for real applications:
- Validation service integration
- Resource processing pipelines
- Multi-library coordination
- Performance optimization patterns
- Global provider patterns

```bash
cargo run --example integration_with_fhir_libraries --features embedded-providers
```

## Getting Started

### Prerequisites

1. **Build precompiled schemas** (for embedded provider examples):
```bash
just build-precompiled-schemas
```

2. **Install dependencies**:
```bash
cargo build --features embedded-providers,dynamic-caching
```

### Running Examples

All examples can be run with:
```bash
# Run specific example
cargo run --example <example_name> --features embedded-providers

# Run with all features
cargo run --example <example_name> --all-features

# Run tests for examples
cargo test --examples --features embedded-providers
```

## Key Features Demonstrated

### ⚡ Performance
- **Zero I/O startup**: Embedded schemas included in binary
- **O(1) lookups**: Hash-based resource type checking
- **Microsecond response times**: Optimized for high-throughput applications

### 🔧 Flexibility
- **Multi-version support**: R4, R4B, R5, R6 FHIR versions
- **Intelligent fallback**: Automatic provider selection
- **Custom packages**: Dynamic loading with caching

### 🛡️ Production Ready
- **Thread-safe**: Arc-wrapped providers for sharing
- **Error handling**: Graceful fallback and error recovery
- **Memory efficient**: Optimized schema storage

## Integration Patterns

### Singleton Pattern
```rust
static GLOBAL_PROVIDER: OnceCell<Arc<CompositeModelProvider>> = OnceCell::const_new();

async fn get_provider() -> &'static Arc<CompositeModelProvider> {
    GLOBAL_PROVIDER.get_or_try_init(|| async {
        CompositeModelProvider::r4().await.map(Arc::new)
    }).await.unwrap()
}
```

### Service Integration
```rust
struct FhirService {
    provider: Arc<CompositeModelProvider>,
}

impl FhirService {
    async fn validate_resource(&self, resource_type: &str) -> Result<bool> {
        self.provider.resource_type_exists(resource_type)
    }
}
```

### Multi-Version Manager
```rust
struct VersionManager {
    r4: CompositeModelProvider,
    r5: CompositeModelProvider,
}

impl VersionManager {
    fn get_provider(&self, version: FhirVersion) -> &CompositeModelProvider {
        match version {
            FhirVersion::R4 => &self.r4,
            FhirVersion::R5 => &self.r5,
            _ => &self.r4,
        }
    }
}
```

## Performance Benchmarks

Typical performance characteristics (on modern hardware):

| Operation | EmbeddedProvider | CompositeProvider |
|-----------|------------------|-------------------|
| Startup | <1ms | <10ms |
| Resource type check | ~50ns | ~100ns |
| Schema retrieval | ~1μs | ~10μs |
| Type hierarchy | ~10μs | ~50μs |

## Best Practices

### 1. Choose the Right Provider
- **EmbeddedProvider**: Maximum performance, static schemas
- **CompositeProvider**: Production flexibility, intelligent fallback
- **DynamicProvider**: Custom packages, development scenarios

### 2. Sharing Providers
- Wrap in `Arc` for thread-safe sharing
- Use global singletons for application-wide access
- Consider lazy initialization for reduced startup time

### 3. Error Handling
- Always handle provider creation errors gracefully
- Use fallback providers for robustness
- Log performance metrics for monitoring

### 4. Memory Management
- Providers are designed to be long-lived
- Share instances rather than creating multiple
- Clear caches periodically in long-running applications

## Testing

Run all example tests:
```bash
cargo test --examples --features embedded-providers,dynamic-caching
```

Each example includes comprehensive test cases demonstrating:
- Basic functionality validation
- Error handling scenarios
- Performance characteristics
- Integration patterns

## Further Reading

- [Main Documentation](../README.md)
- [Provider Architecture](../docs/providers.md)
- [Performance Guide](../docs/performance.md)
- [Integration Guide](../docs/integration.md)