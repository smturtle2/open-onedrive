#!/usr/bin/env bash
set -euo pipefail

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

fetch_url() {
  local url="$1"
  if have_cmd curl; then
    curl -fsSL "$url"
    return
  fi
  if have_cmd wget; then
    wget -qO- "$url"
    return
  fi

  echo "curl or wget is required to bootstrap open-onedrive." >&2
  exit 1
}

run_local_install_if_available() {
  local script_source="${BASH_SOURCE[0]:-}"
  if [[ -z "$script_source" || ! -f "$script_source" ]]; then
    return 1
  fi

  local script_dir
  script_dir="$(cd "$(dirname "$script_source")" && pwd)"
  if [[ -f "$script_dir/Cargo.toml" && -f "$script_dir/scripts/install.sh" ]]; then
    exec bash "$script_dir/scripts/install.sh" "$@"
  fi
}

main() {
  run_local_install_if_available "$@" || true

  if ! have_cmd tar; then
    echo "tar is required to bootstrap open-onedrive." >&2
    exit 1
  fi

  local repo="${OPEN_ONEDRIVE_REPO:-smturtle2/open-onedrive}"
  local ref="${OPEN_ONEDRIVE_REF:-main}"
  local archive_url="https://codeload.github.com/${repo}/tar.gz/${ref}"
  local temp_dir
  temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/open-onedrive-install.XXXXXX")"
  trap 'rm -rf "$temp_dir"' EXIT

  echo "Downloading ${repo}@${ref}..."
  fetch_url "$archive_url" | tar -xzf - -C "$temp_dir"

  local source_dir
  source_dir="$(find "$temp_dir" -mindepth 1 -maxdepth 1 -type d -print -quit)"
  if [[ -z "$source_dir" || ! -f "$source_dir/scripts/install.sh" ]]; then
    echo "Downloaded archive does not contain scripts/install.sh." >&2
    exit 1
  fi

  exec bash "$source_dir/scripts/install.sh" "$@"
}

main "$@"
