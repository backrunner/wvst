# Version management and preview releases — 2026-09-07

## Audit and policy

Before this change Rust/SDK/example manifests said 0.1.0, docs said 0.0.0,
SDK/Studio handshakes contained literal product versions, and the repository had
no tags, CHANGELOG or release workflow. Wire/schema versions already existed
and remain independent of product versions.

Product versions now originate in Cargo.toml workspace.package.version. The
Rust wvst-release binary in wvst-packager manages manifests, both lockfiles,
generated SDK WVST_VERSION and dated changelog sections. Version checks run in
normal CI and tag validation. Initial untagged 0.1.0 values were not a public
release; the first prepared package version is 0.1.0-alpha.1.

The JSON editor validates JSON and replaces selected structural value ranges,
preserving formatting and external dependency versions. It intentionally avoids
serde_json/preserve_order: Cargo feature unification enlarges serde_json::Value
across runtime crates and triggers large-error Clippy regressions.

## Release outputs

Release Preview responds to v* tags, requires a prerelease SemVer and matching
notes/manifests, runs reusable full CI, then builds macOS arm64/x64, Linux x64,
Windows x64 and an SDK tarball. The final step requires the complete artifact
set, produces SHA256SUMS and release-manifest.json with source commit, and
creates a draft prerelease. Published releases are never overwritten.

Native archives are portable developer previews with matching --version output
from both binaries, bundled license texts, startup/update instructions and a
wvst-runtime.json identity file. Existing service installer templates bake in
build-user paths; the preview pipeline does not distribute those installers.
Preview downloads are unsigned; macOS is first, Linux/Windows experimental.
Stable tags are blocked until signing/notarization and real-plugin evidence gates
are integrated. The SDK remains private in the workspace; no registry publication
occurs. Its tarball is installed and checked in an isolated app by the workflow.

## Local validation

- 477 Rust tests and workspace Clippy passed, including eight new version/JSON/
  artifact boundary tests; Rust formatting passed.
- 27 SDK tests, SDK types, documentation check/build passed.
- Actionlint passed for CI and the release workflow.
- Native --version output and a portable macOS arm64 package were exercised.
- SDK tarball installed into a separate temporary app; product version and both
  worker entrypoints resolved correctly.
- Version drift/tag checks, release notes, exact artifact inventory, duplicate/
  regressing versions, invalid calendar dates and non-mutating rejection covered.
- Bilingual release guides and version badge are part of the docs; update the
  Pages deployment separately after release changes.

No claim of third-party plugin compatibility or platform signing is made by
these checks. Public release state is determined by GitHub Releases, not by
manifest version values or existence of a private draft.

## First remote packaging validation

The alpha.1 source commit passed main CI. Its tag workflow exposed Windows
CRLF conversion in generated SDK text: the exact byte comparison incorrectly
reported version drift. Normalize only CRLF for that comparison, retain actual
version drift rejection, add a regression test and check versions on macOS and
Windows in normal CI. The alpha.1 tag remains unchanged; no release was created.
The corrected candidate is alpha.2. A Linux test fixture also hit transient
ETXTBSY while spawning a temporary Python worker; rerunning the failed quality
job passed. No runtime retry or test suppression was introduced.
