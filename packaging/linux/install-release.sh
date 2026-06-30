#!/usr/bin/env bash
set -euo pipefail

repo="wwwhynot3/slippy-diff"
version="${SLIPPY_VERSION:-latest}"
release_tag="${SLIPPY_RELEASE_TAG:-}"

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

resolve_latest_release_tag() {
  local redirect_headers=""
  local redirected_url=""
  local tag=""
  local release_json=""

  redirect_headers="$(curl -fsSLI "https://github.com/${repo}/releases/latest" 2>/dev/null || true)"
  redirected_url="$(printf '%s' "${redirect_headers}" | sed -n 's/^[Ll]ocation: *\(.*\)\r$/\1/p' | tail -n 1)"
  tag="$(printf '%s' "${redirected_url}" | sed -n 's|.*/tag/\(slippy-v[^/?#[:space:]]*\).*|\1|p' | tail -n 1)"
  if [[ -n "${tag}" ]]; then
    echo "${tag}"
    return
  fi

  release_json="$(curl -fsSL "https://api.github.com/repos/${repo}/releases/latest" 2>/dev/null || true)"
  tag="$(printf '%s' "${release_json}" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
  if [[ -n "${tag}" ]]; then
    echo "${tag}"
    return
  fi

  return 1
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

if [[ -n "${release_tag}" ]]; then
  version="${release_tag#slippy-}"
elif [[ "${version}" == "latest" ]]; then
  release_tag="$(resolve_latest_release_tag || true)"
  if [[ -z "${release_tag}" ]]; then
    echo "Could not determine latest Slippy release tag. Set SLIPPY_VERSION=v... or SLIPPY_RELEASE_TAG=slippy-v... and retry." >&2
    exit 1
  fi
  version="${release_tag#slippy-}"
else
  release_tag="slippy-${version}"
fi

asset="slippy-${version}-linux-${arch}-${backend}-bundle.tar.gz"
url="https://github.com/${repo}/releases/download/${release_tag}/${asset}"

echo "Resolved release tag: ${release_tag}"
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
