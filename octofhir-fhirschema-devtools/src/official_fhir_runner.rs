use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use octofhir_fhir_model::provider::FhirVersion as ModelFhirVersion;
use octofhir_fhirpath::FhirPathEngine;
use octofhir_fhirschema::{
    DynamicSchemaProvider, FhirSchema, FhirValidator, FhirVersion, StructureDefinition,
    get_schemas, translate,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;
use zip::ZipArchive;

const FHIR_TEST_CASES_URL: &str =
    "https://github.com/FHIR/fhir-test-cases/archive/refs/heads/master.zip";
const VERSION_FILE: &str = "VERSION";
const VERSION_MARKER: &str = "master-snapshot";

#[derive(Debug, Parser)]
#[command(name = "official-fhir-runner")]
#[command(about = "Run OctoFHIR against the official FHIR validator test cases")]
struct Args {
    #[arg(
        long,
        default_value = "target/official-fhir-runner/fhir-test-cases",
        help = "Cache directory for the extracted FHIR/fhir-test-cases repository"
    )]
    cache_dir: PathBuf,

    #[arg(
        long,
        default_value = "target/official-fhir-runner",
        help = "Directory for JSON reports"
    )]
    output: PathBuf,

    #[arg(long, default_value = FHIR_TEST_CASES_URL)]
    download_url: String,

    #[arg(long, help = "Delete and re-download the official test case cache")]
    force_download: bool,

    #[arg(long, help = "Limit runnable R4 JSON cases for smoke runs")]
    max_tests: Option<usize>,

    #[arg(long, help = "Only run cases from one manifest module")]
    module: Option<String>,

    #[arg(long, value_enum, default_value_t = OctofhirRunner::Library)]
    runner: OctofhirRunner,

    #[arg(
        long,
        help = "Path to validation-lab when --runner cli is used. Defaults to sibling binary."
    )]
    octofhir_cli: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = OctofhirProfileMode::ResourceTypeAndMetaProfile)]
    octofhir_profile_mode: OctofhirProfileMode,

    #[arg(long, help = "Print each test result")]
    verbose: bool,

    #[arg(long, help = "Exit non-zero if any Java-comparable case disagrees")]
    fail_on_mismatch: bool,

    #[arg(
        long,
        help = "Enable required-binding validation via a terminology server (default tx.fhir.org/r4). Off by default (offline)."
    )]
    terminology: bool,

    #[arg(
        long,
        default_value = "https://tx.fhir.org/r4",
        help = "Terminology server base URL used when --terminology is set."
    )]
    terminology_server: String,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
enum OctofhirRunner {
    Library,
    Cli,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
enum OctofhirProfileMode {
    ResourceType,
    MetaProfile,
    ResourceTypeAndMetaProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    #[serde(rename = "test-cases")]
    test_cases: Vec<TestCase>,
    #[serde(default)]
    versions: HashMap<String, String>,
    #[serde(default)]
    modules: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestCase {
    name: String,
    file: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    module: Option<String>,
    #[serde(default)]
    profiles: Vec<String>,
    #[serde(default)]
    supporting: Vec<String>,
    #[serde(default)]
    profile: Option<ProfileTest>,
    #[serde(default)]
    scoring: Option<ScoringTest>,
    #[serde(rename = "allow-comments", default)]
    allow_comments: bool,
    #[serde(rename = "use-test", default = "default_true")]
    use_test: bool,
    #[serde(default)]
    java: Option<ValidatorOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum ValidatorOutcome {
    Path(String),
    Inline(InlineOutcome),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileTest {
    source: String,
    #[serde(default)]
    supporting: Vec<String>,
    #[serde(default)]
    java: Option<ValidatorOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScoringTest {
    profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InlineOutcome {
    #[serde(default)]
    error_count: Option<usize>,
    #[serde(default)]
    outcome: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedOutcome {
    resource_type: String,
    #[serde(default)]
    issue: Vec<OutcomeIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OutcomeIssue {
    severity: String,
}

#[derive(Debug, Serialize)]
struct Report {
    suite_url: String,
    validator_dir: PathBuf,
    runner: OctofhirRunner,
    fhirpath_constraints: bool,
    octofhir_profile_mode: OctofhirProfileMode,
    module_filter: Option<String>,
    max_tests: Option<usize>,
    manifest_cases: usize,
    runnable_cases: usize,
    java_comparable_cases: usize,
    completed_cases: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
    java_matches: usize,
    java_mismatches: usize,
    agreement_percent: f64,
    elapsed_ms: f64,
    avg_ms_per_completed_case: f64,
    cases_per_second: f64,
    cases: Vec<CaseReport>,
}

#[derive(Debug, Serialize)]
struct CaseReport {
    name: String,
    module: Option<String>,
    file: String,
    expected_valid: Option<bool>,
    actual_valid: Option<bool>,
    passed: bool,
    skipped: bool,
    skip_reason: Option<String>,
    mismatch: bool,
    resource_type: Option<String>,
    schema_names: Vec<String>,
    error_count: usize,
    errors: Vec<IssueSummary>,
    elapsed_ms: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct IssueSummary {
    error_type: String,
    message: Option<String>,
    path: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct OctofhirCliOutput {
    schema_names: Vec<String>,
    valid: bool,
    error_count: usize,
    errors: Vec<IssueSummary>,
}

struct RunContext {
    validator_dir: PathBuf,
    validator: FhirValidator,
    current_cli: Option<PathBuf>,
    profile_mode: OctofhirProfileMode,
    runner: OctofhirRunner,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    fs::create_dir_all(&args.output)
        .with_context(|| format!("failed to create {}", args.output.display()))?;

    let validator_dir = ensure_test_cases(&args).await?;
    let manifest = load_manifest(&validator_dir)?;
    let cli = match args.runner {
        OctofhirRunner::Library => None,
        OctofhirRunner::Cli => Some(resolve_octofhir_cli(args.octofhir_cli.as_ref())?),
    };
    let questionnaire_provider = Arc::new(build_questionnaire_provider(&validator_dir, &manifest));
    let supporting_schemas = load_supporting_structure_definitions(&validator_dir, &manifest);
    let context = RunContext {
        validator_dir: validator_dir.clone(),
        validator: create_r4_validator_with_fhirpath(
            args.terminology.then(|| args.terminology_server.clone()),
            Some(questionnaire_provider),
            supporting_schemas,
        )
        .await?,
        current_cli: cli,
        profile_mode: args.octofhir_profile_mode,
        runner: args.runner,
    };

    let runnable = manifest
        .test_cases
        .iter()
        .filter(|case| case.should_run())
        .filter(|case| {
            args.module
                .as_deref()
                .is_none_or(|module| case.module.as_deref() == Some(module))
        })
        .collect::<Vec<_>>();
    let runnable_cases = runnable.len();
    let selected = runnable
        .into_iter()
        .filter(|case| case.java_expected_valid(&validator_dir).is_some())
        .take(args.max_tests.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();

    println!(
        "official FHIR suite: {} manifest cases, {} runnable R4 JSON, {} Java-comparable selected",
        manifest.test_cases.len(),
        runnable_cases,
        selected.len()
    );
    if let Some(module) = &args.module {
        println!("module filter: {module}");
    }

    let started = Instant::now();
    let mut cases = Vec::with_capacity(selected.len());
    for (index, case) in selected.iter().enumerate() {
        if args.verbose {
            println!("running {} ({}/{})", case.name, index + 1, selected.len());
        } else if index > 0 && index % 25 == 0 {
            print!(".");
            if index % 250 == 0 {
                println!(" {index}/{}", selected.len());
            }
        }

        let report = run_case(case, &context).await;
        if args.verbose {
            print_case_result(&report);
        }
        cases.push(report);
    }
    if !args.verbose {
        println!();
    }

    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let completed_cases = cases.iter().filter(|case| !case.skipped).count();
    let passed = cases.iter().filter(|case| case.passed).count();
    let skipped = cases.iter().filter(|case| case.skipped).count();
    let failed = cases.len().saturating_sub(passed + skipped);
    let java_matches = cases
        .iter()
        .filter(|case| !case.skipped && !case.mismatch)
        .count();
    let java_mismatches = cases
        .iter()
        .filter(|case| !case.skipped && case.mismatch)
        .count();
    let agreement_percent = if completed_cases == 0 {
        0.0
    } else {
        (java_matches as f64 / completed_cases as f64) * 100.0
    };
    let report = Report {
        suite_url: args.download_url,
        validator_dir,
        runner: args.runner,
        fhirpath_constraints: true,
        octofhir_profile_mode: args.octofhir_profile_mode,
        module_filter: args.module,
        max_tests: args.max_tests,
        manifest_cases: manifest.test_cases.len(),
        runnable_cases,
        java_comparable_cases: selected.len(),
        completed_cases,
        passed,
        failed,
        skipped,
        java_matches,
        java_mismatches,
        agreement_percent,
        elapsed_ms,
        avg_ms_per_completed_case: if completed_cases == 0 {
            0.0
        } else {
            elapsed_ms / completed_cases as f64
        },
        cases_per_second: if elapsed_ms == 0.0 {
            0.0
        } else {
            completed_cases as f64 / (elapsed_ms / 1000.0)
        },
        cases,
    };

    let report_path = args.output.join("official-fhir-runner-report.json");
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("failed to write {}", report_path.display()))?;

    print_summary(&report, &report_path);
    if args.fail_on_mismatch && report.java_mismatches > 0 {
        bail!("official FHIR Java agreement mismatches found");
    }

    Ok(())
}

async fn ensure_test_cases(args: &Args) -> Result<PathBuf> {
    let validator_dir = args.cache_dir.join("validator");
    let manifest_path = validator_dir.join("manifest.json");
    let version_path = args.cache_dir.join(VERSION_FILE);
    if !args.force_download && manifest_path.exists() && version_path.exists() {
        return Ok(validator_dir);
    }

    if args.force_download && args.cache_dir.exists() {
        fs::remove_dir_all(&args.cache_dir)
            .with_context(|| format!("failed to remove {}", args.cache_dir.display()))?;
    }
    fs::create_dir_all(&args.cache_dir)
        .with_context(|| format!("failed to create {}", args.cache_dir.display()))?;

    println!(
        "downloading official FHIR test cases: {}",
        args.download_url
    );
    let bytes = reqwest::get(&args.download_url)
        .await
        .with_context(|| format!("failed to download {}", args.download_url))?
        .bytes()
        .await
        .with_context(|| format!("failed to read {}", args.download_url))?;
    println!("downloaded {} bytes", bytes.len());

    extract_zip_stripping_root(&args.cache_dir, bytes.as_ref())?;
    fs::write(version_path, VERSION_MARKER)?;
    if !manifest_path.exists() {
        bail!(
            "extracted test cases do not contain {}",
            manifest_path.display()
        );
    }
    Ok(validator_dir)
}

fn extract_zip_stripping_root(cache_dir: &Path, bytes: &[u8]) -> Result<()> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).context("failed to open test case zip")?;
    let mut extracted_files = 0usize;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let Some(enclosed) = file.enclosed_name() else {
            continue;
        };
        let relative = enclosed
            .strip_prefix("fhir-test-cases-master")
            .unwrap_or(&enclosed);
        if relative.as_os_str().is_empty() {
            continue;
        }
        let outpath = cache_dir.join(relative);
        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
            continue;
        }
        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        fs::write(&outpath, content)?;
        extracted_files += 1;
    }

    println!(
        "extracted {extracted_files} files into {}",
        cache_dir.display()
    );
    Ok(())
}

fn load_manifest(validator_dir: &Path) -> Result<Manifest> {
    let path = validator_dir.join("manifest.json");
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

async fn run_case(case: &TestCase, context: &RunContext) -> CaseReport {
    let started = Instant::now();
    let expected_valid = case.java_expected_valid(&context.validator_dir);
    let path = context.validator_dir.join(&case.file);

    let result = match fs::read_to_string(&path) {
        Ok(source) => parse_test_resource_json(&source, case)
            .map_err(|error| format!("JSON parse error: {error}"))
            .and_then(|resource| Ok((resource_type(&resource)?, resource))),
        Err(error) => Err(format!("read error: {error}")),
    };

    let (resource_type, resource) = match result {
        Ok(value) => value,
        Err(error) => {
            let actual_valid = Some(false);
            let mismatch = expected_valid.is_some_and(|expected| expected);
            return CaseReport {
                name: case.name.clone(),
                module: case.module.clone(),
                file: case.file.clone(),
                expected_valid,
                actual_valid,
                passed: !mismatch,
                skipped: false,
                skip_reason: Some(error),
                mismatch,
                resource_type: None,
                schema_names: Vec::new(),
                error_count: 1,
                errors: Vec::new(),
                elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
            };
        }
    };

    let schema_names = case.schema_names(&resource, &resource_type, context.profile_mode);
    let octofhir = match context.runner {
        OctofhirRunner::Library => validate_library(&context.validator, &resource, &schema_names)
            .await
            .map_err(|error| error.to_string()),
        OctofhirRunner::Cli => validate_cli(
            context.current_cli.as_ref().expect("CLI path missing"),
            &path,
            context.profile_mode,
        )
        .map(|mut result| {
            if !result.schema_names.is_empty() {
                result
            } else {
                result.schema_names = schema_names.clone();
                result
            }
        })
        .map_err(|error| error.to_string()),
    };

    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    match octofhir {
        Ok(actual) => {
            let mismatch = expected_valid.is_some_and(|expected| expected != actual.valid);
            CaseReport {
                name: case.name.clone(),
                module: case.module.clone(),
                file: case.file.clone(),
                expected_valid,
                actual_valid: Some(actual.valid),
                passed: !mismatch,
                skipped: false,
                skip_reason: None,
                mismatch,
                resource_type: Some(resource_type),
                schema_names: actual.schema_names,
                error_count: actual.error_count,
                errors: actual.errors,
                elapsed_ms,
            }
        }
        Err(error) => CaseReport {
            name: case.name.clone(),
            module: case.module.clone(),
            file: case.file.clone(),
            expected_valid,
            actual_valid: None,
            passed: false,
            skipped: true,
            skip_reason: Some(error),
            mismatch: false,
            resource_type: Some(resource_type),
            schema_names,
            error_count: 0,
            errors: Vec::new(),
            elapsed_ms,
        },
    }
}

async fn validate_library(
    validator: &FhirValidator,
    resource: &Value,
    schema_names: &[String],
) -> Result<OctofhirCliOutput> {
    let result = validator.validate(resource, schema_names.to_vec()).await;
    Ok(OctofhirCliOutput {
        schema_names: schema_names.to_vec(),
        valid: result.valid,
        error_count: result.errors.len(),
        errors: result
            .errors
            .into_iter()
            .map(|error| IssueSummary {
                error_type: error.error_type,
                message: error.message,
                path: error.path,
            })
            .collect(),
    })
}

/// In-memory `Questionnaire` resolver built from the manifest `supporting`
/// files, so a `QuestionnaireResponse` can be validated against its form.
#[derive(Default)]
struct MapQuestionnaireProvider {
    by_url: HashMap<String, Arc<Value>>,
}

#[async_trait::async_trait]
impl octofhir_fhirschema::QuestionnaireProvider for MapQuestionnaireProvider {
    async fn resolve(&self, canonical: &str) -> Option<Arc<Value>> {
        // Match on the full canonical first, then on the version-stripped URL.
        if let Some(q) = self.by_url.get(canonical) {
            return Some(q.clone());
        }
        let base = canonical.split('|').next().unwrap_or(canonical);
        self.by_url.get(base).cloned()
    }
}

/// Scan every test case's `supporting` files and index the ones that are
/// Questionnaires by their canonical `url` (and `url|version`).
fn build_questionnaire_provider(
    validator_dir: &Path,
    manifest: &Manifest,
) -> MapQuestionnaireProvider {
    let mut by_url = HashMap::new();
    for case in &manifest.test_cases {
        for rel in &case.supporting {
            let path = validator_dir.join(rel);
            let Ok(bytes) = fs::read(&path) else { continue };
            let Ok(json) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            if json.get("resourceType").and_then(|v| v.as_str()) != Some("Questionnaire") {
                continue;
            }
            if let Some(url) = json.get("url").and_then(|v| v.as_str()) {
                let url = url.to_string();
                if let Some(version) = json.get("version").and_then(|v| v.as_str()) {
                    by_url.insert(format!("{url}|{version}"), Arc::new(json.clone()));
                }
                by_url.entry(url).or_insert_with(|| Arc::new(json));
            }
        }
    }
    MapQuestionnaireProvider { by_url }
}

/// Translate every `StructureDefinition` referenced as a manifest `supporting`
/// file into a `FhirSchema` and index it by name + url, so a resource's
/// `meta.profile` (or a base definition a profile derives from) resolves during
/// validation instead of failing as an unknown schema.
fn load_supporting_structure_definitions(
    validator_dir: &Path,
    manifest: &Manifest,
) -> HashMap<String, FhirSchema> {
    let mut out = HashMap::new();
    for case in &manifest.test_cases {
        for rel in &case.supporting {
            let path = validator_dir.join(rel);
            let Ok(bytes) = fs::read(&path) else { continue };
            let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            if value.get("resourceType").and_then(Value::as_str) != Some("StructureDefinition") {
                continue;
            }
            let Ok(sd) = serde_json::from_value::<StructureDefinition>(value) else {
                continue;
            };
            let Ok(schema) = translate(sd, None) else {
                continue;
            };
            if !schema.name.is_empty() {
                out.insert(schema.name.clone(), schema.clone());
            }
            if !schema.url.is_empty() {
                out.insert(schema.url.clone(), schema);
            }
        }
    }
    out
}

async fn create_r4_validator_with_fhirpath(
    terminology_server: Option<String>,
    questionnaire_provider: Option<Arc<MapQuestionnaireProvider>>,
    supporting_schemas: HashMap<String, FhirSchema>,
) -> Result<FhirValidator> {
    let mut schemas = get_schemas(FhirVersion::R4).clone();
    // Supporting StructureDefinitions win over embedded ones so a test's own
    // profile/base definition is used when both are present.
    schemas.extend(supporting_schemas);
    let model_provider = Arc::new(DynamicSchemaProvider::new(
        schemas.clone(),
        ModelFhirVersion::R4,
    ));
    let registry = Arc::new(octofhir_fhirpath::create_function_registry());
    let fhirpath_engine = Arc::new(
        FhirPathEngine::new(registry, model_provider)
            .await
            .context("failed to initialize FHIRPath engine")?,
    );

    let mut validator = FhirValidator::from_schemas(schemas, Some(fhirpath_engine));

    // Optionally validate required bindings against a terminology server.
    // Uses the default HTTP terminology provider from octofhir-fhir-model
    // (tx.fhir.org/r4 by default), wrapped into the fhirschema TerminologyService
    // via TerminologyProviderAdapter. Lookup failures are advisory in the
    // validator, so an unreachable server cannot cause false rejections.
    if let Some(base_url) = terminology_server {
        let provider =
            octofhir_fhir_model::terminology::DefaultTerminologyProvider::with_server(&base_url)
                .with_context(|| format!("failed to init terminology provider for {base_url}"))?;
        let adapter = octofhir_fhirschema::TerminologyProviderAdapter::new(Arc::new(provider));
        validator = validator.with_terminology_service(Arc::new(adapter));
    }

    // Validate QuestionnaireResponses against their Questionnaire when a
    // provider (built from manifest `supporting` files) is available.
    if let Some(provider) = questionnaire_provider {
        // Enforce the Java-validator-style checks too (unknown linkId,
        // required-missing gated by enableWhen, answered-while-disabled) now that
        // enableWhen evaluation is implemented.
        validator = validator
            .with_questionnaire_provider(provider)
            .with_questionnaire_strictness(octofhir_fhirschema::QrStrictness::java_like());
    }

    Ok(validator)
}

fn validate_cli(
    bin: &Path,
    input: &Path,
    profile_mode: OctofhirProfileMode,
) -> Result<OctofhirCliOutput> {
    let output = Command::new(bin)
        .arg("--mode")
        .arg("validate-resource")
        .arg("--fixtures")
        .arg(input)
        .arg("--octofhir-profile-mode")
        .arg(profile_mode_arg(profile_mode))
        .output()
        .with_context(|| format!("failed to spawn {}", bin.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    extract_json_value(&stdout)
        .and_then(|value| serde_json::from_value(value).ok())
        .with_context(|| {
            format!(
                "failed to parse OctoFHIR CLI output for {}; status={:?}; stderr={}",
                input.display(),
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            )
        })
}

fn resolve_octofhir_cli(explicit: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        if path.exists() {
            return Ok(path.clone());
        }
        bail!("--octofhir-cli does not exist: {}", path.display());
    }

    let current = env::current_exe().context("failed to resolve current executable")?;
    let sibling = current.with_file_name(format!("validation-lab{}", env::consts::EXE_SUFFIX));
    if sibling.exists() {
        Ok(sibling)
    } else {
        bail!(
            "--runner cli requires validation-lab binary; build it or pass --octofhir-cli. Tried {}",
            sibling.display()
        );
    }
}

fn extract_json_value(stdout: &str) -> Option<Value> {
    for (idx, ch) in stdout.char_indices() {
        if ch == '{'
            && let Ok(value) = serde_json::from_str::<Value>(&stdout[idx..])
        {
            return Some(value);
        }
    }
    None
}

fn resource_type(resource: &Value) -> std::result::Result<String, String> {
    resource
        .get("resourceType")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "resourceType is missing".to_string())
}

fn parse_test_resource_json(
    source: &str,
    case: &TestCase,
) -> std::result::Result<Value, serde_json::Error> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    if let Ok(resource) = serde_json::from_str(source) {
        return Ok(resource);
    }
    if case.allow_comments {
        let without_comments = strip_json_line_comments(source);
        if let Ok(resource) = serde_json::from_str(&without_comments) {
            return Ok(resource);
        }
    }
    serde_json::from_str(source)
}

fn strip_json_line_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for comment_ch in chars.by_ref() {
                if comment_ch == '\n' {
                    output.push('\n');
                    break;
                }
            }
            continue;
        }

        output.push(ch);
    }

    output
}

impl TestCase {
    fn should_run(&self) -> bool {
        self.use_test && self.version.as_deref() == Some("4.0") && self.file.ends_with(".json")
    }

    fn java_expected_valid(&self, validator_dir: &Path) -> Option<bool> {
        expected_valid_from_outcome(self.java.as_ref(), validator_dir).or_else(|| {
            self.profile.as_ref().and_then(|profile| {
                expected_valid_from_outcome(profile.java.as_ref(), validator_dir)
            })
        })
    }

    fn schema_names(
        &self,
        resource: &Value,
        resource_type: &str,
        mode: OctofhirProfileMode,
    ) -> Vec<String> {
        if let Some(scoring) = &self.scoring {
            return vec![scoring.profile.clone()];
        }

        let mut names = match mode {
            OctofhirProfileMode::ResourceType => vec![resource_type.to_string()],
            OctofhirProfileMode::MetaProfile => meta_profiles(resource),
            OctofhirProfileMode::ResourceTypeAndMetaProfile => {
                let mut names = vec![resource_type.to_string()];
                names.extend(meta_profiles(resource));
                names
            }
        };

        if matches!(mode, OctofhirProfileMode::MetaProfile) && names.is_empty() {
            names.push(resource_type.to_string());
        }
        names
    }
}

fn expected_valid_from_outcome(
    outcome: Option<&ValidatorOutcome>,
    validator_dir: &Path,
) -> Option<bool> {
    match outcome? {
        ValidatorOutcome::Path(path) => {
            let path = validator_dir.join("outcomes").join(path);
            fs::read_to_string(path)
                .ok()
                .and_then(|content| serde_json::from_str::<ExpectedOutcome>(&content).ok())
                .map(|outcome| outcome.is_valid())
        }
        ValidatorOutcome::Inline(inline) => Some(inline.is_valid()),
    }
}

impl ExpectedOutcome {
    fn is_valid(&self) -> bool {
        !self.issue.iter().any(|issue| {
            matches!(
                issue.severity.as_str(),
                "fatal" | "error" | "Fatal" | "Error"
            )
        })
    }
}

impl InlineOutcome {
    fn is_valid(&self) -> bool {
        if let Some(error_count) = self.error_count {
            return error_count == 0;
        }
        self.outcome
            .as_ref()
            .and_then(operation_outcome_validity)
            .unwrap_or(true)
    }
}

fn operation_outcome_validity(value: &Value) -> Option<bool> {
    let issues = value.get("issue")?.as_array()?;
    Some(!issues.iter().any(|issue| {
        matches!(
            issue.get("severity").and_then(Value::as_str),
            Some("fatal" | "error")
        )
    }))
}

fn meta_profiles(resource: &Value) -> Vec<String> {
    resource
        .get("meta")
        .and_then(|meta| meta.get("profile"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn profile_mode_arg(mode: OctofhirProfileMode) -> &'static str {
    match mode {
        OctofhirProfileMode::ResourceType => "resource-type",
        OctofhirProfileMode::MetaProfile => "meta-profile",
        OctofhirProfileMode::ResourceTypeAndMetaProfile => "resource-type-and-meta-profile",
    }
}

fn default_true() -> bool {
    true
}

fn print_case_result(report: &CaseReport) {
    let expected = match report.expected_valid {
        Some(true) => "VALID",
        Some(false) => "INVALID",
        None => "N/A",
    };
    let actual = match report.actual_valid {
        Some(true) => "VALID",
        Some(false) => "INVALID",
        None => "N/A",
    };
    if report.skipped {
        println!(
            "  SKIP {} expected={} reason={}",
            report.name,
            expected,
            report.skip_reason.as_deref().unwrap_or("")
        );
    } else if report.mismatch {
        println!(
            "  FAIL {} expected={} actual={} errors={}",
            report.name, expected, actual, report.error_count
        );
    } else {
        println!("  PASS {} {}", report.name, expected);
    }
}

fn print_summary(report: &Report, report_path: &Path) {
    println!("official-fhir-runner report: {}", report_path.display());
    println!("fhirpath constraints: {}", report.fhirpath_constraints);
    println!(
        "selected Java-comparable cases: {}",
        report.java_comparable_cases
    );
    println!(
        "agreement with Java expected outcomes: {}/{} ({:.1}%)",
        report.java_matches, report.completed_cases, report.agreement_percent
    );
    println!(
        "passed={}, failed={}, skipped={}",
        report.passed, report.failed, report.skipped
    );
    println!(
        "speed: {:.1} cases/sec, {:.3} ms/completed case ({:.1} ms total)",
        report.cases_per_second, report.avg_ms_per_completed_case, report.elapsed_ms
    );

    for case in report.cases.iter().filter(|case| case.mismatch).take(20) {
        println!(
            "  mismatch: {} expected={:?} actual={:?} errors={} first={}",
            case.name,
            case.expected_valid,
            case.actual_valid,
            case.error_count,
            case.errors
                .first()
                .and_then(|issue| issue.message.as_deref())
                .unwrap_or("")
        );
    }
}
