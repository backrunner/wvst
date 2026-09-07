use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::versions::check;

const TARGETS: [&str; 4] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
];
pub fn release_targets() -> &'static [&'static str] {
    &TARGETS
}
fn valid_commit(commit: &str) -> Result<(), String> {
    if commit.len() != 40 || !commit.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("commit must be a full 40-character Git SHA".into());
    }
    Ok(())
}
pub(super) fn digest(path: &Path) -> Result<String, String> {
    let mut input = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 32768];
    loop {
        let size = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if size == 0 {
            break;
        }
        hasher.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
pub(super) fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|e| e.to_string())
}

/// Build a portable runtime directory, without baking the build runner's home into installers.
pub fn bundle(
    root: &Path,
    target: &str,
    binaries: &Path,
    out: &Path,
    commit: &str,
) -> Result<PathBuf, String> {
    bundle_with_team(root, target, binaries, out, commit, None)
}

pub(super) fn bundle_with_team(
    root: &Path,
    target: &str,
    binaries: &Path,
    out: &Path,
    commit: &str,
    team: Option<&str>,
) -> Result<PathBuf, String> {
    let version = check(root, None)?;
    valid_commit(commit)?;
    if !TARGETS.contains(&target) {
        return Err(format!("unsupported release target {target}"));
    }
    let suffix = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let names = [
        format!("wvst-bridge-server{suffix}"),
        format!("wvst-host-worker{suffix}"),
    ];
    for name in &names {
        let binary = binaries.join(name);
        let output = Command::new(&binary)
            .arg("--version")
            .output()
            .map_err(|e| format!("cannot check {}: {e}", binary.display()))?;
        let expected = format!("{} {version}", name.trim_end_matches(".exe"));
        if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != expected {
            return Err(format!(
                "{} does not report expected version {expected}",
                binary.display()
            ));
        }
    }
    let destination = out.join(format!("wvst-{version}-{target}"));
    if destination.exists() {
        return Err(format!(
            "{} already exists; use a new output directory",
            destination.display()
        ));
    }
    for license in ["LICENSE-MIT", "LICENSE-APACHE"] {
        if !root.join(license).is_file() {
            return Err(format!("missing {license}"));
        }
    }
    fs::create_dir_all(destination.join("bin")).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for name in &names {
        let file = destination.join("bin").join(name);
        fs::copy(binaries.join(name), &file).map_err(|e| e.to_string())?;
        files.push(json!({"path":format!("bin/{name}"),"sha256":digest(&file)?}));
    }
    for license in ["LICENSE-MIT", "LICENSE-APACHE"] {
        fs::copy(root.join(license), destination.join(license)).map_err(|e| e.to_string())?;
    }
    let start = if target.contains("windows") {
        "$env:WVST_TOKEN = [guid]::NewGuid().ToString('N')\n$env:WVST_HOST_WORKER = (Resolve-Path .\\bin\\wvst-host-worker.exe).Path\n.\\bin\\wvst-bridge-server.exe serve"
    } else {
        "export WVST_TOKEN=\"$(openssl rand -hex 24)\"\nexport WVST_HOST_WORKER=\"$PWD/bin/wvst-host-worker\"\n./bin/wvst-bridge-server serve"
    };
    let mut readme = format!(
        "# WVST {version}\n\nTarget: `{target}`. Commit: `{commit}`.\n\nThis is an unsigned developer preview. macOS is the first plugin runtime target;\nWindows/Linux archives are experimental and do not establish third-party plugin\ncompatibility. No Developer ID notarization or Authenticode signing is claimed.\n\n## Run\n\nExtract the entire archive. In a terminal, change to this directory and run:\n\n```\n{start}\n```\n\nKeep this terminal open. Enter this locally generated WVST_TOKEN in Studio's\nadvanced settings. The endpoint is ws://127.0.0.1:35876. Install a matching-CPU\nVST3 plugin separately. Native security policy may block unsigned downloads;\nuse a source build if you cannot run this preview under your local policy.\n\nThis portable package installs no service and modifies no system configuration.\nStop it with Ctrl+C. To update, stop the old runtime and replace the entire\nfolder so Bridge and worker remain the same version; keep the previous folder\nfor rollback. Preserve application state backups separately.\n\nDocs: https://wvst-docs.pages.dev/docs\n中文: https://wvst-docs.pages.dev/docs/zh\nReleases: https://github.com/backrunner/wvst/releases\n\nValidate the downloaded archive against SHA256SUMS before extracting. Checksums\ndetect corruption; they are not code signatures. See release-manifest.json for\nartifact hashes and source identity.\n"
    );
    if let Some(team) = team {
        readme = readme.replace(
            "This is an unsigned developer preview.",
            &format!("This developer preview uses Developer ID signed binaries (team {team})."),
        ).replace(
            "No Developer ID notarization or Authenticode signing is claimed.",
            "The release DMG must pass Apple notarization, ticket stapling and Gatekeeper.\nSigning does not certify third-party plugin compatibility.",
        ).replace(
            "Extract the entire archive.",
            "Open the DMG and copy the entire WVST folder to a writable local directory.\nEject the disk image after copying; do not run from its read-only volume.",
        ).replace(
            "Native security policy may block unsigned downloads;\nuse a source build if you cannot run this preview under your local policy.",
            "The worker permits third-party plugin libraries; the Bridge retains library validation.",
        );
    }
    fs::write(destination.join("README.md"), readme).map_err(|e| e.to_string())?;
    write_json(
        &destination.join("wvst-runtime.json"),
        &json!({
            "schemaVersion":1,"version":version.to_string(),"target":target,"commit":commit,
            "kind":"portable","signature":if team.is_some() { "developer-id" } else { "unsigned" },
            "teamId":team,"files":files,
            "protocol":{"major":wvst_core::CURRENT_PROTOCOL_VERSION.major,"minor":wvst_core::CURRENT_PROTOCOL_VERSION.minor},
            "audioFrameVersion":wvst_protocol::AUDIO_FRAME_VERSION
        }),
    )?;
    Ok(destination)
}
fn expected_names(version: &str) -> Vec<String> {
    let mut names: Vec<_> = TARGETS
        .iter()
        .map(|target| {
            let extension = if target.contains("windows") {
                "zip"
            } else if target.contains("apple") {
                "dmg"
            } else {
                "tar.gz"
            };
            format!("wvst-{version}-{target}.{extension}")
        })
        .collect();
    names.push(format!("wvst-web-{version}.tgz"));
    names.sort();
    names
}
/// Refuse incomplete or unexpected assets before creating a draft release inventory.
pub fn checksums(root: &Path, directory: &Path, commit: &str) -> Result<(), String> {
    let version = check(root, None)?.to_string();
    valid_commit(commit)?;
    let expected = expected_names(&version);
    let mut actual = Vec::new();
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "non-UTF8 artifact name")?;
        if name == "SHA256SUMS" || name == "release-manifest.json" {
            continue;
        }
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            return Err(format!("not a regular artifact: {name}"));
        }
        if entry.metadata().map_err(|e| e.to_string())?.len() == 0 {
            return Err(format!("empty artifact: {name}"));
        }
        actual.push(name);
    }
    actual.sort();
    if actual != expected {
        return Err(format!(
            "incomplete or unexpected release assets; expected {expected:?}, got {actual:?}"
        ));
    }
    let mut artifacts = Vec::new();
    let mut sums = Vec::new();
    for name in expected {
        let path = directory.join(&name);
        let hash = digest(&path)?;
        sums.push(format!("{hash}  {name}"));
        artifacts.push(json!({"name":name,"sha256":hash,"bytes":fs::metadata(path).map_err(|e|e.to_string())?.len()}));
    }
    let manifest = directory.join("release-manifest.json");
    write_json(
        &manifest,
        &json!({
            "schemaVersion":2,"version":version,"commit":commit,
            "signingPolicy":{"macos":"developer-id-notarized-dmg","windows":"unsigned","linux":"unsigned"},
            "artifacts":artifacts,"targets":TARGETS,
            "compatibility":"macOS-first; Windows/Linux experimental; no real-plugin certification implied"
        }),
    )?;
    sums.push(format!("{}  release-manifest.json", digest(&manifest)?));
    fs::write(
        directory.join("SHA256SUMS"),
        format!("{}\n", sums.join("\n")),
    )
    .map_err(|e| e.to_string())
}
