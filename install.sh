#!/usr/bin/env bash
set -euo pipefail

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

TEMP_DIR=""

cleanup() {
  if [[ -n "$TEMP_DIR" && -d "$TEMP_DIR" ]]; then
    rm -rf "$TEMP_DIR"
    TEMP_DIR=""
  fi
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

main() {
  if ! have_cmd tar; then
    echo "tar is required to bootstrap open-onedrive." >&2
    exit 1
  fi

  local repo="${OPEN_ONEDRIVE_REPO:-smturtle2/open-onedrive}"
  local ref="${OPEN_ONEDRIVE_REF:-main}"
  local archive_url="https://codeload.github.com/${repo}/tar.gz/${ref}"
  TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/open-onedrive-install.XXXXXX")"
  trap cleanup EXIT

  echo "Downloading ${repo}@${ref}..."
  fetch_url "$archive_url" | tar -xzf - -C "$TEMP_DIR"

  local source_dir
  source_dir="$(find "$TEMP_DIR" -mindepth 1 -maxdepth 1 -type d -print -quit)"
  if [[ -z "$source_dir" || ! -f "$source_dir/scripts/install.sh" ]]; then
    echo "Downloaded archive does not contain scripts/install.sh." >&2
    exit 1
  fi

  echo "Installing from temporary checkout at ${source_dir}..."
  bash "$source_dir/scripts/install.sh" "$@"
  echo "Cleaning up temporary checkout..."
}

main "$@"
