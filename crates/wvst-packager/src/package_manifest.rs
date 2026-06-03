use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::MacosPackageConfig;
use crate::linux::LinuxPackageConfig;
use crate::windows::WindowsPackageConfig;

pub(crate) const PACKAGE_MANIFEST_FILE_NAME: &str = "wvst-package-manifest.json";
pub(crate) const VERIFY_REPORT_FILE_NAME: &str = "wvst-verify-report.json";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageEvidenceManifest {
    pub schema_version: u16,
    pub package_name: String,
    pub platform: PackagePlatform,
    pub install: PackageInstallEvidence,
    pub runtime: PackageRuntimeEvidence,
    pub files: Vec<PackageFileEvidence>,
    pub verification: PackageVerificationEvidence,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PackagePlatform {
    Linux,
    Macos,
    Windows,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageInstallEvidence {
    pub service_kind: &'static str,
    pub service_name: String,
    pub install_prefix: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageRuntimeEvidence {
    pub bridge_server: &'static str,
    pub host_worker: &'static str,
    pub bind_addr: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageFileEvidence {
    pub path: String,
    pub role: &'static str,
    pub executable: bool,
}

impl PackageFileEvidence {
    pub(crate) fn new(path: impl Into<String>, role: &'static str, executable: bool) -> Self {
        Self {
            path: path.into(),
            role,
            executable,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageVerificationEvidence {
    pub script: &'static str,
    pub report_path: &'static str,
    pub strict_signature_env: Option<&'static str>,
    pub checks: Vec<&'static str>,
}

pub(crate) fn write_package_manifest(
    root: &Path,
    manifest: PackageEvidenceManifest,
) -> io::Result<PathBuf> {
    let path = root.join(PACKAGE_MANIFEST_FILE_NAME);
    let json = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    std::fs::write(&path, json)?;
    Ok(path)
}

pub(crate) fn macos_package_manifest(config: &MacosPackageConfig) -> PackageEvidenceManifest {
    PackageEvidenceManifest {
        schema_version: 1,
        package_name: "wvst-macos".to_string(),
        platform: PackagePlatform::Macos,
        install: PackageInstallEvidence {
            service_kind: "launchd-user-agent",
            service_name: config.launchd_label.clone(),
            install_prefix: config.install_prefix.display().to_string(),
        },
        runtime: PackageRuntimeEvidence {
            bridge_server: "bin/wvst-bridge-server",
            host_worker: "bin/wvst-host-worker",
            bind_addr: config.bind_addr.clone(),
        },
        files: vec![
            PackageFileEvidence::new("bin/wvst-bridge-server", "bridge-server", true),
            PackageFileEvidence::new("bin/wvst-host-worker", "host-worker", true),
            PackageFileEvidence::new("bin/wvst-bridge-launcher", "launcher", true),
            PackageFileEvidence::new("config/wvst.env.example", "config-template", false),
            PackageFileEvidence::new(
                format!("launchd/{}.plist", config.launchd_label),
                "service-definition",
                false,
            ),
            PackageFileEvidence::new("scripts/install-macos.sh", "install-script", true),
            PackageFileEvidence::new("scripts/uninstall-macos.sh", "uninstall-script", true),
            PackageFileEvidence::new("scripts/diagnose-macos.sh", "diagnose-script", true),
            PackageFileEvidence::new("scripts/rotate-logs-macos.sh", "log-rotate-script", true),
            PackageFileEvidence::new("scripts/verify-macos.sh", "verification-script", true),
            PackageFileEvidence::new(
                "scripts/sign-notarize-macos.sh",
                "sign-notarize-script",
                true,
            ),
            PackageFileEvidence::new("README.md", "readme", false),
            PackageFileEvidence::new(PACKAGE_MANIFEST_FILE_NAME, "package-manifest", false),
        ],
        verification: PackageVerificationEvidence {
            script: "scripts/verify-macos.sh",
            report_path: VERIFY_REPORT_FILE_NAME,
            strict_signature_env: Some("WVST_STRICT_CODESIGN_VERIFY"),
            checks: vec![
                "bundle-files",
                "executable-permissions",
                "env-template",
                "launchd-plist",
                "plist-lint",
                "codesign",
                "gatekeeper-assessment",
                "bridge-diagnose",
            ],
        },
    }
}

pub(crate) fn linux_package_manifest(config: &LinuxPackageConfig) -> PackageEvidenceManifest {
    PackageEvidenceManifest {
        schema_version: 1,
        package_name: "wvst-linux".to_string(),
        platform: PackagePlatform::Linux,
        install: PackageInstallEvidence {
            service_kind: "systemd-user-service",
            service_name: config.systemd_service_name.clone(),
            install_prefix: config.install_prefix.display().to_string(),
        },
        runtime: PackageRuntimeEvidence {
            bridge_server: "bin/wvst-bridge-server",
            host_worker: "bin/wvst-host-worker",
            bind_addr: config.bind_addr.clone(),
        },
        files: vec![
            PackageFileEvidence::new("bin/wvst-bridge-server", "bridge-server", true),
            PackageFileEvidence::new("bin/wvst-host-worker", "host-worker", true),
            PackageFileEvidence::new("bin/wvst-bridge-launcher", "launcher", true),
            PackageFileEvidence::new("config/wvst.env.example", "config-template", false),
            PackageFileEvidence::new(
                format!("systemd/{}.service", config.systemd_service_name),
                "service-definition",
                false,
            ),
            PackageFileEvidence::new("scripts/install-linux.sh", "install-script", true),
            PackageFileEvidence::new("scripts/uninstall-linux.sh", "uninstall-script", true),
            PackageFileEvidence::new("scripts/diagnose-linux.sh", "diagnose-script", true),
            PackageFileEvidence::new("scripts/rotate-logs-linux.sh", "log-rotate-script", true),
            PackageFileEvidence::new("scripts/verify-linux.sh", "verification-script", true),
            PackageFileEvidence::new("README.md", "readme", false),
            PackageFileEvidence::new(PACKAGE_MANIFEST_FILE_NAME, "package-manifest", false),
        ],
        verification: PackageVerificationEvidence {
            script: "scripts/verify-linux.sh",
            report_path: VERIFY_REPORT_FILE_NAME,
            strict_signature_env: None,
            checks: vec![
                "bundle-files",
                "executable-permissions",
                "env-template",
                "systemd-unit",
                "systemd-analyze",
                "bridge-diagnose",
            ],
        },
    }
}

pub(crate) fn windows_package_manifest(config: &WindowsPackageConfig) -> PackageEvidenceManifest {
    PackageEvidenceManifest {
        schema_version: 1,
        package_name: "wvst-windows".to_string(),
        platform: PackagePlatform::Windows,
        install: PackageInstallEvidence {
            service_kind: "windows-scheduled-task",
            service_name: config.scheduled_task_name.clone(),
            install_prefix: windows_path(&config.install_prefix),
        },
        runtime: PackageRuntimeEvidence {
            bridge_server: "bin/wvst-bridge-server.exe",
            host_worker: "bin/wvst-host-worker.exe",
            bind_addr: config.bind_addr.clone(),
        },
        files: vec![
            PackageFileEvidence::new("bin/wvst-bridge-server.exe", "bridge-server", true),
            PackageFileEvidence::new("bin/wvst-host-worker.exe", "host-worker", true),
            PackageFileEvidence::new("bin/wvst-bridge-launcher.ps1", "launcher", false),
            PackageFileEvidence::new("config/wvst.env.example", "config-template", false),
            PackageFileEvidence::new("scripts/install-windows.ps1", "install-script", false),
            PackageFileEvidence::new("scripts/uninstall-windows.ps1", "uninstall-script", false),
            PackageFileEvidence::new("scripts/diagnose-windows.ps1", "diagnose-script", false),
            PackageFileEvidence::new(
                "scripts/rotate-logs-windows.ps1",
                "log-rotate-script",
                false,
            ),
            PackageFileEvidence::new("scripts/verify-windows.ps1", "verification-script", false),
            PackageFileEvidence::new("README.md", "readme", false),
            PackageFileEvidence::new(PACKAGE_MANIFEST_FILE_NAME, "package-manifest", false),
        ],
        verification: PackageVerificationEvidence {
            script: "scripts/verify-windows.ps1",
            report_path: VERIFY_REPORT_FILE_NAME,
            strict_signature_env: Some("WVST_STRICT_AUTHENTICODE_VERIFY"),
            checks: vec![
                "bundle-files",
                "env-template",
                "scheduled-task-scripts",
                "authenticode",
                "bridge-diagnose",
            ],
        },
    }
}

fn windows_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('/', "\\")
}
