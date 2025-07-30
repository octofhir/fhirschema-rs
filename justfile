# FHIRSchema Conversion Demonstrations
# This justfile contains commands to demonstrate real conversion from FHIR StructureDefinition to FHIRSchema

# Default recipe - show all available commands
default:
    @just --list

# Build the CLI tool
build:
    cargo build --release --bin fhirschema

# Load default FHIR R4 core package and convert all StructureDefinitions to FHIRSchema
load-r4-core: build
    @echo "🔄 Downloading FHIR R4 core package (v4.0.1) and converting to FHIRSchema..."
    ./target/release/fhirschema download --package "hl7.fhir.r4.core#4.0.1" --format ndjson --convert --skip-download
    @echo "✅ FHIR R4 core package conversion completed!"
    @echo "   📁 Output directory: .output/"
    @echo "   📄 Schemas file: .output/packages/hl7.fhir.r4.core-4.0.1/converted/schemas.ndjson"
    @echo "   📄 Raw files: .output/packages/hl7.fhir.r4.core-4.0.1/raw/ (658 files)"
    @echo "   📄 Metadata: .output/packages/hl7.fhir.r4.core-4.0.1/metadata/summary.json"
    @echo ""
    @echo "💡 All files are organized in .output/ directory for easy debugging:"
    @echo "   - Raw StructureDefinitions: .output/packages/*/raw/"
    @echo "   - Converted FHIRSchemas: .output/packages/*/converted/"
    @echo "   - Package metadata: .output/packages/*/metadata/"
    @echo "   - Original packages: .output/downloads/packages/"

# Force re-download FHIR R4 core package (ignores existing local copy)
load-r4-core-fresh: build
    @echo "🔄 Force downloading FHIR R4 core package (v4.0.1) and converting to FHIRSchema..."
    ./target/release/fhirschema download --package "hl7.fhir.r4.core#4.0.1" --format ndjson --convert
    @echo "✅ FHIR R4 core package conversion completed!"
    @echo "   📁 Output directory: .output/"
    @echo "   📄 Schemas file: .output/packages/hl7.fhir.r4.core-4.0.1/converted/schemas.ndjson"

# Load FHIR R5 core package and convert all StructureDefinitions to FHIRSchema
load-r5-core: build
    @echo "🔄 Downloading FHIR R5 core package (v5.0.0) and converting to FHIRSchema..."
    ./target/release/fhirschema download --package "hl7.fhir.r5.core#5.0.0" --format ndjson --convert --skip-download
    @echo "✅ FHIR R5 core package conversion completed!"
    @echo "   📁 Output directory: .output/"
    @echo "   📄 Schemas file: .output/packages/hl7.fhir.r5.core-5.0.0/converted/schemas.ndjson"
    @echo "   📄 Raw files: .output/packages/hl7.fhir.r5.core-5.0.0/raw/"
    @echo "   📄 Metadata: .output/packages/hl7.fhir.r5.core-5.0.0/metadata/summary.json"
    @echo ""
    @echo "💡 All files are organized in .output/ directory for easy debugging"

# Force re-download FHIR R5 core package (ignores existing local copy)
load-r5-core-fresh: build
    @echo "🔄 Force downloading FHIR R5 core package (v5.0.0) and converting to FHIRSchema..."
    ./target/release/fhirschema download --package "hl7.fhir.r5.core#5.0.0" --format ndjson --convert
    @echo "✅ FHIR R5 core package conversion completed!"
    @echo "   📁 Output directory: .output/"
    @echo "   📄 Schemas file: .output/packages/hl7.fhir.r5.core-5.0.0/converted/schemas.ndjson"

# Show help for loading custom FHIR packages
load-package: build
    @echo "Usage: just load-custom-package PACKAGE_ID VERSION"
    @echo "Example: just load-custom-package hl7.fhir.us.core 6.1.0"
    @echo ""
    @echo "Available core packages:"
    @echo "  - hl7.fhir.r4.core#4.0.1    (FHIR R4)"
    @echo "  - hl7.fhir.r5.core#5.0.0    (FHIR R5)"
    @echo "  - hl7.fhir.us.core#6.1.0    (US Core)"
    @echo "  - hl7.fhir.uv.ips#1.1.0     (International Patient Summary)"
    @echo ""
    @echo "💡 All packages are saved to .output/ directory with organized structure"

# Load custom FHIR package with parameters (uses skip download for efficiency)
load-custom-package PACKAGE_ID VERSION: build
    @echo "🔄 Downloading FHIR package {{PACKAGE_ID}}#{{VERSION}} and converting to FHIRSchema..."
    ./target/release/fhirschema download --package "{{PACKAGE_ID}}#{{VERSION}}" --format ndjson --convert --skip-download
    @echo "✅ FHIR package {{PACKAGE_ID}}#{{VERSION}} conversion completed!"
    @echo "   📁 Output directory: .output/"
    @echo "   📄 Schemas file: .output/packages/{{PACKAGE_ID}}-{{VERSION}}/converted/schemas.ndjson"
    @echo "   📄 Raw files: .output/packages/{{PACKAGE_ID}}-{{VERSION}}/raw/"
    @echo "   📄 Metadata: .output/packages/{{PACKAGE_ID}}-{{VERSION}}/metadata/summary.json"

# Force re-download custom FHIR package (ignores existing local copy)
load-custom-package-fresh PACKAGE_ID VERSION: build
    @echo "🔄 Force downloading FHIR package {{PACKAGE_ID}}#{{VERSION}} and converting to FHIRSchema..."
    ./target/release/fhirschema download --package "{{PACKAGE_ID}}#{{VERSION}}" --format ndjson --convert
    @echo "✅ FHIR package {{PACKAGE_ID}}#{{VERSION}} conversion completed!"
    @echo "   📁 Output directory: .output/"
    @echo "   📄 Schemas file: .output/packages/{{PACKAGE_ID}}-{{VERSION}}/converted/schemas.ndjson"

# Show statistics for loaded schemas
show-r4-stats:
    @if [ -f ".output/packages/hl7.fhir.r4.core-4.0.1/converted/schemas.ndjson" ]; then \
        echo "📊 FHIR R4 Core Package Statistics:"; \
        echo "================================"; \
        echo "Total schemas: `wc -l < .output/packages/hl7.fhir.r4.core-4.0.1/converted/schemas.ndjson`"; \
        echo "File size: `du -h .output/packages/hl7.fhir.r4.core-4.0.1/converted/schemas.ndjson | cut -f1`"; \
        echo "Raw files: `find .output/packages/hl7.fhir.r4.core-4.0.1/raw -name '*.json' | wc -l` StructureDefinitions"; \
        echo ""; \
        echo "Schema types breakdown:"; \
        grep -o '"type":"[^"]*"' .output/packages/hl7.fhir.r4.core-4.0.1/converted/schemas.ndjson | sort | uniq -c | sort -nr | head -10; \
        echo ""; \
        echo "📁 Files location: .output/packages/hl7.fhir.r4.core-4.0.1/"; \
    else \
        echo "❌ No R4 schemas found. Run 'just load-r4-core' first."; \
    fi

# Show statistics for R5 schemas
show-r5-stats:
    @if [ -f ".output/packages/hl7.fhir.r5.core-5.0.0/converted/schemas.ndjson" ]; then \
        echo "📊 FHIR R5 Core Package Statistics:"; \
        echo "================================"; \
        echo "Total schemas: `wc -l < .output/packages/hl7.fhir.r5.core-5.0.0/converted/schemas.ndjson`"; \
        echo "File size: `du -h .output/packages/hl7.fhir.r5.core-5.0.0/converted/schemas.ndjson | cut -f1`"; \
        echo "Raw files: `find .output/packages/hl7.fhir.r5.core-5.0.0/raw -name '*.json' | wc -l` StructureDefinitions"; \
        echo ""; \
        echo "Schema types breakdown:"; \
        grep -o '"type":"[^"]*"' .output/packages/hl7.fhir.r5.core-5.0.0/converted/schemas.ndjson | sort | uniq -c | sort -nr | head -10; \
        echo ""; \
        echo "📁 Files location: .output/packages/hl7.fhir.r5.core-5.0.0/"; \
    else \
        echo "❌ No R5 schemas found. Run 'just load-r5-core' first."; \
    fi

# Load all core packages
load-all-core: load-r4-core load-r5-core
    @echo "🎉 All core FHIR packages loaded and converted!"
    @echo ""
    @echo "📊 Summary:"
    @echo "==========="
    @if [ -f ".output/packages/hl7.fhir.r4.core-4.0.1/converted/schemas.ndjson" ]; then echo "R4 schemas: `wc -l < .output/packages/hl7.fhir.r4.core-4.0.1/converted/schemas.ndjson`"; fi
    @if [ -f ".output/packages/hl7.fhir.r5.core-5.0.0/converted/schemas.ndjson" ]; then echo "R5 schemas: `wc -l < .output/packages/hl7.fhir.r5.core-5.0.0/converted/schemas.ndjson`"; fi
    @echo ""
    @echo "📁 All files organized in .output/ directory:"
    @echo "   - Converted schemas: .output/packages/*/converted/"
    @echo "   - Raw StructureDefinitions: .output/packages/*/raw/"
    @echo "   - Package metadata: .output/packages/*/metadata/"
    @echo "   - Original packages: .output/downloads/packages/"

# Run all conversion demonstrations
demo-all: demo-basic demo-choice-types demo-constraints demo-references
    @echo "✅ All conversion demonstrations completed!"

# Demonstrate basic StructureDefinition conversion
demo-basic:
    @echo "🔄 Converting basic Patient profile..."
    ./target/release/fhirschema convert -i test-structuredefinition.json -f yaml -o examples/basic-patient.fhirschema.yaml
    ./target/release/fhirschema convert -i test-structuredefinition.json -f json -o examples/basic-patient.fhirschema.json
    @echo "✅ Basic conversion completed"
    @echo "   📄 YAML output: examples/basic-patient.fhirschema.yaml"
    @echo "   📄 JSON output: examples/basic-patient.fhirschema.json"

# Demonstrate choice types conversion
demo-choice-types:
    @echo "🔄 Converting Patient profile with choice types..."
    ./target/release/fhirschema convert -i examples/patient-with-choice-types.json -f yaml -o examples/choice-types.fhirschema.yaml
    ./target/release/fhirschema convert -i examples/patient-with-choice-types.json -f json -o examples/choice-types.fhirschema.json
    @echo "✅ Choice types conversion completed"
    @echo "   📄 YAML output: examples/choice-types.fhirschema.yaml"
    @echo "   📄 JSON output: examples/choice-types.fhirschema.json"

# Demonstrate constraints and bindings conversion
demo-constraints:
    @echo "🔄 Converting Patient profile with constraints and bindings..."
    ./target/release/fhirschema convert -i examples/patient-with-constraints.json -f yaml -o examples/constraints.fhirschema.yaml
    ./target/release/fhirschema convert -i examples/patient-with-constraints.json -f json -o examples/constraints.fhirschema.json
    @echo "✅ Constraints and bindings conversion completed"
    @echo "   📄 YAML output: examples/constraints.fhirschema.yaml"
    @echo "   📄 JSON output: examples/constraints.fhirschema.json"

# Demonstrate reference types conversion
demo-references:
    @echo "🔄 Converting Patient profile with reference types..."
    ./target/release/fhirschema convert -i examples/patient-with-references.json -f yaml -o examples/references.fhirschema.yaml
    ./target/release/fhirschema convert -i examples/patient-with-references.json -f json -o examples/references.fhirschema.json
    @echo "✅ Reference types conversion completed"
    @echo "   📄 YAML output: examples/references.fhirschema.yaml"
    @echo "   📄 JSON output: examples/references.fhirschema.json"

# Show before/after comparison for basic example
show-basic-comparison:
    @echo "📋 ORIGINAL StructureDefinition (test-structuredefinition.json):"
    @echo "================================================================"
    @cat test-structuredefinition.json | jq '.'
    @echo ""
    @echo "📋 CONVERTED FHIRSchema (YAML):"
    @echo "==============================="
    @cat examples/basic-patient.fhirschema.yaml
    @echo ""

# Show before/after comparison for choice types
show-choice-types-comparison:
    @echo "📋 ORIGINAL StructureDefinition (patient-with-choice-types.json):"
    @echo "=================================================================="
    @cat examples/patient-with-choice-types.json | jq '.'
    @echo ""
    @echo "📋 CONVERTED FHIRSchema (YAML):"
    @echo "==============================="
    @cat examples/choice-types.fhirschema.yaml
    @echo ""

# Show before/after comparison for constraints
show-constraints-comparison:
    @echo "📋 ORIGINAL StructureDefinition (patient-with-constraints.json):"
    @echo "================================================================="
    @cat examples/patient-with-constraints.json | jq '.'
    @echo ""
    @echo "📋 CONVERTED FHIRSchema (YAML):"
    @echo "==============================="
    @cat examples/constraints.fhirschema.yaml
    @echo ""

# Show before/after comparison for references
show-references-comparison:
    @echo "📋 ORIGINAL StructureDefinition (patient-with-references.json):"
    @echo "==============================================================="
    @cat examples/patient-with-references.json | jq '.'
    @echo ""
    @echo "📋 CONVERTED FHIRSchema (YAML):"
    @echo "==============================="
    @cat examples/references.fhirschema.yaml
    @echo ""

# Show all comparisons
show-all-comparisons: show-basic-comparison show-choice-types-comparison show-constraints-comparison show-references-comparison
    @echo "✅ All comparisons shown!"

# Clean up generated files
clean:
    rm -f examples/*.fhirschema.yaml examples/*.fhirschema.json
    rm -rf .output
    @echo "🧹 Cleaned up generated FHIRSchema files and .output directory"
    @echo "   Removed: examples/*.fhirschema.* and .output/ (all packages, converted files, raw files, metadata)"

# Run tests to ensure everything works
test:
    cargo test --quiet
    @echo "✅ All tests passed!"

# Full demonstration workflow: build, test, convert, and show results
full-demo: build test demo-all
    @echo ""
    @echo "🎉 Full FHIRSchema conversion demonstration completed!"
    @echo ""
    @echo "📁 Generated files:"
    @ls -la examples/*.fhirschema.* 2>/dev/null || echo "   No files generated yet - run 'just demo-all' first"
    @echo ""
    @echo "💡 Try these commands:"
    @echo ""
    @echo "📦 Package Loading (saves to .output/ directory):"
    @echo "   just load-r4-core              - Download and convert FHIR R4 core package (skip if exists)"
    @echo "   just load-r4-core-fresh        - Force re-download FHIR R4 core package"
    @echo "   just load-r5-core              - Download and convert FHIR R5 core package (skip if exists)"
    @echo "   just load-r5-core-fresh        - Force re-download FHIR R5 core package"
    @echo "   just load-all-core             - Load both R4 and R5 core packages"
    @echo "   just load-custom-package PKG VER - Load custom package with version (skip if exists)"
    @echo "   just load-custom-package-fresh PKG VER - Force re-download custom package"
    @echo ""
    @echo "📊 Statistics:"
    @echo "   just show-r4-stats             - Show R4 package statistics"
    @echo "   just show-r5-stats             - Show R5 package statistics"
    @echo ""
    @echo "🔍 Comparisons:"
    @echo "   just show-basic-comparison     - See before/after for basic example"
    @echo "   just show-choice-types-comparison - See choice types conversion"
    @echo "   just show-constraints-comparison  - See constraints and bindings"
    @echo "   just show-references-comparison   - See reference types conversion"
    @echo "   just show-all-comparisons      - See all comparisons"
    @echo ""
    @echo "🗂️  All files organized in .output/ directory:"
    @echo "   - Converted schemas: .output/packages/*/converted/"
    @echo "   - Raw StructureDefinitions: .output/packages/*/raw/"
    @echo "   - Package metadata: .output/packages/*/metadata/"
    @echo "   - Original packages: .output/downloads/packages/"

# Quick demo with immediate comparison
quick-demo: build demo-basic show-basic-comparison
    @echo "🚀 Quick demo completed! This shows the basic conversion from StructureDefinition to FHIRSchema."
