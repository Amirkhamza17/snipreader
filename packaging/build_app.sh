#!/bin/zsh
# Builds build.noindex/SnipReader.app: release binary + Info.plist + icon, signed.
# (".noindex" keeps this build copy out of Spotlight and Launchpad, so only the copy in
# /Applications shows up there.)
# Usage (from anywhere): ./packaging/build_app.sh
set -euo pipefail

ROOT="${0:A:h:h}"           # app/ (this script lives in app/packaging)
PKG="$ROOT/packaging"
APP="$ROOT/build.noindex/SnipReader.app"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
echo "==> SnipReader $VERSION"

echo "==> cargo build --release"
cargo build --release --manifest-path "$ROOT/Cargo.toml"

echo "==> icon"
swift "$PKG/make_icon.swift" "$WORK/icon_1024.png" >/dev/null
ICONSET="$WORK/AppIcon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s "$WORK/icon_1024.png" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d "$WORK/icon_1024.png" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$WORK/AppIcon.icns"

echo "==> bundle"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$ROOT/target/release/snipreader" "$APP/Contents/MacOS/snipreader"
cp "$WORK/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"
sed "s/__VERSION__/$VERSION/g" "$PKG/Info.plist" > "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"
plutil -lint "$APP/Contents/Info.plist" >/dev/null

# Sign with a real certificate when there is one: macOS then ties Screen Recording /
# Accessibility grants to the team + bundle id, so they survive rebuilds. With an
# ad-hoc signature the grant is tied to this exact build and is lost on every rebuild.
# Override with SIGN_IDENTITY="<name or SHA-1>" (or SIGN_IDENTITY=- for ad-hoc).
IDENTITY="${SIGN_IDENTITY:-$(security find-identity -v -p codesigning 2>/dev/null \
  | sed -n 's/.*) \([0-9A-F]\{40\}\) "Apple Development.*/\1/p' | head -1)}"
IDENTITY="${IDENTITY:--}"
if [[ "$IDENTITY" == "-" ]]; then
  echo "==> sign (ad-hoc: permissions will be asked again after each rebuild)"
else
  echo "==> sign ($(security find-identity -v -p codesigning | grep "$IDENTITY" | sed 's/.*"\(.*\)"/\1/' || echo "$IDENTITY"))"
fi
if security find-identity -v -p codesigning 2>/dev/null | grep "$IDENTITY" | grep -q "Developer ID Application"; then
  # For distribution: hardened runtime + secure timestamp (both required by notarization).
  codesign --force --options runtime --timestamp --entitlements "$PKG/SnipReader.entitlements" \
    --sign "$IDENTITY" --identifier com.sanzhar.snipreader "$APP"
else
  codesign --force --timestamp=none --sign "$IDENTITY" --identifier com.sanzhar.snipreader "$APP"
fi
codesign --verify --strict "$APP"

echo "==> done: $APP"
