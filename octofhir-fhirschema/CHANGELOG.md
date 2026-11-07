# Changelog

## 0.3.42 — 2026-11-07

- `OCTOFHIR_FHIR_VERSIONS` at build time (`R4`, or `R4,R5`) embeds only those versions, for
  builds where size matters, such as WebAssembly. Without it all four are in, as before. A version
  left out has an empty schema map.

## 0.3.41 — 2026-11-06

- `fixed[x]` and `pattern[x]` are checked outside slices too; the content of slices is checked;
  required children of elements are checked; two types of one choice are refused; unknown
  elements inside a Reference are refused, and `Reference.display` of an overlay is kept; a
  primitive with an overlay is no longer asked to be an object.
- Expansion stops at 32 levels.

## 0.3.38 — 2026-11-06

- A slice discriminator reaches into required nested slices: the R4 `bp` profile fixes
  `code.coding.code` in `code.coding:SBPCode`, and its components no longer match both slices.
- A Reference with only an extension (`data-absent-reason`) is valid.
- `elementdefinition-type-must-support` carries into choice variants: with types marked,
  mustSupport covers those alone. `StructureDefinitionExtension` gains `value_boolean`.
- Precompiled R4, R4B and R5 schemas regenerated: `bp` and `vitalsigns`.

## 0.3.36 — 2026-11-04

- Converter: slices are grouped with their own children by element id, so a sliced element's
  children no longer move into its last slice (US Core Provenance.agent, R6 transaction-bundle).
- Converter: a slice listed without its slicing root (`extension:sqlText`) no longer lends its
  cardinality and profile to a repeating element; on a single-valued element
  (`Composition.date:IssueDate`) it still constrains the element. A choice's type slice
  (`value[x]:valueQuantity`) constrains its typed element and no longer narrows the choice. A
  required slice requires the element it slices.
- Converter: an element entered again (a choice, then its type slice) is merged instead of
  replaced, so the choice keeps the types its root narrowed (R6 `bp`).
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
