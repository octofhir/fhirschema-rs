# Build and test commands for octofhir-fhirschema
#
# Key FHIR download commands:
#   just download-r4-core                              # Download all HL7 FHIR R4 core schemas
#   just download-r4-core-filtered "Patient,Observation"  # Download specific resource types
#   just download-package <pkg> <version> <output>     # Download any FHIR package

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
    cargo clippy --all-targets -- -D warnings

# Check formatting without fixing
check-format:
    cargo fmt -- --check

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

# Download and convert HL7 FHIR R4 core package
download-r4-core:
    @echo "🚀 Downloading and converting HL7 FHIR R4 core package..."
    mkdir -p schemas/r4-core
    cargo run --features cli -- download --package hl7.fhir.r4.core --version 4.0.1 --output schemas/r4-core
    @echo "✅ HL7 FHIR R4 core schemas saved to schemas/r4-core/"

# Download and convert HL7 FHIR R4 core package with specific resource types
download-r4-core-filtered types:
    @echo "🚀 Downloading and converting HL7 FHIR R4 core package (filtered: {{types}})..."
    mkdir -p schemas/r4-core-filtered
    cargo run --features cli -- download --package hl7.fhir.r4.core --version 4.0.1 --output schemas/r4-core-filtered --resource-types {{types}}
    @echo "✅ HL7 FHIR R4 core schemas ({{types}}) saved to schemas/r4-core-filtered/"

# Download and convert any FHIR package
download-package package version output:
    @echo "🚀 Downloading and converting FHIR package: {{package}}@{{version}}..."
    mkdir -p {{output}}
    cargo run --features cli -- download --package {{package}} --version {{version}} --output {{output}}
    @echo "✅ FHIR package {{package}}@{{version}} schemas saved to {{output}}/"

