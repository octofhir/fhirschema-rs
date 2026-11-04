# Changelog

## 0.3.36 — 2026-11-04

- Converter: slices are grouped with their own children by element id, so a sliced element's
  children no longer move into its last slice (US Core Provenance.agent, R6 transaction-bundle).
- Converter: a slice listed without its slicing root (`extension:sqlText`) no longer lends its
  cardinality and profile to a repeating element; on a single-valued element
  (`Composition.date:IssueDate`) it still constrains the element. A required slice requires the
  element it slices.
- Converter: an extension slice without a discriminator is matched by `url`; a choice constrained
  without types (`value[x]` 1..1) is kept; a profile's numeric `max` is kept; root invariants come
  from the differential when there is no snapshot.
- Validator: an overlay on a complex type merges over the type's whole inheritance chain
  (inherited `Attachment.extension` stays an array) and keeps slicing, required, excluded and
  extensions; repeating elements are checked against their `min` and `max`.
- Precompiled R4, R4B, R5 and R6 schemas regenerated.

## 0.3.35 — 2026-11-04

- Replace the direct once_cell dependency with standard-library LazyLock.
- Remove the unused URL dependency and use futures-util for reference batches.
- Require the lighter FHIR model 0.1.19; retain default HTTP provider exports and
  all embedded schema versions, with a documented HTTP-free consumer mode.
- Keep CLI support out of the schema development tools' package-manager dependency.
- Update private development tools to current FHIRPath and patched ZIP extraction.
