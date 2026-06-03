use std::io;
use std::path::PathBuf;

use crate::{DEFAULT_BIND_ADDR, copy_binary, ensure_file, home_dir, write_text};

pub const DEFAULT_SCHEDULED_TASK_NAME: &str = "WVSTBridge";

#[derive(Debug, Clone)]
pub struct WindowsPackageConfig {
    pub bridge_server: PathBuf,
    pub host_worker: PathBuf,
    pub output_dir: PathBuf,
    pub install_prefix: PathBuf,
    pub scheduled_task_name: String,
    pub bind_addr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsPackageManifest {
    pub package_root: PathBuf,
    pub files: Vec<PathBuf>,
}

impl WindowsPackageConfig {
    pub fn new(
        bridge_server: impl Into<PathBuf>,
        host_worker: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            bridge_server: bridge_server.into(),
            host_worker: host_worker.into(),
            output_dir: output_dir.into(),
            install_prefix: default_windows_install_prefix(),
            scheduled_task_name: DEFAULT_SCHEDULED_TASK_NAME.to_string(),
            bind_addr: DEFAULT_BIND_ADDR.to_string(),
        }
    }

    pub fn package_root(&self) -> PathBuf {
        self.output_dir.join("wvst-windows")
    }
}

pub fn default_windows_install_prefix() -> PathBuf {
    home_dir().join("AppData").join("Local").join("WVST")
}

pub fn build_windows_package(config: &WindowsPackageConfig) -> io::Result<WindowsPackageManifest> {
    ensure_file(&config.bridge_server)?;
    ensure_file(&config.host_worker)?;

    let root = config.package_root();
    let bin_dir = root.join("bin");
    let config_dir = root.join("config");
    let scripts_dir = root.join("scripts");
    std::fs::create_dir_all(&bin_dir)?;
    std::fs::create_dir_all(&config_dir)?;
    std::fs::create_dir_all(&scripts_dir)?;

    let files = vec![
        copy_binary(
            &config.bridge_server,
            &bin_dir.join("wvst-bridge-server.exe"),
        )?,
        copy_binary(&config.host_worker, &bin_dir.join("wvst-host-worker.exe"))?,
        write_text(
            &bin_dir.join("wvst-bridge-launcher.ps1"),
            &launcher_script(config),
            false,
        )?,
        write_text(
            &config_dir.join("wvst.env.example"),
            &env_example(config),
            false,
        )?,
        write_text(
            &scripts_dir.join("install-windows.ps1"),
            &install_script(config),
            false,
        )?,
        write_text(
            &scripts_dir.join("uninstall-windows.ps1"),
            &uninstall_script(config),
            false,
        )?,
        write_text(&root.join("README.md"), &readme(config), false)?,
    ];

    Ok(WindowsPackageManifest {
        package_root: root,
        files,
    })
}

fn launcher_script(config: &WindowsPackageConfig) -> String {
    let config_file = config.install_prefix.join("config").join("wvst.env");
    let host_worker = config
        .install_prefix
        .join("bin")
        .join("wvst-host-worker.exe");
    let bridge_server = config
        .install_prefix
        .join("bin")
        .join("wvst-bridge-server.exe");

    format!(
        "$ErrorActionPreference = 'Stop'\n$ConfigFile = if ($env:WVST_CONFIG_FILE) {{ $env:WVST_CONFIG_FILE }} else {{ {config_file} }}\nif (Test-Path -LiteralPath $ConfigFile) {{\n  Get-Content -LiteralPath $ConfigFile | ForEach-Object {{\n    if ($_ -match '^\\s*(#|$)') {{ return }}\n    $Parts = $_ -split '=', 2\n    if ($Parts.Count -eq 2) {{ [Environment]::SetEnvironmentVariable($Parts[0].Trim(), $Parts[1], 'Process') }}\n  }}\n}}\nif (-not $env:WVST_HOST_WORKER) {{ $env:WVST_HOST_WORKER = {host_worker} }}\n& {bridge_server} serve\nexit $LASTEXITCODE\n",
        config_file = powershell_quote_path(&config_file),
        host_worker = powershell_quote_path(&host_worker),
        bridge_server = powershell_quote_path(&bridge_server),
    )
}

fn env_example(config: &WindowsPackageConfig) -> String {
    let host_worker = config
        .install_prefix
        .join("bin")
        .join("wvst-host-worker.exe");
    format!(
        "WVST_BIND_ADDR={bind_addr}\nWVST_TOKEN=\nWVST_ALLOWED_ORIGINS=http://localhost:5173\nWVST_ALLOW_LOOPBACK_ORIGINS=1\nWVST_HOST_WORKER={host_worker}\nWVST_WORKER_AUTO_RESTART=1\nWVST_WORKER_TIMEOUT_MS=5000\n",
        bind_addr = config.bind_addr,
        host_worker = windows_path(&host_worker),
    )
}

fn install_script(config: &WindowsPackageConfig) -> String {
    let install_prefix = powershell_quote_path(&config.install_prefix);
    let task_name = powershell_quote(&config.scheduled_task_name);

    format!(
        "$ErrorActionPreference = 'Stop'\n$PackageDir = Resolve-Path (Join-Path $PSScriptRoot '..')\n$InstallPrefix = {install_prefix}\n$TaskName = {task_name}\n$BinDir = Join-Path $InstallPrefix 'bin'\n$ConfigDir = Join-Path $InstallPrefix 'config'\n$ConfigFile = Join-Path $ConfigDir 'wvst.env'\nNew-Item -ItemType Directory -Force -Path $BinDir, $ConfigDir | Out-Null\nCopy-Item -Force -LiteralPath (Join-Path $PackageDir 'bin/wvst-bridge-server.exe') -Destination (Join-Path $BinDir 'wvst-bridge-server.exe')\nCopy-Item -Force -LiteralPath (Join-Path $PackageDir 'bin/wvst-host-worker.exe') -Destination (Join-Path $BinDir 'wvst-host-worker.exe')\nCopy-Item -Force -LiteralPath (Join-Path $PackageDir 'bin/wvst-bridge-launcher.ps1') -Destination (Join-Path $BinDir 'wvst-bridge-launcher.ps1')\nif (-not (Test-Path -LiteralPath $ConfigFile)) {{\n  Copy-Item -LiteralPath (Join-Path $PackageDir 'config/wvst.env.example') -Destination $ConfigFile\n  $Token = ([guid]::NewGuid().ToString('N') + [guid]::NewGuid().ToString('N'))\n  (Get-Content -LiteralPath $ConfigFile) | ForEach-Object {{ if ($_ -like 'WVST_TOKEN=*') {{ \"WVST_TOKEN=$Token\" }} else {{ $_ }} }} | Set-Content -LiteralPath $ConfigFile -Encoding utf8\n}}\n$Launcher = Join-Path $BinDir 'wvst-bridge-launcher.ps1'\n$Action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument \"-NoProfile -ExecutionPolicy Bypass -File `\"$Launcher`\"\"\n$Trigger = New-ScheduledTaskTrigger -AtLogOn\nRegister-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Description 'WVST Bridge Server' -Force | Out-Null\nStart-ScheduledTask -TaskName $TaskName\nWrite-Output \"WVST bridge installed. Config: $ConfigFile\"\nWrite-Output \"Scheduled task: $TaskName\"\n",
        install_prefix = install_prefix,
        task_name = task_name,
    )
}

fn uninstall_script(config: &WindowsPackageConfig) -> String {
    let install_prefix = powershell_quote_path(&config.install_prefix);
    let task_name = powershell_quote(&config.scheduled_task_name);

    format!(
        "$ErrorActionPreference = 'Stop'\n$InstallPrefix = {install_prefix}\n$TaskName = {task_name}\n$Task = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue\nif ($Task) {{\n  Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue\n  Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false\n}}\n$BinDir = Join-Path $InstallPrefix 'bin'\nRemove-Item -Force -ErrorAction SilentlyContinue (Join-Path $BinDir 'wvst-bridge-server.exe'), (Join-Path $BinDir 'wvst-host-worker.exe'), (Join-Path $BinDir 'wvst-bridge-launcher.ps1')\nWrite-Output 'WVST bridge scheduled task removed. Config was left in place.'\n",
        install_prefix = install_prefix,
        task_name = task_name,
    )
}

fn readme(config: &WindowsPackageConfig) -> String {
    format!(
        "# WVST Windows Bundle\n\nInstall the per-user scheduled task with PowerShell:\n\n```powershell\nSet-ExecutionPolicy -Scope Process Bypass\n.\\scripts\\install-windows.ps1\n```\n\nThe installer copies the bridge server, host worker, and launcher into `{install_prefix}`; creates `config\\wvst.env` with a generated token if missing; registers the `{task}` scheduled task; and starts it for the current user.\n\nUninstall the scheduled task and installed binaries with:\n\n```powershell\n.\\scripts\\uninstall-windows.ps1\n```\n\nConfiguration is intentionally preserved by uninstall.\n",
        install_prefix = windows_path(&config.install_prefix),
        task = config.scheduled_task_name,
    )
}

fn powershell_quote_path(path: &std::path::Path) -> String {
    powershell_quote(&windows_path(path))
}

fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn windows_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('/', "\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn builds_windows_bundle_with_scheduled_task_scripts() {
        let root = unique_temp_dir();
        let bridge = root.join("source").join("wvst-bridge-server.exe");
        let worker = root.join("source").join("wvst-host-worker.exe");
        std::fs::create_dir_all(bridge.parent().expect("source dir")).expect("source dir");
        std::fs::write(&bridge, b"bridge").expect("bridge binary");
        std::fs::write(&worker, b"worker").expect("worker binary");

        let mut config = WindowsPackageConfig::new(&bridge, &worker, root.join("out"));
        config.install_prefix = root.join("Install Prefix");
        config.scheduled_task_name = "WVSTBridgeTest".to_string();

        let manifest = build_windows_package(&config).expect("package");

        assert!(manifest.package_root.ends_with("wvst-windows"));
        assert!(
            manifest
                .files
                .iter()
                .any(|path| path.ends_with("README.md"))
        );
        assert!(
            manifest
                .files
                .iter()
                .any(|path| path.ends_with("bin/wvst-bridge-launcher.ps1"))
        );

        let env = std::fs::read_to_string(
            manifest
                .package_root
                .join("config")
                .join("wvst.env.example"),
        )
        .expect("env example");
        assert!(env.contains("WVST_HOST_WORKER="));
        assert!(env.contains("WVST_TOKEN="));

        let install = std::fs::read_to_string(
            manifest
                .package_root
                .join("scripts")
                .join("install-windows.ps1"),
        )
        .expect("install script");
        assert!(install.contains("New-ScheduledTaskAction"));
        assert!(install.contains("Register-ScheduledTask"));
        assert!(install.contains("WVST_TOKEN=$Token"));

        let launcher = std::fs::read_to_string(
            manifest
                .package_root
                .join("bin")
                .join("wvst-bridge-launcher.ps1"),
        )
        .expect("launcher");
        assert!(launcher.contains("WVST_HOST_WORKER"));
        assert!(launcher.contains("wvst-bridge-server.exe"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_missing_binaries() {
        let root = unique_temp_dir();
        let config = WindowsPackageConfig::new(
            root.join("missing-bridge.exe"),
            root.join("missing-worker.exe"),
            root.join("out"),
        );

        let error = build_windows_package(&config).expect_err("missing binary");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        let _ = std::fs::remove_dir_all(root);
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "wvst-windows-packager-{}-{nanos}",
            std::process::id()
        ))
    }
}
