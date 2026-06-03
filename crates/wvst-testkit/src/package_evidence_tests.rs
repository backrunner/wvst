use super::*;

#[test]
fn passes_valid_package_evidence_with_default_budget() {
    let manifest = sample_manifest(PackageEvidencePlatform::Macos);
    let verify = sample_verify(PackageEvidencePlatform::Macos);

    let evaluation =
        PackageEvidenceEvaluation::evaluate(&manifest, &verify, &PackageEvidenceBudget::default());

    assert!(evaluation.passed);
    assert!(evaluation.violations.is_empty());
    assert_eq!(evaluation.platform, PackageEvidencePlatform::Macos);
    assert_eq!(evaluation.package_name, "wvst-macos");
}

#[test]
fn reports_schema_platform_status_and_counter_violations() {
    let mut manifest = sample_manifest(PackageEvidencePlatform::Macos);
    manifest.schema_version = 99;
    let mut verify = sample_verify(PackageEvidencePlatform::Windows);
    verify.schema_version = 2;
    verify.status = PackageVerifyStatus::Failed;
    verify.checks_failed = 1;
    verify.checks_run = 0;
    verify.warnings = 2;
    let budget = PackageEvidenceBudget {
        expected_platform: Some(PackageEvidencePlatform::Linux),
        max_warnings: Some(0),
        ..PackageEvidenceBudget::default()
    };

    let evaluation = PackageEvidenceEvaluation::evaluate(&manifest, &verify, &budget);

    assert!(!evaluation.passed);
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::UnsupportedSchemaVersion {
                artifact: "package-manifest",
                expected: PACKAGE_EVIDENCE_SCHEMA_VERSION,
                actual: 99,
            })
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::UnsupportedSchemaVersion {
                artifact: "verify-report",
                expected: PACKAGE_EVIDENCE_SCHEMA_VERSION,
                actual: 2,
            })
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::PlatformMismatch {
                source: "verify-report",
                expected: PackageEvidencePlatform::Macos,
                actual: PackageEvidencePlatform::Windows,
            })
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::VerifyStatusFailed)
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::MaximumExceeded {
                metric: "checksFailed",
                max: 0,
                actual: 1,
            })
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::MinimumNotMet {
                metric: "checksRun",
                min: 1,
                actual: 0,
            })
    );
}

#[test]
fn gates_strict_signature_skips_warnings_and_manifest_requirements() {
    let mut manifest = sample_manifest(PackageEvidencePlatform::Windows);
    manifest.files.push(PackageFileEvidence {
        path: "bin/wvst-host-worker.exe".to_string(),
        role: "duplicate-worker".to_string(),
        executable: true,
    });
    let mut verify = sample_verify(PackageEvidencePlatform::Windows);
    verify.strict_signature = false;
    verify.checks_skipped = 2;
    verify.warnings = 1;
    let budget = PackageEvidenceBudget {
        require_strict_signature: true,
        max_skipped_checks: Some(0),
        max_warnings: Some(0),
        required_file_roles: vec!["package-manifest".to_string(), "missing-role".to_string()],
        required_file_paths: vec![
            "scripts/verify-windows.ps1".to_string(),
            "missing/file".to_string(),
        ],
        ..PackageEvidenceBudget::default()
    };

    let evaluation = PackageEvidenceEvaluation::evaluate(&manifest, &verify, &budget);

    assert!(!evaluation.passed);
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::StrictSignatureMissing)
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::MaximumExceeded {
                metric: "checksSkipped",
                max: 0,
                actual: 2,
            })
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::MissingRequiredFileRole {
                role: "missing-role".to_string(),
            })
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::MissingRequiredFilePath {
                path: "missing/file".to_string(),
            })
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::DuplicateFilePath {
                path: "bin/wvst-host-worker.exe".to_string(),
            })
    );
}

#[test]
fn gates_platform_runtime_script_and_verification_check_consistency() {
    let mut manifest = sample_manifest(PackageEvidencePlatform::Macos);
    manifest.install.service_kind = "windows-scheduled-task".to_string();
    manifest.runtime.bridge_server = "bin/missing-bridge".to_string();
    manifest.verification.script = "scripts/missing-verify.sh".to_string();
    manifest
        .verification
        .checks
        .push("service-definition".to_string());
    let host_worker = manifest
        .files
        .iter_mut()
        .find(|file| file.role == "host-worker")
        .expect("host worker file");
    host_worker.role = "worker-binary".to_string();
    host_worker.executable = false;

    let mut verify = sample_verify(PackageEvidencePlatform::Macos);
    verify.checks = vec!["bundle-files".to_string()];
    let budget = PackageEvidenceBudget {
        required_verification_checks: vec!["bridge-diagnose".to_string(), "signing".to_string()],
        ..PackageEvidenceBudget::default()
    };

    let evaluation = PackageEvidenceEvaluation::evaluate(&manifest, &verify, &budget);

    assert!(!evaluation.passed);
    assert!(evaluation.violations.contains(
        &PackageEvidenceViolation::PlatformServiceKindMismatch {
            platform: PackageEvidencePlatform::Macos,
            expected: "launchd-user-agent",
            actual: "windows-scheduled-task".to_string(),
        }
    ));
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::MissingDeclaredFile {
                field: "runtime.bridgeServer",
                path: "bin/missing-bridge".to_string(),
            })
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::DeclaredFileRoleMismatch {
                field: "runtime.hostWorker",
                path: "bin/wvst-host-worker".to_string(),
                expected_role: "host-worker",
                actual_role: "worker-binary".to_string(),
            })
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::DeclaredFileNotExecutable {
                field: "runtime.hostWorker",
                path: "bin/wvst-host-worker".to_string(),
            })
    );
    assert!(
        evaluation
            .violations
            .contains(&PackageEvidenceViolation::MissingDeclaredFile {
                field: "verification.script",
                path: "scripts/missing-verify.sh".to_string(),
            })
    );
    assert!(evaluation.violations.contains(
        &PackageEvidenceViolation::MissingRequiredVerificationCheck {
            source: "verify-report",
            check: "bridge-diagnose".to_string(),
        }
    ));
    assert!(evaluation.violations.contains(
        &PackageEvidenceViolation::MissingRequiredVerificationCheck {
            source: "package-manifest",
            check: "signing".to_string(),
        }
    ));
}

#[test]
fn serializes_evaluation_for_ci_artifacts() {
    let mut verify = sample_verify(PackageEvidencePlatform::Linux);
    verify.checks_failed = 3;

    let evaluation = PackageEvidenceEvaluation::evaluate(
        &sample_manifest(PackageEvidencePlatform::Linux),
        &verify,
        &PackageEvidenceBudget::default(),
    );
    let json = serde_json::to_value(evaluation).expect("evaluation json");

    assert_eq!(json["passed"], false);
    assert_eq!(json["platform"], "linux");
    assert_eq!(json["violations"][0]["kind"], "maximum-exceeded");
    assert_eq!(json["violations"][0]["metric"], "checksFailed");
}

fn sample_manifest(platform: PackageEvidencePlatform) -> PackageEvidenceManifest {
    let package_name = match platform {
        PackageEvidencePlatform::Linux => "wvst-linux",
        PackageEvidencePlatform::Macos => "wvst-macos",
        PackageEvidencePlatform::Windows => "wvst-windows",
    };
    let (bridge, worker, verify_script) = match platform {
        PackageEvidencePlatform::Windows => (
            "bin/wvst-bridge-server.exe",
            "bin/wvst-host-worker.exe",
            "scripts/verify-windows.ps1",
        ),
        PackageEvidencePlatform::Linux => (
            "bin/wvst-bridge-server",
            "bin/wvst-host-worker",
            "scripts/verify-linux.sh",
        ),
        PackageEvidencePlatform::Macos => (
            "bin/wvst-bridge-server",
            "bin/wvst-host-worker",
            "scripts/verify-macos.sh",
        ),
    };

    PackageEvidenceManifest {
        schema_version: PACKAGE_EVIDENCE_SCHEMA_VERSION,
        package_name: package_name.to_string(),
        platform,
        install: PackageInstallEvidence {
            service_kind: service_kind(platform).to_string(),
            service_name: "wvst-test".to_string(),
            install_prefix: "/tmp/wvst".to_string(),
        },
        runtime: PackageRuntimeEvidence {
            bridge_server: bridge.to_string(),
            host_worker: worker.to_string(),
            bind_addr: "127.0.0.1:35876".to_string(),
        },
        files: vec![
            PackageFileEvidence {
                path: bridge.to_string(),
                role: "bridge-server".to_string(),
                executable: true,
            },
            PackageFileEvidence {
                path: worker.to_string(),
                role: "host-worker".to_string(),
                executable: true,
            },
            PackageFileEvidence {
                path: verify_script.to_string(),
                role: "verification-script".to_string(),
                executable: !matches!(platform, PackageEvidencePlatform::Windows),
            },
            PackageFileEvidence {
                path: "wvst-package-manifest.json".to_string(),
                role: "package-manifest".to_string(),
                executable: false,
            },
        ],
        verification: PackageVerificationEvidence {
            script: verify_script.to_string(),
            report_path: "wvst-verify-report.json".to_string(),
            strict_signature_env: Some("WVST_STRICT_VERIFY".to_string()),
            checks: sample_checks(),
        },
    }
}

fn sample_verify(platform: PackageEvidencePlatform) -> PackageVerifyReport {
    PackageVerifyReport {
        schema_version: PACKAGE_EVIDENCE_SCHEMA_VERSION,
        platform,
        package_dir: "/tmp/wvst-package".to_string(),
        status: PackageVerifyStatus::Passed,
        strict_signature: false,
        checks_run: 24,
        checks_failed: 0,
        checks_skipped: 0,
        warnings: 0,
        checks: sample_checks(),
    }
}

fn service_kind(platform: PackageEvidencePlatform) -> &'static str {
    match platform {
        PackageEvidencePlatform::Linux => "systemd-user-service",
        PackageEvidencePlatform::Macos => "launchd-user-agent",
        PackageEvidencePlatform::Windows => "windows-scheduled-task",
    }
}

fn sample_checks() -> Vec<String> {
    vec!["bundle-files".to_string(), "bridge-diagnose".to_string()]
}
