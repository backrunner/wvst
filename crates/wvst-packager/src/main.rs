use std::path::PathBuf;

use wvst_packager::{
    DEFAULT_BIND_ADDR, DEFAULT_LAUNCHD_LABEL, DEFAULT_SYSTEMD_SERVICE_NAME, LinuxPackageConfig,
    MacosPackageConfig, build_linux_package, build_macos_package, default_install_prefix,
    default_linux_install_prefix, default_release_binary,
};

fn main() {
    if let Err(error) = run(std::env::args().skip(1).collect()) {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("macos") => build_macos(args.into_iter().skip(1).collect()),
        Some("linux") => build_linux(args.into_iter().skip(1).collect()),
        Some("--help" | "-h" | "help") => {
            print_help();
            Ok(())
        }
        Some(command) => Err(format!("unknown command: {command}")),
        None => {
            print_help();
            Ok(())
        }
    }
}

fn build_macos(args: Vec<String>) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_macos_help();
        return Ok(());
    }

    let options = MacosOptions::parse(args)?;
    let mut config = MacosPackageConfig::new(
        options
            .bridge_server
            .unwrap_or_else(|| default_release_binary("wvst-bridge-server")),
        options
            .host_worker
            .unwrap_or_else(|| default_release_binary("wvst-host-worker")),
        options.output_dir,
    );
    config.install_prefix = options
        .install_prefix
        .unwrap_or_else(default_install_prefix);
    config.launchd_label = options
        .launchd_label
        .unwrap_or_else(|| DEFAULT_LAUNCHD_LABEL.to_string());
    config.bind_addr = options
        .bind_addr
        .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string());

    let manifest = build_macos_package(&config).map_err(|error| error.to_string())?;
    println!("{}", manifest.package_root.display());
    for file in manifest.files {
        println!("  {}", file.display());
    }
    Ok(())
}

fn build_linux(args: Vec<String>) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_linux_help();
        return Ok(());
    }

    let options = LinuxOptions::parse(args)?;
    let mut config = LinuxPackageConfig::new(
        options
            .bridge_server
            .unwrap_or_else(|| default_release_binary("wvst-bridge-server")),
        options
            .host_worker
            .unwrap_or_else(|| default_release_binary("wvst-host-worker")),
        options.output_dir,
    );
    config.install_prefix = options
        .install_prefix
        .unwrap_or_else(default_linux_install_prefix);
    config.systemd_service_name = options
        .systemd_service
        .unwrap_or_else(|| DEFAULT_SYSTEMD_SERVICE_NAME.to_string());
    config.bind_addr = options
        .bind_addr
        .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string());

    let manifest = build_linux_package(&config).map_err(|error| error.to_string())?;
    println!("{}", manifest.package_root.display());
    for file in manifest.files {
        println!("  {}", file.display());
    }
    Ok(())
}

#[derive(Debug, Default)]
struct MacosOptions {
    bridge_server: Option<PathBuf>,
    host_worker: Option<PathBuf>,
    output_dir: PathBuf,
    install_prefix: Option<PathBuf>,
    launchd_label: Option<String>,
    bind_addr: Option<String>,
}

#[derive(Debug, Default)]
struct LinuxOptions {
    bridge_server: Option<PathBuf>,
    host_worker: Option<PathBuf>,
    output_dir: PathBuf,
    install_prefix: Option<PathBuf>,
    systemd_service: Option<String>,
    bind_addr: Option<String>,
}

impl MacosOptions {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut options = Self {
            output_dir: PathBuf::from("target/wvst-package"),
            ..Self::default()
        };
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--bridge-server" => {
                    options.bridge_server = Some(next_path(&mut args, "--bridge-server")?);
                }
                "--host-worker" => {
                    options.host_worker = Some(next_path(&mut args, "--host-worker")?);
                }
                "--out" | "--output-dir" => {
                    options.output_dir = next_path(&mut args, "--out")?;
                }
                "--install-prefix" => {
                    options.install_prefix = Some(next_path(&mut args, "--install-prefix")?);
                }
                "--launchd-label" => {
                    options.launchd_label = Some(next_value(&mut args, "--launchd-label")?);
                }
                "--bind-addr" => {
                    options.bind_addr = Some(next_value(&mut args, "--bind-addr")?);
                }
                other => return Err(format!("unknown macos option: {other}")),
            }
        }

        Ok(options)
    }
}

impl LinuxOptions {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut options = Self {
            output_dir: PathBuf::from("target/wvst-package"),
            ..Self::default()
        };
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--bridge-server" => {
                    options.bridge_server = Some(next_path(&mut args, "--bridge-server")?);
                }
                "--host-worker" => {
                    options.host_worker = Some(next_path(&mut args, "--host-worker")?);
                }
                "--out" | "--output-dir" => {
                    options.output_dir = next_path(&mut args, "--out")?;
                }
                "--install-prefix" => {
                    options.install_prefix = Some(next_path(&mut args, "--install-prefix")?);
                }
                "--systemd-service" | "--service-name" => {
                    options.systemd_service = Some(next_value(&mut args, "--systemd-service")?);
                }
                "--bind-addr" => {
                    options.bind_addr = Some(next_value(&mut args, "--bind-addr")?);
                }
                other => return Err(format!("unknown linux option: {other}")),
            }
        }

        Ok(options)
    }
}

fn next_path(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<PathBuf, String> {
    next_value(args, flag).map(PathBuf::from)
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn print_help() {
    println!(
        "usage: wvst-packager <command>\n\ncommands:\n  macos    build a macOS release bundle\n  linux    build a Linux systemd user-service bundle"
    );
    println!();
    print_macos_help();
    print_linux_help();
}

fn print_macos_help() {
    println!(
        "usage: wvst-packager macos [--bridge-server PATH] [--host-worker PATH] [--out DIR] [--install-prefix PATH] [--launchd-label LABEL] [--bind-addr HOST:PORT]"
    );
}

fn print_linux_help() {
    println!(
        "usage: wvst-packager linux [--bridge-server PATH] [--host-worker PATH] [--out DIR] [--install-prefix PATH] [--systemd-service NAME] [--bind-addr HOST:PORT]"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_macos_options() {
        let options = MacosOptions::parse(vec![
            "--bridge-server".into(),
            "/tmp/bridge".into(),
            "--host-worker".into(),
            "/tmp/worker".into(),
            "--out".into(),
            "/tmp/out".into(),
            "--install-prefix".into(),
            "/tmp/install".into(),
            "--launchd-label".into(),
            "top.backrunner.test".into(),
            "--bind-addr".into(),
            "127.0.0.1:4000".into(),
        ])
        .expect("options");

        assert_eq!(options.bridge_server, Some(PathBuf::from("/tmp/bridge")));
        assert_eq!(options.host_worker, Some(PathBuf::from("/tmp/worker")));
        assert_eq!(options.output_dir, PathBuf::from("/tmp/out"));
        assert_eq!(options.install_prefix, Some(PathBuf::from("/tmp/install")));
        assert_eq!(options.launchd_label, Some("top.backrunner.test".into()));
        assert_eq!(options.bind_addr, Some("127.0.0.1:4000".into()));
    }

    #[test]
    fn defaults_macos_output_dir() {
        let options = MacosOptions::parse(Vec::new()).expect("options");

        assert_eq!(options.output_dir, PathBuf::from("target/wvst-package"));
    }

    #[test]
    fn parses_linux_options() {
        let options = LinuxOptions::parse(vec![
            "--bridge-server".into(),
            "/tmp/bridge".into(),
            "--host-worker".into(),
            "/tmp/worker".into(),
            "--out".into(),
            "/tmp/out".into(),
            "--install-prefix".into(),
            "/tmp/install".into(),
            "--systemd-service".into(),
            "top.backrunner.test".into(),
            "--bind-addr".into(),
            "127.0.0.1:4000".into(),
        ])
        .expect("options");

        assert_eq!(options.bridge_server, Some(PathBuf::from("/tmp/bridge")));
        assert_eq!(options.host_worker, Some(PathBuf::from("/tmp/worker")));
        assert_eq!(options.output_dir, PathBuf::from("/tmp/out"));
        assert_eq!(options.install_prefix, Some(PathBuf::from("/tmp/install")));
        assert_eq!(options.systemd_service, Some("top.backrunner.test".into()));
        assert_eq!(options.bind_addr, Some("127.0.0.1:4000".into()));
    }

    #[test]
    fn defaults_linux_output_dir() {
        let options = LinuxOptions::parse(Vec::new()).expect("options");

        assert_eq!(options.output_dir, PathBuf::from("target/wvst-package"));
    }
}
