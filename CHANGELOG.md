# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-08-06

### Added
- Initial release of octofhir-fhirschema library
- High-performance FHIR StructureDefinition to FHIRSchema conversion
- FHIRSchema validation capabilities with detailed error reporting
- Async/await support throughout the library for optimal performance
- Memory-efficient schema storage with optional disk-based caching
- CLI tool for standalone usage with conversion and validation commands
- Package download functionality for FHIR packages from registry
- Integration with octofhir-canonical-manager for seamless FHIR resource management
- Support for filtering specific resource types during package conversion
- Comprehensive test suite with golden test compatibility
- Parallel conversion capabilities for improved performance
- Hierarchical caching system for optimized schema access
- Compressed storage support using LZ4 compression
- Cross-platform support (Linux, macOS, Windows)
- Complete documentation with usage examples
- GitHub Actions CI/CD workflows for automated testing and releases
- Rust toolchain configuration for consistent development environment

### Features
- **Core Library**: Convert FHIR StructureDefinitions to FHIRSchema format
- **Validation**: Validate FHIR resources against FHIRSchema definitions
- **CLI Tool**: Command-line interface for conversion and package management
- **Storage**: In-memory and disk-based schema storage options
- **Caching**: Multi-level caching for performance optimization
- **Async Support**: Full async/await implementation for non-blocking operations
- **Package Management**: Download and convert FHIR packages from registries
- **Resource Filtering**: Selective conversion of specific FHIR resource types
- **Compression**: LZ4-based compression for efficient storage
- **Parallel Processing**: Multi-threaded conversion for large datasets

### Technical Details
- Built with Rust 2024 edition for modern language features
- Supports multiple storage backends (memory, disk)
- Configurable feature flags for modular functionality
- Comprehensive error handling with detailed diagnostics
- Cross-platform compatibility with optimized builds
- Integration with OctoFHIR ecosystem components

[0.1.0]: https://github.com/octofhir/fhirschema-rs/releases/tag/v0.1.0
