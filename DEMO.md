# FHIRSchema Conversion Demonstrations

This document explains how to use the demonstration scripts and justfile commands to showcase real conversion from FHIR StructureDefinition to FHIRSchema format.

## Prerequisites

- [Just](https://github.com/casey/just) command runner installed
- Rust toolchain (for building the CLI tool)
- `jq` command-line JSON processor (for pretty-printing comparisons)

## Quick Start

To see the conversion in action immediately:

```bash
just quick-demo
```

This will:
1. Build the CLI tool
2. Convert the basic test StructureDefinition to FHIRSchema
3. Show a before/after comparison

## Available Commands

### Build and Test Commands

- `just build` - Build the CLI tool in release mode
- `just test` - Run all tests to ensure everything works
- `just clean` - Clean up generated FHIRSchema files

### Conversion Demonstrations

- `just demo-all` - Run all conversion demonstrations
- `just demo-basic` - Convert basic Patient profile
- `just demo-choice-types` - Convert Patient profile with choice types (deceased[x], multipleBirth[x])
- `just demo-constraints` - Convert Patient profile with FHIRPath constraints and bindings
- `just demo-references` - Convert Patient profile with reference types and target profiles

### Before/After Comparisons

- `just show-all-comparisons` - Show all before/after comparisons
- `just show-basic-comparison` - Show basic conversion comparison
- `just show-choice-types-comparison` - Show choice types conversion comparison
- `just show-constraints-comparison` - Show constraints and bindings comparison
- `just show-references-comparison` - Show reference types comparison

### Complete Workflows

- `just full-demo` - Complete workflow: build, test, convert all examples, and show results
- `just quick-demo` - Quick demonstration with basic example and immediate comparison

## Example Files

The demonstration uses several example StructureDefinition files located in the `examples/` directory:

### 1. Basic Patient Profile (`test-structuredefinition.json`)
- Simple Patient profile with required name and gender
- Demonstrates basic element conversion and terminology bindings

### 2. Choice Types (`examples/patient-with-choice-types.json`)
- Patient profile with choice type elements
- Demonstrates conversion of `deceased[x]` (boolean|dateTime) and `multipleBirth[x]` (boolean|integer)

### 3. Constraints and Bindings (`examples/patient-with-constraints.json`)
- Patient profile with FHIRPath constraints
- Demonstrates constraint conversion and different binding strengths (required, extensible)
- Shows element-level and resource-level constraints

### 4. Reference Types (`examples/patient-with-references.json`)
- Patient profile with various reference types
- Demonstrates single and multiple target profile references
- Shows nested reference elements

## Sample Output

### Original StructureDefinition (JSON)
```json
{
  "resourceType": "StructureDefinition",
  "url": "http://example.org/StructureDefinition/test-patient",
  "name": "TestPatient",
  "type": "Patient",
  "baseDefinition": "http://hl7.org/fhir/StructureDefinition/Patient",
  "derivation": "constraint",
  "differential": {
    "element": [
      {
        "id": "Patient.name",
        "path": "Patient.name",
        "min": 1,
        "max": "*",
        "type": [{"code": "HumanName"}]
      }
    ]
  }
}
```

### Converted FHIRSchema (YAML)
```yaml
url: http://example.org/StructureDefinition/test-patient
type: Patient
name: TestPatient
derivation: constraint
base: http://hl7.org/fhir/StructureDefinition/Patient
elements:
  Patient.name:
    min: 1
    max: '*'
    type: HumanName
    short: Patient name (required)
    definition: The name of the patient - required in this profile
```

## Generated Files

After running the demonstrations, you'll find the converted files in the `examples/` directory:

- `basic-patient.fhirschema.yaml` / `basic-patient.fhirschema.json`
- `choice-types.fhirschema.yaml` / `choice-types.fhirschema.json`
- `constraints.fhirschema.yaml` / `constraints.fhirschema.json`
- `references.fhirschema.yaml` / `references.fhirschema.json`

## Key Features Demonstrated

### 1. Choice Types Conversion
- FHIR choice types like `deceased[x]` are converted to FHIRSchema choice structures
- Multiple type options are preserved with proper choice semantics

### 2. Constraint Conversion
- FHIRPath constraints are converted with key, expression, human description, and severity
- Both element-level and resource-level constraints are supported

### 3. Terminology Bindings
- ValueSet references are preserved
- Binding strength (required, extensible, preferred, example) is maintained
- Binding descriptions are included

### 4. Reference Types
- Target profile constraints are converted to `refers` properties
- Multiple target profiles are supported
- Nested reference elements are handled correctly

### 5. Cardinality and Metadata
- Min/max cardinality is preserved
- Element descriptions, short descriptions, and comments are maintained
- Aliases and examples are converted when present

## Usage Tips

1. **Start with the quick demo**: `just quick-demo` gives you an immediate overview
2. **Explore specific features**: Use individual demo commands to focus on particular FHIR features
3. **Compare outputs**: Use the comparison commands to see the transformation clearly
4. **Clean up**: Use `just clean` to remove generated files when needed
5. **Full workflow**: Use `just full-demo` for a complete demonstration including tests

## Troubleshooting

- If `just` is not found, install it from [https://github.com/casey/just](https://github.com/casey/just)
- If `jq` is not found, install it for better JSON formatting in comparisons
- If build fails, ensure you have a recent Rust toolchain installed
- Use `just test` to verify the core functionality is working correctly

## Next Steps

After exploring the demonstrations:
1. Try converting your own StructureDefinition files using the CLI directly
2. Explore the generated FHIRSchema files to understand the format
3. Use the converted schemas for validation or code generation
4. Contribute additional example files for more complex scenarios
