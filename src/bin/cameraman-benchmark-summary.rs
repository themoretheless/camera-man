use std::error::Error;
use std::path::{Path, PathBuf};

use camera_man::benchmarking::{
    BaselineFile, BenchmarkReport, DEFAULT_BOOTSTRAP_RESAMPLES, summarize_benchmark_reports,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = arguments()?;
    let reports = arguments
        .reports
        .iter()
        .map(|path| read_json::<BenchmarkReport>(path))
        .collect::<Result<Vec<_>, _>>()?;
    let baseline = arguments
        .baseline
        .as_deref()
        .map(read_json::<BaselineFile>)
        .transpose()?;
    let summary = summarize_benchmark_reports(
        &reports,
        arguments
            .reports
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        baseline.as_ref(),
        arguments.bootstrap_resamples,
    )?;
    let json = serde_json::to_string_pretty(&summary)?;
    if let Some(output) = arguments.output {
        std::fs::write(&output, format!("{json}\n"))?;
        println!("benchmark_summary={}", output.display());
    } else {
        println!("{json}");
    }
    Ok(())
}

struct Arguments {
    baseline: Option<PathBuf>,
    output: Option<PathBuf>,
    bootstrap_resamples: u32,
    reports: Vec<PathBuf>,
}

fn arguments() -> Result<Arguments, Box<dyn Error>> {
    let mut baseline = None;
    let mut output = None;
    let mut bootstrap_resamples = DEFAULT_BOOTSTRAP_RESAMPLES;
    let mut reports = Vec::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--baseline" => baseline = Some(required_path(arguments.next(), "--baseline")?),
            "--output" => output = Some(required_path(arguments.next(), "--output")?),
            "--bootstrap-resamples" => {
                bootstrap_resamples = arguments
                    .next()
                    .ok_or("--bootstrap-resamples requires a value")?
                    .parse()?;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: cameraman-benchmark-summary [--baseline PATH] [--output PATH] \\\n                     [--bootstrap-resamples N] REPORT REPORT [REPORT ...]"
                );
                std::process::exit(0);
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown option: {value}").into());
            }
            path => reports.push(PathBuf::from(path)),
        }
    }
    if reports.len() < 2 {
        return Err("at least two benchmark reports are required".into());
    }
    Ok(Arguments {
        baseline,
        output,
        bootstrap_resamples,
        reports,
    })
}

fn required_path(value: Option<String>, option: &str) -> Result<PathBuf, Box<dyn Error>> {
    value
        .map(PathBuf::from)
        .ok_or_else(|| format!("{option} requires a path").into())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Box<dyn Error>> {
    let bytes = camera_man::read_bounded(path, camera_man::BENCHMARK_REPORT_PARSER_LIMITS)?;
    serde_json::from_slice(&bytes).map_err(Into::into)
}
