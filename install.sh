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

sha256_check() {
  local checksum_file="$1"
  local archive_file="$2"
  if have_cmd sha256sum; then
    (cd "$(dirname "$archive_file")" && sha256sum -c "$(basename "$checksum_file")")
    return
  fi
  if have_cmd shasum; then
    local expected
    expected="$(cut -d' ' -f1 < "$checksum_file")"
    local actual
    actual="$(shasum -a 256 "$archive_file" | cut -d' ' -f1)"
    if [[ "$expected" == "$actual" ]]; then
      return
    fi
    echo "Checksum verification failed for ${archive_file}." >&2
    exit 1
  fi

  echo "sha256sum or shasum is required to verify release assets." >&2
  exit 1
}

install_rclone_helper() {
  if have_cmd rclone; then
    return
  fi

  local repo="$1"
  local ref="$2"
  local temp_dir="$3"
  local helper="$temp_dir/install-rclone.sh"

  echo "rclone not found. Fetching the helper installer..."
  fetch_url "https://raw.githubusercontent.com/${repo}/${ref}/scripts/install-rclone.sh" > "$helper"
  chmod +x "$helper"
  bash "$helper"
}

check_fuse_runtime() {
  if [[ ! -e /dev/fuse ]]; then
    echo "Warning: /dev/fuse is not available. open-onedrive needs FUSE to expose the OneDrive folder." >&2
  fi

  if have_cmd fusermount3 || have_cmd mount.fuse3; then
    return
  fi

  echo "Warning: fuse3 helpers were not found in PATH. Install fuse3 if the filesystem fails to start." >&2
}

write_launcher() {
  local path="$1"
  local bin_dir="$2"
  local libexec_dir="$3"
  cat > "$path" <<EOF
#!/usr/bin/env bash
set -euo pipefail

if command -v systemctl >/dev/null 2>&1; then
  systemctl --user start openonedrived.service >/dev/null 2>&1 || true
fi

if command -v pgrep >/dev/null 2>&1; then
  launcher_user="\${USER:-\$(id -un)}"
  if ! pgrep -u "\$launcher_user" -f "${bin_dir}/openonedrived" >/dev/null 2>&1; then
    "${bin_dir}/openonedrived" >/dev/null 2>&1 &
    disown || true
  fi
fi

exec "${libexec_dir}/open-onedrive-ui" "\$@"
EOF
  chmod +x "$path"
}

write_desktop_entry() {
  local path="$1"
  local bin_dir="$2"
  cat > "$path" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=open-onedrive
Comment=OneDrive desktop client for Linux
Exec=${bin_dir}/open-onedrive
TryExec=${bin_dir}/open-onedrive
Icon=io.github.smturtle2.OpenOneDrive
Terminal=false
Categories=Network;Office;Utility;
StartupNotify=true
EOF
}

write_systemd_service() {
  local path="$1"
  local bin_dir="$2"
  cat > "$path" <<EOF
[Unit]
Description=open-onedrive custom FUSE OneDrive daemon
After=default.target

[Service]
Type=simple
ExecStart=${bin_dir}/openonedrived
Restart=on-failure
RestartSec=3
Environment=RUST_LOG=openonedrived=info

[Install]
WantedBy=default.target
EOF
}

install_release_tree() {
  local extracted_root="$1"
  local home_dir
  home_dir="${HOME:?HOME is not set}"
  local prefix="$home_dir/.local"
  local bin_dir="$prefix/bin"
  local libexec_dir="$prefix/lib/open-onedrive"
  local app_dir="$prefix/share/applications"
  local icon_dir="$prefix/share/icons/hicolor/scalable/apps"
  local plugin_root="$prefix/lib/qt6/plugins/kf6"
  local action_plugin_dir="$plugin_root/kfileitemaction"
  local overlay_plugin_dir="$plugin_root/overlayicon"
  local service_dir="$home_dir/.config/systemd/user"

  mkdir -p \
    "$bin_dir" \
    "$libexec_dir" \
    "$app_dir" \
    "$icon_dir" \
    "$action_plugin_dir" \
    "$overlay_plugin_dir" \
    "$service_dir"

  cp "$extracted_root/openonedrived" "$bin_dir/openonedrived"
  cp "$extracted_root/openonedrivectl" "$bin_dir/openonedrivectl"
  cp "$extracted_root/open-onedrive-ui" "$libexec_dir/open-onedrive-ui"
  cp "$extracted_root/libopen_onedrive_fileitemaction.so" "$action_plugin_dir/libopen_onedrive_fileitemaction.so"
  cp "$extracted_root/libopen_onedrive_overlayicon.so" "$overlay_plugin_dir/libopen_onedrive_overlayicon.so"
  cp "$extracted_root/io.github.smturtle2.OpenOneDrive.svg" "$icon_dir/io.github.smturtle2.OpenOneDrive.svg"
  chmod +x "$bin_dir/openonedrived" "$bin_dir/openonedrivectl" "$libexec_dir/open-onedrive-ui"

  write_launcher "$bin_dir/open-onedrive" "$bin_dir" "$libexec_dir"
  write_desktop_entry "$app_dir/io.github.smturtle2.OpenOneDrive.desktop" "$bin_dir"
  write_systemd_service "$service_dir/openonedrived.service" "$bin_dir"

  if have_cmd systemctl; then
    systemctl --user stop openonedrived.service >/dev/null 2>&1 || true
    systemctl --user daemon-reload >/dev/null 2>&1 || true
    systemctl --user enable --now openonedrived.service >/dev/null 2>&1 || true
  fi
  if have_cmd update-desktop-database; then
    update-desktop-database "$app_dir" >/dev/null 2>&1 || true
  fi
  if have_cmd kbuildsycoca6; then
    kbuildsycoca6 >/dev/null 2>&1 || true
  fi
}

install_from_release() {
  local repo="$1"
  local ref="$2"
  local temp_dir="$3"
  local asset_name="open-onedrive-linux-x86_64.tar.gz"
  local checksum_name="${asset_name}.sha256"
  local base_url
  if [[ -n "$ref" ]]; then
    base_url="https://github.com/${repo}/releases/download/${ref}"
  else
    base_url="https://github.com/${repo}/releases/latest/download"
  fi

  local archive_file="$temp_dir/$asset_name"
  local checksum_file="$temp_dir/$checksum_name"
  echo "Downloading release asset ${asset_name}..."
  fetch_url "${base_url}/${asset_name}" > "$archive_file"
  fetch_url "${base_url}/${checksum_name}" > "$checksum_file"
  sha256_check "$checksum_file" "$archive_file"

  local extract_dir="$temp_dir/release"
  mkdir -p "$extract_dir"
  tar -xzf "$archive_file" -C "$extract_dir"
  local extracted_root="$extract_dir/open-onedrive-linux-x86_64"
  if [[ ! -d "$extracted_root" ]]; then
    echo "Release archive did not contain open-onedrive-linux-x86_64/." >&2
    exit 1
  fi

  install_rclone_helper "$repo" "${ref:-main}" "$temp_dir"
  check_fuse_runtime
  install_release_tree "$extracted_root"
  echo "Installed open-onedrive into \$HOME/.local"
}

install_from_source() {
  local repo="$1"
  local ref="$2"
  local temp_dir="$3"
  shift 3
  local archive_ref="${ref:-main}"
  local archive_url="https://codeload.github.com/${repo}/tar.gz/${archive_ref}"

  echo "Downloading source archive ${repo}@${archive_ref}..."
  fetch_url "$archive_url" | tar -xzf - -C "$temp_dir"

  local source_dir
  source_dir="$(find "$temp_dir" -mindepth 1 -maxdepth 1 -type d -print -quit)"
  if [[ -z "$source_dir" || ! -f "$source_dir/scripts/install.sh" ]]; then
    echo "Downloaded archive does not contain scripts/install.sh." >&2
    exit 1
  fi

  echo "Installing from temporary checkout at ${source_dir}..."
  bash "$source_dir/scripts/install.sh" "$@"
}

main() {
  local repo="${OPEN_ONEDRIVE_REPO:-smturtle2/open-onedrive}"
  local ref="${OPEN_ONEDRIVE_REF:-}"
  local mode="${OPEN_ONEDRIVE_INSTALL_MODE:-release}"
  if [[ "${OPEN_ONEDRIVE_BUILD_FROM_SOURCE:-0}" == "1" ]]; then
    mode="source"
  fi

  local temp_dir
  temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/open-onedrive-install.XXXXXX")"
  trap 'rm -rf "$temp_dir"' EXIT

  case "$mode" in
    release)
      install_from_release "$repo" "$ref" "$temp_dir"
      ;;
    source)
      install_from_source "$repo" "$ref" "$temp_dir" "$@"
      ;;
    *)
      echo "Unsupported OPEN_ONEDRIVE_INSTALL_MODE: ${mode}" >&2
      exit 1
      ;;
  esac
}

main "$@"
