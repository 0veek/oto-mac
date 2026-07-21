#!/usr/bin/env bash
# Install Oto.app into /Applications with a fresh signature so macOS TCC
# (Accessibility, Microphone, Input Monitoring) can bind to a stable identity.
#
# Dragging from target/release/bundle/macos into Applications often:
#   - invalidates the code signature
#   - adds com.apple.quarantine
#   - leaves stale Accessibility entries for the old build path
# so the app runs but never appears / cannot be enabled under Privacy settings.
#
# Usage:
#   ./scripts/install-app.sh
#   ./scripts/install-app.sh /path/to/Oto.app
#   ./scripts/install-app.sh --no-open
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="${ROOT}/src-tauri/target/release/bundle/macos/Oto.app"
DEST="/Applications/Oto.app"
OPEN_AFTER=1

for arg in "$@"; do
  case "$arg" in
    --no-open) OPEN_AFTER=0 ;;
    --help|-h)
      sed -n '2,20p' "$0"
      exit 0
      ;;
    *)
      if [[ -d "$arg" ]]; then
        SRC="$arg"
      else
        echo "error: unknown argument or missing app: $arg" >&2
        exit 1
      fi
      ;;
  esac
done

if [[ ! -d "$SRC" ]]; then
  echo "error: app not found: $SRC" >&2
  echo "Build first: npm run app:build" >&2
  exit 1
fi

echo "==> Source: $SRC"
echo "==> Destination: $DEST"

echo "==> Quitting any running Oto instances..."
# Kill by executable basename only (avoid self-matching pkill -f).
pkill -x oto 2>/dev/null || true
# Give the process a moment to release the bundle if it was launched from DEST.
sleep 0.6

if [[ -d "$DEST" ]]; then
  echo "==> Removing existing $DEST..."
  # Prefer moving to Trash-like temp so a locked app is less sticky.
  rm -rf "$DEST" || {
    echo "error: could not remove $DEST — quit Oto from the menu bar and retry." >&2
    exit 1
  }
fi

echo "==> Copying with ditto (preserves bundle structure)..."
ditto "$SRC" "$DEST"

echo "==> Clearing quarantine / Finder provenance xattrs..."
xattr -cr "$DEST" || true

echo "==> Re-signing at install location (required for Accessibility identity)..."
bash "$ROOT/scripts/sign-app.sh" "$DEST"

echo "==> Signature check..."
codesign --verify --deep --strict --verbose=2 "$DEST"
spctl --assess --type execute --verbose=4 "$DEST" 2>&1 || {
  echo "note: spctl assess may reject ad-hoc signatures; that is expected for local builds."
  echo "      Accessibility still works if you add /Applications/Oto.app with + and enable it."
}

echo
echo "Installed: $DEST"
echo
echo "Grant permissions (order matters):"
echo "  1. Launch Oto from /Applications (not the build folder)."
echo "  2. System Settings → Privacy & Security → Accessibility"
echo "     Unlock → + → choose /Applications/Oto.app → enable the toggle."
echo "  3. Same for Microphone (and Input Monitoring if prompted)."
echo "  4. Quit Oto from the menu bar and reopen it from /Applications."
echo
echo "If an old Oto/oto entry is listed, remove it first, then add /Applications/Oto.app."
echo

if [[ "$OPEN_AFTER" -eq 1 ]]; then
  echo "==> Opening /Applications/Oto.app..."
  open "$DEST"
  sleep 0.8
  # Nudge Accessibility trust dialog + open the settings pane.
  open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility" 2>/dev/null \
    || open "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Accessibility" 2>/dev/null \
    || true
fi
