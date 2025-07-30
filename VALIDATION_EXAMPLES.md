# FHIRSchema Validation Examples

This document demonstrates the FHIRSchema validation engine capabilities with examples of both valid and invalid FHIR resources.

## Command Usage

### Basic Validation Command
```bash
fhirschema validate --schema <schema-file> --resource <resource-file> [--verbose]
```

### Parameters
- `--schema, -s`: Path to the FHIRSchema file (YAML or JSON format)
- `--resource, -r`: Path to the FHIR resource file to validate (JSON or YAML format)
- `--verbose, -v`: Enable verbose output with detailed validation statistics

## Example 1: Valid Resource Validation

### Schema: test-simple-patient.fhirschema.yaml
```yaml
url: http://example.org/StructureDefinition/simple-patient
type: Patient
name: SimplePatient
derivation: specialization
elements:
  Patient.resourceType:
    min: 1
    max: '1'
    type: code
  Patient.id:
    min: 0
    max: '1'
    type: id
  Patient.active:
    min: 0
    max: '1'
    type: boolean
  Patient.gender:
    min: 0
    max: '1'
    type: code
```

### Valid Resource: test-simple-valid-patient.json
```json
{
  "resourceType": "Patient",
  "id": "simple-valid-patient",
  "active": true,
  "gender": "male"
}
```

### Validation Command
```bash
fhirschema validate --schema test-simple-patient.fhirschema.yaml --resource test-simple-valid-patient.json --verbose
```

### Actual Result
```
✅ Validation successful!
Validation Statistics:
  Elements validated: 8
  Constraints evaluated: 0
  Primitives validated: 4
  Duration: 0ms
```

## Example 2: Invalid Resource Validation

### Invalid Resource: test-invalid-patient.json
```json
{
  "resourceType": "Patient",
  "id": "invalid-patient-example",
  "active": "not-a-boolean",
  "name": "should-be-array-not-string",
  "gender": 123,
  "birthDate": "invalid-date-format",
  "telecom": [
    {
      "system": "email",
      "value": "not-a-valid-email"
    },
    {
      "system": "phone"
    }
  ],
  "address": [
    {
      "use": "home",
      "line": "should-be-array",
      "city": "",
      "postalCode": 12345
    }
  ]
}
```

### Validation Command
```bash
fhirschema validate --schema test-simple-patient.fhirschema.yaml --resource test-invalid-patient.json --verbose
```

### Actual Result
```
❌ Validation failed with 4 issues:
🔴 [ERROR] type-mismatch at Patient.active: Element 'Patient.active' has type 'string', but expected 'boolean'
   Context: expected: boolean, actual: string
🔴 [ERROR] invalid-boolean at Patient.active: Value at 'Patient.active' must be a boolean
🔴 [ERROR] type-mismatch at Patient.gender: Element 'Patient.gender' has type 'integer', but expected 'code'
   Context: expected: code, actual: integer
🔴 [ERROR] invalid-code at Patient.gender: Value at 'Patient.gender' must be a string representing a code

Validation Statistics:
  Elements validated: 8
  Constraints evaluated: 0
  Primitives validated: 4
  Duration: 0ms
```

## Validation Capabilities Demonstrated

### 1. Type Validation
- **Boolean Type**: Detected `"not-a-boolean"` string instead of boolean value
- **Code Type**: Detected numeric `123` instead of string code value
- **Complex Type**: Detected string instead of `HumanName` object

### 2. Shape Validation
- **Array vs Scalar**: Detected scalar string where array was expected for `Patient.name`

### 3. Primitive Validation
- **Boolean Primitive**: Validates that boolean fields contain actual boolean values
- **Code Primitive**: Validates that code fields contain string values
- **ID Primitive**: Validates ID format compliance

### 4. Cardinality Validation
- **Required Fields**: Validates minimum cardinality requirements
- **Optional Fields**: Handles optional elements correctly

### 5. Error Reporting
- **Severity Levels**: Distinguishes between errors (🔴) and warnings (🟡)
- **Precise Location**: Shows exact element path where validation failed
- **Context Information**: Provides expected vs actual type information
- **Validation Statistics**: Reports performance metrics and validation counts

## Error Types

| Error Code | Description | Example |
|------------|-------------|---------|
| `type-mismatch` | Element type doesn't match schema expectation | String provided where boolean expected |
| `invalid-boolean` | Value is not a valid boolean | String "true" instead of boolean true |
| `invalid-code` | Value is not a valid code format | Number instead of string |
| `shape-mismatch` | Array/scalar shape doesn't match cardinality | Scalar where array expected |
| `cardinality-min` | Too few elements for minimum requirement | 0 elements where min=1 |
| `cardinality-max` | Too many elements for maximum limit | 3 elements where max=2 |

## Current Limitations

The current validation engine implementation has some limitations:

### Complex Types
- **HumanName, Address, ContactPoint**: Complex FHIR types are not fully supported yet
- **Nested Objects**: Validation of nested object structures is limited
- **Choice Types**: Polymorphic elements (choice[x]) have limited support

### Advanced Features
- **FHIRPath Constraints**: Basic constraint evaluation is implemented but complex expressions may not work
- **Slicing**: Array slicing validation framework exists but discriminator matching is basic
- **References**: Reference validation is not fully implemented
- **Extensions**: Extension validation is not yet supported

### Workarounds
- Use primitive types (string, boolean, code, id, integer, decimal) for reliable validation
- Keep schemas simple with minimal nesting
- Focus on cardinality and basic type validation

### Future Improvements
- Full complex type support
- Enhanced FHIRPath constraint evaluation with fhirpath-rs integration
- Complete slicing validation with all discriminator types
- Reference validation and resolution
- Extension validation support

## Exit Codes

- **0**: Validation successful
- **1**: Validation failed with errors

This makes the validation command suitable for use in CI/CD pipelines and automated testing scenarios.
