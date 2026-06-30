#!/usr/bin/env bash
set -euo pipefail

repo="wwwhynot3/slippy-diff"
version="${SLIPPY_VERSION:-latest}"

detect_arch() {
  if command -v uname >/dev/null 2>&1; then
    case "$(uname -m)" in
      x86_64|amd64)
        echo "amd64"
        return
        ;;
      aarch64|arm64)
        echo "arm64"
        return
        ;;
    esac
  fi

  echo "amd64"
}

detect_backend() {
  if [[ "${XDG_SESSION_TYPE:-}" == "wayland" ]] || [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "wayland"
    return
  fi

  if [[ "${XDG_SESSION_TYPE:-}" == "x11" ]] || [[ -n "${DISPLAY:-}" ]]; then
    echo "x11"
    return
  fi

  echo "x11"
}

arch="${SLIPPY_ARCH:-}"
backend="${SLIPPY_BACKEND:-}"

if [[ -z "${arch}" ]]; then
  arch="$(detect_arch)"
fi

if [[ -z "${backend}" ]]; then
  backend="$(detect_backend)"
fi

case "${arch}" in
  amd64|arm64)
    ;;
  *)
    echo "Unsupported SLIPPY_ARCH: ${arch}" >&2
    exit 1
    ;;
esac

case "${backend}" in
  x11|wayland)
    ;;
  *)
    echo "Unsupported SLIPPY_BACKEND: ${backend}" >&2
    exit 1
    ;;
esac

work_dir="$(mktemp -d)"
trap 'rm -rf "${work_dir}"' EXIT

if [[ "${version}" == "latest" ]]; then
  release_json="$(curl -fsSL "https://api.github.com/repos/${repo}/releases/latest")"
  version="$(printf '%s' "${release_json}" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
  if [[ -z "${version}" ]]; then
    echo "Could not determine latest Slippy release tag." >&2
    exit 1
  fi
fi

asset="slippy-${version}-linux-${arch}-${backend}-bundle.tar.gz"
url="https://github.com/${repo}/releases/download/${version}/${asset}"

echo "Resolved version: ${version}"
echo "Resolved arch: ${arch}"
echo "Resolved backend: ${backend}"
echo "Downloading asset: ${asset}"

archive_path="${work_dir}/${asset}"
bundle_dir="${work_dir}/bundle"
mkdir -p "${bundle_dir}"

curl -fsSL "${url}" -o "${archive_path}"
tar -xzf "${archive_path}" -C "${bundle_dir}"

bundle_root="${bundle_dir}/slippy-${version}-linux-${arch}-${backend}-bundle"
if [[ ! -x "${bundle_root}/install-linux.sh" ]]; then
  echo "Bundle installer missing from ${asset}" >&2
  exit 1
fi

"${bundle_root}/install-linux.sh"
