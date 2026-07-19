use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::CameraManError;
use crate::frame::{Frame, PixelFormat};
use crate::performance::{CopyLedgerSnapshot, percentile_index};

pub const COMPOSITOR_BENCHMARK_SCHEMA_VERSION: u32 = 3;
pub const DEFAULT_FIXTURE_PROFILE: &str = "synthetic-solid-v1";
pub const REPRESENTATIVE_FIXTURE_PROFILE: &str = "representative-patterned-v1";
pub const DEFAULT_BOOTSTRAP_RESAMPLES: u32 = 10_000;
pub const REPRESENTATIVE_FIXTURE_DIMENSIONS: [(u32, u32); 8] = [
    (641, 479),
    (853, 481),
    (1_279, 719),
    (515, 773),
    (959, 539),
    (1_023, 577),
    (631, 355),
    (777, 437),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkEnvironment {
    pub captured_unix_millis: u128,
    pub power_source: String,
    pub battery_percent: Option<u8>,
    pub battery_state: String,
    pub power_mode: Option<u8>,
    pub thermal_warning: String,
    pub performance_warning: String,
    pub cpu_power_status: String,
    pub active_display_count: Option<usize>,
    pub load_average_1m: Option<f64>,
    pub active_process_count: Option<usize>,
    pub background_load_note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub schema_version: u32,
    pub hardware: String,
    pub target_arch: String,
    pub os_version: String,
    pub compiler: String,
    pub power_source: String,
    #[serde(default = "default_fixture_profile")]
    pub fixture_profile: String,
    #[serde(default)]
    pub build_hash: String,
    pub iterations: u32,
    pub warmup_iterations: u32,
    pub case_rotation: usize,
    #[serde(default = "default_case_order_mode")]
    pub case_order_mode: String,
    #[serde(default)]
    pub case_order_seed: Option<u64>,
    #[serde(default)]
    pub environment_before: Option<BenchmarkEnvironment>,
    #[serde(default)]
    pub environment_after: Option<BenchmarkEnvironment>,
    #[serde(default)]
    pub preparation_results: Vec<OperationResult>,
    pub results: Vec<CaseResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseResult {
    pub case_id: String,
    pub width: u32,
    pub height: u32,
    pub sources: usize,
    pub layout: String,
    pub scaling_filter: String,
    pub average_ms: f64,
    pub standard_deviation_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub megapixels_per_second: f64,
    pub checksum: u8,
    #[serde(default)]
    pub copy_ledger_delta: Option<CopyLedgerSnapshot>,
    pub samples_ms: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationResult {
    pub operation_id: String,
    pub average_ms: f64,
    pub standard_deviation_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub bytes_per_operation: u64,
    pub checksum: u8,
    pub copy_ledger_delta: CopyLedgerSnapshot,
    pub samples_ms: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DurationStatistics {
    pub average_ms: f64,
    pub standard_deviation_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub total_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BaselineFile {
    pub schema_version: u32,
    pub max_regression_percent: f64,
    pub required_power_source: Option<String>,
    #[serde(default)]
    pub required_fixture_profile: Option<String>,
    pub baselines: Vec<BaselineEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BaselineEntry {
    pub hardware: String,
    pub case_id: String,
    pub average_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceInterval {
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineDecision {
    Improvement,
    NoSignificantChange,
    Regression,
    EnvironmentMismatch,
    NotCompared,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessCaseSummary {
    pub case_id: String,
    pub process_average_ms: Vec<f64>,
    pub mean_average_ms: f64,
    pub mean_average_ms_ci95: ConfidenceInterval,
    pub baseline_average_ms: Option<f64>,
    pub ratio_to_fixed_baseline: Option<f64>,
    pub ratio_to_fixed_baseline_ci95: Option<ConfidenceInterval>,
    pub decision: BaselineDecision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkSummaryReport {
    pub schema_version: u32,
    pub source_reports: Vec<String>,
    pub hardware: String,
    pub target_arch: String,
    pub fixture_profile: String,
    pub power_source: String,
    pub process_runs: usize,
    pub bootstrap_resamples: u32,
    pub confidence_level: f64,
    pub comparable_to_baseline: bool,
    pub warnings: Vec<String>,
    pub results: Vec<ProcessCaseSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkSummaryError(pub String);

impl fmt::Display for BenchmarkSummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for BenchmarkSummaryError {}

pub fn capture_benchmark_environment(
    background_load_note: impl Into<String>,
) -> BenchmarkEnvironment {
    let battery = command_output("pmset", &["-g", "batt"]);
    let (power_source, battery_percent, battery_state) = parse_battery_status(&battery);
    let thermal = parse_thermal_status(&command_output("pmset", &["-g", "therm"]));
    let power_mode = parse_power_mode(&command_output("pmset", &["-g", "custom"]), &power_source);
    BenchmarkEnvironment {
        captured_unix_millis: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis()),
        power_source,
        battery_percent,
        battery_state,
        power_mode,
        thermal_warning: thermal.thermal_warning,
        performance_warning: thermal.performance_warning,
        cpu_power_status: thermal.cpu_power_status,
        active_display_count: active_display_count(),
        load_average_1m: parse_load_average(&command_output("sysctl", &["-n", "vm.loadavg"])),
        active_process_count: active_process_count(),
        background_load_note: background_load_note.into(),
    }
}

pub fn seeded_case_order(case_count: usize, seed: u64) -> Vec<usize> {
    let mut order = (0..case_count).collect::<Vec<_>>();
    let mut generator = XorShift64::new(seed);
    for upper in (1..case_count).rev() {
        let selected = generator.index(upper + 1);
        order.swap(upper, selected);
    }
    order
}

pub fn duration_statistics(samples: &[Duration]) -> Option<DurationStatistics> {
    if samples.is_empty() {
        return None;
    }
    let total = samples.iter().copied().sum::<Duration>();
    let average_seconds = total.as_secs_f64() / samples.len() as f64;
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let percentile = |value| sorted[percentile_index(sorted.len(), value).unwrap()];
    let variance = samples
        .iter()
        .map(|sample| {
            let difference = sample.as_secs_f64() - average_seconds;
            difference * difference
        })
        .sum::<f64>()
        / samples.len() as f64;
    Some(DurationStatistics {
        average_ms: average_seconds * 1_000.0,
        standard_deviation_ms: variance.sqrt() * 1_000.0,
        p50_ms: percentile(50).as_secs_f64() * 1_000.0,
        p95_ms: percentile(95).as_secs_f64() * 1_000.0,
        p99_ms: percentile(99).as_secs_f64() * 1_000.0,
        min_ms: sorted[0].as_secs_f64() * 1_000.0,
        max_ms: sorted.last().unwrap().as_secs_f64() * 1_000.0,
        total_seconds: total.as_secs_f64(),
    })
}

pub fn representative_fixture_frames(count: usize) -> Result<Vec<Option<Frame>>, CameraManError> {
    (0..count)
        .map(|index| {
            let (width, height) =
                REPRESENTATIVE_FIXTURE_DIMENSIONS[index % REPRESENTATIVE_FIXTURE_DIMENSIONS.len()];
            representative_patterned_frame(width, height, index as u32).map(Some)
        })
        .collect()
}

pub fn representative_patterned_frame(
    width: u32,
    height: u32,
    source: u32,
) -> Result<Frame, CameraManError> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    let mut noise = 0x9e37_79b9_u32 ^ source.wrapping_mul(0x85eb_ca6b);
    for y in 0..height {
        for x in 0..width {
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            let edge = x % 47 == 0 || y % 43 == 0 || x + 1 == width || y + 1 == height;
            let text_like = (y / 9) % 7 == 2 && ((x / 5) ^ (x / 17) ^ source).is_multiple_of(3);
            let checker = ((x / 16) + (y / 16) + source).is_multiple_of(2);
            let mut blue = x.wrapping_mul(17).wrapping_add(y * 3) as u8;
            let mut green = y.wrapping_mul(11).wrapping_add(x * 5) as u8;
            let mut red = x.wrapping_add(y.wrapping_mul(7)) as u8;
            if checker {
                green = green.saturating_add(31);
            }
            if noise & 0x1f == 0 {
                blue ^= 0x3f;
                red ^= 0x1f;
            }
            if edge {
                [blue, green, red] = [255, 255, 255];
            } else if text_like {
                [blue, green, red] = [8, 8, 8];
            }
            pixels.extend_from_slice(&[blue, green, red, 255]);
        }
    }
    Frame::new_checked(width, height, PixelFormat::Bgra8, pixels)
}

pub fn summarize_benchmark_reports(
    reports: &[BenchmarkReport],
    source_reports: Vec<String>,
    baseline: Option<&BaselineFile>,
    bootstrap_resamples: u32,
) -> Result<BenchmarkSummaryReport, BenchmarkSummaryError> {
    if reports.len() < 2 {
        return Err(BenchmarkSummaryError(String::from(
            "process-level summary requires at least two reports",
        )));
    }
    if bootstrap_resamples < 100 {
        return Err(BenchmarkSummaryError(String::from(
            "bootstrap resamples must be at least 100",
        )));
    }
    if let Some(baseline) = baseline {
        if baseline.schema_version != 1 {
            return Err(BenchmarkSummaryError(format!(
                "unsupported baseline schema {}",
                baseline.schema_version
            )));
        }
        if !baseline.max_regression_percent.is_finite() || baseline.max_regression_percent < 0.0 {
            return Err(BenchmarkSummaryError(String::from(
                "baseline regression threshold must be finite and non-negative",
            )));
        }
    }
    let first = &reports[0];
    if !(2..=COMPOSITOR_BENCHMARK_SCHEMA_VERSION).contains(&first.schema_version) {
        return Err(BenchmarkSummaryError(format!(
            "unsupported benchmark schema {}",
            first.schema_version
        )));
    }
    validate_report(first)?;
    let expected_cases = case_ids(first)?;
    for report in &reports[1..] {
        if !(2..=COMPOSITOR_BENCHMARK_SCHEMA_VERSION).contains(&report.schema_version) {
            return Err(BenchmarkSummaryError(format!(
                "unsupported benchmark schema {}",
                report.schema_version
            )));
        }
        if report.hardware != first.hardware
            || report.target_arch != first.target_arch
            || report.os_version != first.os_version
            || report.compiler != first.compiler
            || report.fixture_profile != first.fixture_profile
            || report.build_hash != first.build_hash
            || report.power_source != first.power_source
            || report.iterations != first.iterations
            || report.warmup_iterations != first.warmup_iterations
        {
            return Err(BenchmarkSummaryError(String::from(
                "reports disagree on build, environment, fixture, or iteration policy",
            )));
        }
        validate_report(report)?;
        if case_ids(report)? != expected_cases {
            return Err(BenchmarkSummaryError(String::from(
                "reports do not contain the same case ids",
            )));
        }
    }

    let mut warnings = environment_warnings(reports);
    let power_source_stable = reports.iter().all(|report| {
        [
            report.environment_before.as_ref(),
            report.environment_after.as_ref(),
        ]
        .into_iter()
        .flatten()
        .all(|environment| environment.power_source == report.power_source)
    });
    if !power_source_stable {
        warnings.push(String::from(
            "power source changed within at least one benchmark process",
        ));
    }
    if baseline.is_some() {
        warnings.push(String::from(
            "baseline is a fixed point estimate; ratio intervals include current process variation only",
        ));
    }
    let comparable_to_baseline = baseline.is_some_and(|baseline| {
        let power_matches = baseline
            .required_power_source
            .as_deref()
            .is_none_or(|required| required == first.power_source);
        let fixture_matches = baseline
            .required_fixture_profile
            .as_deref()
            .is_none_or(|required| required == first.fixture_profile);
        power_matches && fixture_matches && power_source_stable
    });
    if let Some(baseline) = baseline {
        if baseline
            .required_power_source
            .as_deref()
            .is_some_and(|required| required != first.power_source)
        {
            warnings.push(format!(
                "baseline requires `{}` but reports use `{}`",
                baseline.required_power_source.as_deref().unwrap(),
                first.power_source
            ));
        }
        if baseline
            .required_fixture_profile
            .as_deref()
            .is_some_and(|required| required != first.fixture_profile)
        {
            warnings.push(format!(
                "baseline requires fixture `{}` but reports use `{}`",
                baseline.required_fixture_profile.as_deref().unwrap(),
                first.fixture_profile
            ));
        }
    }

    let mut results = Vec::with_capacity(expected_cases.len());
    for case_id in expected_cases {
        let values = reports
            .iter()
            .map(|report| {
                report
                    .results
                    .iter()
                    .find(|result| result.case_id == case_id)
                    .map(|result| result.average_ms)
                    .ok_or_else(|| {
                        BenchmarkSummaryError(format!("missing case `{case_id}` in report"))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mean = arithmetic_mean(&values);
        let ci = bootstrap_mean_ci(
            &values,
            bootstrap_resamples,
            stable_seed(&case_id) ^ reports.len() as u64,
        );
        let baseline_average = baseline.and_then(|baseline| {
            baseline
                .baselines
                .iter()
                .find(|entry| entry.hardware == first.hardware && entry.case_id == case_id)
                .map(|entry| entry.average_ms)
        });
        let ratio = baseline_average.map(|baseline| mean / baseline);
        let ratio_ci = baseline_average.map(|baseline| ConfidenceInterval {
            low: ci.low / baseline,
            high: ci.high / baseline,
        });
        let decision = match (baseline, baseline_average, ratio_ci) {
            (None, _, _) | (_, None, _) => BaselineDecision::NotCompared,
            (Some(_), Some(_), _) if !comparable_to_baseline => {
                BaselineDecision::EnvironmentMismatch
            }
            (Some(baseline), Some(_), Some(ratio_ci)) => {
                let allowed = 1.0 + baseline.max_regression_percent / 100.0;
                if ratio_ci.low > allowed {
                    BaselineDecision::Regression
                } else if ratio_ci.high < 1.0 {
                    BaselineDecision::Improvement
                } else {
                    BaselineDecision::NoSignificantChange
                }
            }
            _ => BaselineDecision::NotCompared,
        };
        results.push(ProcessCaseSummary {
            case_id,
            process_average_ms: values,
            mean_average_ms: mean,
            mean_average_ms_ci95: ci,
            baseline_average_ms: baseline_average,
            ratio_to_fixed_baseline: ratio,
            ratio_to_fixed_baseline_ci95: ratio_ci,
            decision,
        });
    }

    Ok(BenchmarkSummaryReport {
        schema_version: 1,
        source_reports,
        hardware: first.hardware.clone(),
        target_arch: first.target_arch.clone(),
        fixture_profile: first.fixture_profile.clone(),
        power_source: first.power_source.clone(),
        process_runs: reports.len(),
        bootstrap_resamples,
        confidence_level: 0.95,
        comparable_to_baseline,
        warnings,
        results,
    })
}

fn default_fixture_profile() -> String {
    String::from(DEFAULT_FIXTURE_PROFILE)
}

fn default_case_order_mode() -> String {
    String::from("rotation")
}

fn command_output(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_default()
}

fn parse_battery_status(status: &str) -> (String, Option<u8>, String) {
    let power_source = status
        .lines()
        .next()
        .and_then(|line| line.split_once('\''))
        .and_then(|(_, tail)| tail.split_once('\''))
        .map_or_else(|| String::from("unknown"), |(source, _)| source.to_owned());
    let detail = status.lines().nth(1).unwrap_or_default();
    let battery_percent = detail.split_whitespace().find_map(|part| {
        part.strip_suffix("%;")
            .or_else(|| part.strip_suffix('%'))
            .and_then(|value| value.parse::<u8>().ok())
    });
    let battery_state = detail
        .split(';')
        .nth(1)
        .map(str::trim)
        .filter(|state| !state.is_empty())
        .unwrap_or("unknown")
        .to_owned();
    (power_source, battery_percent, battery_state)
}

#[derive(Debug, Default)]
struct ThermalStatus {
    thermal_warning: String,
    performance_warning: String,
    cpu_power_status: String,
}

fn parse_thermal_status(status: &str) -> ThermalStatus {
    let value = |needle: &str| {
        status
            .lines()
            .find(|line| line.to_ascii_lowercase().contains(needle))
            .map(|line| line.trim_start_matches("Note: ").trim().to_owned())
            .unwrap_or_else(|| String::from("unknown"))
    };
    ThermalStatus {
        thermal_warning: value("thermal warning"),
        performance_warning: value("performance warning"),
        cpu_power_status: value("cpu power status"),
    }
}

fn parse_power_mode(custom: &str, power_source: &str) -> Option<u8> {
    let mut in_section = false;
    for line in custom.lines() {
        if let Some(section) = line.strip_suffix(':') {
            in_section = section.trim() == power_source;
            continue;
        }
        if in_section {
            let mut fields = line.split_whitespace();
            if fields.next() == Some("powermode") {
                return fields.next().and_then(|value| value.parse().ok());
            }
        }
    }
    None
}

fn parse_load_average(value: &str) -> Option<f64> {
    value
        .trim_matches(|character: char| {
            character == '{' || character == '}' || character.is_whitespace()
        })
        .split_whitespace()
        .next()
        .and_then(|number| number.parse().ok())
}

fn active_display_count() -> Option<usize> {
    let output = command_output("system_profiler", &["SPDisplaysDataType", "-json"]);
    let value: serde_json::Value = serde_json::from_str(&output).ok()?;
    let adapters = value.get("SPDisplaysDataType")?.as_array()?;
    Some(
        adapters
            .iter()
            .filter_map(|adapter| adapter.get("spdisplays_ndrvs")?.as_array())
            .flatten()
            .filter(|display| {
                display
                    .get("spdisplays_online")
                    .and_then(serde_json::Value::as_str)
                    == Some("spdisplays_yes")
            })
            .count(),
    )
}

fn active_process_count() -> Option<usize> {
    let output = command_output("ps", &["-axo", "pid="]);
    (!output.is_empty()).then(|| {
        output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
    })
}

fn validate_report(report: &BenchmarkReport) -> Result<(), BenchmarkSummaryError> {
    if report.iterations == 0 || report.results.is_empty() {
        return Err(BenchmarkSummaryError(String::from(
            "benchmark report must contain measured cases",
        )));
    }
    for result in &report.results {
        let scalar_values = [
            result.average_ms,
            result.standard_deviation_ms,
            result.p50_ms,
            result.p95_ms,
            result.p99_ms,
            result.min_ms,
            result.max_ms,
            result.megapixels_per_second,
        ];
        if scalar_values
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
            || result.average_ms == 0.0
            || result.megapixels_per_second == 0.0
        {
            return Err(BenchmarkSummaryError(format!(
                "case '{}' contains invalid statistics",
                result.case_id
            )));
        }
        if result.samples_ms.len() != report.iterations as usize
            || result
                .samples_ms
                .iter()
                .any(|sample| !sample.is_finite() || *sample < 0.0)
        {
            return Err(BenchmarkSummaryError(format!(
                "case '{}' raw sample count or value is invalid",
                result.case_id
            )));
        }
        let recomputed_average =
            result.samples_ms.iter().sum::<f64>() / result.samples_ms.len() as f64;
        let tolerance = (result.average_ms.abs() * 1e-9).max(1e-9);
        if (recomputed_average - result.average_ms).abs() > tolerance {
            return Err(BenchmarkSummaryError(format!(
                "case '{}' average does not match raw samples",
                result.case_id
            )));
        }
    }
    Ok(())
}

fn case_ids(report: &BenchmarkReport) -> Result<BTreeSet<String>, BenchmarkSummaryError> {
    let ids = report
        .results
        .iter()
        .map(|result| result.case_id.clone())
        .collect::<BTreeSet<_>>();
    if ids.len() != report.results.len() {
        return Err(BenchmarkSummaryError(String::from(
            "report contains duplicate case ids",
        )));
    }
    Ok(ids)
}

fn environment_warnings(reports: &[BenchmarkReport]) -> Vec<String> {
    let mut warnings = BTreeMap::<String, usize>::new();
    for report in reports {
        for environment in [
            report.environment_before.as_ref(),
            report.environment_after.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            for value in [
                &environment.thermal_warning,
                &environment.performance_warning,
                &environment.cpu_power_status,
            ] {
                if value != "unknown" && !value.to_ascii_lowercase().starts_with("no ") {
                    *warnings.entry(value.clone()).or_default() += 1;
                }
            }
        }
    }
    warnings
        .into_iter()
        .map(|(warning, observations)| format!("{warning} ({observations} observations)"))
        .collect()
}

fn arithmetic_mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn bootstrap_mean_ci(values: &[f64], resamples: u32, seed: u64) -> ConfidenceInterval {
    let mut generator = XorShift64::new(seed);
    let mut means = Vec::with_capacity(resamples as usize);
    for _ in 0..resamples {
        let sum = (0..values.len())
            .map(|_| values[generator.index(values.len())])
            .sum::<f64>();
        means.push(sum / values.len() as f64);
    }
    means.sort_by(f64::total_cmp);
    ConfidenceInterval {
        low: means[percentile_index_thousandths(means.len(), 25)],
        high: means[percentile_index_thousandths(means.len(), 975)],
    }
}

fn percentile_index_thousandths(sample_count: usize, percentile: usize) -> usize {
    (sample_count - 1).saturating_mul(percentile.min(1_000)) / 1_000
}

fn stable_seed(value: &str) -> u64 {
    value.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x1000_0000_01b3)
    })
}

struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn index(&mut self, len: usize) -> usize {
        self.next() as usize % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(averages: &[(&str, f64)]) -> BenchmarkReport {
        BenchmarkReport {
            schema_version: 3,
            hardware: String::from("aarch64 | test"),
            target_arch: String::from("aarch64"),
            os_version: String::from("test"),
            compiler: String::from("rustc test"),
            power_source: String::from("AC Power"),
            fixture_profile: String::from(DEFAULT_FIXTURE_PROFILE),
            build_hash: String::from("test"),
            iterations: 100,
            warmup_iterations: 5,
            case_rotation: 0,
            case_order_mode: String::from("rotation"),
            case_order_seed: None,
            environment_before: None,
            environment_after: None,
            preparation_results: Vec::new(),
            results: averages
                .iter()
                .map(|(case_id, average_ms)| CaseResult {
                    case_id: (*case_id).to_owned(),
                    width: 1,
                    height: 1,
                    sources: 1,
                    layout: String::from("grid"),
                    scaling_filter: String::from("nearest"),
                    average_ms: *average_ms,
                    standard_deviation_ms: 0.0,
                    p50_ms: *average_ms,
                    p95_ms: *average_ms,
                    p99_ms: *average_ms,
                    min_ms: *average_ms,
                    max_ms: *average_ms,
                    megapixels_per_second: 1.0,
                    checksum: 0,
                    copy_ledger_delta: None,
                    samples_ms: vec![*average_ms; 100],
                })
                .collect(),
        }
    }

    #[test]
    fn parses_current_mac_power_metadata() {
        let battery = "Now drawing from 'Battery Power'\n -InternalBattery-0\t61%; discharging; 2:11 remaining";
        assert_eq!(
            parse_battery_status(battery),
            (
                String::from("Battery Power"),
                Some(61),
                String::from("discharging")
            )
        );
        let modes = "Battery Power:\n powermode 1\nAC Power:\n powermode 2\n";
        assert_eq!(parse_power_mode(modes, "Battery Power"), Some(1));
        assert_eq!(parse_power_mode(modes, "AC Power"), Some(2));
        assert_eq!(parse_load_average("{ 2.50 3.00 4.00 }"), Some(2.5));
    }

    #[test]
    fn bootstrap_summary_is_deterministic_and_process_level() {
        let reports = vec![
            report(&[("case", 9.0)]),
            report(&[("case", 10.0)]),
            report(&[("case", 11.0)]),
        ];
        let summary = summarize_benchmark_reports(&reports, Vec::new(), None, 1_000).unwrap();
        let result = &summary.results[0];
        assert_eq!(result.mean_average_ms, 10.0);
        assert!(result.mean_average_ms_ci95.low >= 9.0);
        assert!(result.mean_average_ms_ci95.high <= 11.0);
        assert_eq!(result.decision, BaselineDecision::NotCompared);
    }

    #[test]
    fn baseline_decision_respects_environment_and_confidence_interval() {
        let reports = vec![
            report(&[("case", 12.0)]),
            report(&[("case", 12.2)]),
            report(&[("case", 12.4)]),
        ];
        let baseline = BaselineFile {
            schema_version: 1,
            max_regression_percent: 15.0,
            required_power_source: Some(String::from("AC Power")),
            required_fixture_profile: Some(String::from(DEFAULT_FIXTURE_PROFILE)),
            baselines: vec![BaselineEntry {
                hardware: String::from("aarch64 | test"),
                case_id: String::from("case"),
                average_ms: 10.0,
            }],
        };
        let summary =
            summarize_benchmark_reports(&reports, Vec::new(), Some(&baseline), 1_000).unwrap();
        assert!(summary.comparable_to_baseline);
        assert_eq!(summary.results[0].decision, BaselineDecision::Regression);
    }

    #[test]
    fn rejects_mismatched_case_sets() {
        let error = summarize_benchmark_reports(
            &[report(&[("one", 1.0)]), report(&[("two", 1.0)])],
            Vec::new(),
            None,
            100,
        )
        .unwrap_err();
        assert!(error.0.contains("same case ids"));
    }

    #[test]
    fn rejects_mixed_builds_and_invalid_raw_samples() {
        let first = report(&[("case", 1.0)]);
        let mut second = report(&[("case", 1.0)]);
        second.build_hash = String::from("other");
        let error = summarize_benchmark_reports(&[first.clone(), second], Vec::new(), None, 100)
            .unwrap_err();
        assert!(error.0.contains("disagree"));

        let mut invalid = first.clone();
        invalid.results[0].samples_ms.clear();
        let error =
            summarize_benchmark_reports(&[first, invalid], Vec::new(), None, 100).unwrap_err();
        assert!(error.0.contains("raw sample"));
    }

    #[test]
    fn schema_two_defaults_to_original_fixture_profile() {
        let json = r#"{
            "schema_version": 2,
            "hardware": "test",
            "target_arch": "aarch64",
            "os_version": "test",
            "compiler": "test",
            "power_source": "Battery Power",
            "iterations": 1,
            "warmup_iterations": 0,
            "case_rotation": 0,
            "results": []
        }"#;
        let report: BenchmarkReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.fixture_profile, DEFAULT_FIXTURE_PROFILE);
        assert!(report.environment_before.is_none());
        assert_eq!(report.case_order_mode, "rotation");
        assert!(report.preparation_results.is_empty());
    }

    #[test]
    fn seeded_case_order_is_a_stable_permutation() {
        let first = seeded_case_order(16, 42);
        let second = seeded_case_order(16, 42);
        assert_eq!(first, second);
        assert_ne!(first, (0..16).collect::<Vec<_>>());
        let mut sorted = first;
        sorted.sort_unstable();
        assert_eq!(sorted, (0..16).collect::<Vec<_>>());
    }

    #[test]
    fn duration_statistics_cover_distribution_and_empty_input() {
        let samples = [
            Duration::from_millis(1),
            Duration::from_millis(2),
            Duration::from_millis(3),
        ];
        let statistics = duration_statistics(&samples).unwrap();

        assert_eq!(statistics.average_ms, 2.0);
        assert_eq!(statistics.p50_ms, 2.0);
        assert_eq!(statistics.min_ms, 1.0);
        assert_eq!(statistics.max_ms, 3.0);
        assert!(duration_statistics(&[]).is_none());
    }

    #[test]
    fn representative_fixture_is_deterministic_and_odd_sized() {
        let first = representative_fixture_frames(2).unwrap();
        let second = representative_fixture_frames(2).unwrap();

        assert_eq!(first, second);
        assert_eq!(first[0].as_ref().unwrap().width(), 641);
        assert_eq!(first[0].as_ref().unwrap().height(), 479);
        assert_ne!(
            first[0].as_ref().unwrap().bgra_at(1, 1),
            first[0].as_ref().unwrap().bgra_at(48, 1)
        );
    }
}
