# Converter Complex-Type and Logical Resource Support

**Status:** COMPLETE
**Priority:** High  
**Estimated Time:** 0.5 Days  
**Actual Time:** 0.5 Days

## Overview
Implement support for converting complex-type and logical resource StructureDefinitions according to the FHIRSchema specification. This includes proper classification and field mapping for all supported structure definition types.

## Tasks

### 1. Schema Structure Updates ✓
- [x] Add new classification fields to FhirSchema struct (kind, class, base, abstract)
- [x] Add builder methods for new fields (with_kind, with_class, with_base, with_abstract)
- [x] Maintain backward compatibility with legacy fields

### 2. Converter Logic Implementation ✓
- [x] Add classification helper function to determine schema class
- [x] Update sync conversion method to use classification logic
- [x] Update async conversion method to use classification logic
- [x] Handle all specification-defined mappings

### 3. Classification Logic ✓
- [x] Resources: "resource" class by default, "profile" class when derivation is "constraint"
- [x] Complex/primitive types: "type" class by default, "extension" class for Extension type
- [x] Logical models: "logical" class
- [x] Abstract types: properly set the abstract flag
- [x] Base definitions: properly mapped to the base field

### 4. Testing and Verification ✓
- [x] Create comprehensive test suite covering all classification scenarios
- [x] Test complex-type classification
- [x] Test logical resource classification
- [x] Test extension type classification
- [x] Test resource profile classification
- [x] Verify all tests pass

## Implementation Details

### Classification Helper Function
Added `determine_class()` helper function that maps kind, derivation, and type according to specification:

```rust
fn determine_class(kind: &str, derivation: Option<&str>, type_name: &str) -> String {
    match kind {
        "resource" => {
            match derivation {
                Some("constraint") => "profile".to_string(),
                _ => "resource".to_string(),
            }
        }
        "complex-type" | "primitive-type" => {
            if type_name == "Extension" {
                "extension".to_string()
            } else {
                "type".to_string()
            }
        }
        "logical" => "logical".to_string(),
        _ => "resource".to_string(), // fallback
    }
}
```

### Schema Field Mapping
Updated conversion logic to properly map StructureDefinition fields to FhirSchema:
- `kind` → `kind` (direct mapping)
- `derivation` + `kind` + `type` → `class` (via classification logic)
- `baseDefinition` → `base` (direct mapping)
- `abstract` → `abstract_type` (direct mapping)

### Backward Compatibility
Maintained legacy fields (`base_definition`, `derivation`) for backward compatibility while adding new specification-compliant fields.

## Test Results
All 4 classification tests pass:
- ✅ Complex-type classification → "type" class
- ✅ Logical resource classification → "logical" class  
- ✅ Extension type classification → "extension" class
- ✅ Resource profile classification → "profile" class

## Acceptance Criteria
- [x] All structure definition types are properly classified according to specification
- [x] New schema fields are correctly populated from StructureDefinition
- [x] Backward compatibility is maintained with existing fields
- [x] Comprehensive test coverage validates all classification scenarios
- [x] Implementation follows specification requirements exactly

## Dependencies
- Phase 1: Foundation complete
- Phase 2: Converter implementation base

## Deliverables
- ✅ Updated FhirSchema struct with classification fields
- ✅ Classification logic in converter
- ✅ Comprehensive test suite
- ✅ Full specification compliance for complex-type and logical resources

## Notes
This implementation fully satisfies the specification requirements for converting complex-type and logical resource StructureDefinitions. The converter now properly handles all supported structure definition kinds with correct classification and field mapping.
