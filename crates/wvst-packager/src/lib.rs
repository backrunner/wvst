use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub mod release;

mod linux;
mod macos_scripts;
mod package_manifest;
mod windows;

use macos_scripts::{sign_notarize_script, verify_macos_script};
use package_manifest::{
    PACKAGE_MANIFEST_FILE_NAME, VERIFY_REPORT_FILE_NAME, macos_package_manifest,
    write_package_manifest,
};

pub use linux::{
    DEFAULT_SYSTEMD_SERVICE_NAME, LinuxPackageConfig, LinuxPackageManifest, build_linux_package,
    default_linux_install_prefix, default_systemd_user_unit_path,
};
pub use windows::{
    DEFAULT_SCHEDULED_TASK_NAME, WindowsPackageConfig, WindowsPackageManifest,
    build_windows_package, default_windows_install_prefix,
};

pub const DEFAULT_LAUNCHD_LABEL: &str = "top.backrunner.wvst.bridge";
pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:35876";
pub const DEFAULT_LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;
pub const DEFAULT_LOG_KEEP: u8 = 5;

#[derive(Debug, Clone)]
pub struct MacosPackageConfig {
    pub bridge_server: PathBuf,
    pub host_worker: PathBuf,
    pub output_dir: PathBuf,
    pub install_prefix: PathBuf,
    pub launchd_label: String,
    pub bind_addr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacosPackageManifest {
    pub package_root: PathBuf,
    pub files: Vec<PathBuf>,
}

impl MacosPackageConfig {
    pub fn new(
        bridge_server: impl Into<PathBuf>,
        host_worker: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            bridge_server: bridge_server.into(),
            host_worker: host_worker.into(),
            output_dir: output_dir.into(),
            install_prefix: default_install_prefix(),
            launchd_label: DEFAULT_LAUNCHD_LABEL.to_string(),
            bind_addr: DEFAULT_BIND_ADDR.to_string(),
        }
    }

    pub fn package_root(&self) -> PathBuf {
        self.output_dir.join("wvst-macos")
    }
}

pub fn default_release_binary(name: &str) -> PathBuf {
    Path::new("target")
        .join("release")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

pub fn default_install_prefix() -> PathBuf {
    home_dir()
        .join("Library")
        .join("Application Support")
        .join("WVST")
}

pub fn default_launch_agent_path(label: &str) -> PathBuf {
    home_dir()
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{label}.plist"))
}

pub fn default_log_dir() -> PathBuf {
    home_dir().join("Library").join("Logs").join("WVST")
}

pub fn build_macos_package(config: &MacosPackageConfig) -> io::Result<MacosPackageManifest> {
    ensure_file(&config.bridge_server)?;
    ensure_file(&config.host_worker)?;

    let root = config.package_root();
    let bin_dir = root.join("bin");
    let config_dir = root.join("config");
    let launchd_dir = root.join("launchd");
    let scripts_dir = root.join("scripts");
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&config_dir)?;
    fs::create_dir_all(&launchd_dir)?;
    fs::create_dir_all(&scripts_dir)?;

    let mut files = Vec::new();
    files.push(copy_binary(
        &config.bridge_server,
        &bin_dir.join("wvst-bridge-server"),
    )?);
    files.push(copy_binary(
        &config.host_worker,
        &bin_dir.join("wvst-host-worker"),
    )?);
    files.push(write_text(
        &bin_dir.join("wvst-bridge-launcher"),
        &launcher_script(config),
        true,
    )?);
    files.push(write_text(
        &config_dir.join("wvst.env.example"),
        &env_example(config),
        false,
    )?);
    files.push(write_text(
        &launchd_dir.join(format!("{}.plist", config.launchd_label)),
        &launchd_plist(config),
        false,
    )?);
    files.push(write_text(
        &scripts_dir.join("install-macos.sh"),
        &install_script(config),
        true,
    )?);
    files.push(write_text(
        &scripts_dir.join("uninstall-macos.sh"),
        &uninstall_script(config),
        true,
    )?);
    files.push(write_text(
        &scripts_dir.join("diagnose-macos.sh"),
        &diagnose_script(config),
        true,
    )?);
    files.push(write_text(
        &scripts_dir.join("rotate-logs-macos.sh"),
        &rotate_logs_script(),
        true,
    )?);
    files.push(write_text(
        &scripts_dir.join("verify-macos.sh"),
        &verify_macos_script(config),
        true,
    )?);
    files.push(write_text(
        &scripts_dir.join("sign-notarize-macos.sh"),
        &sign_notarize_script(),
        true,
    )?);
    files.push(write_text(&root.join("README.md"), &readme(config), false)?);
    let evidence = macos_package_manifest(config);
    files.push(write_package_manifest(&root, evidence)?);

    Ok(MacosPackageManifest {
        package_root: root,
        files,
    })
}

fn ensure_file(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a file: {}", path.display()),
        ))
    }
}

fn copy_binary(source: &Path, destination: &Path) -> io::Result<PathBuf> {
    fs::copy(source, destination)?;
    set_executable(destination)?;
    Ok(destination.to_path_buf())
}

fn write_text(path: &Path, content: &str, executable: bool) -> io::Result<PathBuf> {
    fs::write(path, content)?;
    if executable {
        set_executable(path)?;
    }
    Ok(path.to_path_buf())
}

fn launcher_script(config: &MacosPackageConfig) -> String {
    let config_file = config.install_prefix.join("config").join("wvst.env");
    let host_worker = config.install_prefix.join("bin").join("wvst-host-worker");
    let bridge_server = config.install_prefix.join("bin").join("wvst-bridge-server");
    let log_rotate = config.install_prefix.join("bin").join("wvst-log-rotate");

    format!(
        "#!/bin/sh\nset -eu\n\nCONFIG_FILE=${{WVST_CONFIG_FILE:-{config_file}}}\nif [ -f \"$CONFIG_FILE\" ]; then\n  set -a\n  . \"$CONFIG_FILE\"\n  set +a\nfi\n\nLOG_ROTATE=${{WVST_LOG_ROTATE:-{log_rotate}}}\nif [ -x \"$LOG_ROTATE\" ]; then\n  \"$LOG_ROTATE\" >/dev/null 2>&1 || true\nfi\n\nexport WVST_HOST_WORKER=${{WVST_HOST_WORKER:-{host_worker}}}\nexec {bridge_server} serve\n",
        config_file = shell_quote_path(&config_file),
        log_rotate = shell_quote_path(&log_rotate),
        host_worker = shell_quote_path(&host_worker),
        bridge_server = shell_quote_path(&bridge_server),
    )
}

fn env_example(config: &MacosPackageConfig) -> String {
    let host_worker = config.install_prefix.join("bin").join("wvst-host-worker");
    format!(
        "WVST_BIND_ADDR={bind_addr}\nWVST_TOKEN=\nWVST_ALLOWED_ORIGINS=http://localhost:5173\nWVST_ALLOW_LOOPBACK_ORIGINS=1\nWVST_HOST_WORKER={host_worker}\nWVST_WORKER_AUTO_RESTART=1\nWVST_WORKER_TIMEOUT_MS=5000\n",
        bind_addr = config.bind_addr,
        host_worker = shell_quote_path(&host_worker),
    )
}

fn launchd_plist(config: &MacosPackageConfig) -> String {
    let launcher = config
        .install_prefix
        .join("bin")
        .join("wvst-bridge-launcher");
    let config_file = config.install_prefix.join("config").join("wvst.env");
    let log_dir = default_log_dir();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{launcher}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>EnvironmentVariables</key>
  <dict>
    <key>WVST_CONFIG_FILE</key>
    <string>{config_file}</string>
  </dict>
  <key>StandardOutPath</key>
  <string>{stdout}</string>
  <key>StandardErrorPath</key>
  <string>{stderr}</string>
</dict>
</plist>
"#,
        label = xml_escape(&config.launchd_label),
        launcher = xml_escape(&launcher.display().to_string()),
        config_file = xml_escape(&config_file.display().to_string()),
        stdout = xml_escape(&log_dir.join("bridge.log").display().to_string()),
        stderr = xml_escape(&log_dir.join("bridge.err.log").display().to_string()),
    )
}

fn install_script(config: &MacosPackageConfig) -> String {
    let install_prefix = shell_quote_path(&config.install_prefix);
    let launch_agent = shell_quote_path(&default_launch_agent_path(&config.launchd_label));
    let log_dir = shell_quote_path(&default_log_dir());
    let launchd_label = shell_quote(&config.launchd_label);
    let plist_name = format!("{}.plist", config.launchd_label);

    format!(
        "#!/bin/sh\nset -eu\n\nPACKAGE_DIR=$(CDPATH= cd -- \"$(dirname -- \"$0\")/..\" && pwd)\nINSTALL_PREFIX={install_prefix}\nLAUNCH_AGENT={launch_agent}\nLAUNCHD_LABEL={launchd_label}\nLOG_DIR={log_dir}\nCONFIG_DIR=\"$INSTALL_PREFIX/config\"\nCONFIG_FILE=\"$CONFIG_DIR/wvst.env\"\n\nmkdir -p \"$INSTALL_PREFIX/bin\" \"$CONFIG_DIR\" \"$LOG_DIR\" \"$(dirname \"$LAUNCH_AGENT\")\"\ncp \"$PACKAGE_DIR/bin/wvst-bridge-server\" \"$INSTALL_PREFIX/bin/wvst-bridge-server\"\ncp \"$PACKAGE_DIR/bin/wvst-host-worker\" \"$INSTALL_PREFIX/bin/wvst-host-worker\"\ncp \"$PACKAGE_DIR/bin/wvst-bridge-launcher\" \"$INSTALL_PREFIX/bin/wvst-bridge-launcher\"\ncp \"$PACKAGE_DIR/scripts/rotate-logs-macos.sh\" \"$INSTALL_PREFIX/bin/wvst-log-rotate\"\nchmod 755 \"$INSTALL_PREFIX/bin/wvst-bridge-server\" \"$INSTALL_PREFIX/bin/wvst-host-worker\" \"$INSTALL_PREFIX/bin/wvst-bridge-launcher\" \"$INSTALL_PREFIX/bin/wvst-log-rotate\"\n\nif [ ! -f \"$CONFIG_FILE\" ]; then\n  cp \"$PACKAGE_DIR/config/wvst.env.example\" \"$CONFIG_FILE\"\n  TOKEN=$(openssl rand -hex 24 2>/dev/null || uuidgen 2>/dev/null | tr -d '-' || date +%s)\n  awk -v token=\"$TOKEN\" '/^WVST_TOKEN=/{{print \"WVST_TOKEN=\" token; next}} {{print}}' \"$CONFIG_FILE\" > \"$CONFIG_FILE.tmp\"\n  mv \"$CONFIG_FILE.tmp\" \"$CONFIG_FILE\"\n  chmod 600 \"$CONFIG_FILE\"\nfi\n\n\"$INSTALL_PREFIX/bin/wvst-log-rotate\" >/dev/null 2>&1 || true\ncp \"$PACKAGE_DIR/launchd/{plist_name}\" \"$LAUNCH_AGENT\"\nlaunchctl bootout \"gui/$(id -u)\" \"$LAUNCH_AGENT\" >/dev/null 2>&1 || true\nlaunchctl bootstrap \"gui/$(id -u)\" \"$LAUNCH_AGENT\"\nlaunchctl kickstart -k \"gui/$(id -u)/$LAUNCHD_LABEL\" >/dev/null 2>&1 || true\n\nprintf 'WVST bridge installed. Config: %s\\n' \"$CONFIG_FILE\"\nprintf 'LaunchAgent: %s\\n' \"$LAUNCH_AGENT\"\nprintf 'Logs: %s\\n' \"$LOG_DIR\"\n",
        install_prefix = install_prefix,
        launch_agent = launch_agent,
        launchd_label = launchd_label,
        log_dir = log_dir,
        plist_name = plist_name,
    )
}

fn uninstall_script(config: &MacosPackageConfig) -> String {
    let install_prefix = shell_quote_path(&config.install_prefix);
    let launch_agent = shell_quote_path(&default_launch_agent_path(&config.launchd_label));

    format!(
        "#!/bin/sh\nset -eu\n\nINSTALL_PREFIX={install_prefix}\nLAUNCH_AGENT={launch_agent}\nlaunchctl bootout \"gui/$(id -u)\" \"$LAUNCH_AGENT\" >/dev/null 2>&1 || true\nrm -f \"$LAUNCH_AGENT\"\nrm -f \"$INSTALL_PREFIX/bin/wvst-bridge-server\" \"$INSTALL_PREFIX/bin/wvst-host-worker\" \"$INSTALL_PREFIX/bin/wvst-bridge-launcher\" \"$INSTALL_PREFIX/bin/wvst-log-rotate\"\nprintf 'WVST bridge service removed. Config and logs were left in place.\\n'\n",
        install_prefix = install_prefix,
        launch_agent = launch_agent,
    )
}

fn diagnose_script(config: &MacosPackageConfig) -> String {
    let install_prefix = shell_quote_path(&config.install_prefix);
    let launchd_label = shell_quote(&config.launchd_label);
    let log_dir = shell_quote_path(&default_log_dir());

    format!(
        "#!/bin/sh\nset -eu\n\nINSTALL_PREFIX={install_prefix}\nLAUNCHD_LABEL={launchd_label}\nLOG_DIR=${{WVST_LOG_DIR:-{log_dir}}}\nCONFIG_FILE=${{WVST_CONFIG_FILE:-$INSTALL_PREFIX/config/wvst.env}}\nif [ -f \"$CONFIG_FILE\" ]; then\n  set -a\n  . \"$CONFIG_FILE\"\n  set +a\nfi\nexport WVST_HOST_WORKER=${{WVST_HOST_WORKER:-$INSTALL_PREFIX/bin/wvst-host-worker}}\nprintf 'WVST LaunchAgent status for %s:\\n' \"$LAUNCHD_LABEL\" >&2\nlaunchctl print \"gui/$(id -u)/$LAUNCHD_LABEL\" >/dev/null 2>&1 && printf 'loaded\\n' >&2 || printf 'not loaded\\n' >&2\nprintf 'WVST log directory: %s\\n' \"$LOG_DIR\" >&2\nls -lh \"$LOG_DIR\" >&2 2>/dev/null || true\nexec \"$INSTALL_PREFIX/bin/wvst-bridge-server\" diagnose\n",
        install_prefix = install_prefix,
        launchd_label = launchd_label,
        log_dir = log_dir,
    )
}

fn rotate_logs_script() -> String {
    let log_dir = default_log_dir();

    format!(
        r#"#!/bin/sh
set -eu

LOG_DIR=${{WVST_LOG_DIR:-{log_dir}}}
MAX_BYTES=${{WVST_LOG_MAX_BYTES:-{max_bytes}}}
KEEP=${{WVST_LOG_KEEP:-{keep}}}

mkdir -p "$LOG_DIR"

rotate_one() {{
  file=$1
  [ -f "$file" ] || return 0
  size=$(wc -c < "$file" | tr -d ' ')
  [ "${{size:-0}}" -le "$MAX_BYTES" ] && return 0

  i=$KEEP
  while [ "$i" -gt 1 ]; do
    prev=$((i - 1))
    if [ -f "$file.$prev" ]; then
      mv "$file.$prev" "$file.$i"
    fi
    i=$prev
  done

  cp -p "$file" "$file.1"
  : > "$file"
}}

rotate_one "$LOG_DIR/bridge.log"
rotate_one "$LOG_DIR/bridge.err.log"
"#,
        log_dir = shell_quote_path(&log_dir),
        max_bytes = DEFAULT_LOG_MAX_BYTES,
        keep = DEFAULT_LOG_KEEP,
    )
}

fn readme(config: &MacosPackageConfig) -> String {
    format!(
        "# WVST macOS Bundle\n\nInstall with:\n\n```sh\n./scripts/install-macos.sh\n```\n\nThe installer copies the bridge server, host worker, launcher, and log rotator into `{install_prefix}`; creates `config/wvst.env` with a generated token if missing; installs the `{label}` user LaunchAgent; and writes logs under `{log_dir}`.\n\nThe bundle includes `{manifest}` as machine-readable package evidence.\n\nVerify bundle structure, plist syntax, local diagnostics, and optional code signature state with:\n\n```sh\n./scripts/verify-macos.sh\nWVST_STRICT_CODESIGN_VERIFY=1 ./scripts/verify-macos.sh\n```\n\nThe verify script writes `{report}` by default; set `WVST_VERIFY_REPORT=/path/to/report.json` to override the report path.\n\nSign and notarize the bundle with:\n\n```sh\nWVST_CODESIGN_IDENTITY=\"Developer ID Application: Example Team (TEAMID)\" \\\nWVST_NOTARY_PROFILE=\"wvst-notary\" \\\n./scripts/sign-notarize-macos.sh\n```\n\nRun local diagnostics with:\n\n```sh\n./scripts/diagnose-macos.sh\n```\n\nRotate logs manually with:\n\n```sh\n./scripts/rotate-logs-macos.sh\n```\n\nUninstall the LaunchAgent and installed binaries with:\n\n```sh\n./scripts/uninstall-macos.sh\n```\n\nConfiguration and logs are intentionally preserved by uninstall.\n",
        install_prefix = config.install_prefix.display(),
        label = config.launchd_label,
        log_dir = default_log_dir().display(),
        manifest = PACKAGE_MANIFEST_FILE_NAME,
        report = VERIFY_REPORT_FILE_NAME,
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.display().to_string())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|home| !home.is_empty()))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn set_executable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;
