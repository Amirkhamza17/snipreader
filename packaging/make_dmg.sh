#!/bin/zsh
# Builds dist/SnipReader-<version>-<arch>.dmg for a GitHub release: the app (signed ad-hoc,
# so no personal certificate details end up in a public download) plus an Applications
# shortcut to drag it onto.
# Usage (from anywhere): ./packaging/make_dmg.sh
set -euo pipefail

ROOT="${0:A:h:h}"
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
ARCH="$(uname -m)"
DMG="$ROOT/dist/SnipReader-$VERSION-$ARCH.dmg"

# Keep the builder's home folder out of the binary (Rust embeds source paths for errors).
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME=~ --remap-path-prefix=$ROOT=."
SIGN_IDENTITY=- "$ROOT/packaging/build_app.sh"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -R "$ROOT/dist/SnipReader.app" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

echo "==> dmg"
rm -f "$DMG"
hdiutil create -quiet -volname "SnipReader $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG"
hdiutil verify -quiet "$DMG"
echo "==> done: $DMG ($(du -h "$DMG" | cut -f1))"
shasum -a 256 "$DMG"
