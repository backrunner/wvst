use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use wvst_testkit::runtime_matrix::RuntimeProbeMatrixReport;
use wvst_testkit::runtime_matrix_manifest::RuntimeProbeMatrixManifest;

const USAGE: &str = "\
usage: wvst-runtime-matrix --worker <wvst-host-worker> --manifest <matrix.json>

Runs a WVST runtime-probe JSON manifest and writes a JSON report to stdout.
";

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(CliResult::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(CliResult::Report(report)) => write_report(report),
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn write_report(report: RuntimeProbeMatrixReport) -> ExitCode {
    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("failed to serialize runtime matrix report: {error}");
            return ExitCode::from(2);
        }
    }

    if report.all_passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<CliResult, String> {
    match parse_options(args)? {
        CliAction::Help => Ok(CliResult::Help),
        CliAction::Run(options) => {
            let manifest_text = fs::read_to_string(&options.manifest_path).map_err(|error| {
                format!(
                    "failed to read manifest {}: {error}",
                    options.manifest_path.display()
                )
            })?;
            let manifest = RuntimeProbeMatrixManifest::from_json_str(&manifest_text)
                .map_err(|error| error.to_string())?;
            let matrix = manifest
                .to_matrix(options.worker_executable)
                .map_err(|error| error.to_string())?;
            Ok(CliResult::Report(matrix.run()))
        }
    }
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<CliAction, String> {
    let mut worker_executable = None;
    let mut manifest_path = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliAction::Help),
            "--worker" => {
                worker_executable = Some(next_value(&mut args, "--worker")?);
            }
            "--manifest" => {
                manifest_path = Some(next_value(&mut args, "--manifest")?);
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }

    let worker_executable =
        worker_executable.ok_or_else(|| "missing required --worker argument".to_string())?;
    let manifest_path =
        manifest_path.ok_or_else(|| "missing required --manifest argument".to_string())?;

    Ok(CliAction::Run(CliOptions {
        worker_executable,
        manifest_path,
    }))
}

fn next_value(
    args: &mut impl Iterator<Item = String>,
    option: &'static str,
) -> Result<PathBuf, String> {
    args.next()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing value for {option}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliAction {
    Help,
    Run(CliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliOptions {
    worker_executable: PathBuf,
    manifest_path: PathBuf,
}

enum CliResult {
    Help,
    Report(RuntimeProbeMatrixReport),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_required_options() {
        let action = parse_options([
            "--worker".to_string(),
            "/tmp/wvst-host-worker".to_string(),
            "--manifest".to_string(),
            "/tmp/matrix.json".to_string(),
        ])
        .expect("options");

        assert_eq!(
            action,
            CliAction::Run(CliOptions {
                worker_executable: PathBuf::from("/tmp/wvst-host-worker"),
                manifest_path: PathBuf::from("/tmp/matrix.json"),
            })
        );
    }

    #[test]
    fn rejects_missing_manifest() {
        let error = parse_options(["--worker".to_string(), "/tmp/wvst-host-worker".to_string()])
            .expect_err("missing manifest");

        assert!(error.contains("--manifest"));
    }
}
