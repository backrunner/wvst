use std::collections::{BTreeMap, BTreeSet};

use super::*;

pub(super) fn validate_package_evidence(
    manifest: &PackageEvidenceManifest,
    verify: &PackageVerifyReport,
    budget: &PackageEvidenceBudget,
    violations: &mut Vec<PackageEvidenceViolation>,
) {
    validate_schema_versions(manifest, verify, violations);
    validate_platform(manifest, verify, budget, violations);
    validate_manifest_shape(manifest, violations);
    validate_verify_report(verify, budget, violations);
    validate_manifest_requirements(manifest, verify, budget, violations);
}

fn validate_schema_versions(
    manifest: &PackageEvidenceManifest,
    verify: &PackageVerifyReport,
    violations: &mut Vec<PackageEvidenceViolation>,
) {
    if manifest.schema_version != PACKAGE_EVIDENCE_SCHEMA_VERSION {
        violations.push(PackageEvidenceViolation::UnsupportedSchemaVersion {
            artifact: "package-manifest",
            expected: PACKAGE_EVIDENCE_SCHEMA_VERSION,
            actual: manifest.schema_version,
        });
    }
    if verify.schema_version != PACKAGE_EVIDENCE_SCHEMA_VERSION {
        violations.push(PackageEvidenceViolation::UnsupportedSchemaVersion {
            artifact: "verify-report",
            expected: PACKAGE_EVIDENCE_SCHEMA_VERSION,
            actual: verify.schema_version,
        });
    }
}

fn validate_platform(
    manifest: &PackageEvidenceManifest,
    verify: &PackageVerifyReport,
    budget: &PackageEvidenceBudget,
    violations: &mut Vec<PackageEvidenceViolation>,
) {
    if manifest.platform != verify.platform {
        violations.push(PackageEvidenceViolation::PlatformMismatch {
            source: "verify-report",
            expected: manifest.platform,
            actual: verify.platform,
        });
    }
    if let Some(expected) = budget.expected_platform
        && manifest.platform != expected
    {
        violations.push(PackageEvidenceViolation::PlatformMismatch {
            source: "budget",
            expected,
            actual: manifest.platform,
        });
    }
    if budget.require_platform_service_kind {
        validate_platform_service_kind(manifest, violations);
    }
}

fn validate_platform_service_kind(
    manifest: &PackageEvidenceManifest,
    violations: &mut Vec<PackageEvidenceViolation>,
) {
    let expected = expected_service_kind(manifest.platform);
    if manifest.install.service_kind.trim() != expected {
        violations.push(PackageEvidenceViolation::PlatformServiceKindMismatch {
            platform: manifest.platform,
            expected,
            actual: manifest.install.service_kind.clone(),
        });
    }
}

fn validate_manifest_shape(
    manifest: &PackageEvidenceManifest,
    violations: &mut Vec<PackageEvidenceViolation>,
) {
    push_empty(&manifest.package_name, "packageName", violations);
    push_empty(
        &manifest.install.service_kind,
        "install.serviceKind",
        violations,
    );
    push_empty(
        &manifest.install.service_name,
        "install.serviceName",
        violations,
    );
    push_empty(
        &manifest.install.install_prefix,
        "install.installPrefix",
        violations,
    );
    push_empty(
        &manifest.runtime.bridge_server,
        "runtime.bridgeServer",
        violations,
    );
    push_empty(
        &manifest.runtime.host_worker,
        "runtime.hostWorker",
        violations,
    );
    push_empty(
        &manifest.verification.script,
        "verification.script",
        violations,
    );
    push_empty(
        &manifest.verification.report_path,
        "verification.reportPath",
        violations,
    );

    let mut seen_paths = BTreeSet::new();
    for file in &manifest.files {
        push_empty(&file.path, "files.path", violations);
        push_empty(&file.role, "files.role", violations);
        if !file.path.trim().is_empty() && !seen_paths.insert(file.path.trim()) {
            violations.push(PackageEvidenceViolation::DuplicateFilePath {
                path: file.path.clone(),
            });
        }
    }
}

fn validate_verify_report(
    verify: &PackageVerifyReport,
    budget: &PackageEvidenceBudget,
    violations: &mut Vec<PackageEvidenceViolation>,
) {
    if budget.require_verify_passed && verify.status != PackageVerifyStatus::Passed {
        violations.push(PackageEvidenceViolation::VerifyStatusFailed);
    }
    if budget.require_strict_signature && !verify.strict_signature {
        violations.push(PackageEvidenceViolation::StrictSignatureMissing);
    }

    push_max(
        violations,
        "checksFailed",
        verify.checks_failed,
        Some(budget.max_failed_checks),
    );
    push_max(
        violations,
        "checksSkipped",
        verify.checks_skipped,
        budget.max_skipped_checks,
    );
    push_max(violations, "warnings", verify.warnings, budget.max_warnings);
    if verify.checks_run < budget.min_checks_run {
        violations.push(PackageEvidenceViolation::MinimumNotMet {
            metric: "checksRun",
            min: budget.min_checks_run,
            actual: verify.checks_run,
        });
    }
}

fn validate_manifest_requirements(
    manifest: &PackageEvidenceManifest,
    verify: &PackageVerifyReport,
    budget: &PackageEvidenceBudget,
    violations: &mut Vec<PackageEvidenceViolation>,
) {
    let roles = manifest
        .files
        .iter()
        .map(|file| file.role.trim())
        .collect::<BTreeSet<_>>();
    let paths = manifest
        .files
        .iter()
        .map(|file| file.path.trim())
        .collect::<BTreeSet<_>>();
    let files = files_by_path(manifest);

    if budget.require_declared_runtime_files {
        validate_declared_file(
            &files,
            "runtime.bridgeServer",
            &manifest.runtime.bridge_server,
            "bridge-server",
            true,
            violations,
        );
        validate_declared_file(
            &files,
            "runtime.hostWorker",
            &manifest.runtime.host_worker,
            "host-worker",
            true,
            violations,
        );
    }
    if budget.require_verification_script_file {
        validate_declared_file(
            &files,
            "verification.script",
            &manifest.verification.script,
            "verification-script",
            !matches!(manifest.platform, PackageEvidencePlatform::Windows),
            violations,
        );
    }

    validate_verification_checks(manifest, verify, budget, violations);

    for role in &budget.required_file_roles {
        if !roles.contains(role.trim()) {
            violations
                .push(PackageEvidenceViolation::MissingRequiredFileRole { role: role.clone() });
        }
    }
    for path in &budget.required_file_paths {
        if !paths.contains(path.trim()) {
            violations
                .push(PackageEvidenceViolation::MissingRequiredFilePath { path: path.clone() });
        }
    }
}

fn validate_declared_file(
    files: &BTreeMap<&str, &PackageFileEvidence>,
    field: &'static str,
    path: &str,
    expected_role: &'static str,
    require_executable: bool,
    violations: &mut Vec<PackageEvidenceViolation>,
) {
    let trimmed_path = path.trim();
    if trimmed_path.is_empty() {
        return;
    }

    let Some(file) = files.get(trimmed_path) else {
        violations.push(PackageEvidenceViolation::MissingDeclaredFile {
            field,
            path: path.to_string(),
        });
        return;
    };
    if file.role.trim() != expected_role {
        violations.push(PackageEvidenceViolation::DeclaredFileRoleMismatch {
            field,
            path: file.path.clone(),
            expected_role,
            actual_role: file.role.clone(),
        });
    }
    if require_executable && !file.executable {
        violations.push(PackageEvidenceViolation::DeclaredFileNotExecutable {
            field,
            path: file.path.clone(),
        });
    }
}

fn validate_verification_checks(
    manifest: &PackageEvidenceManifest,
    verify: &PackageVerifyReport,
    budget: &PackageEvidenceBudget,
    violations: &mut Vec<PackageEvidenceViolation>,
) {
    let manifest_checks = non_empty_set(&manifest.verification.checks);
    let verify_checks = non_empty_set(&verify.checks);
    let min_checks = budget.min_verification_checks as usize;

    if manifest_checks.len() < min_checks {
        violations.push(PackageEvidenceViolation::MinimumNotMet {
            metric: "manifestVerificationChecks",
            min: budget.min_verification_checks,
            actual: manifest_checks.len() as u64,
        });
    }
    if verify_checks.len() < min_checks {
        violations.push(PackageEvidenceViolation::MinimumNotMet {
            metric: "verifyReportChecks",
            min: budget.min_verification_checks,
            actual: verify_checks.len() as u64,
        });
    }

    if budget.require_declared_checks_executed {
        for check in &manifest_checks {
            if !verify_checks.contains(check) {
                violations.push(PackageEvidenceViolation::MissingRequiredVerificationCheck {
                    source: "verify-report",
                    check: (*check).to_string(),
                });
            }
        }
    }

    for check in &budget.required_verification_checks {
        let trimmed = check.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !manifest_checks.contains(trimmed) {
            violations.push(PackageEvidenceViolation::MissingRequiredVerificationCheck {
                source: "package-manifest",
                check: check.clone(),
            });
        }
        if !verify_checks.contains(trimmed) {
            violations.push(PackageEvidenceViolation::MissingRequiredVerificationCheck {
                source: "verify-report",
                check: check.clone(),
            });
        }
    }
}

fn files_by_path(manifest: &PackageEvidenceManifest) -> BTreeMap<&str, &PackageFileEvidence> {
    let mut files = BTreeMap::new();
    for file in &manifest.files {
        let path = file.path.trim();
        if !path.is_empty() {
            files.entry(path).or_insert(file);
        }
    }
    files
}

fn non_empty_set(values: &[String]) -> BTreeSet<&str> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect()
}

fn push_empty(value: &str, field: &'static str, violations: &mut Vec<PackageEvidenceViolation>) {
    if value.trim().is_empty() {
        violations.push(PackageEvidenceViolation::EmptyManifestField { field });
    }
}

fn push_max(
    violations: &mut Vec<PackageEvidenceViolation>,
    metric: &'static str,
    actual: u64,
    max: Option<u64>,
) {
    if let Some(max) = max
        && actual > max
    {
        violations.push(PackageEvidenceViolation::MaximumExceeded {
            metric,
            max,
            actual,
        });
    }
}

const fn expected_service_kind(platform: PackageEvidencePlatform) -> &'static str {
    match platform {
        PackageEvidencePlatform::Linux => "systemd-user-service",
        PackageEvidencePlatform::Macos => "launchd-user-agent",
        PackageEvidencePlatform::Windows => "windows-scheduled-task",
    }
}
