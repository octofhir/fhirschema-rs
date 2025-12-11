# Test Fixtures

This directory contains test fixtures and generated reports for R4B schema verification.

## Directory Structure

```
fixtures/
├── r4b_reference/          # TypeScript reference schemas (auto-generated)
│   ├── Patient.json
│   ├── Observation.json
│   └── ...                 # ~568 R4B resource schemas
│
└── comparison_reports/     # Comparison diff reports (auto-generated)
    ├── comparison_report.html    # Human-readable HTML report
    └── comparison_report.json    # Machine-readable JSON report
```

## TypeScript Reference Schemas

Reference schemas are downloaded from `@atomic-ehr/fhirschema` npm package and generated using Node.js. These serve as the ground truth for validating our Rust implementation.

**Generation:**
```bash
# Automatic (runs during test)
cargo test --test schema_comparison test_r4b_compare_against_typescript_reference -- --ignored --nocapture

# Manual regeneration
rm -rf tests/fixtures/r4b_reference  # Force fresh download
cargo test --test schema_comparison test_r4b_compare_against_typescript_reference -- --ignored
```

**Requirements:**
- **Bun** - Install from https://bun.sh
- Much faster than Node.js/npm (~10x faster package install, ~3x faster execution)

**Cache:**
- Reference schemas are cached for 30 days
- Delete `r4b_reference/` directory to force refresh

## Comparison Reports

Reports are generated automatically when running the TypeScript comparison test.

**HTML Report (`comparison_report.html`):**
- Summary statistics (match rate, average similarity)
- Per-resource comparison results
- Top differences highlighted
- Visual indication of matches vs mismatches

**JSON Report (`comparison_report.json`):**
- Same data in machine-readable format
- Detailed difference information with types and paths
- Suitable for automated analysis and CI integration

**Viewing Reports:**
```bash
# Run comparison test
cargo test --test schema_comparison test_r4b_compare_against_typescript_reference -- --ignored --nocapture

# Open HTML report in browser
open tests/fixtures/comparison_reports/comparison_report.html

# Or check the path printed by the test output
```

## Git Ignore

Both directories are .gitignored to avoid committing large generated files. Reports are regenerated on each test run.
