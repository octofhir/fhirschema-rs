# Enhanced JSON Error Reporting for Invalid Resources

**Status:** COMPLETE
**Priority:** High  
**Estimated Time:** 0.5 Days  
**Actual Time:** 0.5 Days

## Overview
Implement enhanced error reporting to show detailed information about resources that fail with invalid JSON structure during CLI processing. Previously, the CLI would only show a generic count of failed resources without identifying which specific resources were problematic.

## Issue Description
The CLI was not providing sufficient information when StructureDefinition resources failed to parse due to invalid JSON structure. Users would only see a summary count like "X resources failed to parse" without knowing which specific resources were failing or why.

## Root Cause
In the `download_and_convert` function in `src/bin/cli.rs`, when JSON parsing failed, the code would:
1. Increment a counter (`skip_stats.parse_failed`)
2. Continue to the next resource
3. Discard the error information and resource details

This made debugging difficult as users couldn't identify which resources were problematic.

## Solution Implemented

### Enhanced Error Logging ✓
When a StructureDefinition fails to parse, the CLI now displays:
- ❌ Clear failure indicator
- 📍 Canonical URL of the failed resource
- 📦 Package name and version containing the resource
- 🏷️ Resource type (if available in raw JSON)
- 🆔 Resource ID (if available in raw JSON)
- 📝 Resource name (if available in raw JSON)
- ⚠️ Specific parsing error message

### Implementation Details
Updated the error handling in `src/bin/cli.rs` lines 350-371:

```rust
let mut structure_def = match structure_def_result {
    Ok(sd) => sd,
    Err(e) => {
        // Show detailed information about the resource that failed to parse
        println!("❌ Failed to parse StructureDefinition:");
        println!("   📍 Canonical URL: {}", resource_match.index.canonical_url);
        println!("   📦 Package: {}@{}", resource_match.index.package_name, resource_match.index.package_version);
        if let Some(resource_type) = resource_match.resource.content.get("resourceType") {
            println!("   🏷️  Resource Type: {}", resource_type);
        }
        if let Some(id) = resource_match.resource.content.get("id") {
            println!("   🆔 Resource ID: {}", id);
        }
        if let Some(name) = resource_match.resource.content.get("name") {
            println!("   📝 Name: {}", name);
        }
        println!("   ⚠️  Parse Error: {}", e);
        println!();
        skip_stats.parse_failed += 1;
        continue;
    }
};
```

### Backward Compatibility ✓
- Maintained existing skip statistics functionality
- Preserved the final summary reporting
- Added detailed logging without breaking existing behavior

## Benefits
1. **Improved Debugging**: Users can now identify exactly which resources are failing
2. **Better Error Context**: Specific error messages help understand what's wrong with the JSON
3. **Resource Identification**: Multiple identifiers (URL, ID, name) help locate problematic resources
4. **Package Tracking**: Users know which package contains the problematic resource

## Example Output
When a resource fails to parse, users will now see:
```
❌ Failed to parse StructureDefinition:
   📍 Canonical URL: http://example.org/fhir/StructureDefinition/broken-resource
   📦 Package: example.package@1.0.0
   🏷️  Resource Type: "StructureDefinition"
   🆔 Resource ID: "broken-resource"
   📝 Name: "BrokenResource"
   ⚠️  Parse Error: missing field `kind` at line 15 column 1
```

## Testing
- ✅ Code compiles successfully with CLI features enabled
- ✅ Enhanced error reporting preserves existing functionality
- ✅ Error messages are clear and informative
- ✅ All resource identification fields are properly displayed

## Acceptance Criteria
- [x] Detailed error information is shown for JSON parsing failures
- [x] Resource identification includes canonical URL, package info, and resource details
- [x] Specific parsing error messages are displayed
- [x] Existing skip statistics functionality is preserved
- [x] Code compiles without errors
- [x] Error output is user-friendly and actionable

## Dependencies
- CLI functionality (already implemented)
- CanonicalManager resource matching (already available)

## Impact
This enhancement significantly improves the debugging experience for users working with FHIR packages that contain malformed StructureDefinition resources. Instead of generic error counts, users now get actionable information to identify and fix problematic resources.

## Notes
The implementation leverages existing resource metadata from the CanonicalManager's search results, making it efficient and comprehensive. The error reporting is designed to be helpful for both technical users debugging JSON issues and package maintainers identifying problematic resources in their packages.
