# Contributing to OctoFHIR FHIRSchema

We welcome contributions to the OctoFHIR FHIRSchema project! This guide will help you understand our development process and how to contribute effectively.

## Table of Contents

- [Development Guidelines](#development-guidelines)
- [Architecture Decision Records (ADR)](#architecture-decision-records-adr)
- [Task Management](#task-management)
- [Development Workflow](#development-workflow)
- [Code Standards](#code-standards)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)

## Development Guidelines

We follow these established Rust guidelines:

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Coding Guidelines](https://rust-lang.github.io/rust-clippy/master/index.html)
- [Rust Style Guide](https://rust-lang.github.io/rust-style-guide/)

### External Dependencies

- **UCUM Units**: Use our library [ucum-rs](https://github.com/octofhir/ucum-rs) for unit conversions. If you find errors, fix them directly in the library and use the local version for development.
- **FHIRSchema Spec**: Reference the [FHIRSchema specification](https://fhir-schema.github.io/fhir-schema/intro.html) in the `specs` folder.

## Architecture Decision Records (ADR)

Before implementing significant features, you must prepare an ADR following the [ADR template](https://github.com/joelparkerhenderson/architecture-decision-record).

### ADR Process

1. **Planning Phase**: Create an ADR document describing the architectural decision
2. **Task Creation**: Split the ADR implementation into phases/tasks and store them in the `tasks` directory
3. **Implementation**: Only start coding after the ADR is approved and tasks are created

## Task Management

We use a structured task management system in the `tasks` directory:

```
tasks/
├── todo/          # Tasks not yet started
├── in-progress/   # Currently active tasks
└── done/          # Completed tasks
```

### Task Workflow

1. **Before Starting**: Create all task files for the feature in `tasks/todo/`
2. **During Development**: 
   - Move the current task to `tasks/in-progress/`
   - Maintain and update the specific task file while working on it
3. **After Completion**: 
   - Update the task file with implementation details
   - Move completed tasks to `tasks/done/`

### Task File Format

Each task file should include:
- Task description and objectives
- Implementation requirements
- Acceptance criteria
- Status updates
- Notes and decisions made during implementation

## Development Workflow

### Setting Up Development Environment

1. **Clone the repository**:
   ```bash
   git clone https://github.com/octofhir/fhirschema-rs.git
   cd fhirschema-rs
   ```

2. **Install Rust toolchain** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. **Build the project**:
   ```bash
   # Build library only
   cargo build
   
   # Build with CLI support
   cargo build --features cli
   
   # Build with all features
   cargo build --features all
   ```

### Development Features

The project supports multiple feature flags:

- `default`: `["memory-storage", "tokio", "bincode"]`
- `cli`: Command-line interface support
- `memory-storage`: In-memory schema storage with LRU caching
- `disk-storage`: Disk-based persistence
- `server`: HTTP server for schema management
- `bincode`: Binary serialization support
- `all`: All features enabled

### Debugging

For debugging issues:
1. Create a simple test in the `tests` directory
2. Use the test to reproduce and resolve the issue
3. Delete the test file after resolving the issue

## Code Standards

### Code Quality

- **Clippy**: All code must pass `cargo clippy` without warnings
- **Formatting**: Use `cargo fmt` to format code according to project standards
- **Documentation**: All public APIs must have comprehensive documentation
- **Error Handling**: Use `thiserror` for error types and proper error propagation

### Performance Considerations

- Follow the [Rust Performance Book](https://nnethercote.github.io/perf-book/) guidelines
- Use async/await for I/O operations
- Leverage parallel processing where appropriate (using `rayon`)
- Implement efficient memory usage patterns

### API Design

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use consistent naming conventions
- Provide both sync and async APIs where appropriate
- Ensure backward compatibility when possible

## Testing

### Test Categories

1. **Unit Tests**: Test individual components and functions
2. **Integration Tests**: Test component interactions
3. **Golden Tests**: Ensure compatibility with reference TypeScript implementation
4. **Benchmarks**: Performance regression testing

### Running Tests

```bash
# Run all tests
cargo test

# Run golden tests specifically
cargo test golden

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench
```

### Test Coverage

We aim for comprehensive test coverage. When adding new features:
- Write unit tests for all new functions
- Add integration tests for new workflows
- Update golden tests if changing conversion logic
- Add benchmarks for performance-critical code

### Test Organization

- Unit tests: In the same file as the code being tested
- Integration tests: In the `tests/` directory
- Benchmarks: In the `benches/` directory
- Golden test data: In appropriate subdirectories

## Submitting Changes

### Before Submitting

1. **Code Quality Checks**:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   ```

2. **Documentation**:
   ```bash
   cargo doc --no-deps
   ```

3. **Performance**:
   ```bash
   cargo bench
   ```

### Pull Request Process

1. **Fork and Branch**: Create a feature branch from `main`
2. **Implement**: Follow the development workflow and task management process
3. **Test**: Ensure all tests pass and add new tests as needed
4. **Document**: Update documentation and task files
5. **Submit**: Create a pull request with a clear description

### Pull Request Requirements

- **Clear Description**: Explain what changes were made and why
- **Task References**: Reference related task files and ADRs
- **Test Coverage**: Include tests for new functionality
- **Documentation**: Update relevant documentation
- **Breaking Changes**: Clearly mark any breaking changes

### Code Review Process

All contributions go through code review:
- At least one maintainer must approve changes
- All CI checks must pass
- Documentation must be updated
- Breaking changes require additional review

## Getting Help

- **Issues**: Use GitHub issues for bug reports and feature requests
- **Discussions**: Use GitHub discussions for questions and general discussion
- **Documentation**: Check the project documentation and specs
- **Community**: Join our community channels for real-time help

## Recognition

Contributors are recognized in:
- CHANGELOG.md for significant contributions
- README.md contributors section
- Release notes for major features

Thank you for contributing to OctoFHIR FHIRSchema! 🐙🦀
