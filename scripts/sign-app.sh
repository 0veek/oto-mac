#!/usr/bin/env bash
# Ad-hoc codesign Oto.app so macOS Accessibility / TCC can list it as a real app.
# Usage: ./scripts/sign-app.sh [path/to/Oto.app]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="${1:-$ROOT/src-tauri/target/release/bundle/macos/Oto.app}"
ENTITLEMENTS="$ROOT/src-tauri/Entitlements.plist"
BUNDLE_ID="dev.oto.mac"

if [[ ! -d "$APP" ]]; then
  echo "error: app not found: $APP" >&2
  echo "Build first: npm run tauri build -- --bundles app" >&2
  exit 1
fi

echo "Clearing quarantine attributes..."
xattr -cr "$APP" || true

echo "Ad-hoc codesigning $APP as ${BUNDLE_ID}..."
codesign --force --deep --sign - \
  --identifier "${BUNDLE_ID}" \
  --entitlements "$ENTITLEMENTS" \
  --options runtime \
  "$APP"

echo "Verifying..."
codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv --verbose=4 "$APP" 2>&1 | head -20

echo
echo "Done. Next:"
echo "  1. Open $APP (or copy it to /Applications)"
echo "  2. System Settings -> Privacy & Security -> Accessibility"
echo "  3. Unlock -> click + -> select Oto.app -> enable the toggle"
echo "  4. Quit and reopen Oto"
