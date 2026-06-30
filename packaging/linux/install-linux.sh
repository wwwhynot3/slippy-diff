#!/usr/bin/env bash
set -euo pipefail

bundle_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
desktop_id="dev.wwwhynot3.slippy"
binary_src="${bundle_dir}/slippy"
desktop_src="${bundle_dir}/share/applications/${desktop_id}.desktop"
icons_src_root="${bundle_dir}/share/icons/hicolor"

bin_dir="${HOME}/.local/bin"
applications_dir="${HOME}/.local/share/applications"
icons_dir="${HOME}/.local/share/icons/hicolor"

installed_binary="${bin_dir}/slippy"
installed_uninstaller="${bin_dir}/slippy-uninstall"
installed_desktop="${applications_dir}/${desktop_id}.desktop"

mkdir -p "${bin_dir}" "${applications_dir}"
install -m 755 "${binary_src}" "${installed_binary}"
install -m 755 "${bundle_dir}/uninstall-linux.sh" "${installed_uninstaller}"

tmp_desktop="$(mktemp)"
trap 'rm -f "${tmp_desktop}"' EXIT
sed "s|^Exec=.*$|Exec=${installed_binary}|" "${desktop_src}" > "${tmp_desktop}"
install -m 644 "${tmp_desktop}" "${installed_desktop}"

find "${icons_src_root}" -mindepth 2 -maxdepth 2 -type d -name apps | while read -r src_dir; do
  size_dir="$(basename "$(dirname "${src_dir}")")"
  target_dir="${icons_dir}/${size_dir}/apps"
  mkdir -p "${target_dir}"
  install -m 644 "${src_dir}/${desktop_id}.png" "${target_dir}/${desktop_id}.png"
done

echo "Slippy installed to ${installed_binary}"
echo "Launcher installed to ${installed_desktop}"
echo "Run ${installed_uninstaller} to remove the user-scoped install."
