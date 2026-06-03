use serde::{Deserialize, Serialize};

mod validation;

pub const PACKAGE_EVIDENCE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageEvidencePlatform {
    Linux,
    Macos,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageEvidenceManifest {
    pub schema_version: u16,
    pub package_name: String,
    pub platform: PackageEvidencePlatform,
    pub install: PackageInstallEvidence,
    pub runtime: PackageRuntimeEvidence,
    #[serde(default)]
    pub files: Vec<PackageFileEvidence>,
    pub verification: PackageVerificationEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageInstallEvidence {
    pub service_kind: String,
    pub service_name: String,
    pub install_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRuntimeEvidence {
    pub bridge_server: String,
    pub host_worker: String,
    pub bind_addr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFileEvidence {
    pub path: String,
    pub role: String,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageVerificationEvidence {
    pub script: String,
    pub report_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict_signature_env: Option<String>,
    #[serde(default)]
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageVerifyStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageVerifyReport {
    pub schema_version: u16,
    pub platform: PackageEvidencePlatform,
    pub package_dir: String,
    pub status: PackageVerifyStatus,
    pub strict_signature: bool,
    pub checks_run: u64,
    pub checks_failed: u64,
    pub checks_skipped: u64,
    pub warnings: u64,
    #[serde(default)]
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageEvidenceBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_platform: Option<PackageEvidencePlatform>,
    #[serde(default = "default_true")]
    pub require_verify_passed: bool,
    #[serde(default)]
    pub require_strict_signature: bool,
    #[serde(default = "default_zero")]
    pub max_failed_checks: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_skipped_checks: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_warnings: Option<u64>,
    #[serde(default = "default_min_checks_run")]
    pub min_checks_run: u64,
    #[serde(default = "default_true")]
    pub require_platform_service_kind: bool,
    #[serde(default = "default_true")]
    pub require_declared_runtime_files: bool,
    #[serde(default = "default_true")]
    pub require_verification_script_file: bool,
    #[serde(default = "default_true")]
    pub require_declared_checks_executed: bool,
    #[serde(default = "default_min_verification_checks")]
    pub min_verification_checks: u64,
    #[serde(default)]
    pub required_verification_checks: Vec<String>,
    #[serde(default)]
    pub required_file_roles: Vec<String>,
    #[serde(default)]
    pub required_file_paths: Vec<String>,
}

impl Default for PackageEvidenceBudget {
    fn default() -> Self {
        Self {
            expected_platform: None,
            require_verify_passed: true,
            require_strict_signature: false,
            max_failed_checks: 0,
            max_skipped_checks: None,
            max_warnings: None,
            min_checks_run: 1,
            require_platform_service_kind: true,
            require_declared_runtime_files: true,
            require_verification_script_file: true,
            require_declared_checks_executed: true,
            min_verification_checks: 1,
            required_verification_checks: Vec::new(),
            required_file_roles: Vec::new(),
            required_file_paths: Vec::new(),
        }
    }
}

impl PackageEvidenceBudget {
    pub fn strict_release(platform: PackageEvidencePlatform) -> Self {
        let required_verification_checks = required_release_checks(platform)
            .iter()
            .map(|check| (*check).to_string())
            .collect::<Vec<_>>();
        let required_file_roles = required_release_roles(platform)
            .iter()
            .map(|role| (*role).to_string())
            .collect::<Vec<_>>();

        Self {
            expected_platform: Some(platform),
            require_strict_signature: matches!(
                platform,
                PackageEvidencePlatform::Macos | PackageEvidencePlatform::Windows
            ),
            max_skipped_checks: Some(0),
            max_warnings: Some(0),
            min_checks_run: required_verification_checks.len() as u64,
            min_verification_checks: required_verification_checks.len() as u64,
            required_verification_checks,
            required_file_roles,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageEvidenceEvaluation {
    pub passed: bool,
    pub platform: PackageEvidencePlatform,
    pub package_name: String,
    pub verify_status: PackageVerifyStatus,
    pub violations: Vec<PackageEvidenceViolation>,
}

impl PackageEvidenceEvaluation {
    pub fn evaluate(
        manifest: &PackageEvidenceManifest,
        verify: &PackageVerifyReport,
        budget: &PackageEvidenceBudget,
    ) -> Self {
        let mut violations = Vec::new();

        validation::validate_package_evidence(manifest, verify, budget, &mut violations);

        Self {
            passed: violations.is_empty(),
            platform: manifest.platform,
            package_name: manifest.package_name.clone(),
            verify_status: verify.status,
            violations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PackageEvidenceViolation {
    UnsupportedSchemaVersion {
        artifact: &'static str,
        expected: u16,
        actual: u16,
    },
    PlatformMismatch {
        source: &'static str,
        expected: PackageEvidencePlatform,
        actual: PackageEvidencePlatform,
    },
    VerifyStatusFailed,
    StrictSignatureMissing,
    MaximumExceeded {
        metric: &'static str,
        max: u64,
        actual: u64,
    },
    MinimumNotMet {
        metric: &'static str,
        min: u64,
        actual: u64,
    },
    EmptyManifestField {
        field: &'static str,
    },
    MissingRequiredFileRole {
        role: String,
    },
    MissingRequiredFilePath {
        path: String,
    },
    DuplicateFilePath {
        path: String,
    },
    PlatformServiceKindMismatch {
        platform: PackageEvidencePlatform,
        expected: &'static str,
        actual: String,
    },
    MissingDeclaredFile {
        field: &'static str,
        path: String,
    },
    DeclaredFileRoleMismatch {
        field: &'static str,
        path: String,
        expected_role: &'static str,
        actual_role: String,
    },
    DeclaredFileNotExecutable {
        field: &'static str,
        path: String,
    },
    MissingRequiredVerificationCheck {
        source: &'static str,
        check: String,
    },
}

const fn default_true() -> bool {
    true
}

const fn default_zero() -> u64 {
    0
}

const fn default_min_checks_run() -> u64 {
    1
}

const fn default_min_verification_checks() -> u64 {
    1
}

fn required_release_checks(platform: PackageEvidencePlatform) -> &'static [&'static str] {
    match platform {
        PackageEvidencePlatform::Macos => &[
            "bundle-files",
            "executable-permissions",
            "env-template",
            "launchd-plist",
            "plist-lint",
            "codesign",
            "gatekeeper-assessment",
            "bridge-diagnose",
        ],
        PackageEvidencePlatform::Linux => &[
            "bundle-files",
            "executable-permissions",
            "env-template",
            "systemd-unit",
            "systemd-analyze",
            "bridge-diagnose",
        ],
        PackageEvidencePlatform::Windows => &[
            "bundle-files",
            "env-template",
            "scheduled-task-scripts",
            "authenticode",
            "bridge-diagnose",
        ],
    }
}

fn required_release_roles(platform: PackageEvidencePlatform) -> &'static [&'static str] {
    match platform {
        PackageEvidencePlatform::Macos => &[
            "bridge-server",
            "host-worker",
            "launcher",
            "config-template",
            "service-definition",
            "install-script",
            "uninstall-script",
            "diagnose-script",
            "log-rotate-script",
            "verification-script",
            "sign-notarize-script",
            "package-manifest",
        ],
        PackageEvidencePlatform::Linux => &[
            "bridge-server",
            "host-worker",
            "launcher",
            "config-template",
            "service-definition",
            "install-script",
            "uninstall-script",
            "diagnose-script",
            "log-rotate-script",
            "verification-script",
            "package-manifest",
        ],
        PackageEvidencePlatform::Windows => &[
            "bridge-server",
            "host-worker",
            "launcher",
            "config-template",
            "install-script",
            "uninstall-script",
            "diagnose-script",
            "log-rotate-script",
            "verification-script",
            "package-manifest",
        ],
    }
}

#[cfg(test)]
#[path = "package_evidence_tests.rs"]
mod tests;
