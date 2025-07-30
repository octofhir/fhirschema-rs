## Guidelines

Apply the following guidelines when developing fhirpath-core:
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Coding Guidelines](https://rust-lang.github.io/rust-clippy/master/index.html)
- [Rust Style Guide](https://rust-lang.github.io/rust-style-guide/)


Spec reference in `specs` folder
FHIRSchema spec - https://fhir-schema.github.io/fhir-schema/intro.html

Before implementing big features prepare ADR(https://github.com/joelparkerhenderson/architecture-decision-record) and only after that start writing code

For work with units and converts unit use our library https://github.com/octofhir/ucum-rs or in local path ./…/ucum-rs if any error is found, you can fix them in a library directly and use a local library for development

## Planing Phase

For every ADR implementation split record into phases/tasks and store in `tasks` directory. Maintain a specific task file when working on it. Before starting on the first task, create all tasks for future use. After implementing features from a task file update it status
For debugging cases create a simple test inside the test directory and delete it after resolving the issue


## Task executing phase
Update task file for aligh with implemented features


## Test Coverage

To track progress and maintain visibility into implementation completeness:
