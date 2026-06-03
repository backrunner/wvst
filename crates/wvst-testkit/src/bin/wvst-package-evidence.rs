use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use wvst_testkit::package_evidence::{
    PackageEvidenceBudget, PackageEvidenceEvaluation, PackageEvidenceManifest, PackageVerifyReport,
};

const USAGE: &str = "\
usage: wvst-package-evidence --manifest <wvst-package-manifest.json> --verify-report <wvst-verify-report.json> [--budget <package-budget.json>]

Evaluates WVST package manifest and verify-report JSON evidence and writes a JSON report to stdout.
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

fn write_report(report: PackageEvidenceEvaluation) -> ExitCode {
    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("failed to serialize package evidence report: {error}");
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
            let manifest_text = read_to_string(&options.manifest_path, "manifest")?;
            let verify_text = read_to_string(&options.verify_report_path, "verify report")?;
            let budget_text = options
                .budget_path
                .as_ref()
                .map(|path| read_to_string(path, "budget"))
                .transpose()?;
            evaluate_json(&manifest_text, &verify_text, budget_text.as_deref())
                .map(CliResult::Report)
        }
    }
}

fn evaluate_json(
    manifest_text: &str,
    verify_text: &str,
    budget_text: Option<&str>,
) -> Result<PackageEvidenceEvaluation, String> {
    let manifest = serde_json::from_str::<PackageEvidenceManifest>(manifest_text)
        .map_err(|error| format!("invalid manifest json: {error}"))?;
    let verify = serde_json::from_str::<PackageVerifyReport>(verify_text)
        .map_err(|error| format!("invalid verify report json: {error}"))?;
    let budget = budget_text
        .map(|text| {
            serde_json::from_str::<PackageEvidenceBudget>(text)
                .map_err(|error| format!("invalid budget json: {error}"))
        })
        .transpose()?
        .unwrap_or_default();

    Ok(PackageEvidenceEvaluation::evaluate(
        &manifest, &verify, &budget,
    ))
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<CliAction, String> {
    let mut manifest_path = None;
    let mut verify_report_path = None;
    let mut budget_path = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliAction::Help),
            "--manifest" => manifest_path = Some(next_value(&mut args, "--manifest")?),
            "--verify-report" => {
                verify_report_path = Some(next_value(&mut args, "--verify-report")?);
            }
            "--budget" => budget_path = Some(next_value(&mut args, "--budget")?),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }

    let manifest_path =
        manifest_path.ok_or_else(|| "missing required --manifest argument".to_string())?;
    let verify_report_path = verify_report_path
        .ok_or_else(|| "missing required --verify-report argument".to_string())?;

    Ok(CliAction::Evaluate(CliOptions {
        manifest_path,
        verify_report_path,
        budget_path,
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
    manifest_path: PathBuf,
    verify_report_path: PathBuf,
    budget_path: Option<PathBuf>,
}

enum CliResult {
    Help,
    Report(PackageEvidenceEvaluation),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_required_options_with_budget() {
        let action = parse_options([
            "--manifest".to_string(),
            "/tmp/manifest.json".to_string(),
            "--verify-report".to_string(),
            "/tmp/verify.json".to_string(),
            "--budget".to_string(),
            "/tmp/budget.json".to_string(),
        ])
        .expect("options");

        assert_eq!(
            action,
            CliAction::Evaluate(CliOptions {
                manifest_path: PathBuf::from("/tmp/manifest.json"),
                verify_report_path: PathBuf::from("/tmp/verify.json"),
                budget_path: Some(PathBuf::from("/tmp/budget.json")),
            })
        );
    }

    #[test]
    fn rejects_missing_verify_report() {
        let error = parse_options(["--manifest".to_string(), "/tmp/manifest.json".to_string()])
            .expect_err("missing verify report");

        assert!(error.contains("missing required --verify-report"));
    }

    #[test]
    fn evaluates_json_payloads() {
        let manifest = json!({
            "schemaVersion": 1,
            "packageName": "wvst-linux",
            "platform": "linux",
            "install": {
                "serviceKind": "systemd-user-service",
                "serviceName": "top.backrunner.wvst.bridge",
                "installPrefix": "/home/test/.local/share/wvst"
            },
            "runtime": {
                "bridgeServer": "bin/wvst-bridge-server",
                "hostWorker": "bin/wvst-host-worker",
                "bindAddr": "127.0.0.1:35876"
            },
            "files": [
                {
                    "path": "bin/wvst-bridge-server",
                    "role": "bridge-server",
                    "executable": true
                },
                {
                    "path": "bin/wvst-host-worker",
                    "role": "host-worker",
                    "executable": true
                },
                {
                    "path": "scripts/verify-linux.sh",
                    "role": "verification-script",
                    "executable": true
                }
            ],
            "verification": {
                "script": "scripts/verify-linux.sh",
                "reportPath": "wvst-verify-report.json",
                "checks": ["bundle-files", "bridge-diagnose"]
            }
        });
        let verify = json!({
            "schemaVersion": 1,
            "platform": "linux",
            "packageDir": "/tmp/wvst-linux",
            "status": "passed",
            "strictSignature": false,
            "checksRun": 12,
            "checksFailed": 0,
            "checksSkipped": 1,
            "warnings": 0,
            "checks": ["bundle-files", "bridge-diagnose"]
        });
        let budget = json!({
            "expectedPlatform": "linux",
            "maxSkippedChecks": 0,
            "requiredFileRoles": ["bridge-server", "verification-script"]
        });

        let report = evaluate_json(
            &manifest.to_string(),
            &verify.to_string(),
            Some(&budget.to_string()),
        )
        .expect("report");

        assert!(!report.passed);
        assert_eq!(
            report.violations[0].kind_name_for_test(),
            "maximum-exceeded"
        );
    }

    trait ViolationTestExt {
        fn kind_name_for_test(&self) -> &'static str;
    }

    impl ViolationTestExt for wvst_testkit::package_evidence::PackageEvidenceViolation {
        fn kind_name_for_test(&self) -> &'static str {
            match self {
                Self::UnsupportedSchemaVersion { .. } => "unsupported-schema-version",
                Self::PlatformMismatch { .. } => "platform-mismatch",
                Self::VerifyStatusFailed => "verify-status-failed",
                Self::StrictSignatureMissing => "strict-signature-missing",
                Self::MaximumExceeded { .. } => "maximum-exceeded",
                Self::MinimumNotMet { .. } => "minimum-not-met",
                Self::EmptyManifestField { .. } => "empty-manifest-field",
                Self::MissingRequiredFileRole { .. } => "missing-required-file-role",
                Self::MissingRequiredFilePath { .. } => "missing-required-file-path",
                Self::DuplicateFilePath { .. } => "duplicate-file-path",
                Self::PlatformServiceKindMismatch { .. } => "platform-service-kind-mismatch",
                Self::MissingDeclaredFile { .. } => "missing-declared-file",
                Self::DeclaredFileRoleMismatch { .. } => "declared-file-role-mismatch",
                Self::DeclaredFileNotExecutable { .. } => "declared-file-not-executable",
                Self::MissingRequiredVerificationCheck { .. } => {
                    "missing-required-verification-check"
                }
            }
        }
    }
}
