use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use wvst_testkit::latency::LatencySnapshot;
use wvst_testkit::latency_snapshot::LatencySnapshotInput;

const USAGE: &str = "\
usage: wvst-latency-snapshot --input <latency-observations.json>

Builds a WVST LatencySnapshot JSON document from scripted per-block observations.
The output is suitable for wvst-stability-budget --snapshot.
";

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(CliResult::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(CliResult::Snapshot(snapshot)) => write_snapshot(*snapshot),
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<CliResult, String> {
    match parse_options(args)? {
        CliAction::Help => Ok(CliResult::Help),
        CliAction::Build(options) => {
            let input_text = fs::read_to_string(&options.input_path).map_err(|error| {
                format!(
                    "failed to read input {}: {error}",
                    options.input_path.display()
                )
            })?;
            build_snapshot(&input_text).map(|snapshot| CliResult::Snapshot(Box::new(snapshot)))
        }
    }
}

fn build_snapshot(input_text: &str) -> Result<LatencySnapshot, String> {
    serde_json::from_str::<LatencySnapshotInput>(input_text)
        .map_err(|error| format!("invalid latency observations json: {error}"))?
        .into_snapshot()
        .map_err(|error| format!("invalid latency observations: {error}"))
}

fn write_snapshot(snapshot: LatencySnapshot) -> ExitCode {
    match serde_json::to_string_pretty(&snapshot) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to serialize latency snapshot: {error}");
            ExitCode::from(2)
        }
    }
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<CliAction, String> {
    let mut input_path = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliAction::Help),
            "--input" => input_path = Some(next_value(&mut args, "--input")?),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }

    let input_path = input_path.ok_or_else(|| "missing required --input argument".to_string())?;
    Ok(CliAction::Build(CliOptions { input_path }))
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
    Build(CliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliOptions {
    input_path: PathBuf,
}

enum CliResult {
    Help,
    Snapshot(Box<LatencySnapshot>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_input_option() {
        let action = parse_options(["--input".to_string(), "/tmp/latency.json".to_string()])
            .expect("options");

        assert_eq!(
            action,
            CliAction::Build(CliOptions {
                input_path: PathBuf::from("/tmp/latency.json"),
            })
        );
    }

    #[test]
    fn builds_snapshot_json() {
        let input = json!({
            "schemaVersion": 1,
            "config": {
                "sampleRateHz": 48000,
                "blockFrames": 128
            },
            "observations": [
                {
                    "roundTripFrames": 128,
                    "routeLatencyUs": 250
                },
                {
                    "outcome": "dropped"
                }
            ]
        });

        let snapshot = build_snapshot(&input.to_string()).expect("snapshot");

        assert_eq!(snapshot.observations, 1);
        assert_eq!(snapshot.dropped_frames, 1);
        assert_eq!(snapshot.route_latency_us.p99, Some(250));
    }
}
