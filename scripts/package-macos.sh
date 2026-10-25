#!/bin/sh
# Builds target/dist/Revisor.app, Revisor-<version>.dmg, and the in-app update package
# Revisor-<version>-macos-arm64.tar.gz (Apple Silicon).
set -eu
cd "$(dirname "$0")/.."

signing_identity=${REVISOR_SIGNING_IDENTITY:-}
if [ -n "$signing_identity" ] && ! security find-identity -v -p codesigning | awk -v requested="$signing_identity" '
    /"Developer ID Application:/ && ($2 == requested || index($0, "\"" requested "\"") > 0) { found = 1 }
    END { exit !found }
'; then
    echo "Set REVISOR_SIGNING_IDENTITY to an installed Developer ID Application certificate name or SHA-1 hash." >&2
    exit 1
fi

version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/revisor-app/Cargo.toml)
dist=target/dist
app=$dist/Revisor.app

cargo build --release -p revisor-app

rm -rf "$dist"
mkdir -p "$app/Contents/MacOS"
cp target/release/revisor "$app/Contents/MacOS/Revisor"

iconset=target/Revisor.iconset
rm -rf "$iconset"
mkdir -p "$iconset" "$app/Contents/Resources"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" brand/icon-1024.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" brand/icon-1024.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Revisor.icns"
cp LICENSE.md "$app/Contents/Resources/"

min_os=$(vtool -show-build "$app/Contents/MacOS/Revisor" | awk '/minos/ {print $2; exit}')

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Revisor</string>
  <key>CFBundleDisplayName</key><string>Revisor</string>
  <key>CFBundleIdentifier</key><string>com.revisor.app</string>
  <key>CFBundleExecutable</key><string>Revisor</string>
  <key>CFBundleIconFile</key><string>Revisor</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>$min_os</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

if [ -n "$signing_identity" ]; then
    codesign --force --options runtime --timestamp --sign "$signing_identity" "$app"
else
    codesign --force --sign - "$app"
fi
codesign --verify --strict "$app"

ln -s /Applications "$dist/Applications"
# Same steps as the README: the app has no Apple notarization, so macOS blocks the first start.
cat > "$dist/If Revisor does not open.txt" <<'TXT'
Revisor has no Apple notarization, so macOS blocks the first start.
Do these steps one time:

1. Drag Revisor to Applications.
2. Open Revisor. macOS shows a warning. Click Done.
3. Open System Settings > Privacy & Security. Below Security, click
   Open Anyway. Enter your password.

Or run this command in Terminal:

    xattr -dr com.apple.quarantine /Applications/Revisor.app

Updates from inside Revisor do not show this warning again.
TXT
hdiutil create -quiet -volname Revisor -srcfolder "$dist" -ov -format UDZO "target/Revisor-$version.dmg"
rm "$dist/Applications" "$dist/If Revisor does not open.txt"
mv "target/Revisor-$version.dmg" "$dist/"
# COPYFILE_DISABLE: no AppleDouble ._ files; unpacked, they break the code signature.
COPYFILE_DISABLE=1 tar -czf "$dist/Revisor-$version-macos-arm64.tar.gz" -C "$dist" Revisor.app
echo "Built $dist/Revisor-$version.dmg and $dist/Revisor-$version-macos-arm64.tar.gz"
