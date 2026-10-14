#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
profile="${1:-release}"
case "$profile" in
  debug) cargo build --locked ;;
  release) cargo build --locked --release ;;
  *) printf 'Usage: %s [debug|release]\n' "$0" >&2; exit 1 ;;
esac

bundle="target/$profile/AgentDesk.app"
mkdir -p "$bundle/Contents/MacOS"
cp "target/$profile/agentdesk" "$bundle/Contents/MacOS/agentdesk"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
  <key>CFBundleDisplayName</key><string>AgentDesk</string>
  <key>CFBundleExecutable</key><string>agentdesk</string>
  <key>CFBundleIdentifier</key><string>io.github.hezhangjian.agentdesk</string>
  <key>CFBundleName</key><string>AgentDesk</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.0.1</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
printf '%s\n' "$(pwd)/$bundle"
