#!/bin/zsh
# Builds dist/SnipReader-<version>-<arch>.dmg for a GitHub release: the app plus an
# Applications shortcut to drag it onto.
#
# With a "Developer ID Application" certificate in the keychain and a notarytool profile
# (default name "snipreader", set NOTARY_PROFILE to change it), the app and the disk image
# are signed with Developer ID, sent to Apple for notarization and stapled, so the
# download opens without Gatekeeper warnings. Create the profile once with:
#   xcrun notarytool store-credentials snipreader --apple-id <apple id> --team-id <team id>
# Without them the app is signed ad-hoc (no personal certificate details in a public file).
#
# Usage (from anywhere): ./packaging/make_dmg.sh
set -euo pipefail

ROOT="${0:A:h:h}"
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
ARCH="$(uname -m)"
DMG="$ROOT/dist/SnipReader-$VERSION-$ARCH.dmg"
PROFILE="${NOTARY_PROFILE:-snipreader}"

DEV_ID="$(security find-identity -v -p codesigning 2>/dev/null \
  | sed -n 's/.*) \([0-9A-F]\{40\}\) "Developer ID Application.*/\1/p' | head -1)"
NOTARIZE=0
if [[ -n "$DEV_ID" ]] && xcrun notarytool history --keychain-profile "$PROFILE" >/dev/null 2>&1; then
  NOTARIZE=1
  echo "==> Developer ID + notarization (profile \"$PROFILE\")"
else
  DEV_ID="-"
  echo "==> no Developer ID / notarytool profile: ad-hoc signature, not notarized"
fi

# Keep the builder's home folder out of the binary (Rust embeds source paths for errors).
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME=~ --remap-path-prefix=$ROOT=."
SIGN_IDENTITY="$DEV_ID" "$ROOT/packaging/build_app.sh"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -R "$ROOT/build.noindex/SnipReader.app" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

echo "==> dmg"
mkdir -p "$ROOT/dist"
rm -f "$DMG"
hdiutil create -quiet -volname "SnipReader $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG"
hdiutil verify -quiet "$DMG"

if [[ "$NOTARIZE" == 1 ]]; then
  codesign --force --timestamp --sign "$DEV_ID" "$DMG"
  echo "==> notarize (usually a few minutes)"
  xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait
  echo "==> staple"
  xcrun stapler staple "$DMG"
  xcrun stapler validate "$DMG"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
fi

echo "==> done: $DMG ($(du -h "$DMG" | cut -f1))"
shasum -a 256 "$DMG"
