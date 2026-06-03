use std::io;
use std::path::PathBuf;

use crate::{
    DEFAULT_BIND_ADDR, copy_binary, ensure_file, home_dir, shell_quote, shell_quote_path,
    write_text,
};

pub const DEFAULT_SYSTEMD_SERVICE_NAME: &str = "top.backrunner.wvst.bridge";

#[derive(Debug, Clone)]
pub struct LinuxPackageConfig {
    pub bridge_server: PathBuf,
    pub host_worker: PathBuf,
    pub output_dir: PathBuf,
    pub install_prefix: PathBuf,
    pub systemd_service_name: String,
    pub bind_addr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxPackageManifest {
    pub package_root: PathBuf,
    pub files: Vec<PathBuf>,
}

impl LinuxPackageConfig {
    pub fn new(
        bridge_server: impl Into<PathBuf>,
        host_worker: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            bridge_server: bridge_server.into(),
            host_worker: host_worker.into(),
            output_dir: output_dir.into(),
            install_prefix: default_linux_install_prefix(),
            systemd_service_name: DEFAULT_SYSTEMD_SERVICE_NAME.to_string(),
            bind_addr: DEFAULT_BIND_ADDR.to_string(),
        }
    }

    pub fn package_root(&self) -> PathBuf {
        self.output_dir.join("wvst-linux")
    }
}

pub fn default_linux_install_prefix() -> PathBuf {
    home_dir().join(".local").join("share").join("wvst")
}

pub fn default_systemd_user_unit_path(service_name: &str) -> PathBuf {
    home_dir()
        .join(".config")
        .join("systemd")
        .join("user")
        .join(format!("{service_name}.service"))
}

pub fn build_linux_package(config: &LinuxPackageConfig) -> io::Result<LinuxPackageManifest> {
    ensure_file(&config.bridge_server)?;
    ensure_file(&config.host_worker)?;

    let root = config.package_root();
    let bin_dir = root.join("bin");
    let config_dir = root.join("config");
    let systemd_dir = root.join("systemd");
    let scripts_dir = root.join("scripts");
    std::fs::create_dir_all(&bin_dir)?;
    std::fs::create_dir_all(&config_dir)?;
    std::fs::create_dir_all(&systemd_dir)?;
    std::fs::create_dir_all(&scripts_dir)?;

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
        &systemd_dir.join(format!("{}.service", config.systemd_service_name)),
        &systemd_user_service(config),
        false,
    )?);
    files.push(write_text(
        &scripts_dir.join("install-linux.sh"),
        &install_script(config),
        true,
    )?);
    files.push(write_text(
        &scripts_dir.join("uninstall-linux.sh"),
        &uninstall_script(config),
        true,
    )?);
    files.push(write_text(
        &scripts_dir.join("diagnose-linux.sh"),
        &diagnose_script(config),
        true,
    )?);
    files.push(write_text(&root.join("README.md"), &readme(config), false)?);

    Ok(LinuxPackageManifest {
        package_root: root,
        files,
    })
}

fn launcher_script(config: &LinuxPackageConfig) -> String {
    let config_file = config.install_prefix.join("config").join("wvst.env");
    let host_worker = config.install_prefix.join("bin").join("wvst-host-worker");
    let bridge_server = config.install_prefix.join("bin").join("wvst-bridge-server");

    format!(
        "#!/bin/sh\nset -eu\n\nCONFIG_FILE=${{WVST_CONFIG_FILE:-{config_file}}}\nif [ -f \"$CONFIG_FILE\" ]; then\n  set -a\n  . \"$CONFIG_FILE\"\n  set +a\nfi\n\nexport WVST_HOST_WORKER=${{WVST_HOST_WORKER:-{host_worker}}}\nexec {bridge_server} serve\n",
        config_file = shell_quote_path(&config_file),
        host_worker = shell_quote_path(&host_worker),
        bridge_server = shell_quote_path(&bridge_server),
    )
}

fn env_example(config: &LinuxPackageConfig) -> String {
    let host_worker = config.install_prefix.join("bin").join("wvst-host-worker");
    format!(
        "WVST_BIND_ADDR={bind_addr}\nWVST_TOKEN=\nWVST_ALLOWED_ORIGINS=http://localhost:5173\nWVST_ALLOW_LOOPBACK_ORIGINS=1\nWVST_HOST_WORKER={host_worker}\nWVST_WORKER_AUTO_RESTART=1\nWVST_WORKER_TIMEOUT_MS=5000\n",
        bind_addr = config.bind_addr,
        host_worker = shell_quote_path(&host_worker),
    )
}

fn systemd_user_service(config: &LinuxPackageConfig) -> String {
    let launcher = config
        .install_prefix
        .join("bin")
        .join("wvst-bridge-launcher");
    let config_file = config.install_prefix.join("config").join("wvst.env");

    format!(
        "[Unit]\nDescription=WVST Bridge Server\nAfter=default.target\n\n[Service]\nType=simple\nEnvironment=WVST_CONFIG_FILE={config_file}\nExecStart={launcher}\nRestart=on-failure\nRestartSec=2\nKillMode=control-group\n\n[Install]\nWantedBy=default.target\n",
        config_file = systemd_escape(&config_file.display().to_string()),
        launcher = systemd_escape(&launcher.display().to_string()),
    )
}

fn install_script(config: &LinuxPackageConfig) -> String {
    let install_prefix = shell_quote_path(&config.install_prefix);
    let service_name = shell_quote(&config.systemd_service_name);
    let service_file_name = format!("{}.service", config.systemd_service_name);

    format!(
        "#!/bin/sh\nset -eu\n\nPACKAGE_DIR=$(CDPATH= cd -- \"$(dirname -- \"$0\")/..\" && pwd)\nINSTALL_PREFIX={install_prefix}\nSERVICE_NAME={service_name}\nSYSTEMD_USER_DIR=\"${{XDG_CONFIG_HOME:-$HOME/.config}}/systemd/user\"\nUNIT_FILE=\"$SYSTEMD_USER_DIR/$SERVICE_NAME.service\"\nCONFIG_DIR=\"$INSTALL_PREFIX/config\"\nCONFIG_FILE=\"$CONFIG_DIR/wvst.env\"\n\nif ! command -v systemctl >/dev/null 2>&1; then\n  printf 'systemctl is required for the WVST Linux user service.\\n' >&2\n  exit 1\nfi\n\nmkdir -p \"$INSTALL_PREFIX/bin\" \"$CONFIG_DIR\" \"$SYSTEMD_USER_DIR\"\ncp \"$PACKAGE_DIR/bin/wvst-bridge-server\" \"$INSTALL_PREFIX/bin/wvst-bridge-server\"\ncp \"$PACKAGE_DIR/bin/wvst-host-worker\" \"$INSTALL_PREFIX/bin/wvst-host-worker\"\ncp \"$PACKAGE_DIR/bin/wvst-bridge-launcher\" \"$INSTALL_PREFIX/bin/wvst-bridge-launcher\"\nchmod 755 \"$INSTALL_PREFIX/bin/wvst-bridge-server\" \"$INSTALL_PREFIX/bin/wvst-host-worker\" \"$INSTALL_PREFIX/bin/wvst-bridge-launcher\"\n\nif [ ! -f \"$CONFIG_FILE\" ]; then\n  cp \"$PACKAGE_DIR/config/wvst.env.example\" \"$CONFIG_FILE\"\n  TOKEN=$(openssl rand -hex 24 2>/dev/null || uuidgen 2>/dev/null | tr -d '-' || date +%s)\n  awk -v token=\"$TOKEN\" '/^WVST_TOKEN=/{{print \"WVST_TOKEN=\" token; next}} {{print}}' \"$CONFIG_FILE\" > \"$CONFIG_FILE.tmp\"\n  mv \"$CONFIG_FILE.tmp\" \"$CONFIG_FILE\"\n  chmod 600 \"$CONFIG_FILE\"\nfi\n\ncp \"$PACKAGE_DIR/systemd/{service_file_name}\" \"$UNIT_FILE\"\nsystemctl --user daemon-reload\nsystemctl --user enable --now \"$SERVICE_NAME.service\"\n\nprintf 'WVST bridge installed. Config: %s\\n' \"$CONFIG_FILE\"\nprintf 'systemd user service: %s\\n' \"$SERVICE_NAME.service\"\n",
        install_prefix = install_prefix,
        service_name = service_name,
        service_file_name = service_file_name,
    )
}

fn uninstall_script(config: &LinuxPackageConfig) -> String {
    let install_prefix = shell_quote_path(&config.install_prefix);
    let service_name = shell_quote(&config.systemd_service_name);

    format!(
        "#!/bin/sh\nset -eu\n\nINSTALL_PREFIX={install_prefix}\nSERVICE_NAME={service_name}\nSYSTEMD_USER_DIR=\"${{XDG_CONFIG_HOME:-$HOME/.config}}/systemd/user\"\nUNIT_FILE=\"$SYSTEMD_USER_DIR/$SERVICE_NAME.service\"\n\nif command -v systemctl >/dev/null 2>&1; then\n  systemctl --user disable --now \"$SERVICE_NAME.service\" >/dev/null 2>&1 || true\nfi\nrm -f \"$UNIT_FILE\"\nrm -f \"$INSTALL_PREFIX/bin/wvst-bridge-server\" \"$INSTALL_PREFIX/bin/wvst-host-worker\" \"$INSTALL_PREFIX/bin/wvst-bridge-launcher\"\nif command -v systemctl >/dev/null 2>&1; then\n  systemctl --user daemon-reload >/dev/null 2>&1 || true\nfi\nprintf 'WVST bridge user service removed. Config was left in place.\\n'\n",
        install_prefix = install_prefix,
        service_name = service_name,
    )
}

fn diagnose_script(config: &LinuxPackageConfig) -> String {
    let install_prefix = shell_quote_path(&config.install_prefix);
    let service_name = shell_quote(&config.systemd_service_name);

    format!(
        "#!/bin/sh\nset -eu\n\nINSTALL_PREFIX={install_prefix}\nSERVICE_NAME={service_name}\nCONFIG_FILE=${{WVST_CONFIG_FILE:-$INSTALL_PREFIX/config/wvst.env}}\nif [ -f \"$CONFIG_FILE\" ]; then\n  set -a\n  . \"$CONFIG_FILE\"\n  set +a\nfi\nexport WVST_HOST_WORKER=${{WVST_HOST_WORKER:-$INSTALL_PREFIX/bin/wvst-host-worker}}\nprintf 'WVST systemd user service status for %s:\\n' \"$SERVICE_NAME.service\" >&2\nsystemctl --user is-active \"$SERVICE_NAME.service\" >&2 || true\nexec \"$INSTALL_PREFIX/bin/wvst-bridge-server\" diagnose\n",
        install_prefix = install_prefix,
        service_name = service_name,
    )
}

fn readme(config: &LinuxPackageConfig) -> String {
    format!(
        "# WVST Linux Bundle\n\nInstall the user service with:\n\n```sh\n./scripts/install-linux.sh\n```\n\nThe installer copies the bridge server, host worker, and launcher into `{install_prefix}`; creates `config/wvst.env` with a generated token if missing; installs the `{service}` systemd user service; and starts it with `systemctl --user enable --now {service}.service`.\n\nCheck service status with:\n\n```sh\nsystemctl --user status {service}.service\njournalctl --user -u {service}.service\n```\n\nRun local diagnostics with:\n\n```sh\n./scripts/diagnose-linux.sh\n```\n\nUninstall the service and installed binaries with:\n\n```sh\n./scripts/uninstall-linux.sh\n```\n\nConfiguration is intentionally preserved by uninstall.\n",
        install_prefix = config.install_prefix.display(),
        service = config.systemd_service_name,
    )
}

fn systemd_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace(' ', "\\x20")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn builds_linux_bundle_with_systemd_and_scripts() {
        let root = unique_temp_dir();
        let bridge = root.join("source").join("wvst-bridge-server");
        let worker = root.join("source").join("wvst-host-worker");
        std::fs::create_dir_all(bridge.parent().expect("source dir")).expect("source dir");
        std::fs::write(&bridge, b"bridge").expect("bridge binary");
        std::fs::write(&worker, b"worker").expect("worker binary");

        let mut config = LinuxPackageConfig::new(&bridge, &worker, root.join("out"));
        config.install_prefix = root.join("install prefix");
        config.systemd_service_name = "top.backrunner.wvst.test".to_string();

        let manifest = build_linux_package(&config).expect("package");

        assert!(manifest.package_root.ends_with("wvst-linux"));
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
                .any(|path| path.ends_with("bin/wvst-bridge-launcher"))
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

        let unit = std::fs::read_to_string(
            manifest
                .package_root
                .join("systemd")
                .join("top.backrunner.wvst.test.service"),
        )
        .expect("systemd unit");
        assert!(unit.contains("[Service]"));
        assert!(unit.contains("ExecStart="));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("KillMode=control-group"));

        let install = std::fs::read_to_string(
            manifest
                .package_root
                .join("scripts")
                .join("install-linux.sh"),
        )
        .expect("install script");
        assert!(install.contains("systemctl --user enable --now"));
        assert!(install.contains("openssl rand -hex 24"));

        let diagnose = std::fs::read_to_string(
            manifest
                .package_root
                .join("scripts")
                .join("diagnose-linux.sh"),
        )
        .expect("diagnose script");
        assert!(diagnose.contains("wvst-bridge-server\" diagnose"));
        assert!(diagnose.contains("WVST_HOST_WORKER"));
        assert!(diagnose.contains("systemctl --user is-active"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_missing_binaries() {
        let root = unique_temp_dir();
        let config = LinuxPackageConfig::new(
            root.join("missing-bridge"),
            root.join("missing-worker"),
            root.join("out"),
        );

        let error = build_linux_package(&config).expect_err("missing binary");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        let _ = std::fs::remove_dir_all(root);
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "wvst-linux-packager-{}-{nanos}",
            std::process::id()
        ))
    }
}
