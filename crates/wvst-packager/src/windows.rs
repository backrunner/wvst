use std::io;
use std::path::PathBuf;

use crate::package_manifest::{
    PACKAGE_MANIFEST_FILE_NAME, VERIFY_REPORT_FILE_NAME, windows_package_manifest,
    write_package_manifest,
};
use crate::{
    DEFAULT_BIND_ADDR, DEFAULT_LOG_KEEP, DEFAULT_LOG_MAX_BYTES, copy_binary, ensure_file, home_dir,
    write_text,
};

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
        write_text(
            &scripts_dir.join("diagnose-windows.ps1"),
            &diagnose_script(config),
            false,
        )?,
        write_text(
            &scripts_dir.join("rotate-logs-windows.ps1"),
            &rotate_logs_script(config),
            false,
        )?,
        write_text(
            &scripts_dir.join("verify-windows.ps1"),
            &verify_script(),
            false,
        )?,
        write_text(&root.join("README.md"), &readme(config), false)?,
        write_package_manifest(&root, windows_package_manifest(config))?,
    ];

    Ok(WindowsPackageManifest {
        package_root: root,
        files,
    })
}

fn launcher_script(config: &WindowsPackageConfig) -> String {
    let install_prefix = powershell_quote_path(&config.install_prefix);
    let config_file = config.install_prefix.join("config").join("wvst.env");
    let host_worker = config
        .install_prefix
        .join("bin")
        .join("wvst-host-worker.exe");
    let bridge_server = config
        .install_prefix
        .join("bin")
        .join("wvst-bridge-server.exe");
    let log_rotate = config
        .install_prefix
        .join("bin")
        .join("wvst-log-rotate.ps1");

    format!(
        "$ErrorActionPreference = 'Stop'\n$InstallPrefix = {install_prefix}\n$ConfigFile = if ($env:WVST_CONFIG_FILE) {{ $env:WVST_CONFIG_FILE }} else {{ {config_file} }}\nif (Test-Path -LiteralPath $ConfigFile) {{\n  Get-Content -LiteralPath $ConfigFile | ForEach-Object {{\n    if ($_ -match '^\\s*(#|$)') {{ return }}\n    $Parts = $_ -split '=', 2\n    if ($Parts.Count -eq 2) {{ [Environment]::SetEnvironmentVariable($Parts[0].Trim(), $Parts[1], 'Process') }}\n  }}\n}}\nif (-not $env:WVST_HOST_WORKER) {{ $env:WVST_HOST_WORKER = {host_worker} }}\n$LogDir = if ($env:WVST_LOG_DIR) {{ $env:WVST_LOG_DIR }} else {{ Join-Path $InstallPrefix 'logs' }}\nNew-Item -ItemType Directory -Force -Path $LogDir | Out-Null\n$LogRotate = if ($env:WVST_LOG_ROTATE) {{ $env:WVST_LOG_ROTATE }} else {{ {log_rotate} }}\nif (Test-Path -LiteralPath $LogRotate) {{ try {{ & $LogRotate -LogDir $LogDir *> $null }} catch {{ }} }}\n$StdoutLog = Join-Path $LogDir 'bridge.log'\n$StderrLog = Join-Path $LogDir 'bridge.err.log'\n& {bridge_server} serve 1>> $StdoutLog 2>> $StderrLog\nexit $LASTEXITCODE\n",
        install_prefix = install_prefix,
        config_file = powershell_quote_path(&config_file),
        host_worker = powershell_quote_path(&host_worker),
        bridge_server = powershell_quote_path(&bridge_server),
        log_rotate = powershell_quote_path(&log_rotate),
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
        "$ErrorActionPreference = 'Stop'\n$PackageDir = Resolve-Path (Join-Path $PSScriptRoot '..')\n$InstallPrefix = {install_prefix}\n$TaskName = {task_name}\n$BinDir = Join-Path $InstallPrefix 'bin'\n$ConfigDir = Join-Path $InstallPrefix 'config'\n$LogDir = Join-Path $InstallPrefix 'logs'\n$ConfigFile = Join-Path $ConfigDir 'wvst.env'\nNew-Item -ItemType Directory -Force -Path $BinDir, $ConfigDir, $LogDir | Out-Null\nCopy-Item -Force -LiteralPath (Join-Path $PackageDir 'bin/wvst-bridge-server.exe') -Destination (Join-Path $BinDir 'wvst-bridge-server.exe')\nCopy-Item -Force -LiteralPath (Join-Path $PackageDir 'bin/wvst-host-worker.exe') -Destination (Join-Path $BinDir 'wvst-host-worker.exe')\nCopy-Item -Force -LiteralPath (Join-Path $PackageDir 'bin/wvst-bridge-launcher.ps1') -Destination (Join-Path $BinDir 'wvst-bridge-launcher.ps1')\nCopy-Item -Force -LiteralPath (Join-Path $PackageDir 'scripts/rotate-logs-windows.ps1') -Destination (Join-Path $BinDir 'wvst-log-rotate.ps1')\nif (-not (Test-Path -LiteralPath $ConfigFile)) {{\n  Copy-Item -LiteralPath (Join-Path $PackageDir 'config/wvst.env.example') -Destination $ConfigFile\n  $Token = ([guid]::NewGuid().ToString('N') + [guid]::NewGuid().ToString('N'))\n  (Get-Content -LiteralPath $ConfigFile) | ForEach-Object {{ if ($_ -like 'WVST_TOKEN=*') {{ \"WVST_TOKEN=$Token\" }} else {{ $_ }} }} | Set-Content -LiteralPath $ConfigFile -Encoding utf8\n}}\n& (Join-Path $BinDir 'wvst-log-rotate.ps1') -LogDir $LogDir *> $null\n$Launcher = Join-Path $BinDir 'wvst-bridge-launcher.ps1'\n$Action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument \"-NoProfile -ExecutionPolicy Bypass -File `\"$Launcher`\"\"\n$Trigger = New-ScheduledTaskTrigger -AtLogOn\nRegister-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Description 'WVST Bridge Server' -Force | Out-Null\nStart-ScheduledTask -TaskName $TaskName\nWrite-Output \"WVST bridge installed. Config: $ConfigFile\"\nWrite-Output \"Scheduled task: $TaskName\"\nWrite-Output \"Logs: $LogDir\"\n",
        install_prefix = install_prefix,
        task_name = task_name,
    )
}

fn uninstall_script(config: &WindowsPackageConfig) -> String {
    let install_prefix = powershell_quote_path(&config.install_prefix);
    let task_name = powershell_quote(&config.scheduled_task_name);

    format!(
        "$ErrorActionPreference = 'Stop'\n$InstallPrefix = {install_prefix}\n$TaskName = {task_name}\n$Task = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue\nif ($Task) {{\n  Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue\n  Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false\n}}\n$BinDir = Join-Path $InstallPrefix 'bin'\nRemove-Item -Force -ErrorAction SilentlyContinue (Join-Path $BinDir 'wvst-bridge-server.exe'), (Join-Path $BinDir 'wvst-host-worker.exe'), (Join-Path $BinDir 'wvst-bridge-launcher.ps1'), (Join-Path $BinDir 'wvst-log-rotate.ps1')\nWrite-Output 'WVST bridge scheduled task removed. Config and logs were left in place.'\n",
        install_prefix = install_prefix,
        task_name = task_name,
    )
}

fn diagnose_script(config: &WindowsPackageConfig) -> String {
    let install_prefix = powershell_quote_path(&config.install_prefix);
    let task_name = powershell_quote(&config.scheduled_task_name);

    format!(
        "$ErrorActionPreference = 'Stop'\n$InstallPrefix = {install_prefix}\n$TaskName = {task_name}\n$ConfigFile = if ($env:WVST_CONFIG_FILE) {{ $env:WVST_CONFIG_FILE }} else {{ Join-Path $InstallPrefix 'config\\wvst.env' }}\nif (Test-Path -LiteralPath $ConfigFile) {{\n  Get-Content -LiteralPath $ConfigFile | ForEach-Object {{\n    if ($_ -match '^\\s*(#|$)') {{ return }}\n    $Parts = $_ -split '=', 2\n    if ($Parts.Count -eq 2) {{ [Environment]::SetEnvironmentVariable($Parts[0].Trim(), $Parts[1], 'Process') }}\n  }}\n}}\nif (-not $env:WVST_HOST_WORKER) {{ $env:WVST_HOST_WORKER = Join-Path $InstallPrefix 'bin\\wvst-host-worker.exe' }}\n$LogDir = if ($env:WVST_LOG_DIR) {{ $env:WVST_LOG_DIR }} else {{ Join-Path $InstallPrefix 'logs' }}\n$Task = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue\nif ($Task) {{ [Console]::Error.WriteLine(\"WVST scheduled task state: $($Task.State)\") }} else {{ [Console]::Error.WriteLine('WVST scheduled task state: not registered') }}\n[Console]::Error.WriteLine(\"WVST log directory: $LogDir\")\nif (Test-Path -LiteralPath $LogDir) {{ Get-ChildItem -LiteralPath $LogDir | Sort-Object LastWriteTime -Descending | Select-Object -First 10 | Format-Table -AutoSize | Out-String | ForEach-Object {{ [Console]::Error.WriteLine($_) }} }}\n& (Join-Path $InstallPrefix 'bin\\wvst-bridge-server.exe') diagnose\nexit $LASTEXITCODE\n",
        install_prefix = install_prefix,
        task_name = task_name,
    )
}

fn rotate_logs_script(config: &WindowsPackageConfig) -> String {
    let install_prefix = powershell_quote_path(&config.install_prefix);

    format!(
        r#"param(
  [string]$LogDir = $(if ($env:WVST_LOG_DIR) {{ $env:WVST_LOG_DIR }} else {{ Join-Path {install_prefix} 'logs' }}),
  [int64]$MaxBytes = $(if ($env:WVST_LOG_MAX_BYTES) {{ [int64]$env:WVST_LOG_MAX_BYTES }} else {{ {max_bytes} }}),
  [int]$Keep = $(if ($env:WVST_LOG_KEEP) {{ [int]$env:WVST_LOG_KEEP }} else {{ {keep} }})
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

function Rotate-WVSTLog([string]$Path) {{
  if (-not (Test-Path -LiteralPath $Path)) {{ return }}
  $Item = Get-Item -LiteralPath $Path
  if ($Item.Length -le $MaxBytes) {{ return }}

  for ($Index = $Keep; $Index -gt 1; $Index--) {{
    $Previous = "$Path.$($Index - 1)"
    $Next = "$Path.$Index"
    if (Test-Path -LiteralPath $Previous) {{
      Move-Item -Force -LiteralPath $Previous -Destination $Next
    }}
  }}

  Copy-Item -Force -LiteralPath $Path -Destination "$Path.1"
  Clear-Content -LiteralPath $Path
}}

Rotate-WVSTLog (Join-Path $LogDir 'bridge.log')
Rotate-WVSTLog (Join-Path $LogDir 'bridge.err.log')
"#,
        install_prefix = install_prefix,
        max_bytes = DEFAULT_LOG_MAX_BYTES,
        keep = DEFAULT_LOG_KEEP,
    )
}

fn verify_script() -> String {
    r#"$ErrorActionPreference = 'Stop'
$PackageDir = Resolve-Path (Join-Path $PSScriptRoot '..')
$StrictSignature = $env:WVST_STRICT_AUTHENTICODE_VERIFY -eq '1'
$ReportPath = if ($env:WVST_VERIFY_REPORT) { $env:WVST_VERIFY_REPORT } else { Join-Path $PackageDir 'wvst-verify-report.json' }
$ChecksRun = 0
$ChecksFailed = 0
$ChecksSkipped = 0
$Warnings = 0
$Failed = $false

function Add-WVSTVerifyPass {
  $script:ChecksRun += 1
}

function Fail-WVSTVerify([string]$Message) {
  [Console]::Error.WriteLine("WVST Windows bundle verification failed: $Message")
  $script:Failed = $true
  $script:ChecksRun += 1
  $script:ChecksFailed += 1
}

function Skip-WVSTVerify([string]$Message) {
  [Console]::Error.WriteLine("WVST Windows bundle verification skipped: $Message")
  $script:ChecksSkipped += 1
}

function Warn-WVSTVerify([string]$Message) {
  [Console]::Error.WriteLine("WVST Windows bundle verification warning: $Message")
  $script:ChecksRun += 1
  $script:Warnings += 1
}

function Write-WVSTVerifyReport {
  $Status = if ($script:Failed) { 'failed' } else { 'passed' }
  $ReportDir = Split-Path -Parent $ReportPath
  if ($ReportDir) { New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null }
  $Report = [ordered]@{
    schemaVersion = 1
    platform = 'windows'
    packageDir = $PackageDir.Path
    status = $Status
    strictSignature = [bool]$StrictSignature
    checksRun = $script:ChecksRun
    checksFailed = $script:ChecksFailed
    checksSkipped = $script:ChecksSkipped
    warnings = $script:Warnings
    checks = @('bundle-files', 'env-template', 'scheduled-task-scripts', 'authenticode', 'bridge-diagnose')
  }
  $Report | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ReportPath -Encoding utf8
}

function Require-File([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    Fail-WVSTVerify "missing file $Path"
  } else {
    Add-WVSTVerifyPass
  }
}

function Require-Text([string]$Path, [string]$Text) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    Fail-WVSTVerify "missing file $Path"
    return
  }
  Add-WVSTVerifyPass
  $Content = Get-Content -LiteralPath $Path -Raw
  if (-not $Content.Contains($Text)) {
    Fail-WVSTVerify "missing text '$Text' in $Path"
  } else {
    Add-WVSTVerifyPass
  }
}

trap {
  Fail-WVSTVerify $_.Exception.Message
  Write-WVSTVerifyReport
  exit 1
}

$Bridge = Join-Path $PackageDir 'bin/wvst-bridge-server.exe'
$Worker = Join-Path $PackageDir 'bin/wvst-host-worker.exe'
$Launcher = Join-Path $PackageDir 'bin/wvst-bridge-launcher.ps1'
$EnvExample = Join-Path $PackageDir 'config/wvst.env.example'
$Install = Join-Path $PackageDir 'scripts/install-windows.ps1'
$Uninstall = Join-Path $PackageDir 'scripts/uninstall-windows.ps1'
$Diagnose = Join-Path $PackageDir 'scripts/diagnose-windows.ps1'
$Rotate = Join-Path $PackageDir 'scripts/rotate-logs-windows.ps1'

Require-File $Bridge
Require-File $Worker
Require-File $Launcher
Require-File $EnvExample
Require-File $Install
Require-File $Uninstall
Require-File $Diagnose
Require-File $Rotate
Require-File (Join-Path $PackageDir 'README.md')

Require-Text $EnvExample 'WVST_BIND_ADDR='
Require-Text $EnvExample 'WVST_HOST_WORKER='
Require-Text $Install 'Register-ScheduledTask'
Require-Text $Install 'Start-ScheduledTask'
Require-Text $Diagnose 'Get-ScheduledTask'
Require-Text $Diagnose 'wvst-bridge-server.exe'
Require-Text $Rotate 'Rotate-WVSTLog'

foreach ($Target in @($Bridge, $Worker)) {
  if (Get-Command Get-AuthenticodeSignature -ErrorAction SilentlyContinue) {
    $Signature = Get-AuthenticodeSignature -LiteralPath $Target
    if ($Signature.Status -ne 'Valid') {
      if ($StrictSignature) {
        Fail-WVSTVerify "Authenticode signature is $($Signature.Status) for $Target"
      } else {
        Warn-WVSTVerify "Authenticode signature is $($Signature.Status) for $Target; set WVST_STRICT_AUTHENTICODE_VERIFY=1 to fail unsigned dev bundles."
      }
    } else {
      Add-WVSTVerifyPass
    }
  } else {
    Skip-WVSTVerify "Get-AuthenticodeSignature is unavailable for $Target"
  }
}

$env:WVST_HOST_WORKER = $Worker
& $Bridge diagnose *> $null
if ($LASTEXITCODE -ne 0) {
  Fail-WVSTVerify 'wvst-bridge-server diagnose failed'
} else {
  Add-WVSTVerifyPass
}

Write-WVSTVerifyReport

if ($Failed) {
  exit 1
}

Write-Output "WVST Windows bundle verification passed: $PackageDir"
"#
    .to_string()
}

fn readme(config: &WindowsPackageConfig) -> String {
    format!(
        "# WVST Windows Bundle\n\nInstall the per-user scheduled task with PowerShell:\n\n```powershell\nSet-ExecutionPolicy -Scope Process Bypass\n.\\scripts\\install-windows.ps1\n```\n\nThe installer copies the bridge server, host worker, launcher, and log rotator into `{install_prefix}`; creates `config\\wvst.env` with a generated token if missing; registers the `{task}` scheduled task; starts it for the current user; and writes logs under `logs`.\n\nThe bundle includes `{manifest}` as machine-readable package evidence.\n\nVerify bundle structure, Scheduled Task script content, optional Authenticode signature state, and local diagnostics with:\n\n```powershell\n.\\scripts\\verify-windows.ps1\n$env:WVST_STRICT_AUTHENTICODE_VERIFY = '1'; .\\scripts\\verify-windows.ps1\n```\n\nThe verify script writes `{report}` by default; set `$env:WVST_VERIFY_REPORT = 'C:\\path\\to\\report.json'` to override the report path.\n\nRun local diagnostics with:\n\n```powershell\n.\\scripts\\diagnose-windows.ps1\n```\n\nRotate logs manually with:\n\n```powershell\n.\\scripts\\rotate-logs-windows.ps1\n```\n\nUninstall the scheduled task and installed binaries with:\n\n```powershell\n.\\scripts\\uninstall-windows.ps1\n```\n\nConfiguration and logs are intentionally preserved by uninstall.\n",
        install_prefix = windows_path(&config.install_prefix),
        task = config.scheduled_task_name,
        manifest = PACKAGE_MANIFEST_FILE_NAME,
        report = VERIFY_REPORT_FILE_NAME,
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
#[path = "windows_tests.rs"]
mod tests;
