//! Developer ID distribution for the portable runtime. No unsigned fallback.
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

use super::artifacts::{bundle_with_team, digest, write_json};
use super::check;

const WORKER_ENTITLEMENTS: &str = include_str!("worker-entitlements.plist");
const LIBRARY_VALIDATION: &str = "com.apple.security.cs.disable-library-validation";

fn macos_only() -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("macOS signing requires a macOS host".into());
    }
    Ok(())
}
fn required(name: &str) -> Result<String, String> {
    let value = env::var(name).map_err(|_| format!("missing {name}"))?;
    if value.trim().is_empty() || value.contains(['\n', '\r', '\0']) {
        return Err(format!("invalid or empty {name}"));
    }
    Ok(value)
}
fn run(command: &mut Command) -> Result<Output, String> {
    let output = command.output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "{} failed: {}",
            command.get_program().to_string_lossy(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output)
}

/// Apple ID's app-specific password goes through stdin, never command arguments.
/// notarytool validates the account before saving it into the runner's Keychain.
pub fn store_notary_credentials() -> Result<(), String> {
    macos_only()?;
    let profile = required("WVST_NOTARY_PROFILE")?;
    let account = required("APPLE_ID")?;
    let team = required("APPLE_TEAM_ID")?;
    let password = required("APPLE_PASSWORD")?;
    let mut child = Command::new("xcrun")
        .args([
            "notarytool",
            "store-credentials",
            &profile,
            "--apple-id",
            &account,
            "--team-id",
            &team,
        ])
        .env_remove("APPLE_PASSWORD")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start notarytool: {e}"))?;
    let write_result = writeln!(
        child.stdin.take().ok_or("missing credential stdin")?,
        "{password}"
    );
    let status = child
        .wait()
        .map_err(|_| "cannot wait for credential validation")?;
    if write_result.is_err() || !status.success() {
        return Err("Apple credential validation failed; check the app-specific password, team and account access (credential output suppressed)".into());
    }
    println!("Apple notarization credentials validated and stored in Keychain");
    Ok(())
}

fn verify_details(details: &str, identity: &str, team: &str, hardened: bool) -> Result<(), String> {
    if !identity.starts_with("Developer ID Application: ")
        || !identity.ends_with(&format!("({team})"))
        || !details
            .lines()
            .any(|s| s == format!("TeamIdentifier={team}"))
        || !details
            .lines()
            .any(|s| s == format!("Authority={identity}"))
        || !details.lines().any(|s| s.starts_with("Timestamp="))
        || (hardened
            && !details
                .lines()
                .any(|s| s.contains("flags=") && s.contains("runtime")))
    {
        return Err("signature must match Developer ID identity/team, secure timestamp and hardened runtime policy".into());
    }
    Ok(())
}
fn verify_signature(path: &Path, identity: &str, team: &str, hardened: bool) -> Result<(), String> {
    run(Command::new("codesign")
        .args(["--verify", "--strict", "--verbose=2"])
        .arg(path))?;
    let details = run(Command::new("codesign")
        .args(["-d", "--verbose=4"])
        .arg(path))?;
    verify_details(
        &String::from_utf8_lossy(&details.stderr),
        identity,
        team,
        hardened,
    )
}
fn verify_entitlement_value(value: &Value, worker: bool) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("entitlements must be a dictionary")?;
    let valid = if worker {
        object.len() == 1 && object.get(LIBRARY_VALIDATION) == Some(&Value::Bool(true))
    } else {
        object.is_empty()
    };
    if !valid {
        return Err(
            "unexpected runtime entitlements; only the worker may disable library validation"
                .into(),
        );
    }
    Ok(())
}
fn verify_entitlements(path: &Path, worker: bool) -> Result<(), String> {
    let output = run(Command::new("codesign")
        .args(["-d", "--entitlements", ":-"])
        .arg(path))?;
    if output.stdout.is_empty() {
        return verify_entitlement_value(&json!({}), worker);
    }
    let mut child = Command::new("plutil")
        .args(["-convert", "json", "-o", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("missing plist stdin")?
        .write_all(&output.stdout)
        .map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("cannot decode signed entitlements".into());
    }
    verify_entitlement_value(
        &serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?,
        worker,
    )
}
fn accepted_submission(output: &[u8]) -> Result<Value, String> {
    let value: Value =
        serde_json::from_slice(output).map_err(|_| "invalid notarization response")?;
    if value["status"] != "Accepted" || value["id"].as_str().is_none_or(str::is_empty) {
        return Err(format!(
            "Apple notarization was not Accepted (id: {}, status: {})",
            value["id"], value["status"]
        ));
    }
    Ok(value)
}

/// Verify signatures on extracted runtime binaries without submitting to Apple.
pub fn verify_macos_signatures(binaries: &Path) -> Result<(), String> {
    macos_only()?;
    let identity = required("APPLE_SIGNING_IDENTITY")?;
    let team = required("APPLE_TEAM_ID")?;
    for name in ["wvst-bridge-server", "wvst-host-worker"] {
        let path = binaries.join(name);
        verify_signature(&path, &identity, &team, true)?;
        verify_entitlements(&path, name == "wvst-host-worker")?;
    }
    Ok(())
}

/// Sign copies, hash the signed bytes, notarize a signed DMG, staple its ticket,
/// assess Gatekeeper and verify the exact files read back from that DMG.
pub fn bundle_macos(
    root: &Path,
    target: &str,
    binaries: &Path,
    out: &Path,
    commit: &str,
) -> Result<PathBuf, String> {
    macos_only()?;
    if !["aarch64-apple-darwin", "x86_64-apple-darwin"].contains(&target) {
        return Err("bundle-macos requires an Apple runtime target".into());
    }
    let version = check(root, None)?;
    let identity = required("APPLE_SIGNING_IDENTITY")?;
    let team = required("APPLE_TEAM_ID")?;
    let profile = required("WVST_NOTARY_PROFILE")?;
    if !identity.starts_with("Developer ID Application: ")
        || !identity.ends_with(&format!("({team})"))
    {
        return Err("expected a Developer ID Application identity matching APPLE_TEAM_ID".into());
    }
    let scratch = root.join(format!(".wvst-macos-{target}"));
    fs::create_dir(&scratch).map_err(|e| format!("use a clean staging directory: {e}"))?;
    let signed = scratch.join("signed");
    fs::create_dir(&signed).map_err(|e| e.to_string())?;
    let entitlements = scratch.join("worker-entitlements.plist");
    fs::write(&entitlements, WORKER_ENTITLEMENTS).map_err(|e| e.to_string())?;
    let arch = if target.starts_with("aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    for name in ["wvst-bridge-server", "wvst-host-worker"] {
        let path = signed.join(name);
        fs::copy(binaries.join(name), &path).map_err(|e| e.to_string())?;
        run(Command::new("lipo").arg(&path).args(["-verify_arch", arch]))?;
        let worker = name == "wvst-host-worker";
        let mut command = Command::new("codesign");
        command.args([
            "--force",
            "--timestamp",
            "--options",
            "runtime",
            "--sign",
            &identity,
        ]);
        if worker {
            command.arg("--entitlements").arg(&entitlements);
        }
        run(command.arg(&path))?;
        verify_signature(&path, &identity, &team, true)?;
        verify_entitlements(&path, worker)?;
    }
    let content = scratch.join("content");
    let package = bundle_with_team(root, target, &signed, &content, commit, Some(&team))?;
    let name = format!("wvst-{version}-{target}");
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let dmg = out.join(format!("{name}.dmg"));
    if dmg.exists() {
        return Err("refusing to replace an existing DMG".into());
    }
    run(Command::new("hdiutil")
        .args([
            "create",
            "-format",
            "UDZO",
            "-fs",
            "HFS+",
            "-volname",
            &format!("WVST {version}"),
            "-srcfolder",
        ])
        .arg(&content)
        .arg(&dmg))?;
    run(Command::new("codesign")
        .args(["--timestamp", "--sign", &identity])
        .arg(&dmg))?;
    verify_signature(&dmg, &identity, &team, false)?;
    let submission = run(Command::new("xcrun")
        .args(["notarytool", "submit"])
        .arg(&dmg)
        .args([
            "--keychain-profile",
            &profile,
            "--wait",
            "--timeout",
            "30m",
            "--output-format",
            "json",
        ]))?;
    let submission = accepted_submission(&submission.stdout)?;
    println!("Apple notarization Accepted: {}", submission["id"]);
    run(Command::new("xcrun").args(["stapler", "staple"]).arg(&dmg))?;
    run(Command::new("xcrun")
        .args(["stapler", "validate"])
        .arg(&dmg))?;
    verify_signature(&dmg, &identity, &team, false)?;
    run(Command::new("spctl")
        .args([
            "--assess",
            "--type",
            "open",
            "--context",
            "context:primary-signature",
            "--verbose=4",
        ])
        .arg(&dmg))?;
    verify_mounted_dmg(&dmg, &scratch, &package, &name, &identity, &team)?;
    write_json(
        &scratch.join("verification.json"),
        &json!({
            "schemaVersion":1,"version":version.to_string(),"commit":commit,"target":target,
            "teamId":team,"notarizationId":submission["id"],"status":"Accepted",
            "sha256":digest(&dmg)?,"stapler":true,"gatekeeper":true,"mountedContentsVerified":true
        }),
    )?;
    Ok(dmg)
}

fn verify_mounted_dmg(
    dmg: &Path,
    scratch: &Path,
    package: &Path,
    name: &str,
    identity: &str,
    team: &str,
) -> Result<(), String> {
    let mount = scratch.join("mount");
    fs::create_dir(&mount).map_err(|e| e.to_string())?;
    run(Command::new("hdiutil")
        .args(["attach", "-readonly", "-nobrowse", "-mountpoint"])
        .arg(&mount)
        .arg(dmg))?;
    let result = (|| {
        for file in [
            "bin/wvst-bridge-server",
            "bin/wvst-host-worker",
            "wvst-runtime.json",
            "README.md",
            "LICENSE-MIT",
            "LICENSE-APACHE",
        ] {
            let path = mount.join(name).join(file);
            if digest(&path)? != digest(&package.join(file))? {
                return Err(format!("DMG contents differ: {file}"));
            }
            if file.starts_with("bin/") {
                verify_signature(&path, identity, team, true)?;
                verify_entitlements(&path, file.ends_with("wvst-host-worker"))?;
            }
        }
        Ok(())
    })();
    // Always detach after verification, including a failed content check.
    let detached = run(Command::new("hdiutil").arg("detach").arg(&mount));
    result?;
    detached?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notarization_requires_explicit_acceptance_and_submission_identity() {
        assert!(accepted_submission(br#"{"status":"Accepted","id":"submission-id"}"#).is_ok());
        for value in [
            br#"{"status":"Invalid","id":"id"}"#.as_slice(),
            br#"{"status":"In Progress","id":"id"}"#,
            br#"{"status":"Accepted"}"#,
        ] {
            assert!(accepted_submission(value).is_err());
        }
    }
    #[test]
    fn bridge_cannot_receive_worker_or_debugging_entitlements() {
        let worker = json!({LIBRARY_VALIDATION:true});
        assert!(verify_entitlement_value(&json!({}), false).is_ok());
        assert!(verify_entitlement_value(&worker, true).is_ok());
        assert!(verify_entitlement_value(&worker, false).is_err());
        assert!(verify_entitlement_value(&json!({}), true).is_err());
        assert!(
            verify_entitlement_value(
                &json!({LIBRARY_VALIDATION:true,"com.apple.security.get-task-allow":true}),
                true
            )
            .is_err()
        );
    }
    #[test]
    fn rejects_wrong_team_adhoc_missing_timestamp_or_unhardened_signatures() {
        let identity = "Developer ID Application: Test (TEAM)";
        let details = format!(
            "Authority={identity}\nTeamIdentifier=TEAM\nTimestamp=Sep 7, 2026\nCodeDirectory flags=0x10000(runtime)\n"
        );
        assert!(verify_details(&details, identity, "TEAM", true).is_ok());
        for bad in [
            details.replace("TeamIdentifier=TEAM", "TeamIdentifier=OTHER"),
            details.replace(&format!("Authority={identity}"), "Signature=adhoc"),
            details.replace("Timestamp=", "Signed Time="),
            details.replace("runtime", "none"),
        ] {
            assert!(verify_details(&bad, identity, "TEAM", true).is_err());
        }
    }
}
