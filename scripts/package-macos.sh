#!/bin/sh
# Builds target/dist/Revisor.app and target/dist/Revisor-<version>.dmg (Apple Silicon).
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
hdiutil create -quiet -volname Revisor -srcfolder "$dist" -ov -format UDZO "target/Revisor-$version.dmg"
rm "$dist/Applications"
mv "target/Revisor-$version.dmg" "$dist/"
echo "Built $dist/Revisor-$version.dmg"
