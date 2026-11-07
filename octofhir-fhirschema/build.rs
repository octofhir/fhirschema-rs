//! Which FHIR versions to embed. `OCTOFHIR_FHIR_VERSIONS` (for example `R4` or `R4,R5`) leaves
//! the others out, for builds where size matters, such as WebAssembly; without it all four are
//! in. A version left out has no schemas: `get_schemas` gives an empty map for it.

const VERSIONS: [&str; 4] = ["r4", "r4b", "r5", "r6"];

fn main() {
    println!("cargo::rerun-if-env-changed=OCTOFHIR_FHIR_VERSIONS");
    for version in VERSIONS {
        println!("cargo::rustc-check-cfg=cfg(fhirschema_{version})");
    }
    let wanted = std::env::var("OCTOFHIR_FHIR_VERSIONS").unwrap_or_default();
    let wanted: Vec<String> = wanted
        .split(',')
        .map(|version| version.trim().to_lowercase())
        .filter(|version| !version.is_empty())
        .collect();
    for version in VERSIONS {
        if wanted.is_empty() || wanted.iter().any(|wanted| wanted == version) {
            println!("cargo::rustc-cfg=fhirschema_{version}");
        }
    }
}
