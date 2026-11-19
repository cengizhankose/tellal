#!/usr/bin/env bash
# Builds, signs, notarizes and staples Tellal.app and a Tellal-<version>.dmg (arm64).
#
#   scripts/package-macos.sh <version> [out-dir]      # out-dir defaults to dist/ (git-ignored)
#
# Needs: a "Developer ID Application" identity in a keychain on the search list, and a notarytool
# keychain profile (`xcrun notarytool store-credentials tellal-notary ...`).
# Environment:
#   CARGO_TARGET_DIR              cargo target dir (default: target/dev, so a running
#                                 target/release/tellal is never replaced)
#   TELLAL_SIGN_IDENTITY          codesign identity (default: "Developer ID Application")
#   TELLAL_NOTARY_PROFILE         notarytool keychain profile (default: tellal-notary)
#   TELLAL_KEYCHAIN               keychain holding the identity and profile; unlocked first
#   TELLAL_KEYCHAIN_PASSWORD_FILE file with that keychain's password (else unlock prompts)
#
# Voice models are not bundled: Tellal downloads them on first run.
set -euo pipefail
# Bundle files must be world-readable whatever the caller's umask.
umask 022

# ONNX Runtime's prebuilt static library targets macOS 13.4; gpui (10.15.7) and Rust's arm64
# default (11.0) are lower, so 13.4 is the floor.
MIN_MACOS="13.4"
BUNDLE_ID="io.github.cengizhankose.tellal"

die() {
  echo "error: $*" >&2
  exit 1
}

[[ $# -ge 1 && $# -le 2 ]] || die "usage: $0 <version> [out-dir]"
version="$1"
[[ "$version" =~ ^([0-9]+\.[0-9]+\.[0-9]+)(-[0-9A-Za-z.-]+)?$ ]] ||
  die "version must look like 1.2.3 or 1.2.3-beta.1, got '$version'"
short_version="${BASH_REMATCH[1]}"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
packaging="$root/packaging/macos"
if [[ $# -eq 2 ]]; then
  out="$2"
else
  out="$root/dist"
  mkdir -p "$out"
  [[ -e "$out/.gitignore" ]] || echo '*' >"$out/.gitignore"
fi
mkdir -p "$out"
out="$(cd "$out" && pwd)"

identity="${TELLAL_SIGN_IDENTITY:-Developer ID Application}"
profile="${TELLAL_NOTARY_PROFILE:-tellal-notary}"
kc="${TELLAL_KEYCHAIN:-}"
if [[ -n "$kc" ]]; then
  if [[ -n "${TELLAL_KEYCHAIN_PASSWORD_FILE:-}" ]]; then
    security unlock-keychain -p "$(<"$TELLAL_KEYCHAIN_PASSWORD_FILE")" "$kc"
  else
    security unlock-keychain "$kc"
  fi
fi
# `${kc:+--keychain "$kc"}` below passes the keychain only when one was given (no empty arrays,
# which macOS's bash 3.2 rejects under `set -u`).

matches="$(security find-identity -v -p codesigning ${kc:+"$kc"} | grep -cF "\"$identity" || true)"
[[ "$matches" -eq 1 ]] ||
  die "need exactly one valid codesigning identity matching '$identity', found $matches (security find-identity -v -p codesigning)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "==> cargo build --release (macOS $MIN_MACOS+)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/dev}"
MACOSX_DEPLOYMENT_TARGET="$MIN_MACOS" cargo build --release --locked -p tellal --manifest-path "$root/Cargo.toml"
binary="$CARGO_TARGET_DIR/release/tellal"
non_system="$(otool -L "$binary" | tail -n +2 | awk '{print $1}' | grep -vE '^(/System/Library/|/usr/lib/)' || true)"
[[ -z "$non_system" ]] || die "binary links non-system libraries that would need bundling: $non_system"

echo "==> Tellal.app"
app="$work/Tellal.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
install -m 0755 "$binary" "$app/Contents/MacOS/tellal"
sed -e "s/@VERSION@/$version/g" -e "s/@SHORT_VERSION@/$short_version/g" -e "s/@MIN_MACOS@/$MIN_MACOS/g" \
  "$packaging/Info.plist.in" >"$app/Contents/Info.plist"
plutil -lint -s "$app/Contents/Info.plist"
"$packaging/make-icns.sh" "$root/docs/media/icon.svg" "$app/Contents/Resources/icon.icns"
install -m 0644 "$root/LICENSE" "$root/THIRD_PARTY_NOTICES.md" "$app/Contents/Resources/"

echo "==> codesign Tellal.app"
codesign --force --options runtime --timestamp --entitlements "$packaging/Tellal.entitlements" \
  --identifier "$BUNDLE_ID" --sign "$identity" ${kc:+--keychain "$kc"} "$app"
codesign --verify --deep --strict "$app"

# Submits $1 for notarization and waits; prints the submission id and fails unless Accepted.
notarize() {
  local file="$1" result id status
  # notarytool may exit non-zero for a rejected upload; the JSON status decides.
  result="$(xcrun notarytool submit "$file" --keychain-profile "$profile" ${kc:+--keychain "$kc"} \
    --wait --timeout 1h --output-format json)" || true
  id="$(plutil -extract id raw - <<<"$result")"
  status="$(plutil -extract status raw - <<<"$result")"
  echo "notarization $(basename "$file"): id $id, status $status"
  if [[ "$status" != "Accepted" ]]; then
    xcrun notarytool log "$id" --keychain-profile "$profile" ${kc:+--keychain "$kc"} >&2 || true
    die "notarization of $(basename "$file") ended with status $status"
  fi
}

echo "==> notarize Tellal.app"
ditto -c -k --keepParent "$app" "$work/Tellal.zip"
notarize "$work/Tellal.zip"
xcrun stapler staple -q "$app"

echo "==> Tellal-$version.dmg"
dmg="$out/Tellal-$version.dmg"
stage="$work/dmg"
mkdir "$stage"
ditto "$app" "$stage/Tellal.app"
ln -s /Applications "$stage/Applications"
rm -f "$dmg"
hdiutil create -quiet -volname "Tellal" -srcfolder "$stage" -fs HFS+ -format ULMO "$dmg"
codesign --force --timestamp --sign "$identity" ${kc:+--keychain "$kc"} "$dmg"

echo "==> notarize Tellal-$version.dmg"
notarize "$dmg"
xcrun stapler staple -q "$dmg"

echo "==> done"
echo "$dmg ($(du -h "$dmg" | cut -f1))"
shasum -a 256 "$dmg"
