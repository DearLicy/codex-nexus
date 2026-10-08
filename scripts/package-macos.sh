#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

export PATH="${HOME}/.cargo/bin:${PATH}"
cargo build --release --manifest-path native/Cargo.toml

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' native/Cargo.toml | head -1)"
staging="$repo_root/native/target/package"
app="$staging/Codex Nexus.app"
rm -rf "$staging"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp native/target/release/codex-nexus-native "$app/Contents/MacOS/codex-nexus-native"
chmod 755 "$app/Contents/MacOS/codex-nexus-native"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDisplayName</key><string>Codex Nexus</string>
    <key>CFBundleExecutable</key><string>codex-nexus-native</string>
    <key>CFBundleIdentifier</key><string>com.dearlicy.codexnexus</string>
    <key>CFBundleName</key><string>Codex Nexus</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${version}</string>
    <key>CFBundleVersion</key><string>${version}</string>
    <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

if command -v codesign >/dev/null 2>&1 && [[ -n "${CODEX_NEXUS_SIGNING_IDENTITY:-}" ]]; then
  codesign --force --deep --options runtime --timestamp \
    --sign "$CODEX_NEXUS_SIGNING_IDENTITY" "$app"
fi

if command -v hdiutil >/dev/null 2>&1; then
  hdiutil create -volname "Codex Nexus" -srcfolder "$app" \
    -ov -format UDZO "$staging/Codex-Nexus-${version}-macOS.dmg" >/dev/null
fi

echo "Created $app"
[[ -f "$staging/Codex-Nexus-${version}-macOS.dmg" ]] && echo "Created $staging/Codex-Nexus-${version}-macOS.dmg"
