# FHIRSchema Troubleshooting Guide

This guide helps you diagnose and resolve common issues when working with FHIRSchema tools.

## Table of Contents

- [Installation Issues](#installation-issues)
- [Conversion Problems](#conversion-problems)
- [Validation Errors](#validation-errors)
- [CLI Issues](#cli-issues)
- [Performance Problems](#performance-problems)
- [Common Error Messages](#common-error-messages)
- [Getting Help](#getting-help)

## Installation Issues

### Rust Toolchain Problems

**Problem**: `rustc` or `cargo` not found
```bash
error: command not found: rustc
```

**Solution**: Install Rust using rustup:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

**Problem**: Outdated Rust version
```bash
error: package requires Rust 1.70 or newer
```

**Solution**: Update Rust:
```bash
rustup update
```

### Dependency Issues

**Problem**: Failed to compile regex crate
```bash
error: failed to compile `regex v1.x.x`
```

**Solution**: This usually indicates insufficient system resources or missing system dependencies. Try:
```bash
# Increase available memory for compilation
export CARGO_BUILD_JOBS=1
cargo build

# Or install system dependencies (Ubuntu/Debian)
sudo apt-get update
sudo apt-get install build-essential
```

## Conversion Problems

### Invalid StructureDefinition Format

**Problem**: Conversion fails with JSON parsing errors
```bash
Error: Failed to parse JSON: expected value at line 1 column 1
```

**Solutions**:
1. Validate your JSON syntax using a JSON validator
2. Ensure the file contains a valid FHIR StructureDefinition
3. Check that the `resourceType` is set to "StructureDefinition"

**Example of valid minimal StructureDefinition**:
```json
{
  "resourceType": "StructureDefinition",
  "url": "http://example.org/StructureDefinition/my-patient",
  "name": "MyPatient",
  "status": "active",
  "kind": "resource",
  "type": "Patient",
  "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
  "derivation": "constraint"
}
```

### Missing Required Fields

**Problem**: Conversion fails due to missing required fields
```bash
Error: StructureDefinition missing required field 'url'
```

**Solution**: Ensure all required fields are present:
- `resourceType`: Must be "StructureDefinition"
- `url`: Canonical URL for the profile
- `name`: Computer-friendly name
- `status`: Publication status (draft, active, retired, unknown)
- `kind`: Kind of structure (primitive-type, complex-type, resource, logical)
- `type`: Type being constrained
- `baseDefinition`: Base StructureDefinition being profiled

### Slicing Conversion Issues

**Problem**: Slicing information not converted properly
```bash
Warning: Failed to convert slicing: Discriminator missing 'type' field
```

**Solution**: Ensure slicing discriminators have required fields:
```json
{
  "slicing": {
    "discriminator": [
      {
        "type": "value",
        "path": "system"
      }
    ],
    "rules": "open"
  }
}
```

Valid discriminator types: `value`, `exists`, `pattern`, `type`, `profile`

### FHIRPath Expression Errors

**Problem**: Invalid FHIRPath expressions in constraints
```bash
Error: Invalid FHIRPath expression: Unmatched opening parenthesis
```

**Solutions**:
1. Check parentheses are balanced
2. Ensure quotes are properly closed
3. Validate FHIRPath syntax

**Common FHIRPath issues**:
- Unmatched parentheses: `name.exists() and (family.exists()`
- Unmatched quotes: `code = 'active`
- Invalid operators: `name == 'test'` (use `=` instead of `==`)

## Validation Errors

### Schema Not Found

**Problem**: Validation fails because schema cannot be loaded
```bash
Error: Schema not found: http://example.org/StructureDefinition/my-patient
```

**Solutions**:
1. Check the schema file path is correct
2. Ensure the schema URL matches the file content
3. Verify file permissions allow reading

### Resource Type Mismatch

**Problem**: Resource doesn't match expected type
```bash
Error: Expected resource type 'Patient', found 'Observation'
```

**Solution**: Ensure the resource being validated matches the schema's target type.

### Constraint Violations

**Problem**: Resource fails constraint validation
```bash
❌ Validation failed: [ERROR] name-1 at Patient.name: Name must be present
```

**Solutions**:
1. Review the constraint requirements in the schema
2. Modify the resource to satisfy the constraints
3. Check if the constraint is correctly defined in the profile

## CLI Issues

### Command Not Found

**Problem**: `fhirschema` command not available
```bash
command not found: fhirschema
```

**Solutions**:
1. Install the CLI: `cargo install --path crates/fhirschema-cli`
2. Add cargo bin to PATH: `export PATH="$HOME/.cargo/bin:$PATH"`
3. Use cargo run: `cargo run --bin fhirschema -- --help`

### File Permission Errors

**Problem**: Cannot read input files or write output files
```bash
Error: Permission denied (os error 13)
```

**Solutions**:
1. Check file permissions: `ls -la input-file.json`
2. Ensure read permissions: `chmod +r input-file.json`
3. Ensure write permissions for output directory: `chmod +w output-directory`

### Shell Completion Not Working

**Problem**: Tab completion doesn't work after installing completions

**Solutions**:
1. Generate completions: `fhirschema completion bash > ~/.bash_completion.d/fhirschema`
2. Reload shell: `source ~/.bashrc` or restart terminal
3. For zsh: `fhirschema completion zsh > ~/.zsh/completions/_fhirschema`

## Performance Problems

### Slow Conversion

**Problem**: Large StructureDefinitions take too long to convert

**Solutions**:
1. Use batch conversion for multiple files: `fhirschema convert --batch --input ./profiles/`
2. Increase available memory: `export RUST_MIN_STACK=8388608`
3. Consider breaking large profiles into smaller ones

### Memory Usage

**Problem**: High memory usage during conversion
```bash
Error: memory allocation of X bytes failed
```

**Solutions**:
1. Process files individually instead of batch processing
2. Increase system memory or swap space
3. Use streaming processing for very large files

### Benchmark Performance

**Problem**: Want to measure conversion performance

**Solution**: Run benchmarks:
```bash
cd crates/fhirschema-converter
cargo bench
```

View HTML reports in `target/criterion/` directory.

## Common Error Messages

### "Invalid canonical URL format"

**Cause**: Malformed canonical URL in StructureDefinition
**Fix**: Ensure URL follows pattern: `http://domain/StructureDefinition/name`

### "Circular reference detected"

**Cause**: StructureDefinition references create a circular dependency
**Fix**: Review base definitions and remove circular references

### "Unsupported FHIR version"

**Cause**: StructureDefinition uses unsupported FHIR version
**Fix**: Currently supports FHIR R4 (4.0.1). Convert to supported version.

### "Element path not found"

**Cause**: Element path in differential doesn't match base definition
**Fix**: Verify element paths match the base StructureDefinition

### "Constraint key already exists"

**Cause**: Duplicate constraint keys in element definition
**Fix**: Ensure constraint keys are unique within each element

## Debug Mode

Enable debug logging for more detailed error information:

```bash
# Set log level
export RUST_LOG=debug

# Or use verbose flag
fhirschema --verbose convert --input profile.json
```

## Validation Debug

For validation issues, use verbose mode to see detailed validation steps:

```bash
fhirschema validate --verbose --schema schema.yaml --resource resource.json
```

## Getting Help

### Check Version and Features

```bash
fhirschema --version
```

### View Available Commands

```bash
fhirschema --help
fhirschema convert --help
fhirschema validate --help
```

### Enable Detailed Logging

```bash
RUST_LOG=trace fhirschema convert --input profile.json
```

### Report Issues

When reporting issues, please include:

1. **Version information**: `fhirschema --version`
2. **Command used**: Full command line with arguments
3. **Input files**: Sample StructureDefinition or resource (anonymized)
4. **Error output**: Complete error message and stack trace
5. **Environment**: OS, Rust version (`rustc --version`)

### Community Resources

- **GitHub Issues**: Report bugs and feature requests
- **Documentation**: Check the docs/ directory for detailed guides
- **Examples**: See examples/ directory for usage patterns

### Performance Profiling

For performance issues, you can profile the application:

```bash
# Install profiling tools
cargo install flamegraph

# Profile conversion
cargo flamegraph --bin fhirschema -- convert --input large-profile.json

# View flamegraph.svg in browser
```

### Memory Profiling

```bash
# Install valgrind (Linux)
sudo apt-get install valgrind

# Profile memory usage
valgrind --tool=massif cargo run --bin fhirschema -- convert --input profile.json

# Analyze with ms_print
ms_print massif.out.* > memory-profile.txt
```

## Advanced Troubleshooting

### Custom Logging Configuration

Create a custom logging configuration:

```bash
# Create log config file
cat > log4rs.yaml << EOF
appenders:
  stdout:
    kind: console
    encoder:
      pattern: "{d} [{l}] {m}{n}"
  file:
    kind: file
    path: "fhirschema.log"
    encoder:
      pattern: "{d} [{l}] {t} - {m}{n}"
root:
  level: debug
  appenders:
    - stdout
    - file
EOF

# Use custom logging
RUST_LOG_CONFIG=log4rs.yaml fhirschema convert --input profile.json
```

### Environment Variables

Useful environment variables for troubleshooting:

- `RUST_LOG`: Set logging level (error, warn, info, debug, trace)
- `RUST_BACKTRACE`: Show stack traces (0, 1, full)
- `RUST_MIN_STACK`: Increase stack size for deep recursion
- `CARGO_BUILD_JOBS`: Limit parallel compilation jobs

### Testing Your Setup

Verify your installation with the test suite:

```bash
# Run all tests
cargo test

# Run specific test suite
cargo test --package fhirschema-converter

# Run benchmarks
cargo bench --package fhirschema-converter
```

This troubleshooting guide should help you resolve most common issues. If you encounter problems not covered here, please check the documentation or report an issue with detailed information about your environment and the problem you're experiencing.
