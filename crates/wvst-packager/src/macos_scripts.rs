use crate::{MacosPackageConfig, shell_quote};

pub(crate) fn sign_notarize_script() -> String {
    r#"#!/bin/sh
set -eu

PACKAGE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
IDENTITY=${WVST_CODESIGN_IDENTITY:-}
ENTITLEMENTS=${WVST_CODESIGN_ENTITLEMENTS:-}
SKIP_NOTARIZATION=${WVST_SKIP_NOTARIZATION:-0}
ARCHIVE=${WVST_NOTARY_ARCHIVE:-$(dirname "$PACKAGE_DIR")/wvst-macos-notary.zip}
NOTARY_PROFILE=${WVST_NOTARY_PROFILE:-}
APPLE_ID=${WVST_APPLE_ID:-}
TEAM_ID=${WVST_TEAM_ID:-}
APP_PASSWORD=${WVST_APP_SPECIFIC_PASSWORD:-}
STRICT_SPCTL=${WVST_STRICT_SPCTL:-0}

if [ -z "$IDENTITY" ]; then
  printf 'WVST_CODESIGN_IDENTITY is required, for example Developer ID Application: Example Team (TEAMID).\n' >&2
  exit 2
fi

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf '%s is required for macOS signing/notarization.\n' "$1" >&2
    exit 2
  fi
}

require_tool codesign
require_tool file

sign_macho() {
  target=$1
  [ -f "$target" ] || return 0
  kind=$(file -b "$target" 2>/dev/null || true)
  case "$kind" in
    *Mach-O*) ;;
    *)
      printf 'Skipping non-Mach-O file: %s\n' "$target" >&2
      return 0
      ;;
  esac

  printf 'Signing %s\n' "$target" >&2
  if [ -n "$ENTITLEMENTS" ]; then
    codesign --force --timestamp --options runtime --entitlements "$ENTITLEMENTS" --sign "$IDENTITY" "$target"
  else
    codesign --force --timestamp --options runtime --sign "$IDENTITY" "$target"
  fi
  codesign --verify --strict --verbose=2 "$target"
}

find "$PACKAGE_DIR/bin" -type f -perm -111 | while IFS= read -r target; do
  sign_macho "$target"
done

if [ "$SKIP_NOTARIZATION" = "1" ]; then
  printf 'Skipping notarization because WVST_SKIP_NOTARIZATION=1.\n' >&2
  exit 0
fi

require_tool xcrun
require_tool ditto
require_tool spctl

rm -f "$ARCHIVE"
ditto -c -k --keepParent "$PACKAGE_DIR" "$ARCHIVE"

if [ -n "$NOTARY_PROFILE" ]; then
  xcrun notarytool submit "$ARCHIVE" --keychain-profile "$NOTARY_PROFILE" --wait
elif [ -n "$APPLE_ID" ] && [ -n "$TEAM_ID" ] && [ -n "$APP_PASSWORD" ]; then
  xcrun notarytool submit "$ARCHIVE" --apple-id "$APPLE_ID" --team-id "$TEAM_ID" --password "$APP_PASSWORD" --wait
else
  printf 'Set WVST_NOTARY_PROFILE or WVST_APPLE_ID/WVST_TEAM_ID/WVST_APP_SPECIFIC_PASSWORD for notarization.\n' >&2
  exit 2
fi

STAPLE_TARGETS=$(find "$PACKAGE_DIR" \( -name '*.app' -o -name '*.pkg' -o -name '*.dmg' \) -print)
if [ -z "$STAPLE_TARGETS" ]; then
  printf 'No .app, .pkg, or .dmg artifacts were found to staple; notarization was submitted for the zip archive.\n' >&2
else
  printf '%s\n' "$STAPLE_TARGETS" | while IFS= read -r artifact; do
    xcrun stapler staple "$artifact"
    xcrun stapler validate "$artifact"
  done
fi

for target in "$PACKAGE_DIR/bin/wvst-bridge-server" "$PACKAGE_DIR/bin/wvst-host-worker"; do
  if ! spctl --assess --type execute --verbose "$target"; then
    if [ "$STRICT_SPCTL" = "1" ]; then
      exit 1
    fi
    printf 'spctl assessment did not pass for %s; continuing because WVST_STRICT_SPCTL is not 1.\n' "$target" >&2
  fi
done

printf 'WVST macOS bundle signed. Notarization archive: %s\n' "$ARCHIVE"
"#
    .to_string()
}

pub(crate) fn verify_macos_script(config: &MacosPackageConfig) -> String {
    let plist_name = format!("{}.plist", config.launchd_label);
    let launchd_label = shell_quote(&config.launchd_label);

    format!(
        r#"#!/bin/sh
set -eu

PACKAGE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
LAUNCHD_LABEL={launchd_label}
PLIST="$PACKAGE_DIR/launchd/{plist_name}"
STRICT_CODESIGN=${{WVST_STRICT_CODESIGN_VERIFY:-0}}
REPORT_PATH=${{WVST_VERIFY_REPORT:-$PACKAGE_DIR/wvst-verify-report.json}}
CHECKS_RUN=0
CHECKS_FAILED=0
CHECKS_SKIPPED=0
WARNINGS=0
FAILED=0

json_escape() {{
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}}

pass_check() {{
  CHECKS_RUN=$((CHECKS_RUN + 1))
}}

fail() {{
  printf 'WVST macOS bundle verification failed: %s\n' "$1" >&2
  CHECKS_RUN=$((CHECKS_RUN + 1))
  CHECKS_FAILED=$((CHECKS_FAILED + 1))
  FAILED=1
}}

skip_check() {{
  printf 'WVST macOS bundle verification skipped: %s\n' "$1" >&2
  CHECKS_SKIPPED=$((CHECKS_SKIPPED + 1))
}}

warn_check() {{
  printf 'WVST macOS bundle verification warning: %s\n' "$1" >&2
  CHECKS_RUN=$((CHECKS_RUN + 1))
  WARNINGS=$((WARNINGS + 1))
}}

write_report() {{
  status=passed
  [ "$FAILED" -ne 0 ] && status=failed
  strict_codesign=false
  [ "$STRICT_CODESIGN" = "1" ] && strict_codesign=true
  report_dir=$(dirname "$REPORT_PATH")
  mkdir -p "$report_dir"
  package_dir_json=$(json_escape "$PACKAGE_DIR")
  cat > "$REPORT_PATH" <<EOF
{{
  "schemaVersion": 1,
  "platform": "macos",
  "packageDir": "$package_dir_json",
  "status": "$status",
  "strictSignature": $strict_codesign,
  "checksRun": $CHECKS_RUN,
  "checksFailed": $CHECKS_FAILED,
  "checksSkipped": $CHECKS_SKIPPED,
  "warnings": $WARNINGS,
  "checks": ["bundle-files", "executable-permissions", "env-template", "launchd-plist", "plist-lint", "codesign", "gatekeeper-assessment", "bridge-diagnose"]
}}
EOF
}}

trap write_report EXIT

require_file() {{
  if [ -f "$1" ]; then
    pass_check
  else
    fail "missing file $1"
  fi
}}

require_executable() {{
  if [ -f "$1" ]; then
    pass_check
    if [ -x "$1" ]; then
      pass_check
    else
      fail "not executable $1"
    fi
  else
    fail "missing file $1"
  fi
}}

require_text() {{
  file=$1
  pattern=$2
  require_file "$file"
  if ! grep -F "$pattern" "$file" >/dev/null 2>&1; then
    fail "missing text '$pattern' in $file"
  else
    pass_check
  fi
}}

verify_signature() {{
  target=$1
  if command -v file >/dev/null 2>&1; then
    kind=$(file -b "$target" 2>/dev/null || true)
    case "$kind" in
      *Mach-O*) ;;
      *)
        skip_check "signature check not applicable for non-Mach-O file $target"
        return 0
        ;;
    esac
  fi

  if ! command -v codesign >/dev/null 2>&1; then
    skip_check "codesign not found for $target"
    return 0
  fi

  if codesign --verify --strict --verbose=2 "$target" >/dev/null 2>&1; then
    codesign -dv "$target" >&2 2>/dev/null || true
    pass_check
    return 0
  fi

  if [ "$STRICT_CODESIGN" = "1" ]; then
    fail "codesign verification failed for $target"
  else
    warn_check "codesign verification did not pass for $target; set WVST_STRICT_CODESIGN_VERIFY=1 to fail unsigned dev bundles"
  fi
}}

require_executable "$PACKAGE_DIR/bin/wvst-bridge-server"
require_executable "$PACKAGE_DIR/bin/wvst-host-worker"
require_executable "$PACKAGE_DIR/bin/wvst-bridge-launcher"
require_executable "$PACKAGE_DIR/scripts/install-macos.sh"
require_executable "$PACKAGE_DIR/scripts/uninstall-macos.sh"
require_executable "$PACKAGE_DIR/scripts/diagnose-macos.sh"
require_executable "$PACKAGE_DIR/scripts/rotate-logs-macos.sh"
require_executable "$PACKAGE_DIR/scripts/sign-notarize-macos.sh"
require_file "$PACKAGE_DIR/config/wvst.env.example"
require_file "$PLIST"
require_file "$PACKAGE_DIR/README.md"

require_text "$PACKAGE_DIR/config/wvst.env.example" "WVST_BIND_ADDR="
require_text "$PACKAGE_DIR/config/wvst.env.example" "WVST_HOST_WORKER="
require_text "$PACKAGE_DIR/scripts/install-macos.sh" "launchctl bootstrap"
require_text "$PACKAGE_DIR/scripts/sign-notarize-macos.sh" "xcrun notarytool submit"
require_text "$PACKAGE_DIR/scripts/diagnose-macos.sh" "wvst-bridge-server\" diagnose"
require_text "$PLIST" "$LAUNCHD_LABEL"

if command -v plutil >/dev/null 2>&1; then
  if plutil -lint "$PLIST" >/dev/null; then
    pass_check
  else
    fail "LaunchAgent plist is invalid"
  fi
else
  skip_check "plutil not found; skipping plist lint"
fi

verify_signature "$PACKAGE_DIR/bin/wvst-bridge-server"
verify_signature "$PACKAGE_DIR/bin/wvst-host-worker"

if command -v spctl >/dev/null 2>&1; then
  for target in "$PACKAGE_DIR/bin/wvst-bridge-server" "$PACKAGE_DIR/bin/wvst-host-worker"; do
    if spctl --assess --type execute --verbose "$target" >/dev/null 2>&1; then
      pass_check
    else
      if [ "$STRICT_CODESIGN" = "1" ]; then
        fail "spctl assessment failed for $target"
      else
        warn_check "spctl assessment did not pass for $target; set WVST_STRICT_CODESIGN_VERIFY=1 after Developer ID signing/notarization"
      fi
    fi
  done
else
  skip_check "spctl not found; skipping Gatekeeper assessment"
fi

if [ -x "$PACKAGE_DIR/bin/wvst-bridge-server" ]; then
  if WVST_HOST_WORKER="$PACKAGE_DIR/bin/wvst-host-worker" "$PACKAGE_DIR/bin/wvst-bridge-server" diagnose >/dev/null; then
    pass_check
  else
    fail "wvst-bridge-server diagnose failed"
  fi
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

printf 'WVST macOS bundle verification passed: %s\n' "$PACKAGE_DIR"
"#,
        launchd_label = launchd_label,
        plist_name = plist_name,
    )
}
