---
title: Versions and Releases
description: Product and protocol versions, preview downloads, verification and upgrades, and the maintainer release workflow.
order: 10
---

# Versions and Releases

WVST uses one product version for the Rust workspace, Bridge, host worker, SDK, examples and docs package. The first public preview was `0.1.0-alpha.2`; `0.1.0-alpha.3` introduces the signed macOS release pipeline. Navigation shows the current docs build's version; Git tags retain historical source and documentation.

## Product and protocol versions

| Identifier | Purpose |
| --- | --- |
| `0.1.0-alpha.2` | SemVer product version; alpha/beta/rc identify preview stages. |
| `v0.1.0-alpha.2` | Git tag connecting a source commit to a GitHub Release. |
| Control, audio-frame and worker IPC versions | Independently evolving wire compatibility; not incremented for every product update. |
| State snapshot and diagnostic schemas | Compatibility of specific data structures, not SDK version numbers. |
| Svedocs version | Documentation framework dependency, independent of WVST. |

Fixes normally increment patch and features increment minor. Before 1.0, incompatible APIs increment minor and require migration notes. Previews progress through `alpha.1 → alpha.2 → beta.1 → rc.1`. Never reuse a published tag or replace published assets.

## Choose a download

Select an explicitly marked preview at [GitHub Releases](https://github.com/backrunner/wvst/releases). Drafts are visible only to maintainers and are not public releases. A completed release workflow produces the complete set for review.

| Target in filename | Intended machine |
| --- | --- |
| `aarch64-apple-darwin` | Apple Silicon macOS. |
| `x86_64-apple-darwin` | Intel macOS. |
| `x86_64-unknown-linux-gnu` | Experimental x64 Linux runtime, built on Ubuntu 22.04. |
| `x86_64-pc-windows-msvc` | Experimental x64 Windows runtime. |
| `wvst-web-VERSION.tgz` | Matching SDK; install with `npm install ./wvst-web-VERSION.tgz`. |

Native names include version and target, for example `wvst-0.1.0-alpha.3-aarch64-apple-darwin.dmg`. macOS is the first plugin runtime target. Windows/Linux downloads do not establish third-party compatibility. The published alpha.2 archives are unsigned. `0.1.0-alpha.3` macOS packages use Developer ID signed, notarized `.dmg` files; Windows/Linux remain unsigned. Refer to each release’s actual files and signing status.

## Verify and start

Download the selected archive, `SHA256SUMS` and `release-manifest.json`. Checksum tools report other platforms as missing if you have not downloaded every asset; compare the selected file's line. On macOS:

```sh
shasum -a 256 wvst-0.1.0-alpha.3-aarch64-apple-darwin.dmg
```

Use `sha256sum` on Linux or `Get-FileHash -Algorithm SHA256` on Windows. The digest must exactly match `SHA256SUMS`. The release manifest records source commit, version, file sizes and hashes. Checksums detect corruption; they are not code signatures.

For a signed macOS release, open the DMG and copy its entire WVST folder to a writable local directory, then eject the image. For tar/zip downloads, extract the entire directory. Follow the included README. `bin` holds the matching Bridge and worker; `wvst-runtime.json` identifies the bundle. Inspect versions without starting a service:

```sh
./bin/wvst-bridge-server --version
./bin/wvst-host-worker --version
```

Windows filenames include `.exe`. Portable archives install no service. Configure a token on startup and enter the same value in Studio's advanced settings. Install compatible plugins separately, as with source builds.

## Upgrade and roll back

Stop the old Bridge and instances, extract the new archive into a separate folder, and start the matching Bridge/worker pair. Keep the old directory for rollback; do not replace only one executable. Prefer the matching product version of the SDK and use handshake negotiation for protocol compatibility.

Your app owns plugin-state persistence. Back up snapshots and retain the original plugin version before upgrading. Read breaking changes in CHANGELOG; matching WVST versions cannot establish private-state compatibility between third-party plugin versions.

## Maintainers: prepare a version

`workspace.package.version` in root `Cargo.toml` is the source of product version. Do not independently edit each package.json.

1. Record user-facing changes, fixes, compatibility and migrations under `[Unreleased]` in root `CHANGELOG.md`.
2. Run from the repository root:

```sh
npm run release:check
npm run release:prepare -- 0.1.0-alpha.4 --date 2026-09-08
npm run release:check -- --tag v0.1.0-alpha.4
npm run release:notes -- 0.1.0-alpha.4
```

These are example values for a subsequent release; choose the actual version and date. Prepare updates Rust/npm manifests, both lockfiles and the generated SDK version, and archives Unreleased under dated release notes. All edits are parsed/validated before writing; failed writes attempt rollback.

Use `npm run release:sync` to repair workspace version drift without creating a release record. Normal bumps use prepare. The tool does not automatically commit or push tags; review the diff and complete project checks first.

## Maintainers: from tag to release

After pushing the version commit and passing CI, create an annotated tag:

```sh
git tag -a v0.1.0-alpha.4 -m 'WVST 0.1.0-alpha.4'
git push origin v0.1.0-alpha.4
```

`Release Preview` validates tag/version/notes, reuses full CI, and builds four native archives plus an SDK tarball. Only after every build succeeds does it generate SHA256SUMS, a source manifest and a draft GitHub Release. Reruns can refresh that draft; published releases cannot be overwritten.

Download/verify the assets and review platform limitations and migrations before publishing the draft as a prerelease. The workflow accepts preview tags only. Stable releases need platform signing/notarization and real-plugin/sustained-audio evidence for the source commit; successful compilation is not compatibility certification.

Registry publication is separate: this workflow runs neither `npm publish` nor `cargo publish`. Update the docs separately with `npm run docs:deploy`. See [Development](/docs/development) for evidence boundaries.

## Maintainers: macOS signing setup

The release workflow requires the following repository Secrets in **backrunner/wvst**, configured before pushing a new version tag. AIPass is a workflow reference; its repository Secrets cannot be read back or automatically shared.

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded P12 containing the selected Developer ID Application certificate and its private key. |
| `APPLE_CERTIFICATE_PASSWORD` | Password protecting that P12 export. |
| `APPLE_SIGNING_IDENTITY` | Full Developer ID Application identity, matching the certificate. |
| `APPLE_TEAM_ID` | Developer team ID matching the certificate. |
| `APPLE_ID` | Apple account with access to that team. |
| `APPLE_PASSWORD` | Apple app-specific password, separate from the account and P12 passwords. |

The local Developer ID identity has been tested with WVST binaries. All six repository Secrets are configured. The alpha.3 release workflow passed Apple ID authentication, notarization, stapling and Gatekeeper for both macOS architectures. Alpha.2 remains unsigned and unchanged.

The workflow imports the certificate into the runner Keychain. The Rust `notary-credentials` command sends the app-specific password to `notarytool` over stdin, validates the account, and stores a temporary profile. `bundle-macos` uses that profile, without a password in command arguments. No CloudKit provisioning profile or Tauri updater key is needed for WVST.

Bridge and worker both use hardened runtime and a secure timestamp. Only the worker receives `com.apple.security.cs.disable-library-validation`, because third-party VST libraries belong to other developers. The Bridge retains library validation. Neither binary gets debugging or JIT permissions; plugins requiring additional exceptions need separate compatibility testing.

The packager hashes binaries **after signing**, checks their versions and CPU architecture, signs a DMG, requires an explicit Apple `Accepted` result, staples/validates the ticket, and runs Gatekeeper assessment. It then mounts the DMG read-only and verifies the packaged binaries, metadata and licenses against staging. The workflow retains a `signing-TARGET` verification report, with submission ID, source commit and the final DMG hash. Missing credentials, rejected notarization and failed verification stop release creation.

Use an existing local `notarytool` Keychain profile for manual packaging:

```sh
# Set APPLE_SIGNING_IDENTITY, APPLE_TEAM_ID and WVST_NOTARY_PROFILE locally.
# Build both runtime binaries for the target first.
cargo run --locked -p wvst-packager --bin wvst-release -- bundle-macos \
  aarch64-apple-darwin target/aarch64-apple-darwin/release output/release FULL_COMMIT_SHA
```

The command requires a fresh `.wvst-macos-TARGET` staging directory and refuses to overwrite a DMG. Do not put credential values in source, command history or chat. To verify a downloaded DMG on macOS:

```sh
codesign --verify --strict --verbose=2 /path/to/wvst-VERSION-TARGET.dmg
xcrun stapler validate /path/to/wvst-VERSION-TARGET.dmg
spctl --assess --type open --context context:primary-signature --verbose=4 /path/to/wvst-VERSION-TARGET.dmg
```

The portable binaries themselves cannot carry a stapled ticket; distribute and retain the DMG. Signing and notarization are distribution checks, separate from real-plugin and audio-stability evidence. The release manifest’s `signingPolicy` records required distribution policy; SHA256SUMS alone does not verify Apple signatures.
