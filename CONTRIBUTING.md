# Contributing to FHIRSchema

We welcome contributions to the FHIRSchema project! This document provides guidelines for contributing to ensure a smooth and productive collaboration.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Process](#development-process)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Documentation](#documentation)
- [Submitting Changes](#submitting-changes)
- [Architecture Decision Records](#architecture-decision-records)

## Code of Conduct

This project adheres to a code of conduct that promotes a welcoming and inclusive environment. Please be respectful and professional in all interactions.

## Getting Started

### Prerequisites

- Rust 1.70.0 or later
- Git
- A GitHub account

### Setting Up Development Environment

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/fhirschema.git
   cd fhirschema
   ```
3. Add the upstream repository:
   ```bash
   git remote add upstream https://github.com/octofhir/fhirschema.git
   ```
4. Build the project:
   ```bash
   cargo build
   ```
5. Run tests to ensure everything works:
   ```bash
   cargo test
   ```

## Development Process

### Branching Strategy

- `main` - Stable release branch
- `develop` - Integration branch for features
- `feature/*` - Feature development branches
- `bugfix/*` - Bug fix branches
- `hotfix/*` - Critical fixes for production

### Workflow

1. Create a feature branch from `develop`:
   ```bash
   git checkout develop
   git pull upstream develop
   git checkout -b feature/your-feature-name
   ```

2. Make your changes following the coding standards

3. Test your changes thoroughly

4. Commit your changes with clear, descriptive messages

5. Push to your fork and create a pull request

## Coding Standards

### Rust Guidelines

Follow these established Rust guidelines:

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Rust Coding Guidelines](https://rust-lang.github.io/rust-clippy/master/index.html)
- [Rust Style Guide](https://rust-lang.github.io/rust-style-guide/)

### Code Formatting

- Use `cargo fmt` to format your code
- Configuration is in `rustfmt.toml`
- All code must pass formatting checks

### Linting

- Use `cargo clippy` to check for common issues
- Configuration is in `clippy.toml`
- All code must pass clippy checks with no warnings

### Naming Conventions

- Use `snake_case` for functions, variables, and modules
- Use `PascalCase` for types, structs, enums, and traits
- Use `SCREAMING_SNAKE_CASE` for constants
- Use descriptive names that clearly indicate purpose

### Error Handling

- Use `Result<T, E>` for fallible operations
- Use `thiserror` for custom error types
- Provide meaningful error messages with context
- Chain errors appropriately using `#[from]` or manual conversion

### Documentation

- Document all public APIs with `///` comments
- Include examples in documentation where helpful
- Use `//!` for module-level documentation
- Follow rustdoc conventions

## Testing Guidelines

### Test Coverage

- Maintain >90% test coverage for all core functionality
- Write unit tests for individual functions and methods
- Write integration tests for end-to-end scenarios
- Use property-based testing with `proptest` where appropriate

### Test Organization

- Unit tests go in the same file as the code being tested
- Integration tests go in the `tests/` directory
- Use descriptive test names that explain what is being tested

### Test Data

- Use realistic FHIR examples from the specification
- Store test data in appropriate directories
- Document the source and purpose of test data

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with coverage
cargo llvm-cov --all-features --workspace

# Run specific test
cargo test test_name

# Run tests for specific crate
cargo test -p fhirschema-core
```

## Documentation

### API Documentation

- All public APIs must be documented
- Include examples in documentation
- Document error conditions and edge cases
- Use proper markdown formatting

### Architecture Documentation

- Update ADRs when making architectural decisions
- Document design rationales and trade-offs
- Keep implementation plans up to date

### User Documentation

- Update README.md for user-facing changes
- Provide clear usage examples
- Document breaking changes in release notes

## Submitting Changes

### Pull Request Process

1. Ensure your branch is up to date with the target branch
2. Run the full test suite and ensure all checks pass
3. Update documentation as needed
4. Create a pull request with:
   - Clear title describing the change
   - Detailed description of what was changed and why
   - Reference to any related issues
   - Screenshots or examples if applicable

### Pull Request Requirements

- [ ] All tests pass
- [ ] Code is properly formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation is updated
- [ ] Test coverage is maintained or improved
- [ ] Breaking changes are documented

### Review Process

- All pull requests require at least one review
- Address reviewer feedback promptly
- Be open to suggestions and improvements
- Maintain a collaborative and respectful tone

## Architecture Decision Records

### When to Create an ADR

Create an ADR for:
- Significant architectural decisions
- Technology choices
- Design patterns and approaches
- Breaking changes to public APIs

### ADR Process

1. Create ADR document in the root directory
2. Follow the established ADR template
3. Discuss with maintainers before implementation
4. Update status as decisions evolve

### Implementation Planning

- Break large features into phases/tasks
- Store task files in the `tasks/` directory
- Update task status as work progresses
- Create simple tests for debugging during development

## Getting Help

- Open an issue for bugs or feature requests
- Use discussions for questions and general help
- Join our community channels (if available)
- Reach out to maintainers for guidance

## Recognition

Contributors will be recognized in:
- Release notes for significant contributions
- README acknowledgments
- Git commit history

Thank you for contributing to FHIRSchema! Your efforts help make FHIR validation more accessible and reliable for the healthcare community.
