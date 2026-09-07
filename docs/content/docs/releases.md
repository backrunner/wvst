---
title: Versions and Releases
description: Product and protocol versions, preview downloads, verification and upgrades, and the maintainer release workflow.
order: 10
---

# Versions and Releases

WVST uses one product version for the Rust workspace, Bridge, host worker, SDK, examples and docs package. The first packaged version is `0.1.0-alpha.1`. Navigation shows the current docs build's version; Git tags retain historical source and documentation.

## Product and protocol versions

| Identifier | Purpose |
| --- | --- |
| `0.1.0-alpha.1` | SemVer product version; alpha/beta/rc identify preview stages. |
| `v0.1.0-alpha.1` | Git tag connecting a source commit to a GitHub Release. |
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

Native names include version and target, for example `wvst-0.1.0-alpha.1-aarch64-apple-darwin.tar.gz`. macOS is the first plugin runtime target. Windows/Linux downloads do not establish third-party compatibility. Previews have no Developer ID notarization or Authenticode signature, so OS policy may block execution; use a source build where unsigned previews are unsuitable.

## Verify and start

Download the selected archive, `SHA256SUMS` and `release-manifest.json`. Checksum tools report other platforms as missing if you have not downloaded every asset; compare the selected file's line. On macOS:

```sh
shasum -a 256 wvst-0.1.0-alpha.1-aarch64-apple-darwin.tar.gz
```

Use `sha256sum` on Linux or `Get-FileHash -Algorithm SHA256` on Windows. The digest must exactly match `SHA256SUMS`. The release manifest records source commit, version, file sizes and hashes. Checksums detect corruption; they are not code signatures.

Extract the entire directory and follow its README. `bin` holds the matching Bridge and worker; `wvst-runtime.json` identifies the bundle. Inspect versions without starting a service:

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
npm run release:prepare -- 0.1.0-alpha.2 --date 2026-09-08
npm run release:check -- --tag v0.1.0-alpha.2
npm run release:notes -- 0.1.0-alpha.2
```

These are example values for a subsequent release; choose the actual version and date. Prepare updates Rust/npm manifests, both lockfiles and the generated SDK version, and archives Unreleased under dated release notes. All edits are parsed/validated before writing; failed writes attempt rollback.

Use `npm run release:sync` to repair workspace version drift without creating a release record. Normal bumps use prepare. The tool does not automatically commit or push tags; review the diff and complete project checks first.

## Maintainers: from tag to release

After pushing the version commit and passing CI, create an annotated tag:

```sh
git tag -a v0.1.0-alpha.2 -m 'WVST 0.1.0-alpha.2'
git push origin v0.1.0-alpha.2
```

`Release Preview` validates tag/version/notes, reuses full CI, and builds four native archives plus an SDK tarball. Only after every build succeeds does it generate SHA256SUMS, a source manifest and a draft GitHub Release. Reruns can refresh that draft; published releases cannot be overwritten.

Download/verify the assets and review platform limitations and migrations before publishing the draft as a prerelease. The workflow accepts preview tags only. Stable releases need platform signing/notarization and real-plugin/sustained-audio evidence for the source commit; successful compilation is not compatibility certification.

Registry publication is separate: this workflow runs neither `npm publish` nor `cargo publish`. Update the docs separately with `npm run docs:deploy`. See [Development](/docs/development) for evidence boundaries.
