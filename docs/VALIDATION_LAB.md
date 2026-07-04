# Validation Lab

`validation-lab` is a devtools binary for validator parity and repeatable local
performance checks.

The primary correctness reference is the FHIR specification as represented by
StructureDefinitions and FHIRPath constraints. The HL7 Java validator is the main
external comparison target. RH can still be used as a secondary reference.

Java validator output is reported in two forms:

- `java_valid`: raw Java validator result.
- `java_comparable_valid`: Java result after excluding known Java policy checks
  that are not StructureDefinition/FHIRPath/spec constraints.

`--fail-on-mismatch` uses `java_comparable_valid`. Raw Java differences are still
reported as `java_policy_difference` so they do not disappear.

Run local octofhir-only throughput over repository fixtures:

```sh
just validation-lab
```

The octofhir throughput number is `sequential_hot_loop`: one validator instance,
one validation awaited at a time, no parallel batch execution and no CLI/JVM
startup in the measured loop.

For correctness parity the default OctoFHIR schema selection is
`resource-type-and-meta-profile`, matching the shape of RH `validate_auto`: validate
the base resource type and every URL in `meta.profile[]`. Use
`--octofhir-profile-mode resource-type` only for base structural speed numbers.

Run Java parity:

```sh
just validation-java-parity
```

Run US Core parity over the RH-style Patient fixtures:

```sh
just validation-us-core-java-parity
```

The US Core parity recipe loads `hl7.fhir.us.core#6.1.0` through
`octofhir-canonical-manager` for OctoFHIR and passes the local US Core `.tgz` to
Java as `-ig`. Java may still download transitive terminology/IG packages into
`target/validation-lab/java-home/.fhir/packages` on the first run.

If no jar is provided, `validation-lab` downloads the latest HL7 Java validator
from the `hapifhir/org.hl7.fhir.core` GitHub release asset and caches it at:

```text
target/validation-lab/validator_cli.jar
```

You can fetch the jar without running parity:

```sh
just fetch-java-validator
```

You can still pin a local jar:

```sh
just validation-java-parity /path/to/validator_cli.jar
```

or set:

```sh
HL7_VALIDATOR_JAR=/path/to/validator_cli.jar \
  cargo run -p octofhir-fhirschema-devtools --bin validation-lab -- --fail-on-mismatch
```

The report is written to:

```text
target/validation-lab/validation-lab-report.json
```

By default Java is invoked with:

```text
java -jar validator_cli.jar <fixture.json> -version 4.0.1 -tx n/a -output <operation-outcome.json>
```

`-tx n/a` keeps the default parity loop offline and focused on structural
validation. Use `--java-tx <url>` when terminology parity is the target.

Known Java policy exclusions
----------------------------

By default the runner excludes Java `OperationOutcome` errors with message id:

```text
TYPE_SPECIFIC_CHECKS_DT_URL_EXAMPLE
```

This is Java validator's policy that rejects `example.org`/`example.com` style
URLs in instance content. It is not a FHIR R4 datatype rule for `uri`, `url`, or
`canonical`, and should not make the OctoFHIR core validator stricter than the
spec. The raw Java error remains visible in `java_issues`, `java_raw_mismatch`,
and `java_policy_difference`.

Override the excluded message ids with repeated `--ignore-java-message-id` flags.
Passing any explicit value replaces the default list. Use `--strict-java-policy`
to disable exclusions and make raw Java validity the comparable result.

Optionally compare RH as a secondary reference:

```sh
RH_BIN=/path/to/rh \
  cargo run -p octofhir-fhirschema-devtools --bin validation-lab -- \
    --fail-on-mismatch
```
