# Changelog

All notable product changes are recorded here. Product versions follow SemVer;
protocol and state-schema versions evolve independently. This history starts
with the first packaged preview; earlier 0.1.0 manifest values were untagged
work-in-progress versions, not published releases.

## [Unreleased]

## [0.1.0-alpha.3] - 2026-09-07

### Added

- macOS release signing with Developer ID, hardened runtime, Apple notarization,
  stapled DMG tickets, Gatekeeper assessment and mounted-content verification.
- Worker-only third-party library entitlement; Bridge keeps library validation.
- Required Apple signing/notarization credentials are checked before release
  builds. The published alpha.2 archives remain unsigned and unchanged.

## [0.1.0-alpha.2] - 2026-09-07

### Added

- First distributable developer preview of the Bridge, isolated VST3 worker,
  WebAudio SDK and bilingual Live Studio documented below in alpha.1.

### Fixed

- Accept Windows CRLF checkouts when checking the generated SDK version,
  while continuing to reject actual version drift. Check product versions on
  macOS and Windows in normal CI before attempting release packaging.

### Release history

- The alpha.1 tag is retained for traceability, but its Windows packaging gate
  failed and no release was published. This version replaces that attempt.
- Preview limitations listed below still apply: unsigned native binaries,
  macOS-first plugin support, and experimental Windows/Linux builds.

## [0.1.0-alpha.1] - 2026-09-07

### Added

- Rust Bridge and isolated VST3 host worker runtime, with loopback authorization,
  plugin discovery, instance lifecycle, parameters, state and MIDI primitives.
- WebAudio SDK using AudioWorklet, DedicatedWorker and shared buffers, with
  diagnostics and explicit low-latency prerequisites.
- Bilingual documentation, custom WVST branding and an interactive Live Studio
  with local audio, generated synth loop, waveform, stereo effect rack and meters.
- Unified product-version tooling, version drift checks, release notes, portable
  runtime archives, SDK tarball, SHA-256 checksums and source-commit manifests.

### Fixed

- Closed WebSocket requests, failed binary-send queue handling, audio worker
  authorization, partial instance cleanup and media-source reuse on reconnect.
- Documentation builds from clean checkouts and a Windows-only compilation warning.

### Preview limits

- macOS is the first native plugin target. Windows/Linux runtime archives are
  experimental; build and protocol tests do not establish real-plugin compatibility.
- Native artifacts are unsigned developer previews without Developer ID
  notarization or Authenticode. No third-party plugins are bundled.
- SDK is distributed as a GitHub Release tarball; npm registry publication is
  not enabled. Native plugin editors, project export and zero-latency guarantees
  are outside this preview's scope.
