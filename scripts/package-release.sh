#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY_NAME="heeforge"

usage() {
  cat <<'EOF'
Usage:
  ./scripts/package-release.sh <target-triple>

Examples:
  ./scripts/package-release.sh x86_64-unknown-linux-gnu
  ./scripts/package-release.sh aarch64-apple-darwin
EOF
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    printf 'error: required command not found: %s\n' "$1" >&2
    exit 1
  }
}

cargo_version() {
  sed -n 's/^version = "\(.*\)"/\1/p' "${ROOT_DIR}/Cargo.toml" | head -n 1
}

checksum_file() {
  local file_path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$(basename "$file_path")" > "$(basename "$file_path").sha256"
    return 0
  fi

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$(basename "$file_path")" > "$(basename "$file_path").sha256"
    return 0
  fi

  printf 'warning: sha256sum or shasum not found, checksum file skipped for %s\n' "$file_path" >&2
}

main() {
  if [ "${1:-}" = "" ]; then
    usage
    exit 1
  fi

  need_cmd cargo
  need_cmd tar

  local target="$1"
  local out_dir="${OUT_DIR:-${ROOT_DIR}/dist}"
  local version
  local stage_dir
  local tmp_dir
  local binary_path
  local archive_name
  local archive_path

  version="$(cargo_version)"
  tmp_dir="$(mktemp -d)"
  trap "rm -rf \"$tmp_dir\"" EXIT

  cargo build --release --locked --target "$target" --manifest-path "${ROOT_DIR}/Cargo.toml"

  binary_path="${ROOT_DIR}/target/${target}/release/${BINARY_NAME}"
  [ -x "$binary_path" ] || {
    printf 'error: built binary not found at %s\n' "$binary_path" >&2
    exit 1
  }

  stage_dir="${tmp_dir}/${BINARY_NAME}-${version}-${target}"
  mkdir -p "$stage_dir"
  cp "$binary_path" "${stage_dir}/${BINARY_NAME}"
  cp "${ROOT_DIR}/README.md" "${stage_dir}/README.md"

  mkdir -p "$out_dir"
  archive_name="${BINARY_NAME}-${target}.tar.gz"
  archive_path="${out_dir}/${archive_name}"

  tar -C "$stage_dir" -czf "$archive_path" "${BINARY_NAME}" README.md

  (
    cd "$out_dir"
    checksum_file "$archive_path"
  )

  printf 'Packaged %s\n' "$archive_path"
  if [ -f "${archive_path}.sha256" ]; then
    printf 'Checksum %s\n' "${archive_path}.sha256"
  fi
}

main "$@"
