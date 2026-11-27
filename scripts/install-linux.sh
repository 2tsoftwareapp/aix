#!/usr/bin/env bash
# User-local Linux install for AIX: binary, icons, and desktop entry.
# No sudo needed — everything lands in ~/.local.
#
# Usage:
#   scripts/install-linux.sh [path-to-binary]
#
# Binary resolution order:
#   1. explicit argument
#   2. $CARGO_TARGET_DIR/release/aix-desktop
#   3. /data/cargo-target/release/aix-desktop (this repo's CI target dir)
#   4. apps/desktop/src-tauri/target/release/aix-desktop (cargo default)
set -euo pipefail

APP_NAME="AIX"
APP_ID="aix-desktop"
COMMENT="speckit-native desktop AI agent"

BINARY="${1:-}"
if [[ -z "$BINARY" ]]; then
  for candidate in \
    "${CARGO_TARGET_DIR:-/nonexistent}/release/aix-desktop" \
    /data/cargo-target/release/aix-desktop \
    "$(dirname "$0")/../apps/desktop/src-tauri/target/release/aix-desktop"; do
    if [[ -x "$candidate" ]]; then
      BINARY="$candidate"
      break
    fi
  done
fi
[[ -n "$BINARY" && -x "$BINARY" ]] || { echo "error: no release binary found — run 'pnpm desktop:build' first" >&2; exit 1; }

ICON_SRC="$(cd "$(dirname "$0")/.." && pwd)/apps/desktop/src-tauri/icons"
BIN_DIR="$HOME/.local/bin"
ICON_DIR="$HOME/.local/share/icons/hicolor"
DESKTOP_DIR="$HOME/.local/share/applications"

mkdir -p "$BIN_DIR" "$DESKTOP_DIR"
for size in 32x32 64x64 128x128; do
  if [[ -f "$ICON_SRC/$size.png" ]]; then
    mkdir -p "$ICON_DIR/$size/apps"
    cp "$ICON_SRC/$size.png" "$ICON_DIR/$size/apps/$APP_ID.png"
  fi
done
if [[ -f "$ICON_SRC/128x128@2x.png" ]]; then
  mkdir -p "$ICON_DIR/256x256/apps"
  cp "$ICON_SRC/128x128@2x.png" "$ICON_DIR/256x256/apps/$APP_ID.png"
fi
mkdir -p "$ICON_DIR/512x512/apps"
cp "$ICON_SRC/icon.png" "$ICON_DIR/512x512/apps/$APP_ID.png"

cp "$BINARY" "$BIN_DIR/$APP_ID"
chmod +x "$BIN_DIR/$APP_ID"

cat > "$DESKTOP_DIR/$APP_ID.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Comment=$COMMENT
Exec=$BIN_DIR/$APP_ID
Icon=$APP_ID
Terminal=false
Categories=Development;
StartupWMClass=$APP_ID
Keywords=ai;agent;spec;development;
EOF

command -v update-desktop-database >/dev/null && update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t -f "$ICON_DIR" 2>/dev/null || true

echo "installed: $BIN_DIR/$APP_ID"
echo "desktop entry: $DESKTOP_DIR/$APP_ID.desktop"
echo "icons: $ICON_DIR/*/apps/$APP_ID.png"
