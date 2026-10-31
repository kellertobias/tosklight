#!/usr/bin/env bash
# Import release credentials into an isolated, disposable CI keychain.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/tools/artifact-paths.sh"
light_init_artifact_paths "$ROOT"

if [[ "${1:-}" == cleanup ]]; then
  if [[ -n "${LIGHT_MACOS_KEYCHAIN:-}" ]]; then
    security delete-keychain "$LIGHT_MACOS_KEYCHAIN"
    rm -rf -- "$(dirname "$LIGHT_MACOS_KEYCHAIN")"
  fi
  exit 0
fi
[[ "${1:-}" == setup ]] || { echo 'usage: macos-signing-credentials.sh setup|cleanup' >&2; exit 2; }
[[ "$(uname -s)" == Darwin ]] || { echo 'error: macOS is required' >&2; exit 1; }
: "${GITHUB_ENV:?This setup writes signing configuration to GITHUB_ENV}"

names=(APPLE_CERTIFICATE_BASE64 APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY APPLE_ID APPLE_TEAM_ID APPLE_APP_SPECIFIC_PASSWORD)
present=0
for name in "${names[@]}"; do
  if [[ -n "${!name:-}" ]]; then present=$((present + 1)); fi
done
if [[ "$present" == 0 ]]; then
  echo 'LIGHT_MACOS_SIGNING_MODE=adhoc' >> "$GITHUB_ENV"
  echo 'Apple credentials absent: building ad-hoc signed testing downloads.'
  exit 0
fi
for name in "${names[@]}"; do
  [[ -n "${!name:-}" ]] || { echo "error: incomplete Apple release credentials: $name is missing" >&2; exit 1; }
done
[[ "$APPLE_SIGNING_IDENTITY" == 'Developer ID Application: '* ]] || {
  echo 'error: APPLE_SIGNING_IDENTITY must name a Developer ID Application certificate' >&2; exit 1;
}
[[ "$APPLE_SIGNING_IDENTITY" != *$'\n'* && "$APPLE_TEAM_ID" != *$'\n'* ]] || exit 1
mkdir -p "$LIGHT_TMP_DIR"
stage="$(mktemp -d "$LIGHT_TMP_DIR/apple-signing.XXXXXX")"
keychain="$stage/release.keychain-db"
success=0
trap 'rm -f -- "$stage/certificate.p12"; if [[ "$success" == 0 ]]; then security delete-keychain "$keychain" >/dev/null 2>&1 || true; rm -rf -- "$stage"; fi' EXIT
umask 077
printf '%s' "$APPLE_CERTIFICATE_BASE64" | base64 --decode > "$stage/certificate.p12"
password="$(openssl rand -hex 32)"
security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
security import "$stage/certificate.p12" -k "$keychain" -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$password" "$keychain" >/dev/null
security find-identity -v -p codesigning "$keychain" | grep -F -- "\"$APPLE_SIGNING_IDENTITY\"" >/dev/null || {
  echo 'error: the imported certificate does not match APPLE_SIGNING_IDENTITY' >&2; exit 1;
}
{
  echo 'LIGHT_MACOS_SIGNING_MODE=developer-id'
  printf 'LIGHT_MACOS_SIGNING_IDENTITY=%s\n' "$APPLE_SIGNING_IDENTITY"
  printf 'LIGHT_MACOS_KEYCHAIN=%s\n' "$keychain"
} >> "$GITHUB_ENV"
success=1
echo 'Developer ID certificate imported; release downloads will require notarization.'
