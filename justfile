# Build and test commands for octofhir-fhirschema
#
# Core development commands:
#   just check                 # Check formatting and linting
#   just test                  # Run all tests
#   just ci                    # Run CI checks (format, lint, test, docs, audit)

# Default task - run tests and check
default: test check

# Build the project
build:
    cargo build

# Build with all features
build-all:
    cargo build --all-features

# Run tests
test:
    cargo test

# Run tests with all features
test-all:
    cargo test --all-features

# Run tests with coverage
test-coverage:
    cargo tarpaulin --out Html --output-dir coverage

# Check code formatting and linting
check: format-check clippy

# Comprehensive check - format, clippy, and compilation
check-all: format-check clippy check-compile check-tests
    @echo "✓ All checks passed"

# Check if code compiles
check-compile:
    cargo check --all-targets

# Check if tests compile
check-tests:
    cargo check --tests

# Check with strict clippy (deny warnings)
check-strict:
    cargo clippy --all-features --all-targets -- -D warnings

# Check formatting without fixing
check-format:
    cargo fmt --all -- --check

# Format code
format:
    cargo fmt --all

# Check if code is formatted
format-check:
    cargo fmt --all -- --check

# Run clippy lints
clippy:
    cargo clippy --all-features

# Fix clippy issues automatically
clippy-fix:
    cargo clippy --all-features --fix --allow-dirty --allow-staged

# Fix all format, check, and clippy issues
fix-all: format clippy-fix cargo-fix
    @echo "✓ Applied all automatic fixes"

# Run cargo fix to apply automatic code fixes
cargo-fix:
    cargo fix --all-targets --allow-dirty --allow-staged

# Fix clippy issues for specific targets
clippy-fix-lib:
    cargo clippy --fix --lib --allow-dirty --allow-staged

clippy-fix-tests:
    cargo clippy --fix --tests --allow-dirty --allow-staged

clippy-fix-all-targets:
    cargo clippy --fix --all-targets --allow-dirty --allow-staged

# Show what clippy would fix (dry run)
clippy-check:
    cargo clippy --all-targets

# Show formatting issues without fixing
format-diff:
    cargo fmt -- --check --verbose

# Build documentation
docs:
    cargo doc --all-features --no-deps --open

# Clean build artifacts
clean:
    cargo clean

# Run benchmarks
bench:
    cargo bench

# Install development dependencies
install-dev:
    cargo install cargo-tarpaulin
    cargo install cargo-audit
    cargo install cargo-outdated

# Security audit
audit:
    cargo audit

# Check for outdated dependencies
outdated:
    cargo outdated

# Create a new release (requires argument: major, minor, patch)
release version:
    cargo release {{version}}

# Run all quality checks
ci: format-check clippy test-all docs audit

# Development workflow - watch for changes
watch:
    cargo watch -x "test --all-features"

# Quick development check
dev: format clippy test

# Pre-commit checks - format, fix, check, and test
pre-commit: fix-all check-all test
    @echo "✓ Pre-commit checks completed"

# Full development cycle - fix, check, test, and verify
full-check: fix-all check-all test-all
    @echo "✓ Full development cycle completed"

# Example usage - create sample test
example-usage:
    @echo "🚀 Running example usage of the FHIR schema library..."
    cargo run --example simple_trait_test
    @echo "✅ Example completed successfully"

# Resource type extraction test
test-resource-types:
    @echo "🧪 Testing resource type extraction and O(1) checking..."
    cargo test --test resource_type_extraction_tests -- --nocapture
    @echo "✅ Resource type tests completed"

# Generate precompiled FHIR schemas for fast startup
build-precompiled-schemas:
    @echo "🚀 Building Precompiled FHIR Schemas with Real Conversion"
    @echo "========================================================"
    
    # Build the schema builder first
    @echo "🔧 Building schema-builder binary..."
    cargo build --bin schema-builder --release
    @echo "  ✅ Schema builder ready"
    
    # Run the schema builder to generate real schemas
    @echo "🏭 Generating schemas from FHIR StructureDefinitions..."
    ./target/release/schema-builder --output-dir precompiled_schemas --version all
    @echo "  ✅ Schema conversion completed"
    
    @echo ""
    @echo "✅ Precompiled schemas generation completed!"
    @echo ""
    @echo "🚀 Next steps:"
    @echo "  1. Build with: cargo build --features embedded-providers"  
    @echo "  2. Use CompositeModelProvider for best performance"
    @echo "  3. Test with: just test-embedded"

# Test the embedded provider with precompiled schemas
test-embedded:
    @echo "🧪 Testing EmbeddedModelProvider with precompiled schemas..."
    just build-precompiled-schemas
    cargo test --example embedded_provider_usage --features embedded-providers -- --nocapture
    @echo "✅ Embedded provider tests completed"

# Build with all performance optimizations
build-optimized:
    @echo "🚀 Building with all performance optimizations..."
    just build-precompiled-schemas
    cargo build --release --features embedded-providers,dynamic-caching
    @echo "✅ Optimized build completed"

# Build precompiled schemas for a specific FHIR version
build-precompiled-version version:
    @echo "🚀 Building Precompiled FHIR Schemas for {{version}}"
    @echo "=================================================="
    
    # Build the schema builder first
    @echo "🔧 Building schema-builder binary..."
    cargo build --bin schema-builder --release
    @echo "  ✅ Schema builder ready"
    
    # Run the schema builder for specific version
    @echo "🏭 Generating schemas for FHIR {{version}}..."
    ./target/release/schema-builder --output-dir precompiled_schemas --version {{version}}
    @echo "  ✅ Schema conversion completed for {{version}}"

# Clean precompiled schemas
clean-precompiled-schemas:
    @echo "🧹 Cleaning precompiled schemas..."
    rm -rf precompiled_schemas/
    @echo "  ✅ Precompiled schemas cleaned"

# Show schema statistics
schema-stats:
    #!/bin/bash
    echo "📊 Precompiled Schema Statistics"
    echo "================================"
    if [ -d precompiled_schemas ]; then
        for file in precompiled_schemas/*.bin; do
            if [ -f "$file" ]; then
                size=$(wc -c < "$file" | tr -d ' ')
                if command -v numfmt >/dev/null 2>&1; then
                    human_size=$(numfmt --to=iec-i --suffix=B "$size" 2>/dev/null || echo "$size bytes")
                else
                    human_size="$size bytes"
                fi
                filename=$(basename "$file")
                echo "  📁 $filename: $human_size"
            fi
        done
        total_size=$(du -sh precompiled_schemas 2>/dev/null | cut -f1 || echo "0")
        echo "  📦 Total: $total_size"
    else
        echo "  ❌ No precompiled schemas found. Run 'just build-precompiled-schemas' first."
    fi

