#!/usr/bin/env bash
# Ad-hoc codesign Oto.app so macOS Accessibility / TCC can list it as a real app.
#
# Always re-sign AFTER moving/copying the app (e.g. into /Applications). A Finder
# drag often breaks the signature or adds quarantine, which is why the app runs
# but never shows up under Accessibility / Microphone.
#
# Usage:
#   ./scripts/sign-app.sh
#   ./scripts/sign-app.sh /Applications/Oto.app
#   npm run app:sign -- /Applications/Oto.app
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="${1:-$ROOT/src-tauri/target/release/bundle/macos/Oto.app}"
ENTITLEMENTS="$ROOT/src-tauri/Entitlements.plist"
BUNDLE_ID="dev.oto.mac"

if [[ ! -d "$APP" ]]; then
  echo "error: app not found: $APP" >&2
  echo "Build first: npm run app:build" >&2
  exit 1
fi

# Resolve to a stable absolute path (helps TCC identity + error messages).
APP="$(cd "$APP" && pwd)"

echo "Clearing quarantine / Finder provenance attributes on $APP ..."
xattr -cr "$APP" || true

# Sign nested Mach-Os first, then the bundle (more reliable than --deep alone
# on some Xcode/CLT combinations).
echo "Ad-hoc codesigning $APP as ${BUNDLE_ID}..."
if [[ -d "$APP/Contents/MacOS" ]]; then
  while IFS= read -r -d '' bin; do
    if file "$bin" | grep -q 'Mach-O'; then
      codesign --force --sign - \
        --identifier "${BUNDLE_ID}" \
        --entitlements "$ENTITLEMENTS" \
        --options runtime \
        "$bin" 2>/dev/null || true
    fi
  done < <(find "$APP/Contents/MacOS" -type f -print0 2>/dev/null)
fi

codesign --force --deep --sign - \
  --identifier "${BUNDLE_ID}" \
  --entitlements "$ENTITLEMENTS" \
  --options runtime \
  "$APP"

echo "Verifying..."
codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv --verbose=4 "$APP" 2>&1 | head -20

echo
echo "Done. Signed: $APP"
echo
if [[ "$APP" == /Applications/Oto.app ]]; then
  echo "This is the install location. Next:"
  echo "  1. open /Applications/Oto.app"
  echo "  2. System Settings → Privacy & Security → Accessibility"
  echo "  3. Remove any stale Oto/oto rows, then + → /Applications/Oto.app → enable"
  echo "  4. Quit and reopen Oto from /Applications"
else
  echo "For a stable Accessibility identity, install (do not drag-copy):"
  echo "  npm run app:install"
  echo "That copies into /Applications, re-signs there, and launches."
fi
