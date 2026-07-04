use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use octofhir_canonical_manager::{CanonicalManager, FcmConfig};
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
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

#[derive(Debug, Parser)]
#[command(name = "validation-lab")]
#[command(about = "FHIR validation parity and performance lab")]
struct Args {
    #[arg(long, value_enum, default_value_t = Mode::JavaParity)]
    mode: Mode,

    #[arg(
        long,
        help = "Root directory with JSON fixtures",
        default_value = "octofhir-fhirschema/tests/fixtures/r4"
    )]
    fixtures: PathBuf,

    #[arg(
        long,
        help = "HL7 Java validator jar. Falls back to HL7_VALIDATOR_JAR, then cached download."
    )]
    java_validator_jar: Option<PathBuf>,

    #[arg(
        long,
        default_value = "https://github.com/hapifhir/org.hl7.fhir.core/releases/latest/download/validator_cli.jar",
        help = "URL used when downloading the HL7 Java validator"
    )]
    java_validator_url: String,

    #[arg(
        long,
        help = "Cache path for downloaded HL7 Java validator jar. Defaults under --output."
    )]
    java_validator_cache: Option<PathBuf>,

    #[arg(long, help = "Optional RH CLI binary. Falls back to RH_BIN.")]
    rh_bin: Option<PathBuf>,

    #[arg(
        long,
        default_value = "4.0.1",
        help = "FHIR version passed to Java validator"
    )]
    java_fhir_version: String,

    #[arg(
        long,
        default_value = "n/a",
        help = "Terminology endpoint passed as '-tx'. Use 'n/a' for offline structural parity."
    )]
    java_tx: String,

    #[arg(
        long = "java-ig",
        help = "ImplementationGuide/package passed to Java validator as '-ig'. Can be repeated."
    )]
    java_igs: Vec<String>,

    #[arg(long, value_enum, default_value_t = OctofhirProfileMode::ResourceTypeAndMetaProfile)]
    octofhir_profile_mode: OctofhirProfileMode,

    #[arg(long, value_enum, default_value_t = OctofhirRunner::Cli)]
    octofhir_runner: OctofhirRunner,

    #[arg(
        long = "schema-package-dir",
        help = "FHIR package directory containing StructureDefinition JSON files to add to OctoFHIR schemas. Can be repeated."
    )]
    schema_package_dirs: Vec<PathBuf>,

    #[arg(
        long = "schema-package",
        help = "FHIR package spec installed through octofhir-canonical-manager before validation, e.g. hl7.fhir.us.core#6.1.0. Can be repeated."
    )]
    schema_packages: Vec<String>,

    #[arg(
        long,
        help = "Isolated Java user.home for validator package/cache writes. Defaults under --output."
    )]
    java_user_home: Option<PathBuf>,

    #[arg(
        long,
        default_value_t = 90,
        help = "Per-resource Java validator timeout in seconds"
    )]
    java_timeout_secs: u64,

    #[arg(
        long,
        default_value_t = 1000,
        help = "Iterations for in-process octofhir throughput measurement"
    )]
    iterations: usize,

    #[arg(
        long,
        default_value = "target/validation-lab",
        help = "Directory for generated reports and Java OperationOutcome files"
    )]
    output: PathBuf,

    #[arg(long, help = "Exit non-zero when Java and octofhir validity disagree")]
    fail_on_mismatch: bool,

    #[arg(
        long = "ignore-java-message-id",
        help = "Java OperationOutcome message-id to exclude from spec-comparable parity. Can be repeated. Defaults to known Java policy checks."
    )]
    ignore_java_message_ids: Vec<String>,

    #[arg(
        long,
        help = "Do not exclude known Java policy checks from parity; raw Java validity becomes the comparable result."
    )]
    strict_java_policy: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    FetchJavaValidator,
    JavaParity,
    OctofhirOnly,
    ValidateResource,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OctofhirProfileMode {
    /// Validate only against resourceType, e.g. Patient.
    ResourceType,
    /// Validate only against meta.profile entries when present; fallback to resourceType.
    MetaProfile,
    /// Validate against resourceType and every meta.profile entry, matching RH validate_auto shape.
    ResourceTypeAndMetaProfile,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OctofhirRunner {
    Cli,
    Library,
}

#[derive(Debug)]
struct FixtureCase {
    name: String,
    path: PathBuf,
    resource_type: String,
    octofhir_schema_names: Vec<String>,
    expected: ExpectedValidity,
    resource: Value,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum ExpectedValidity {
    Valid,
    Invalid,
    Unknown,
}

#[derive(Debug, Serialize)]
struct Report {
    fixtures_root: PathBuf,
    case_count: usize,
    octofhir: OctofhirSummary,
    java: Option<JavaSummary>,
    rh: Option<RhSummary>,
    cases: Vec<CaseReport>,
}

#[derive(Debug, Serialize)]
struct OctofhirSummary {
    strategy: &'static str,
    fhirpath_constraints: bool,
    iterations: usize,
    total_validations: usize,
    elapsed_ms: f64,
    validations_per_second: f64,
}

#[derive(Debug, Serialize)]
struct JavaSummary {
    validator_jar: PathBuf,
    fhir_version: String,
    tx: String,
    cases_run: usize,
    raw_mismatches: usize,
    mismatches: usize,
    java_policy_differences: usize,
    ignored_message_ids: Vec<String>,
    elapsed_ms: f64,
}

#[derive(Debug, Serialize)]
struct RhSummary {
    rh_bin: PathBuf,
    cases_run: usize,
    mismatches_with_octofhir: usize,
    mismatches_with_java: Option<usize>,
}

#[derive(Debug, Serialize)]
struct CaseReport {
    name: String,
    path: PathBuf,
    resource_type: String,
    expected: ExpectedValidity,
    octofhir_schema_names: Vec<String>,
    octofhir_valid: bool,
    octofhir_error_count: usize,
    octofhir_errors: Vec<ValidationIssueSummary>,
    octofhir_cli_status: Option<i32>,
    octofhir_cli_elapsed_ms: Option<f64>,
    octofhir_cli_stderr: Option<String>,
    octofhir_avg_us: f64,
    octofhir_validations_per_second: f64,
    java_valid: Option<bool>,
    java_error_count: Option<usize>,
    java_comparable_valid: Option<bool>,
    java_comparable_error_count: Option<usize>,
    java_ignored_policy_error_count: Option<usize>,
    java_elapsed_ms: Option<f64>,
    java_raw_mismatch: bool,
    mismatch: bool,
    java_policy_difference: bool,
    java_issues: Vec<ExternalIssueSummary>,
    java_status: Option<i32>,
    java_stderr: Option<String>,
    rh_valid: Option<bool>,
    rh_error_count: Option<usize>,
    rh_elapsed_ms: Option<f64>,
    rh_mismatch_with_octofhir: bool,
    rh_mismatch_with_java: Option<bool>,
    rh_status: Option<i32>,
    rh_stderr: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let ignored_java_message_ids = ignored_java_message_ids(&args);
    fs::create_dir_all(&args.output)
        .with_context(|| format!("failed to create {}", args.output.display()))?;
    let java_user_home = args
        .java_user_home
        .clone()
        .unwrap_or_else(|| args.output.join("java-home"));

    if matches!(args.mode, Mode::FetchJavaValidator) {
        let jar = resolve_java_validator_jar(&args).await?;
        fs::create_dir_all(&java_user_home)
            .with_context(|| format!("failed to create {}", java_user_home.display()))?;
        println!("java validator jar: {}", jar.display());
        println!("java user.home: {}", java_user_home.display());
        return Ok(());
    }

    if matches!(args.mode, Mode::ValidateResource) {
        let cases = load_cases(&args.fixtures, args.octofhir_profile_mode)
            .with_context(|| format!("failed to load fixture from {}", args.fixtures.display()))?;
        if cases.len() != 1 {
            bail!("validate-resource mode requires exactly one JSON fixture");
        }
        let validator =
            create_r4_validator_with_fhirpath(&args.schema_package_dirs, &args.schema_packages)
                .await?;
        let case = &cases[0];
        let result = validator
            .validate(&case.resource, case.octofhir_schema_names.clone())
            .await;
        let output = OctofhirCliOutput {
            name: case.name.clone(),
            resource_type: case.resource_type.clone(),
            schema_names: case.octofhir_schema_names.clone(),
            valid: result.valid,
            error_count: result.errors.len(),
            errors: result
                .errors
                .iter()
                .map(|error| ValidationIssueSummary {
                    error_type: error.error_type.clone(),
                    message: error.message.clone(),
                    path: error.path.clone(),
                })
                .collect(),
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    let cases = load_cases(&args.fixtures, args.octofhir_profile_mode)
        .with_context(|| format!("failed to load fixtures from {}", args.fixtures.display()))?;
    if cases.is_empty() {
        bail!("no JSON fixtures found under {}", args.fixtures.display());
    }

    let validator =
        create_r4_validator_with_fhirpath(&args.schema_package_dirs, &args.schema_packages).await?;
    let current_exe = env::current_exe().context("failed to resolve current executable")?;

    let mut case_reports = Vec::with_capacity(cases.len());
    let mut octofhir_initial = Vec::with_capacity(cases.len());
    for case in &cases {
        let octofhir_result = match args.octofhir_runner {
            OctofhirRunner::Library => validate_octofhir_library(&validator, case).await,
            OctofhirRunner::Cli => run_octofhir_cli(
                &current_exe,
                &case.path,
                args.octofhir_profile_mode,
                &args.schema_package_dirs,
                &args.schema_packages,
            )?,
        };
        octofhir_initial.push((octofhir_result.valid, octofhir_result.error_count));
        case_reports.push(CaseReport {
            name: case.name.clone(),
            path: case.path.clone(),
            resource_type: case.resource_type.clone(),
            expected: case.expected,
            octofhir_schema_names: case.octofhir_schema_names.clone(),
            octofhir_valid: octofhir_result.valid,
            octofhir_error_count: octofhir_result.error_count,
            octofhir_errors: octofhir_result.errors,
            octofhir_cli_status: octofhir_result.status,
            octofhir_cli_elapsed_ms: octofhir_result.elapsed_ms,
            octofhir_cli_stderr: octofhir_result.stderr,
            octofhir_avg_us: 0.0,
            octofhir_validations_per_second: 0.0,
            java_valid: None,
            java_error_count: None,
            java_comparable_valid: None,
            java_comparable_error_count: None,
            java_ignored_policy_error_count: None,
            java_elapsed_ms: None,
            java_raw_mismatch: false,
            mismatch: false,
            java_policy_difference: false,
            java_issues: vec![],
            java_status: None,
            java_stderr: None,
            rh_valid: None,
            rh_error_count: None,
            rh_elapsed_ms: None,
            rh_mismatch_with_octofhir: false,
            rh_mismatch_with_java: None,
            rh_status: None,
            rh_stderr: None,
        });
    }

    let started = Instant::now();
    for _ in 0..args.iterations {
        for case in &cases {
            let _ = validator
                .validate(&case.resource, case.octofhir_schema_names.clone())
                .await;
        }
    }
    let elapsed = started.elapsed();
    let total_validations = args.iterations * cases.len();
    let elapsed_secs = elapsed.as_secs_f64();
    let octofhir = OctofhirSummary {
        strategy: "sequential_hot_loop",
        fhirpath_constraints: true,
        iterations: args.iterations,
        total_validations,
        elapsed_ms: elapsed_secs * 1000.0,
        validations_per_second: if elapsed_secs > 0.0 {
            total_validations as f64 / elapsed_secs
        } else {
            0.0
        },
    };

    for (idx, case) in cases.iter().enumerate() {
        let started = Instant::now();
        for _ in 0..args.iterations {
            let _ = validator
                .validate(&case.resource, case.octofhir_schema_names.clone())
                .await;
        }
        let elapsed_secs = started.elapsed().as_secs_f64();
        let validations_per_second = if elapsed_secs > 0.0 {
            args.iterations as f64 / elapsed_secs
        } else {
            0.0
        };
        let report = &mut case_reports[idx];
        report.octofhir_validations_per_second = validations_per_second;
        report.octofhir_avg_us = if validations_per_second > 0.0 {
            1_000_000.0 / validations_per_second
        } else {
            0.0
        };
    }

    let mut java_summary = None;
    if matches!(args.mode, Mode::JavaParity) {
        let jar = resolve_java_validator_jar(&args).await?;

        let java_out_dir = args.output.join("java-operationoutcomes");
        fs::create_dir_all(&java_out_dir)
            .with_context(|| format!("failed to create {}", java_out_dir.display()))?;
        fs::create_dir_all(&java_user_home)
            .with_context(|| format!("failed to create {}", java_user_home.display()))?;

        let mut raw_mismatches = 0;
        let mut mismatches = 0;
        let mut java_policy_differences = 0;
        let java_started = Instant::now();
        for (idx, case) in cases.iter().enumerate() {
            let java_result = run_java_validator(
                &jar,
                &java_user_home,
                &case.path,
                &java_out_dir.join(format!("{}.json", sanitize_filename(&case.name))),
                &args.java_fhir_version,
                &args.java_tx,
                &args.java_igs,
                Duration::from_secs(args.java_timeout_secs),
            )
            .with_context(|| format!("failed to run Java validator for {}", case.path.display()))?;

            let octo_valid = octofhir_initial[idx].0;
            let comparable = java_result.comparable(&ignored_java_message_ids);
            let raw_mismatch = octo_valid != java_result.valid;
            let mismatch = octo_valid != comparable.valid;
            let policy_difference = raw_mismatch && !mismatch;
            if raw_mismatch {
                raw_mismatches += 1;
            }
            if mismatch {
                mismatches += 1;
            }
            if policy_difference {
                java_policy_differences += 1;
            }

            let report = &mut case_reports[idx];
            report.java_valid = Some(java_result.valid);
            report.java_error_count = Some(java_result.error_count);
            report.java_comparable_valid = Some(comparable.valid);
            report.java_comparable_error_count = Some(comparable.error_count);
            report.java_ignored_policy_error_count = Some(comparable.ignored_error_count);
            report.java_elapsed_ms = Some(java_result.elapsed_ms);
            report.java_raw_mismatch = raw_mismatch;
            report.mismatch = mismatch;
            report.java_policy_difference = policy_difference;
            report.java_issues = java_result.issues;
            report.java_status = java_result.status;
            report.java_stderr = java_result.stderr;
        }
        let java_elapsed_ms = java_started.elapsed().as_secs_f64() * 1000.0;

        java_summary = Some(JavaSummary {
            validator_jar: jar,
            fhir_version: args.java_fhir_version.clone(),
            tx: args.java_tx.clone(),
            cases_run: cases.len(),
            raw_mismatches,
            mismatches,
            java_policy_differences,
            ignored_message_ids: ignored_java_message_ids,
            elapsed_ms: java_elapsed_ms,
        });
    }

    let mut rh_summary = None;
    if let Some(rh_bin) = args
        .rh_bin
        .clone()
        .or_else(|| env::var_os("RH_BIN").map(PathBuf::from))
    {
        let mut mismatches_with_octofhir = 0;
        let mut mismatches_with_java = 0;
        let compare_with_java = java_summary.is_some();

        for (idx, case) in cases.iter().enumerate() {
            let rh_result = run_rh_validator(&rh_bin, &case.path).with_context(|| {
                format!("failed to run RH validator for {}", case.path.display())
            })?;

            let octo_valid = octofhir_initial[idx].0;
            let rh_mismatch_with_octofhir = octo_valid != rh_result.valid;
            if rh_mismatch_with_octofhir {
                mismatches_with_octofhir += 1;
            }

            let rh_mismatch_with_java = case_reports[idx]
                .java_comparable_valid
                .map(|java_valid| java_valid != rh_result.valid);
            if rh_mismatch_with_java == Some(true) {
                mismatches_with_java += 1;
            }

            let report = &mut case_reports[idx];
            report.rh_valid = Some(rh_result.valid);
            report.rh_error_count = Some(rh_result.error_count);
            report.rh_elapsed_ms = Some(rh_result.elapsed_ms);
            report.rh_mismatch_with_octofhir = rh_mismatch_with_octofhir;
            report.rh_mismatch_with_java = rh_mismatch_with_java;
            report.rh_status = rh_result.status;
            report.rh_stderr = rh_result.stderr;
        }

        rh_summary = Some(RhSummary {
            rh_bin,
            cases_run: cases.len(),
            mismatches_with_octofhir,
            mismatches_with_java: compare_with_java.then_some(mismatches_with_java),
        });
    }

    let report = Report {
        fixtures_root: args.fixtures.clone(),
        case_count: cases.len(),
        octofhir,
        java: java_summary,
        rh: rh_summary,
        cases: case_reports,
    };

    let report_path = args.output.join("validation-lab-report.json");
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("failed to write {}", report_path.display()))?;

    print_summary(&report, &report_path);

    if args.fail_on_mismatch && report.java.as_ref().is_some_and(|java| java.mismatches > 0) {
        bail!("Java parity mismatches found");
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct ValidationIssueSummary {
    error_type: String,
    message: Option<String>,
    path: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExternalIssueSummary {
    severity: Option<String>,
    code: Option<String>,
    message_id: Option<String>,
    details: Option<String>,
    expression: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OctofhirCliOutput {
    name: String,
    resource_type: String,
    schema_names: Vec<String>,
    valid: bool,
    error_count: usize,
    errors: Vec<ValidationIssueSummary>,
}

#[derive(Debug)]
struct OctofhirRunResult {
    valid: bool,
    error_count: usize,
    errors: Vec<ValidationIssueSummary>,
    status: Option<i32>,
    elapsed_ms: Option<f64>,
    stderr: Option<String>,
}

async fn resolve_java_validator_jar(args: &Args) -> Result<PathBuf> {
    if let Some(path) = args
        .java_validator_jar
        .clone()
        .or_else(|| env::var_os("HL7_VALIDATOR_JAR").map(PathBuf::from))
    {
        if !path.exists() {
            bail!("Java validator jar does not exist: {}", path.display());
        }
        return Ok(path);
    }

    let cache_path = args
        .java_validator_cache
        .clone()
        .unwrap_or_else(|| args.output.join("validator_cli.jar"));
    if cache_path.exists() {
        return Ok(cache_path);
    }

    download_java_validator(&args.java_validator_url, &cache_path).await?;
    Ok(cache_path)
}

async fn download_java_validator(url: &str, cache_path: &Path) -> Result<()> {
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    println!("downloading HL7 Java validator: {url}");
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("failed to download {url}"))?;
    if !response.status().is_success() {
        bail!("failed to download {url}: HTTP {}", response.status());
    }

    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("failed to read response body from {url}"))?;
    if bytes.len() < 1024 * 1024 {
        bail!(
            "downloaded validator jar is unexpectedly small: {} bytes",
            bytes.len()
        );
    }

    let tmp_path = cache_path.with_extension("jar.part");
    fs::write(&tmp_path, bytes)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    fs::rename(&tmp_path, cache_path)
        .with_context(|| format!("failed to move {} into place", cache_path.display()))?;
    Ok(())
}

fn load_cases(root: &Path, octofhir_profile_mode: OctofhirProfileMode) -> Result<Vec<FixtureCase>> {
    let mut files = Vec::new();
    let name_root;
    if root.is_file() {
        if root.extension() == Some(OsStr::new("json")) {
            files.push(root.to_path_buf());
        }
        name_root = root.parent().unwrap_or_else(|| Path::new(""));
    } else {
        collect_json_files(root, &mut files)?;
        name_root = root;
    }
    files.sort();

    files
        .into_iter()
        .map(|path| {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let resource: Value = serde_json::from_str(&content)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            let resource_type = resource
                .get("resourceType")
                .and_then(Value::as_str)
                .unwrap_or("Resource")
                .to_string();
            let octofhir_schema_names =
                octofhir_schema_names(&resource, &resource_type, octofhir_profile_mode);
            let rel = path.strip_prefix(name_root).unwrap_or(&path);
            let name = rel
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");

            Ok(FixtureCase {
                expected: infer_expected_validity(&path),
                name,
                path,
                resource_type,
                octofhir_schema_names,
                resource,
            })
        })
        .collect()
}

fn octofhir_schema_names(
    resource: &Value,
    resource_type: &str,
    mode: OctofhirProfileMode,
) -> Vec<String> {
    let profiles = resource
        .get("meta")
        .and_then(|meta| meta.get("profile"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string);

    match mode {
        OctofhirProfileMode::ResourceType => vec![resource_type.to_string()],
        OctofhirProfileMode::MetaProfile => {
            let profiles = profiles.collect::<Vec<_>>();
            if profiles.is_empty() {
                vec![resource_type.to_string()]
            } else {
                profiles
            }
        }
        OctofhirProfileMode::ResourceTypeAndMetaProfile => {
            let mut names = vec![resource_type.to_string()];
            names.extend(profiles);
            names
        }
    }
}

fn collect_json_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_json_files(&path, out)?;
        } else if path.extension() == Some(OsStr::new("json")) {
            out.push(path);
        }
    }
    Ok(())
}

fn infer_expected_validity(path: &Path) -> ExpectedValidity {
    let parts: Vec<_> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect();
    if parts.iter().any(|part| part == "valid") {
        ExpectedValidity::Valid
    } else if parts.iter().any(|part| part == "invalid") {
        ExpectedValidity::Invalid
    } else {
        ExpectedValidity::Unknown
    }
}

#[derive(Debug)]
struct JavaRunResult {
    valid: bool,
    error_count: usize,
    issues: Vec<ExternalIssueSummary>,
    elapsed_ms: f64,
    status: Option<i32>,
    stderr: Option<String>,
}

#[derive(Debug)]
struct ComparableJavaResult {
    valid: bool,
    error_count: usize,
    ignored_error_count: usize,
}

impl JavaRunResult {
    fn comparable(&self, ignored_message_ids: &[String]) -> ComparableJavaResult {
        let mut error_count = 0usize;
        let mut ignored_error_count = 0usize;

        for issue in &self.issues {
            if !is_error_severity(issue.severity.as_deref()) {
                continue;
            }

            let ignored = issue
                .message_id
                .as_deref()
                .is_some_and(|message_id| ignored_message_ids.iter().any(|id| id == message_id));
            if ignored {
                ignored_error_count += 1;
            } else {
                error_count += 1;
            }
        }

        if self.issues.is_empty() && self.error_count > 0 {
            error_count = self.error_count;
        }

        ComparableJavaResult {
            valid: error_count == 0,
            error_count,
            ignored_error_count,
        }
    }
}

#[derive(Debug)]
struct RhRunResult {
    valid: bool,
    error_count: usize,
    elapsed_ms: f64,
    status: Option<i32>,
    stderr: Option<String>,
}

async fn validate_octofhir_library(
    validator: &FhirValidator,
    case: &FixtureCase,
) -> OctofhirRunResult {
    let result = validator
        .validate(&case.resource, case.octofhir_schema_names.clone())
        .await;
    OctofhirRunResult {
        valid: result.valid,
        error_count: result.errors.len(),
        errors: result
            .errors
            .iter()
            .map(|error| ValidationIssueSummary {
                error_type: error.error_type.clone(),
                message: error.message.clone(),
                path: error.path.clone(),
            })
            .collect(),
        status: None,
        elapsed_ms: None,
        stderr: None,
    }
}

async fn create_r4_validator_with_fhirpath(
    schema_package_dirs: &[PathBuf],
    schema_packages: &[String],
) -> Result<FhirValidator> {
    let mut schemas = get_schemas(FhirVersion::R4).clone();
    let package_schema_count = load_package_schemas(schema_package_dirs, &mut schemas)?;
    if package_schema_count > 0 {
        println!("loaded {package_schema_count} package-dir StructureDefinition schemas");
    }
    let canonical_schema_count =
        load_canonical_package_schemas(schema_packages, &mut schemas).await?;
    if canonical_schema_count > 0 {
        println!("loaded {canonical_schema_count} canonical-manager StructureDefinition schemas");
    }

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

    Ok(FhirValidator::from_schemas(schemas, Some(fhirpath_engine)))
}

async fn load_canonical_package_schemas(
    package_specs: &[String],
    schemas: &mut HashMap<String, FhirSchema>,
) -> Result<usize> {
    if package_specs.is_empty() {
        return Ok(0);
    }

    let mut config = FcmConfig::load()
        .await
        .context("failed to load canonical manager config")?;
    config.storage.cache_dir = PathBuf::from("target/validation-lab/fcm/cache");
    config.storage.packages_dir = PathBuf::from("target/validation-lab/fcm/packages");
    let canonical_manager = CanonicalManager::new(config)
        .await
        .context("failed to initialize canonical manager")?;

    let mut loaded = 0usize;
    for spec in package_specs {
        let (name, version) = parse_package_spec(spec)?;
        println!("installing FHIR package through canonical manager: {name}#{version}");
        canonical_manager
            .install_package(&name, &version)
            .await
            .with_context(|| format!("failed to install {name}#{version}"))?;

        let resource_indices = canonical_manager
            .find_by_type_and_package("StructureDefinition", &name)
            .await
            .with_context(|| format!("failed to list StructureDefinitions for {name}"))?;

        for resource_index in resource_indices {
            let resolved = canonical_manager
                .resolve_with_fhir_version(
                    &resource_index.canonical_url,
                    &resource_index.fhir_version,
                )
                .await
                .with_context(|| {
                    format!(
                        "failed to resolve {} for FHIR {}",
                        resource_index.canonical_url, resource_index.fhir_version
                    )
                })?;
            let structure_definition: StructureDefinition =
                serde_json::from_value(resolved.resource.content.clone()).with_context(|| {
                    format!(
                        "failed to decode StructureDefinition {}",
                        resource_index.canonical_url
                    )
                })?;
            let schema = translate(structure_definition, None).with_context(|| {
                format!(
                    "failed to translate StructureDefinition {}",
                    resource_index.canonical_url
                )
            })?;

            let id = resolved
                .resource
                .content
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(&schema.name)
                .to_string();
            insert_schema_aliases(schemas, id, schema.clone());
            schemas.insert(resource_index.canonical_url.clone(), schema);
            loaded += 1;
        }
    }

    Ok(loaded)
}

fn parse_package_spec(spec: &str) -> Result<(String, String)> {
    let Some((name, version)) = spec.split_once('#') else {
        bail!("schema package must be name#version, got {spec}");
    };
    if name.is_empty() || version.is_empty() {
        bail!("schema package must be name#version, got {spec}");
    }
    Ok((name.to_string(), version.to_string()))
}

fn load_package_schemas(
    package_dirs: &[PathBuf],
    schemas: &mut HashMap<String, FhirSchema>,
) -> Result<usize> {
    let mut loaded = 0usize;
    for package_dir in package_dirs {
        let mut files = Vec::new();
        collect_json_files(package_dir, &mut files)
            .with_context(|| format!("failed to scan {}", package_dir.display()))?;

        for path in files {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let value: Value = serde_json::from_str(&content)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            if value.get("resourceType").and_then(Value::as_str) != Some("StructureDefinition") {
                continue;
            }

            let structure_definition: StructureDefinition = serde_json::from_value(value)
                .with_context(|| {
                    format!("failed to decode StructureDefinition {}", path.display())
                })?;
            let schema = translate(structure_definition, None)
                .with_context(|| format!("failed to translate {}", path.display()))?;
            insert_schema_aliases(schemas, schema.name.clone(), schema);
            loaded += 1;
        }
    }
    Ok(loaded)
}

fn insert_schema_aliases(
    schemas: &mut HashMap<String, FhirSchema>,
    primary_key: String,
    schema: FhirSchema,
) {
    let url = schema.url.clone();
    let name = schema.name.clone();
    schemas.insert(primary_key, schema.clone());
    schemas.insert(name, schema.clone());
    schemas.insert(url, schema);
}

fn run_octofhir_cli(
    bin: &Path,
    input: &Path,
    profile_mode: OctofhirProfileMode,
    schema_package_dirs: &[PathBuf],
    schema_packages: &[String],
) -> Result<OctofhirRunResult> {
    let started = Instant::now();
    let mut command = Command::new(bin);
    command
        .arg("--mode")
        .arg("validate-resource")
        .arg("--fixtures")
        .arg(input)
        .arg("--octofhir-profile-mode")
        .arg(profile_mode_arg(profile_mode));
    for package_dir in schema_package_dirs {
        command.arg("--schema-package-dir").arg(package_dir);
    }
    for package in schema_packages {
        command.arg("--schema-package").arg(package);
    }

    let output_result = command
        .output()
        .with_context(|| "failed to spawn octofhir CLI")?;

    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let stdout = String::from_utf8_lossy(&output_result.stdout);
    let parsed = extract_json_value(&stdout)
        .and_then(|value| serde_json::from_value::<OctofhirCliOutput>(value).ok());

    if let Some(parsed) = parsed {
        Ok(OctofhirRunResult {
            valid: parsed.valid,
            error_count: parsed.error_count,
            errors: parsed.errors,
            status: output_result.status.code(),
            elapsed_ms: Some(elapsed_ms),
            stderr: non_empty_string(output_result.stderr),
        })
    } else {
        let stderr = non_empty_string(output_result.stderr);
        Ok(OctofhirRunResult {
            valid: output_result.status.success(),
            error_count: usize::from(!output_result.status.success()),
            errors: vec![],
            status: output_result.status.code(),
            elapsed_ms: Some(elapsed_ms),
            stderr,
        })
    }
}

fn profile_mode_arg(mode: OctofhirProfileMode) -> &'static str {
    match mode {
        OctofhirProfileMode::ResourceType => "resource-type",
        OctofhirProfileMode::MetaProfile => "meta-profile",
        OctofhirProfileMode::ResourceTypeAndMetaProfile => "resource-type-and-meta-profile",
    }
}

fn ignored_java_message_ids(args: &Args) -> Vec<String> {
    if args.strict_java_policy {
        Vec::new()
    } else if args.ignore_java_message_ids.is_empty() {
        vec!["TYPE_SPECIFIC_CHECKS_DT_URL_EXAMPLE".to_string()]
    } else {
        args.ignore_java_message_ids.clone()
    }
}

fn run_java_validator(
    jar: &Path,
    java_user_home: &Path,
    input: &Path,
    output: &Path,
    fhir_version: &str,
    tx: &str,
    igs: &[String],
    timeout: Duration,
) -> Result<JavaRunResult> {
    let started = Instant::now();
    let mut command = Command::new("java");
    command
        .arg(format!("-Duser.home={}", java_user_home.display()))
        .arg("-jar")
        .arg(jar)
        .arg(input)
        .arg("-version")
        .arg(fhir_version)
        .arg("-tx")
        .arg(tx);
    for ig in igs {
        command.arg("-ig").arg(ig);
    }
    command
        .arg("-output")
        .arg(output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().with_context(|| "failed to spawn java")?;
    let timed_out = match child.wait_timeout(timeout)? {
        Some(_) => false,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            true
        }
    };
    let output_result = child
        .wait_with_output()
        .with_context(|| "failed to collect java output")?;

    let outcome = fs::read_to_string(output)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| operation_outcome_status(&value));

    let (valid, error_count, issues) = outcome.unwrap_or_else(|| {
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        let error_count = count_error_like_lines(&stderr);
        (
            output_result.status.success() && error_count == 0,
            error_count,
            Vec::new(),
        )
    });

    Ok(JavaRunResult {
        valid: !timed_out && valid,
        error_count: if timed_out {
            error_count + 1
        } else {
            error_count
        },
        issues,
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        status: output_result.status.code(),
        stderr: if timed_out {
            Some(format!(
                "Java validator timed out after {}s",
                timeout.as_secs()
            ))
        } else {
            non_empty_string(output_result.stderr)
        },
    })
}

fn run_rh_validator(rh_bin: &Path, input: &Path) -> Result<RhRunResult> {
    let started = Instant::now();
    let output_result = Command::new(rh_bin)
        .arg("validate")
        .arg("resource")
        .arg("--input")
        .arg(input)
        .arg("--report-format")
        .arg("json")
        .output()
        .with_context(|| "failed to spawn rh")?;

    let stdout = String::from_utf8_lossy(&output_result.stdout);
    let parsed = extract_json_value(&stdout)
        .and_then(|value| {
            let valid = value.get("valid")?.as_bool()?;
            let error_count = value
                .get("errors")
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize;
            Some((valid, error_count))
        })
        .unwrap_or_else(|| {
            let stderr = String::from_utf8_lossy(&output_result.stderr);
            let error_count = count_error_like_lines(&stderr);
            (
                output_result.status.success() && error_count == 0,
                error_count,
            )
        });

    Ok(RhRunResult {
        valid: parsed.0,
        error_count: parsed.1,
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        status: output_result.status.code(),
        stderr: non_empty_string(output_result.stderr),
    })
}

fn extract_json_value(stdout: &str) -> Option<Value> {
    for (idx, ch) in stdout.char_indices() {
        if ch == '{' {
            if let Ok(value) = serde_json::from_str::<Value>(&stdout[idx..]) {
                return Some(value);
            }
        }
    }
    None
}

fn operation_outcome_status(value: &Value) -> Option<(bool, usize, Vec<ExternalIssueSummary>)> {
    let issues = value.get("issue")?.as_array()?;
    let summaries = issues
        .iter()
        .map(external_issue_summary)
        .collect::<Vec<_>>();
    let error_count = summaries
        .iter()
        .filter(|issue| is_error_severity(issue.severity.as_deref()))
        .count();
    Some((error_count == 0, error_count, summaries))
}

fn external_issue_summary(issue: &Value) -> ExternalIssueSummary {
    ExternalIssueSummary {
        severity: issue
            .get("severity")
            .and_then(Value::as_str)
            .map(str::to_string),
        code: issue
            .get("code")
            .and_then(Value::as_str)
            .map(str::to_string),
        message_id: operation_outcome_message_id(issue),
        details: issue
            .get("details")
            .and_then(|details| details.get("text"))
            .and_then(Value::as_str)
            .map(str::to_string),
        expression: issue
            .get("expression")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
    }
}

fn operation_outcome_message_id(issue: &Value) -> Option<String> {
    issue
        .get("extension")
        .and_then(Value::as_array)?
        .iter()
        .find(|extension| {
            extension.get("url").and_then(Value::as_str)
                == Some("http://hl7.org/fhir/StructureDefinition/operationoutcome-message-id")
        })
        .and_then(|extension| extension.get("valueCode"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn is_error_severity(severity: Option<&str>) -> bool {
    matches!(severity, Some("fatal" | "error"))
}

fn count_error_like_lines(stderr: &str) -> usize {
    stderr
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("error") || lower.contains("fatal")
        })
        .count()
}

fn non_empty_string(bytes: Vec<u8>) -> Option<String> {
    let text = String::from_utf8_lossy(&bytes).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn print_summary(report: &Report, report_path: &Path) {
    println!("validation-lab report: {}", report_path.display());
    println!("fixtures: {}", report.case_count);
    println!(
        "octofhir {} (fhirpath_constraints={}): {:.0} validations/sec, {:.4} ms/resource ({} validations, {:.1} ms)",
        report.octofhir.strategy,
        report.octofhir.fhirpath_constraints,
        report.octofhir.validations_per_second,
        report.octofhir.elapsed_ms / report.octofhir.total_validations as f64,
        report.octofhir.total_validations,
        report.octofhir.elapsed_ms
    );

    if let Some(java) = &report.java {
        println!(
            "java parity: {} cases, {} spec-comparable mismatches, {} raw mismatches, {} java-policy differences",
            java.cases_run, java.mismatches, java.raw_mismatches, java.java_policy_differences
        );
        if !java.ignored_message_ids.is_empty() {
            println!(
                "java parity: ignored policy message ids: {}",
                java.ignored_message_ids.join(", ")
            );
        }
        for case in report.cases.iter().filter(|case| case.mismatch) {
            println!(
                "  mismatch: {} octofhir={} java_comparable={:?} java_raw={:?}",
                case.name, case.octofhir_valid, case.java_comparable_valid, case.java_valid
            );
        }
        for case in report
            .cases
            .iter()
            .filter(|case| case.java_policy_difference)
        {
            println!(
                "  java-policy: {} octofhir={} java_raw={:?} ignored_errors={:?}",
                case.name,
                case.octofhir_valid,
                case.java_valid,
                case.java_ignored_policy_error_count
            );
        }
    }

    if let Some(rh) = &report.rh {
        println!(
            "rh reference: {} cases, {} mismatches vs octofhir",
            rh.cases_run, rh.mismatches_with_octofhir
        );
        if let Some(mismatches) = rh.mismatches_with_java {
            println!("rh reference: {mismatches} mismatches vs java");
        }
    }
}
