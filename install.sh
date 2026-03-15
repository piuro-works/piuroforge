#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="heeforge"
DEFAULT_REPO="johwanghee/heeforge"
DEFAULT_INSTALL_DIR="${HOME}/.local/bin"

log() {
  printf '%s\n' "$*"
}

warn() {
  printf 'warning: %s\n' "$*" >&2
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

download() {
  local url="$1"
  local output="$2"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$output"
    return 0
  fi

  if command -v wget >/dev/null 2>&1; then
    wget -qO "$output" "$url"
    return 0
  fi

  fail "curl or wget is required to download release assets"
}

verify_checksum() {
  local archive_path="$1"
  local checksum_path="$2"

  if ! [ -f "$checksum_path" ]; then
    warn "checksum file not found; skipping checksum verification"
    return 0
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$(dirname "$archive_path")" && sha256sum -c "$(basename "$checksum_path")")
    return 0
  fi

  if command -v shasum >/dev/null 2>&1; then
    (cd "$(dirname "$archive_path")" && shasum -a 256 -c "$(basename "$checksum_path")")
    return 0
  fi

  warn "sha256sum or shasum is not available; skipping checksum verification"
}

detect_target() {
  local os
  local arch

  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux) os="unknown-linux-gnu" ;;
    Darwin) os="apple-darwin" ;;
    *)
      fail "unsupported operating system: $os"
      ;;
  esac

  case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *)
      fail "unsupported architecture: $arch"
      ;;
  esac

  case "${arch}-${os}" in
    x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu|x86_64-apple-darwin|aarch64-apple-darwin)
      printf '%s\n' "${arch}-${os}"
      ;;
    *)
      fail "no release artifact configured for target ${arch}-${os}"
      ;;
  esac
}

asset_url() {
  local archive_name="$1"
  local version="$2"
  local repo="${HEEFORGE_REPO:-$DEFAULT_REPO}"

  if [ -n "${HEEFORGE_DOWNLOAD_URL:-}" ]; then
    printf '%s\n' "$HEEFORGE_DOWNLOAD_URL"
    return 0
  fi

  if [ "$version" = "latest" ]; then
    printf 'https://github.com/%s/releases/latest/download/%s\n' "$repo" "$archive_name"
    return 0
  fi

  printf 'https://github.com/%s/releases/download/%s/%s\n' "$repo" "$version" "$archive_name"
}

main() {
  need_cmd uname
  need_cmd tar
  need_cmd mktemp
  need_cmd install

  local install_dir="${HEEFORGE_INSTALL_DIR:-$DEFAULT_INSTALL_DIR}"
  local version="${HEEFORGE_VERSION:-latest}"
  local target
  local archive_name
  local download_url
  local checksum_url
  local tmp_dir
  local archive_path
  local checksum_path
  local extracted_binary

  target="$(detect_target)"
  archive_name="${BINARY_NAME}-${target}.tar.gz"
  download_url="$(asset_url "$archive_name" "$version")"
  checksum_url="${download_url}.sha256"

  tmp_dir="$(mktemp -d)"
  trap "rm -rf \"$tmp_dir\"" EXIT

  archive_path="${tmp_dir}/${archive_name}"
  checksum_path="${archive_path}.sha256"

  log "Installing ${BINARY_NAME} for target ${target}"
  log "Download: ${download_url}"

  download "$download_url" "$archive_path"

  if download "$checksum_url" "$checksum_path"; then
    verify_checksum "$archive_path" "$checksum_path"
  else
    warn "unable to download checksum file from ${checksum_url}"
  fi

  tar -xzf "$archive_path" -C "$tmp_dir"

  extracted_binary="${tmp_dir}/${BINARY_NAME}"
  if ! [ -f "$extracted_binary" ]; then
    extracted_binary="$(find "$tmp_dir" -maxdepth 2 -type f -name "$BINARY_NAME" | head -n 1)"
  fi

  if ! [ -n "$extracted_binary" ] || ! [ -f "$extracted_binary" ]; then
    fail "downloaded archive does not contain ${BINARY_NAME}"
  fi

  mkdir -p "$install_dir"
  install -m 755 "$extracted_binary" "${install_dir}/${BINARY_NAME}"

  log ""
  log "Installed ${BINARY_NAME} to ${install_dir}/${BINARY_NAME}"
  log "Run: ${BINARY_NAME} --help"

  case ":${PATH}:" in
    *":${install_dir}:"*) ;;
    *)
      log ""
      log "Add ${install_dir} to PATH if it is not already available in your shell."
      ;;
  esac
}

main "$@"
