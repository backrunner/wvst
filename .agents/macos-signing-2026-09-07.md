# macOS signing and Apple ID notarization

Reference: ../aipass/.github/workflows/release.yml and the macos-signing-setup
skill. User explicitly selected Apple ID + app-specific password notarization.
WVST needs neither CloudKit provisioning nor AIPass's Tauri updater key.

## Implementation

Release Preview now requires six Apple repository Secrets before builds. macOS
jobs import the selected Developer ID P12 using apple-actions/import-codesign-certs,
validate Apple ID credentials with the Rust notary-credentials command, and
store a runner-only notarytool Keychain profile. APPLE_PASSWORD is delivered
through stdin, omitted from child environment/argv and suppressed in credential
validation output. Public signing identity/team are checked against signatures.

The Rust bundle-macos command signs copied Bridge/worker binaries with hardened
runtime and timestamp. Only the worker gets disable-library-validation to load
third-party VST libraries. No debugging/JIT exceptions are granted. Version and
architecture checks precede packaging; runtime metadata hashes signed bytes.
DMGs are signed, submitted to Apple with explicit Accepted status required,
stapled, checked by Gatekeeper, then mounted read-only to verify actual contents.
Verification reports retain submission ID, source commit and final DMG hash as
90-day Actions artifacts. Release inventory now expects two macOS DMGs, Linux
and Windows archives, and SDK tarball. Its schema 2 signingPolicy describes
required policy rather than treating checksums as cryptographic signing proof.

## Validation and current limits

- Packager 24 tests passed, including acceptance status, strict entitlement and
  signature identity/timestamp/runtime policy boundaries.
- Workspace Clippy, formatting, actionlint and bilingual docs check/build passed.
- The exact local Developer ID identity signed copies of both alpha.2 binaries;
  both executed --version and passed the Rust signature/entitlement verifier.
- A signed DMG was built, mounted, and its binaries, licenses, README, runtime
  identity and signed-byte hashes verified against staging.
- Missing notarytool credentials correctly rejected completion. No notarization,
  stapled ticket or Gatekeeper acceptance is claimed by these local tests.
- Published alpha.2 artifacts and tags are unchanged. Signing changes are under
  Unreleased until a new version is prepared after credential setup.

Repository Secrets and live release state must be checked on GitHub. Never copy
AIPass secrets through workflow output or expose passwords/P12/private keys in
logs. User supplies APPLE_ID and APPLE_PASSWORD directly in WVST Actions Secrets;
local Developer ID export can configure the certificate pair, identity and team.

## Credential setup verified

The exact Developer ID Application identity for team PB8H83VL3Z was exported
through the macOS Security API, with a fresh random P12 password. OpenSSL
verified the certificate type/name/team, at least 30 days remaining validity
and matching private/public keys before upload. APPLE_CERTIFICATE,
APPLE_CERTIFICATE_PASSWORD, APPLE_SIGNING_IDENTITY and APPLE_TEAM_ID were
configured in backrunner/wvst using stdin. P12 and private key stayed in memory;
the temporary helper was removed. APPLE_ID and APPLE_PASSWORD are pending user
configuration directly in the target repository's Actions Secrets.

## Alpha.3 remote release verification

User configured APPLE_ID and APPLE_PASSWORD on 2026-09-07. All six Secrets
are present; both macOS runners validated Apple ID credentials successfully.
Release Preview run 34140196133 passed all quality/build/signing jobs at source
5122476bf7026ab9e2856059ad0fb40becf383ba, tag v0.1.0-alpha.3.

- aarch64-apple-darwin: Apple Accepted, submission 640f9400-fb5a-43ff-adb9-98ce47095e0b; final DMG SHA256 b4364762e9d051c4adb91f30faf15f4f202dcdcb5174fc731d034c70bc6597c2.
- x86_64-apple-darwin: Apple Accepted, submission f2bdf9f3-db59-4ed8-809b-ea6ed1a05ff4; final DMG SHA256 60cdb6c9cd3146131b0b2501123e6831b169355fe63b603660a14522b52a58b8.

Downloaded Release draft assets independently verified: all archive and manifest
hashes match, source and product identity match the tag; both DMGs pass codesign,
stapler and Gatekeeper locally (Notarized Developer ID). Both DMGs mounted
read-only and verified binary hashes/entitlements/team; ARM programs executed
--version successfully. Windows/Linux and SDK archives also verified.
Windows/Linux remain unsigned and experimental. Alpha.2 assets are unchanged.
