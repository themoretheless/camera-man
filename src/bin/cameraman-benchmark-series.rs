use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use camera_man::benchmarking::{
    BaselineFile, BenchmarkEnvironment, BenchmarkReport, DEFAULT_BOOTSTRAP_RESAMPLES,
    DEFAULT_FIXTURE_PROFILE, capture_benchmark_environment, summarize_benchmark_reports,
};
use serde::Serialize;

const DEFAULT_RUNS: u32 = 3;
const DEFAULT_ITERATIONS: u32 = 100;
const DEFAULT_WARMUPS: u32 = 5;
const DEFAULT_BASE_SEED: u64 = 0x4341_4d45_5241_4d41;

#[derive(Debug)]
struct Arguments {
    output_dir: PathBuf,
    baseline: PathBuf,
    runs: u32,
    iterations: u32,
    warmups: u32,
    bootstrap_resamples: u32,
    base_seed: u64,
    mode: String,
    fixture: String,
    background_load: String,
    dry_run: bool,
}

#[derive(Debug, Serialize)]
struct ArtifactDigest {
    path: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct BenchmarkInvocation {
    program: String,
    arguments: Vec<String>,
    environment: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct ReproducibilityManifest {
    schema_version: u32,
    generated_unix_millis: u128,
    git_revision: String,
    git_dirty: bool,
    build_hash: String,
    rustc: String,
    cargo: String,
    os_version: String,
    required_power_source: String,
    fixture_profile: String,
    mode: String,
    iterations: u32,
    warmup_iterations: u32,
    bootstrap_resamples: u32,
    base_seed: u64,
    commands: Vec<BenchmarkInvocation>,
    environment_preflights: Vec<BenchmarkEnvironment>,
    baseline: ArtifactDigest,
    benchmark_executable: Option<ArtifactDigest>,
    artifacts: Vec<ArtifactDigest>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = arguments()?;
    let baseline = read_json::<BaselineFile>(&arguments.baseline)?;
    if baseline.schema_version != 1 {
        return Err(format!("unsupported baseline schema {}", baseline.schema_version).into());
    }
    let required_power = baseline
        .required_power_source
        .as_deref()
        .unwrap_or("AC Power");
    if baseline
        .required_fixture_profile
        .as_deref()
        .is_some_and(|required| required != arguments.fixture)
    {
        return Err(format!(
            "baseline requires fixture '{}' but series requested '{}'",
            baseline.required_fixture_profile.as_deref().unwrap(),
            arguments.fixture
        )
        .into());
    }

    let first_environment = strict_preflight(required_power, &arguments.background_load)?;
    let output_dir = absolute_path(&arguments.output_dir)?;
    let commands = benchmark_commands(&arguments, &output_dir);
    if arguments.dry_run {
        println!("preflight=pass power_source={required_power}");
        for command in commands {
            println!("{}", serde_json::to_string(&command)?);
        }
        return Ok(());
    }

    prepare_output_directory(&output_dir)?;
    let baseline_input_path = output_dir.join("baseline-input.json");
    fs::copy(&arguments.baseline, &baseline_input_path)?;
    let mut environments = vec![first_environment];
    let mut report_paths = Vec::with_capacity(arguments.runs as usize);
    for (run, command) in commands.iter().enumerate() {
        environments.push(strict_preflight(
            required_power,
            &arguments.background_load,
        )?);
        let report_path = output_dir.join(format!("compositor-run-{:02}.json", run + 1));
        run_benchmark(command, run, &report_path)?;
        report_paths.push(report_path);
        environments.push(strict_preflight(
            required_power,
            &arguments.background_load,
        )?);
    }

    let reports = report_paths
        .iter()
        .map(|path| read_json::<BenchmarkReport>(path))
        .collect::<Result<Vec<_>, _>>()?;
    let summary = summarize_benchmark_reports(
        &reports,
        report_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        Some(&baseline),
        arguments.bootstrap_resamples,
    )?;
    let summary_path = output_dir.join("compositor-summary.json");
    write_json(&summary_path, &summary)?;

    let mut artifact_paths = report_paths;
    artifact_paths.push(summary_path);
    let artifacts = artifact_paths
        .iter()
        .map(|path| artifact_digest(path))
        .collect::<Result<Vec<_>, _>>()?;
    let benchmark_executable = newest_benchmark_executable("compositor-")
        .as_deref()
        .map(artifact_digest)
        .transpose()?;
    let manifest = ReproducibilityManifest {
        schema_version: 1,
        generated_unix_millis: unix_millis(),
        git_revision: command_output("git", &["rev-parse", "HEAD"]),
        git_dirty: !command_output("git", &["status", "--porcelain"]).is_empty(),
        build_hash: env!("CAMERAMAN_BUILD_HASH").to_owned(),
        rustc: command_output("rustc", &["-Vv"]),
        cargo: command_output("cargo", &["-V"]),
        os_version: command_output("sw_vers", &["-productVersion"]),
        required_power_source: required_power.to_owned(),
        fixture_profile: arguments.fixture.clone(),
        mode: arguments.mode.clone(),
        iterations: arguments.iterations,
        warmup_iterations: arguments.warmups,
        bootstrap_resamples: arguments.bootstrap_resamples,
        base_seed: arguments.base_seed,
        commands,
        environment_preflights: environments,
        baseline: artifact_digest(&baseline_input_path)?,
        benchmark_executable,
        artifacts,
    };
    let manifest_path = output_dir.join("manifest.json");
    write_json(&manifest_path, &manifest)?;

    println!("benchmark_series={}", output_dir.display());
    println!("process_runs={}", arguments.runs);
    println!(
        "summary={}",
        output_dir.join("compositor-summary.json").display()
    );
    println!("manifest={}", manifest_path.display());
    Ok(())
}

fn run_benchmark(
    invocation: &BenchmarkInvocation,
    run: usize,
    report_path: &Path,
) -> Result<(), Box<dyn Error>> {
    let status = Command::new(&invocation.program)
        .args(&invocation.arguments)
        .envs(&invocation.environment)
        .status()?;
    if !status.success() {
        return Err(format!("benchmark process {} failed with {status}", run + 1).into());
    }
    if !report_path.is_file() {
        return Err(format!("benchmark did not create {}", report_path.display()).into());
    }
    Ok(())
}

fn strict_preflight(
    required_power: &str,
    background_load: &str,
) -> Result<BenchmarkEnvironment, Box<dyn Error>> {
    let environment = capture_benchmark_environment(background_load);
    if environment.power_source != required_power {
        return Err(format!(
            "benchmark requires '{required_power}', current source is '{}'; connect power and retry",
            environment.power_source
        )
        .into());
    }
    for (name, value) in [
        ("thermal", environment.thermal_warning.as_str()),
        ("performance", environment.performance_warning.as_str()),
        ("CPU power", environment.cpu_power_status.as_str()),
    ] {
        if value != "unknown" && !value.to_ascii_lowercase().starts_with("no ") {
            return Err(format!("{name} preflight failed: {value}").into());
        }
    }
    Ok(environment)
}

fn benchmark_commands(arguments: &Arguments, output_dir: &Path) -> Vec<BenchmarkInvocation> {
    (0..arguments.runs)
        .map(|run| BenchmarkInvocation {
            program: String::from("cargo"),
            arguments: vec![
                String::from("bench"),
                String::from("--locked"),
                String::from("--bench"),
                String::from("compositor"),
                String::from("--no-default-features"),
            ],
            environment: BTreeMap::from([
                (String::from("CAMERAMAN_BENCH_MODE"), arguments.mode.clone()),
                (
                    String::from("CAMERAMAN_BENCH_FIXTURE"),
                    arguments.fixture.clone(),
                ),
                (
                    String::from("CAMERAMAN_BENCH_ITERATIONS"),
                    arguments.iterations.to_string(),
                ),
                (
                    String::from("CAMERAMAN_BENCH_WARMUP_ITERATIONS"),
                    arguments.warmups.to_string(),
                ),
                (
                    String::from("CAMERAMAN_BENCH_CASE_ORDER"),
                    String::from("random"),
                ),
                (
                    String::from("CAMERAMAN_BENCH_CASE_SEED"),
                    seed_for_run(arguments.base_seed, run).to_string(),
                ),
                (
                    String::from("CAMERAMAN_BENCH_JSON"),
                    output_dir
                        .join(format!("compositor-run-{:02}.json", run + 1))
                        .display()
                        .to_string(),
                ),
                (
                    String::from("CAMERAMAN_BENCH_BACKGROUND_LOAD"),
                    arguments.background_load.clone(),
                ),
            ]),
        })
        .collect()
}

fn seed_for_run(base_seed: u64, run: u32) -> u64 {
    base_seed.wrapping_add(u64::from(run).wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

fn artifact_digest(path: &Path) -> Result<ArtifactDigest, Box<dyn Error>> {
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()?;
    if !output.status.success() {
        return Err(format!("failed to hash {}", path.display()).into());
    }
    let sha256 = String::from_utf8(output.stdout)?
        .split_whitespace()
        .next()
        .ok_or("shasum returned no digest")?
        .to_owned();
    Ok(ArtifactDigest {
        path: path.display().to_string(),
        sha256,
    })
}

fn newest_benchmark_executable(prefix: &str) -> Option<PathBuf> {
    fs::read_dir("target/release/deps")
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix) && !name.ends_with(".d"))
        })
        .filter_map(|path| {
            let modified = path.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn arguments() -> Result<Arguments, Box<dyn Error>> {
    let mut output_dir = None;
    let mut baseline = PathBuf::from("benchmarks/baselines.json");
    let mut runs = DEFAULT_RUNS;
    let mut iterations = DEFAULT_ITERATIONS;
    let mut warmups = DEFAULT_WARMUPS;
    let mut bootstrap_resamples = DEFAULT_BOOTSTRAP_RESAMPLES;
    let mut base_seed = DEFAULT_BASE_SEED;
    let mut mode = String::from("matrix");
    let mut fixture = String::from(DEFAULT_FIXTURE_PROFILE);
    let mut background_load = String::from("benchmark series; no intentional background load");
    let mut dry_run = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output-dir" => output_dir = Some(required_path(arguments.next(), "--output-dir")?),
            "--baseline" => baseline = required_path(arguments.next(), "--baseline")?,
            "--runs" => runs = required_u32(arguments.next(), "--runs")?,
            "--iterations" => iterations = required_u32(arguments.next(), "--iterations")?,
            "--warmups" => warmups = required_u32(arguments.next(), "--warmups")?,
            "--bootstrap-resamples" => {
                bootstrap_resamples = required_u32(arguments.next(), "--bootstrap-resamples")?
            }
            "--base-seed" => {
                base_seed = arguments
                    .next()
                    .ok_or("--base-seed requires a value")?
                    .parse()?
            }
            "--mode" => mode = required_string(arguments.next(), "--mode")?,
            "--fixture" => fixture = required_string(arguments.next(), "--fixture")?,
            "--background-load" => {
                background_load = required_string(arguments.next(), "--background-load")?
            }
            "--dry-run" => dry_run = true,
            "--help" | "-h" => {
                println!(
                    "Usage: cameraman-benchmark-series [--output-dir PATH] [--baseline PATH] \
                     [--runs N] [--iterations N] [--warmups N] [--bootstrap-resamples N] \
                     [--base-seed N] [--mode MODE] [--fixture PROFILE] [--dry-run]"
                );
                std::process::exit(0);
            }
            value => return Err(format!("unknown option: {value}").into()),
        }
    }
    if runs < 3 {
        return Err("--runs must be at least 3 for process-level statistics".into());
    }
    if bootstrap_resamples < 100 {
        return Err("--bootstrap-resamples must be at least 100".into());
    }
    let output_dir = output_dir.unwrap_or_else(|| {
        PathBuf::from("benchmarks/runs").join(format!("series-{}", unix_millis()))
    });
    Ok(Arguments {
        output_dir,
        baseline,
        runs,
        iterations,
        warmups,
        bootstrap_resamples,
        base_seed,
        mode,
        fixture,
        background_load,
        dry_run,
    })
}

fn required_path(value: Option<String>, option: &str) -> Result<PathBuf, Box<dyn Error>> {
    value
        .map(PathBuf::from)
        .ok_or_else(|| format!("{option} requires a path").into())
}

fn required_string(value: Option<String>, option: &str) -> Result<String, Box<dyn Error>> {
    value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{option} requires a value").into())
}

fn required_u32(value: Option<String>, option: &str) -> Result<u32, Box<dyn Error>> {
    value
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{option} requires a positive integer").into())
}

fn absolute_path(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn prepare_output_directory(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.exists() {
        if !path.is_dir() {
            return Err(format!("output path is not a directory: {}", path.display()).into());
        }
        if fs::read_dir(path)?.next().transpose()?.is_some() {
            return Err(format!("output directory is not empty: {}", path.display()).into());
        }
    } else {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

fn command_output(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| String::from("unknown"))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Box<dyn Error>> {
    let bytes = camera_man::read_bounded(path, camera_man::BENCHMARK_REPORT_PARSER_LIMITS)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), Box<dyn Error>> {
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_invocation_matches_the_executed_environment() {
        let arguments = Arguments {
            output_dir: PathBuf::from("unused"),
            baseline: PathBuf::from("benchmarks/baselines.json"),
            runs: 3,
            iterations: 100,
            warmups: 5,
            bootstrap_resamples: 10_000,
            base_seed: 42,
            mode: String::from("matrix"),
            fixture: String::from(DEFAULT_FIXTURE_PROFILE),
            background_load: String::from("quiet profile"),
            dry_run: false,
        };
        let invocations = benchmark_commands(&arguments, Path::new("/tmp/cameraman-series"));

        assert_eq!(invocations.len(), 3);
        assert_eq!(invocations[0].program, "cargo");
        assert_eq!(
            invocations[0].environment["CAMERAMAN_BENCH_BACKGROUND_LOAD"],
            "quiet profile"
        );
        assert_eq!(
            invocations[0].environment["CAMERAMAN_BENCH_CASE_SEED"],
            "42"
        );
        assert_ne!(
            invocations[0].environment["CAMERAMAN_BENCH_CASE_SEED"],
            invocations[1].environment["CAMERAMAN_BENCH_CASE_SEED"]
        );
        assert!(
            invocations[2].environment["CAMERAMAN_BENCH_JSON"].ends_with("compositor-run-03.json")
        );
    }
}
