# Build and test commands for octofhir-fhirschema
#
# Core development commands:
#   just check                 # Check formatting and linting
#   just test                  # Run all tests
#   just ci                    # Run CI checks (format, lint, test, docs)
#   just generate-schemas      # Generate precompiled FHIR schemas

# Default task
default: test check

# Build the project
build:
    cargo build

# Run tests
test:
    cargo test

# Check code formatting and linting
check: format-check clippy

# Format code
format:
    cargo fmt --all

# Check if code is formatted
format-check:
    cargo fmt --all -- --check

# Run clippy lints
clippy:
    cargo clippy --all-features

# Fix all format and clippy issues
fix-all: format
    cargo clippy --all-features --fix --allow-dirty --allow-staged
    cargo fix --all-targets --allow-dirty --allow-staged

# Build documentation
docs:
    cargo doc --all-features --no-deps --open

# Clean build artifacts
clean:
    cargo clean

# Run all quality checks
ci: format-check clippy test docs

# Generate precompiled FHIR schemas for all versions (default)
generate-schemas:
    @echo "🚀 Generating Precompiled FHIR Schemas (All Versions)"
    @echo "===================================================="

    # Build the schema generator first
    @echo "🔧 Building schema-generator binary..."
    cargo build --bin schema-generator --release -p octofhir-fhirschema-devtools
    @echo "  ✅ Schema generator ready"

    @echo "🏭 Generating schemas for all FHIR versions..."
    ./target/release/schema-generator --all-versions --output octofhir-fhirschema/precompiled_schemas

    @echo ""
    @echo "Generated files:"
    @ls -la octofhir-fhirschema/precompiled_schemas/*.json || echo "No .json files found"

# Generate schemas for a specific FHIR version
generate-schemas-version version:
    @echo "🚀 Generating Precompiled FHIR Schemas for {{version}}"
    @echo "===================================================="

    @echo "🔧 Building schema-generator binary..."
    cargo build --bin schema-generator --release -p octofhir-fhirschema-devtools
    @echo "  ✅ Schema generator ready"

    @echo "🏭 Generating schemas for FHIR {{version}}..."
    ./target/release/schema-generator --version {{version}} --output octofhir-fhirschema/precompiled_schemas
    @echo "  ✅ Schema conversion completed for {{version}}"

# Generate schemas with only core resource types (faster)
generate-schemas-core:
    @echo "🚀 Generating Core FHIR Schemas (Core Resources Only)"
    @echo "===================================================="

    @echo "🔧 Building schema-generator binary..."
    cargo build --bin schema-generator --release -p octofhir-fhirschema-devtools
    @echo "  ✅ Schema generator ready"

    @echo "🏭 Generating core schemas for all FHIR versions..."
    ./target/release/schema-generator --all-versions --core-only --output octofhir-fhirschema/precompiled_schemas

    @echo ""
    @echo "✅ Core schemas generation completed!"

# Generate schemas as individual JSON files (for debugging)
generate-schemas-individual version="r4":
    @echo "🚀 Generating Individual Schema Files for FHIR {{version}}"
    @echo "======================================================="

    @echo "🔧 Building schema-generator binary..."
    cargo build --bin schema-generator --release -p octofhir-fhirschema-devtools
    @echo "  ✅ Schema generator ready"

    @echo "🏭 Generating individual schemas for FHIR {{version}}..."
    ./target/release/schema-generator --version {{version}} --output schema_output --individual
    @echo "  ✅ Individual schema files generated in schema_output/{{version}}_schemas/"

# Clean precompiled schemas
clean-schemas:
    @echo "🧹 Cleaning precompiled schemas..."
    rm -rf octofhir-fhirschema/precompiled_schemas/
    rm -rf schema_output/
    @echo "  ✅ Schemas cleaned"

# Show schema statistics
schema-stats:
    #!/bin/bash
    echo "📊 Precompiled Schema Statistics"
    echo "================================"
    if [ -d octofhir-fhirschema/precompiled_schemas ]; then
        for file in octofhir-fhirschema/precompiled_schemas/*.json; do
            if [ -f "$file" ]; then
                size=$(wc -c < "$file" | tr -d ' ')
                if command -v numfmt >/dev/null 2>&1; then
                    human_size=$(numfmt --to=iec-i --suffix=B "$size" 2>/dev/null || echo "$size bytes")
                else
                    human_size="$size bytes"
                fi
                filename=$(basename "$file")
                # Extract schema count from JSON file
                if command -v jq >/dev/null 2>&1; then
                    count=$(jq 'length' "$file" 2>/dev/null || echo "?")
                    echo "  📁 $filename: $human_size ($count schemas)"
                else
                    echo "  📁 $filename: $human_size"
                fi
            fi
        done
        total_size=$(du -sh octofhir-fhirschema/precompiled_schemas 2>/dev/null | cut -f1 || echo "0")
        echo "  📦 Total: $total_size"
        echo ""
        echo "💡 Install 'jq' to see schema counts: brew install jq"
    else
        echo "  ❌ No precompiled schemas found. Run 'just generate-schemas' first."
    fi

# Test the embedded schemas functionality
test-embedded:
    @echo "🧪 Testing embedded schemas functionality..."
    just generate-schemas
    cargo test --lib embedded::tests -- --nocapture
    @echo "✅ Embedded schema tests completed"

