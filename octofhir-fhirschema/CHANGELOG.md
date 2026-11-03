# Changelog

## 0.3.35 — 2026-11-04

- Replace the direct once_cell dependency with standard-library LazyLock.
- Remove the unused URL dependency and use futures-util for reference batches.
- Require the lighter FHIR model 0.1.19; retain default HTTP provider exports and
  all embedded schema versions, with a documented HTTP-free consumer mode.
- Keep CLI support out of the schema development tools' package-manager dependency.
- Update private development tools to current FHIRPath and patched ZIP extraction.
