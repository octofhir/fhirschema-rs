# Build and test commands for octofhir-fhirschema

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

# Format code
format:
    cargo fmt

# Check if code is formatted
format-check:
    cargo fmt -- --check

# Run clippy lints
clippy:
    cargo clippy --all-features -- -D warnings

# Fix clippy issues automatically
clippy-fix:
    cargo clippy --all-features --fix --allow-dirty --allow-staged

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

# Generate test data
gen-test-data:
    mkdir -p tests/data
    echo "Test data generation commands go here"

# Test StructureDefinition to FHIRSchema conversion
test-conversion:
    cargo build --features cli
    ./target/debug/octofhir-fhirschema convert-structure-definition \
        --input examples/simple-patient-structuredefinition.json \
        --output examples/test-converted-patient.fhirschema.json
    @echo "✓ Conversion test completed"
    @echo "Generated schema:"
    @head -20 examples/test-converted-patient.fhirschema.json
    @echo "..."
    @rm examples/test-converted-patient.fhirschema.json

# Test conversion with validation
test-conversion-validate:
    cargo build --features cli
    ./target/debug/octofhir-fhirschema convert-structure-definition \
        --input examples/simple-patient-structuredefinition.json \
        --output examples/test-converted-patient.fhirschema.json
    ./target/debug/octofhir-fhirschema validate \
        --schema examples/test-converted-patient.fhirschema.json
    ./target/debug/octofhir-fhirschema info \
        --schema examples/test-converted-patient.fhirschema.json
    @rm examples/test-converted-patient.fhirschema.json