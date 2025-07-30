# Phase 4: Code Generation

**Status**: Not Started  
**Priority**: Medium  
**Dependencies**: Phase 1 (Core Foundation), Phase 3 (Repository and Management)  
**Estimated Duration**: 5-6 weeks  

## Overview
Implement code generation capabilities from FHIRSchema, starting with Rust struct generation and expanding to support validation code and other language targets.

## Tasks

### Task 4.1: Implement fhirschema-codegen Core Engine
**Status**: Not Started  
**Estimated Duration**: 2 weeks  
**Priority**: High  

#### Subtasks:
- [ ] Create `CodeGenerator` trait interface
  - [ ] Language-agnostic generation interface
  - [ ] Template-based generation support
  - [ ] Configuration and customization options
  - [ ] Output formatting and organization
- [ ] Implement template engine
  - [ ] Template parsing and rendering
  - [ ] Variable substitution and logic
  - [ ] Template inheritance and composition
  - [ ] Custom helper functions
- [ ] Create schema analysis utilities
  - [ ] Schema dependency analysis
  - [ ] Type hierarchy extraction
  - [ ] Constraint analysis for code generation
  - [ ] Optimization opportunities identification
- [ ] Add code generation configuration
  - [ ] Output directory structure
  - [ ] Naming conventions and styles
  - [ ] Feature flags and options
  - [ ] Language-specific settings

#### Acceptance Criteria:
- Code generator provides flexible, extensible interface
- Template engine supports complex generation scenarios
- Schema analysis provides comprehensive metadata
- Configuration system accommodates various use cases
- Generated code follows language conventions

### Task 4.2: Implement TypeScript Code Generation
**Status**: Not Started  
**Estimated Duration**: 2-3 weeks  
**Priority**: High  

#### Subtasks:
- [ ] Create `TypeScriptGenerator` implementation
  - [ ] TypeScript interface generation from FHIRSchema
  - [ ] TypeScript class generation with plain classes support
  - [ ] Field type mapping (primitives, arrays, optionals, unions)
  - [ ] Nested interface and class generation
  - [ ] Choice type handling with union types
- [ ] Implement TypeScript-specific features
  - [ ] JSDoc comment generation from schema documentation
  - [ ] Export/import statement generation
  - [ ] Module organization and namespace handling
  - [ ] Type assertion and type guard generation
- [ ] Add validation code generation
  - [ ] Runtime validation functions
  - [ ] Type predicate functions
  - [ ] Custom validation logic integration
  - [ ] Error handling and reporting
- [ ] Create TypeScript code formatting
  - [ ] Prettier integration
  - [ ] Code style configuration
  - [ ] Import organization and sorting
  - [ ] Documentation formatting

#### Acceptance Criteria:
- Generated TypeScript interfaces and classes compile without errors
- Plain interfaces provide clean data structures
- Plain classes support instantiation and methods
- Generated code follows TypeScript best practices
- Runtime validation integrates seamlessly
- Documentation is comprehensive and accurate

### Task 4.3: Implement Template System
**Status**: Not Started  
**Estimated Duration**: 1-2 weeks  
**Priority**: Medium  

#### Subtasks:
- [ ] Design template format and syntax
  - [ ] Variable interpolation syntax
  - [ ] Control flow constructs (loops, conditionals)
  - [ ] Template composition and inheritance
  - [ ] Custom function support
- [ ] Create template library for TypeScript (primary focus)
  - [ ] Interface definition templates
  - [ ] Class definition templates
  - [ ] Union type templates for choice types
  - [ ] Module and namespace organization templates
- [ ] Create template library for Rust (secondary)
  - [ ] Struct definition templates
  - [ ] Enum definition templates
  - [ ] Implementation block templates
  - [ ] Module and file organization templates
- [ ] Add template customization
  - [ ] User-defined templates
  - [ ] Template override mechanisms
  - [ ] Template validation and testing
  - [ ] Template documentation and examples
- [ ] Implement template debugging
  - [ ] Template error reporting
  - [ ] Variable inspection
  - [ ] Generation tracing
  - [ ] Performance profiling

#### Acceptance Criteria:
- Template system is intuitive and powerful
- Built-in TypeScript templates cover common scenarios
- Built-in Rust templates support secondary use cases
- Customization allows for specific requirements
- Debugging tools help troubleshoot issues
- Performance is acceptable for large schemas

### Task 4.3b: Implement Secondary Rust Code Generation
**Status**: Not Started  
**Estimated Duration**: 1-2 weeks  
**Priority**: Low  

#### Subtasks:
- [ ] Create `RustGenerator` implementation (secondary priority)
  - [ ] Rust struct generation from FHIRSchema
  - [ ] Field type mapping (primitives, arrays, options)
  - [ ] Nested struct generation
  - [ ] Choice type handling with enums
- [ ] Implement Rust-specific features
  - [ ] Serde serialization/deserialization attributes
  - [ ] Documentation comment generation
  - [ ] Derive macro applications
  - [ ] Module organization and imports
- [ ] Add Rust validation code generation
  - [ ] Constraint validation methods
  - [ ] Integration with fhirschema-validator
  - [ ] Error handling and reporting
- [ ] Create Rust code formatting
  - [ ] rustfmt integration
  - [ ] Code style configuration

#### Acceptance Criteria:
- Generated Rust structs compile without errors
- Serde serialization works correctly with FHIR data
- Generated validation code integrates with validator
- Code follows Rust best practices and conventions
- Secondary to TypeScript implementation

### Task 4.4: Build System Integration
**Status**: Not Started  
**Estimated Duration**: 1 week  
**Priority**: Medium  

#### Subtasks:
- [ ] Create TypeScript/Node.js build integration (primary focus)
  - [ ] npm package generation
  - [ ] TypeScript declaration file generation
  - [ ] Webpack/Vite integration
  - [ ] Automatic code regeneration with file watchers
- [ ] Add Node.js tooling support
  - [ ] package.json generation with proper dependencies
  - [ ] TypeScript configuration (tsconfig.json)
  - [ ] ESLint and Prettier configuration
  - [ ] Jest test setup for generated code
- [ ] Create Cargo build script integration (secondary)
  - [ ] build.rs script generation
  - [ ] Automatic code regeneration
  - [ ] Dependency tracking
  - [ ] Incremental generation
- [ ] Add procedural macro support (secondary)
  - [ ] Derive macro for FHIRSchema structs
  - [ ] Attribute macro for validation
- [ ] Create IDE integration
  - [ ] TypeScript language server support
  - [ ] VS Code extension compatibility
  - [ ] Code completion for generated types
  - [ ] Error highlighting and diagnostics

#### Acceptance Criteria:
- TypeScript/Node.js build integration works seamlessly
- npm packages are properly structured and functional
- Generated TypeScript code compiles without errors
- IDE integration enhances TypeScript developer experience
- Cargo integration works for secondary Rust support
- Documentation guides integration setup for both ecosystems

### Task 4.5: Enhanced CLI Code Generation Commands
**Status**: Not Started  
**Estimated Duration**: 1 week  
**Priority**: Medium  

#### Subtasks:
- [ ] Add `generate` command group to CLI
  - [ ] Language target selection
  - [ ] Output directory specification
  - [ ] Template selection and customization
  - [ ] Generation configuration options
- [ ] Implement code generation commands
  - [ ] `typescript` command for TypeScript code generation (primary)
  - [ ] `rust` command for Rust code generation (secondary)
  - [ ] `validate` command for generated code validation
  - [ ] `clean` command for cleanup operations
  - [ ] `watch` command for automatic regeneration
  - [ ] `init` command for TypeScript project initialization
- [ ] Add generation reporting
  - [ ] Generation statistics and metrics
  - [ ] File change tracking
  - [ ] Error and warning summaries
  - [ ] Performance measurements
- [ ] Create project scaffolding
  - [ ] New project initialization
  - [ ] Template project creation
  - [ ] Configuration file generation
  - [ ] Documentation and examples

#### Acceptance Criteria:
- CLI provides intuitive code generation interface
- Commands handle various generation scenarios
- Reporting provides useful feedback
- Project scaffolding accelerates adoption
- Integration with existing CLI is seamless

### Task 4.6: Testing and Documentation
**Status**: Not Started  
**Estimated Duration**: 1 week  
**Priority**: High  

#### Subtasks:
- [ ] Create comprehensive code generation test suite
  - [ ] Unit tests for all generation components
  - [ ] Integration tests with real schemas
  - [ ] Generated code compilation tests
  - [ ] Template rendering tests
- [ ] Add code generation compliance testing
  - [ ] Test with complex FHIRSchema examples
  - [ ] Cross-platform generation testing
  - [ ] Performance and scalability tests
  - [ ] Memory usage validation
- [ ] Write code generation documentation
  - [ ] API documentation with examples
  - [ ] Template development guide
  - [ ] Integration tutorials
  - [ ] Best practices and patterns
- [ ] Create example projects
  - [ ] Simple schema to code examples
  - [ ] Complex real-world scenarios
  - [ ] Integration with validation
  - [ ] Performance optimization examples

#### Acceptance Criteria:
- Test coverage > 90% for code generation functionality
- Generated code compiles and works correctly
- Documentation enables easy adoption
- Examples demonstrate practical usage
- Performance meets acceptable standards

## Deliverables
- `fhirschema-codegen` crate with complete code generation engine
- TypeScript code generator with interfaces and classes (primary focus)
- Rust code generator with comprehensive features (secondary)
- Template system with built-in TypeScript and Rust templates
- Build system integration for TypeScript/Node.js and Rust/Cargo
- Enhanced CLI with TypeScript-first code generation commands
- Comprehensive test suite and documentation

## Success Criteria
- Successfully generates working TypeScript interfaces and classes from FHIRSchema
- Generated TypeScript code supports plain interfaces and classes as requested
- Generated code integrates with validation engine
- Template system enables customization and extension for multiple languages
- Build integration provides seamless development experience for TypeScript and Rust
- CLI provides intuitive TypeScript-first code generation workflow

## Risks and Mitigation
- **Risk**: Generated code quality doesn't meet production standards
  - **Mitigation**: Extensive testing and validation, follow language best practices
- **Risk**: Template system becomes too complex for users
  - **Mitigation**: Provide good defaults, comprehensive documentation, examples
- **Risk**: Build integration causes compatibility issues
  - **Mitigation**: Test with various Rust versions and build configurations

## Future Extensions
- Support for additional language targets (Python, Go, C#, Java)
- Advanced optimization techniques for generated code
- Integration with other FHIR tooling ecosystems
- Visual code generation tools and editors

## Next Phase
Phase 5: Server and Services - Implement HTTP server for FHIRSchema services
