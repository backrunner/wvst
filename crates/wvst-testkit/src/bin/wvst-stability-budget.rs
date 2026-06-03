use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use wvst_testkit::latency::LatencySnapshot;
use wvst_testkit::stability_budget::{
    BridgeStabilityMetrics, StabilityBudget, StabilityBudgetReport, WebAudioLoopbackMetrics,
};

const USAGE: &str = "\
usage: wvst-stability-budget --snapshot <latency-snapshot.json> --budget <budget.json> [--webaudio <loopback-metrics-or-browser-smoke.json>] [--bridge <bridge-metrics-or-web-bridge-smoke.json>]

Evaluates a WVST latency/stability snapshot against a JSON stability budget and writes a JSON report to stdout.
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

fn write_report(report: StabilityBudgetReport) -> ExitCode {
    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("failed to serialize stability budget report: {error}");
            return ExitCode::from(2);
        }
    }

    if report.passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<CliResult, String> {
    match parse_options(args)? {
        CliAction::Help => Ok(CliResult::Help),
        CliAction::Evaluate(options) => {
            let snapshot_text = read_to_string(&options.snapshot_path, "snapshot")?;
            let budget_text = read_to_string(&options.budget_path, "budget")?;
            let webaudio_text = options
                .webaudio_path
                .as_ref()
                .map(|path| read_to_string(path, "webaudio"))
                .transpose()?;
            let bridge_text = options
                .bridge_path
                .as_ref()
                .map(|path| read_to_string(path, "bridge"))
                .transpose()?;
            evaluate_json(
                &snapshot_text,
                &budget_text,
                webaudio_text.as_deref(),
                bridge_text.as_deref(),
            )
            .map(CliResult::Report)
        }
    }
}

fn evaluate_json(
    snapshot_text: &str,
    budget_text: &str,
    webaudio_text: Option<&str>,
    bridge_text: Option<&str>,
) -> Result<StabilityBudgetReport, String> {
    let snapshot = serde_json::from_str::<LatencySnapshot>(snapshot_text)
        .map_err(|error| format!("invalid snapshot json: {error}"))?;
    let budget = serde_json::from_str::<StabilityBudget>(budget_text)
        .map_err(|error| format!("invalid budget json: {error}"))?;
    let webaudio = webaudio_text
        .map(|text| {
            WebAudioLoopbackMetrics::from_json_str(text)
                .map_err(|error| format!("invalid webaudio json: {error}"))
        })
        .transpose()?;
    let bridge = bridge_text
        .map(|text| {
            BridgeStabilityMetrics::from_json_str(text)
                .map_err(|error| format!("invalid bridge json: {error}"))
        })
        .transpose()?;

    Ok(StabilityBudgetReport::from_snapshot_and_metrics(
        &snapshot, budget, webaudio, bridge,
    ))
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<CliAction, String> {
    let mut snapshot_path = None;
    let mut budget_path = None;
    let mut webaudio_path = None;
    let mut bridge_path = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliAction::Help),
            "--snapshot" => snapshot_path = Some(next_value(&mut args, "--snapshot")?),
            "--budget" => budget_path = Some(next_value(&mut args, "--budget")?),
            "--webaudio" => webaudio_path = Some(next_value(&mut args, "--webaudio")?),
            "--bridge" => bridge_path = Some(next_value(&mut args, "--bridge")?),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }

    let snapshot_path =
        snapshot_path.ok_or_else(|| "missing required --snapshot argument".to_string())?;
    let budget_path =
        budget_path.ok_or_else(|| "missing required --budget argument".to_string())?;

    Ok(CliAction::Evaluate(CliOptions {
        snapshot_path,
        budget_path,
        webaudio_path,
        bridge_path,
    }))
}

fn read_to_string(path: &PathBuf, label: &'static str) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|error| format!("failed to read {label} {}: {error}", path.display()))
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
    Evaluate(CliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliOptions {
    snapshot_path: PathBuf,
    budget_path: PathBuf,
    webaudio_path: Option<PathBuf>,
    bridge_path: Option<PathBuf>,
}

enum CliResult {
    Help,
    Report(StabilityBudgetReport),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_required_options_with_optional_webaudio() {
        let action = parse_options([
            "--snapshot".to_string(),
            "/tmp/snapshot.json".to_string(),
            "--budget".to_string(),
            "/tmp/budget.json".to_string(),
            "--webaudio".to_string(),
            "/tmp/webaudio.json".to_string(),
            "--bridge".to_string(),
            "/tmp/bridge.json".to_string(),
        ])
        .expect("options");

        assert_eq!(
            action,
            CliAction::Evaluate(CliOptions {
                snapshot_path: PathBuf::from("/tmp/snapshot.json"),
                budget_path: PathBuf::from("/tmp/budget.json"),
                webaudio_path: Some(PathBuf::from("/tmp/webaudio.json")),
                bridge_path: Some(PathBuf::from("/tmp/bridge.json")),
            })
        );
    }

    #[test]
    fn evaluates_json_payloads() {
        let snapshot = json!({
            "config": {
                "sampleRateHz": 48000,
                "blockFrames": 128
            },
            "observations": 1,
            "droppedFrames": 0,
            "sequenceGapEvents": 0,
            "sequenceGapFrames": 0,
            "duplicateFrames": 0,
            "outOfOrderFrames": 0,
            "lateFrames": 0,
            "timeoutFrames": 0,
            "silenceFrames": 0,
            "processErrorFrames": 0,
            "routeLatencyUs": {
                "count": 1,
                "p50": 400,
                "p95": 400,
                "p99": 400
            },
            "roundTripFrames": {
                "count": 1,
                "p50": 128,
                "p95": 128,
                "p99": 128
            },
            "roundTripUs": {
                "count": 1,
                "p50": 2666,
                "p95": 2666,
                "p99": 2666
            }
        });
        let budget = json!({
            "minObservations": 1,
            "maxDroppedFrames": 0,
            "maxRouteLatencyP95Us": 250,
            "maxWebAudioUnderflows": 0,
            "maxWebAudioDroppedInputQuanta": 0,
            "maxWebAudioTransportFailures": 0,
            "maxWebAudioEndToEndRoundTripP95Us": 4000,
            "minSharedMemoryPumpEventsDrained": 1,
            "maxSharedMemoryPumpEventsDropped": 0,
            "maxSharedMemoryProcessLatencyP95Us": 1000,
            "maxSharedMemoryPumpMaxProcessMicros": 4000
        });
        let webaudio = json!({
            "inputFrames": 128,
            "outputFrames": 128,
            "underflows": 1,
            "overflows": 0,
            "droppedInputQuanta": 2,
            "droppedOutputQuanta": 0,
            "droppedMidiEvents": 0,
            "droppedParameterEvents": 0,
            "lateMidiEvents": 0,
            "lateParameterEvents": 0,
            "transportFailures": 1,
            "endToEndRoundTripUs": {
                "count": 2,
                "p50": 3000,
                "p95": 5000,
                "p99": 5000
            },
            "inputSequence": 1,
            "inputConsumedSequence": 1,
            "outputSequence": 1,
            "outputConsumedSequence": 1,
            "pendingInputQuanta": 0,
            "pendingOutputQuanta": 0
        });
        let bridge = json!({
            "sharedMemoryPumpEventsDrained": 0,
            "sharedMemoryPumpEventsDropped": 2,
            "sharedMemoryProcessLatency": {
                "count": 2,
                "p50Us": 500,
                "p95Us": 2000,
                "p99Us": 5000
            },
            "sharedMemoryPumpMaxProcessMicros": 5000
        });

        let report = evaluate_json(
            &snapshot.to_string(),
            &budget.to_string(),
            Some(&webaudio.to_string()),
            Some(&bridge.to_string()),
        )
        .expect("budget report");

        assert!(!report.passed);
        assert_eq!(report.violations.len(), 9);
    }

    #[test]
    fn evaluates_browser_smoke_payloads() {
        let snapshot = json!({
            "config": {
                "sampleRateHz": 48000,
                "blockFrames": 128
            },
            "observations": 1,
            "droppedFrames": 0,
            "sequenceGapEvents": 0,
            "sequenceGapFrames": 0,
            "duplicateFrames": 0,
            "outOfOrderFrames": 0,
            "lateFrames": 0,
            "timeoutFrames": 0,
            "silenceFrames": 0,
            "processErrorFrames": 0,
            "routeLatencyUs": {
                "count": 1,
                "p50": 400,
                "p95": 400,
                "p99": 400
            },
            "roundTripFrames": {
                "count": 1,
                "p50": 128,
                "p95": 128,
                "p99": 128
            },
            "roundTripUs": {
                "count": 1,
                "p50": 2666,
                "p95": 2666,
                "p99": 2666
            }
        });
        let budget = json!({
            "minObservations": 1,
            "maxWebAudioUnderflows": 0,
            "maxWebAudioEndToEndRoundTripP95Us": 4000
        });
        let browser_smoke = json!({
            "ok": true,
            "metrics": {
                "inputFrames": 128,
                "outputFrames": 128,
                "underflows": 0,
                "overflows": 0,
                "endToEndRoundTripUs": {
                    "count": 2,
                    "p50": 3000,
                    "p95": 5000,
                    "p99": 5000
                },
                "inputSequence": 1,
                "inputConsumedSequence": 1,
                "outputSequence": 1,
                "outputConsumedSequence": 1,
                "pendingInputQuanta": 0,
                "pendingOutputQuanta": 0
            }
        });

        let report = evaluate_json(
            &snapshot.to_string(),
            &budget.to_string(),
            Some(&browser_smoke.to_string()),
            None,
        )
        .expect("budget report");

        assert_eq!(report.violations.len(), 1);
        assert_eq!(
            report.violations[0],
            wvst_testkit::stability_budget::StabilityBudgetViolation::MaximumExceeded {
                metric: "webAudio.endToEndRoundTripUs.p95",
                max: 4000,
                actual: 5000,
            }
        );
    }

    #[test]
    fn rejects_missing_budget() {
        let error = parse_options(["--snapshot".to_string(), "/tmp/snapshot.json".to_string()])
            .expect_err("missing budget");

        assert!(error.contains("--budget"));
    }
}
