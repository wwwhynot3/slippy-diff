#!/usr/bin/env bash
set -euo pipefail

desktop_id="dev.wwwhynot3.slippy"

rm -f "${HOME}/.local/bin/slippy"
rm -f "${HOME}/.local/bin/slippy-uninstall"
rm -f "${HOME}/.local/share/applications/${desktop_id}.desktop"

for size in 16x16 32x32 48x48 64x64 128x128 256x256 512x512; do
  rm -f "${HOME}/.local/share/icons/hicolor/${size}/apps/${desktop_id}.png"
done

echo "Removed Slippy user-scoped desktop integration."
