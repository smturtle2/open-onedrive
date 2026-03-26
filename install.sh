#!/usr/bin/env bash
set -euo pipefail

# Keep this aligned with the latest stable tag so raw tagged installers stay pinned.
OPEN_ONEDRIVE_STABLE_REF="${OPEN_ONEDRIVE_STABLE_REF:-v1.4.1}"

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
  local expected
  expected="$(awk '{print $1}' < "$checksum_file")"

  if have_cmd sha256sum; then
    local actual
    actual="$(sha256sum "$archive_file" | awk '{print $1}')"
    [[ "$expected" == "$actual" ]] && return
    echo "Checksum verification failed for ${archive_file}." >&2
    exit 1
  fi

  if have_cmd shasum; then
    local actual
    actual="$(shasum -a 256 "$archive_file" | cut -d' ' -f1)"
    [[ "$expected" == "$actual" ]] && return
    echo "Checksum verification failed for ${archive_file}." >&2
    exit 1
  fi

  echo "sha256sum or shasum is required to verify release assets." >&2
  exit 1
}

is_dry_run() {
  [[ "${OPEN_ONEDRIVE_DRY_RUN:-0}" == "1" ]]
}

assume_yes() {
  [[ "${OPEN_ONEDRIVE_ASSUME_YES:-0}" == "1" ]]
}

run_cmd() {
  if is_dry_run; then
    printf '+'
    printf ' %q' "$@"
    printf '\n'
    return 0
  fi
  "$@"
}

run_privileged() {
  if [[ "$(id -u)" -eq 0 ]]; then
    run_cmd "$@"
  elif have_cmd sudo; then
    run_cmd sudo "$@"
  else
    echo "sudo is required to install rclone automatically." >&2
    return 1
  fi
}

replace_installed_file() {
  local src="$1"
  local dest="$2"
  local mode="${3:-}"
  local dest_dir tmp_file
  dest_dir="$(dirname "$dest")"
  tmp_file="$(mktemp "${dest_dir}/.$(basename "$dest").XXXXXX")"
  cp "$src" "$tmp_file"
  if [[ -n "$mode" ]]; then
    chmod "$mode" "$tmp_file"
  fi
  mv -f "$tmp_file" "$dest"
}

wait_for_pattern_exit() {
  local user_name="$1"
  local pattern="$2"

  if ! have_cmd pgrep; then
    return
  fi

  local attempt
  for attempt in 1 2 3 4 5 6 7 8 9 10; do
    if ! pgrep -u "$user_name" -f "$pattern" >/dev/null 2>&1; then
      return
    fi
    sleep 0.2
  done
}

stop_running_open_onedrive() {
  local prefix="$1"
  local user_name
  user_name="${USER:-$(id -un)}"
  local bin_dir="$prefix/bin"
  local libexec_dir="$prefix/lib/open-onedrive"

  if have_cmd systemctl; then
    systemctl --user stop openonedrived.service >/dev/null 2>&1 || true
  fi

  if have_cmd pkill; then
    pkill -u "$user_name" -f "${bin_dir}/openonedrived" >/dev/null 2>&1 || true
    pkill -u "$user_name" -f "${libexec_dir}/open-onedrive-ui" >/dev/null 2>&1 || true
    pkill -u "$user_name" -f "${libexec_dir}/open-onedrive-tray" >/dev/null 2>&1 || true
    pkill -u "$user_name" -f "${bin_dir}/open-onedrive\$" >/dev/null 2>&1 || true
  fi

  wait_for_pattern_exit "$user_name" "${bin_dir}/openonedrived"
  wait_for_pattern_exit "$user_name" "${libexec_dir}/open-onedrive-ui"
  wait_for_pattern_exit "$user_name" "${libexec_dir}/open-onedrive-tray"
  wait_for_pattern_exit "$user_name" "${bin_dir}/open-onedrive\$"
}

install_with_apt() {
  run_privileged apt-get update
  run_privileged apt-get install -y rclone
}

install_with_dnf() {
  run_privileged dnf install -y rclone
}

install_with_pacman() {
  run_privileged pacman -S --needed --noconfirm rclone
}

install_with_zypper() {
  run_privileged zypper --non-interactive install rclone
}

install_with_apk() {
  run_privileged apk add rclone
}

install_with_official_script() {
  local downloader=()
  if have_cmd curl; then
    downloader=(curl -fsSL https://rclone.org/install.sh)
  elif have_cmd wget; then
    downloader=(wget -qO- https://rclone.org/install.sh)
  else
    echo "curl or wget is required for the official rclone installer fallback." >&2
    return 1
  fi

  if is_dry_run; then
    printf '+'
    printf ' %q' "${downloader[@]}"
    printf ' |'
    if [[ "$(id -u)" -eq 0 ]]; then
      printf ' %q' bash
    else
      printf ' %q %q' sudo bash
    fi
    printf '\n'
    return 0
  fi

  if [[ "$(id -u)" -eq 0 ]]; then
    "${downloader[@]}" | bash
  else
    sudo -v
    "${downloader[@]}" | sudo bash
  fi
}

attempt_install() {
  local label="$1"
  shift

  echo "rclone not found. Trying ${label}..."
  if "$@"; then
    return 0
  fi

  echo "Failed to install rclone with ${label}." >&2
  return 1
}

ensure_rclone_installed() {
  if have_cmd rclone; then
    return
  fi

  if have_cmd apt-get && attempt_install "apt-get" install_with_apt; then
    is_dry_run || have_cmd rclone && return
  fi
  if have_cmd dnf && attempt_install "dnf" install_with_dnf; then
    is_dry_run || have_cmd rclone && return
  fi
  if have_cmd pacman && attempt_install "pacman" install_with_pacman; then
    is_dry_run || have_cmd rclone && return
  fi
  if have_cmd zypper && attempt_install "zypper" install_with_zypper; then
    is_dry_run || have_cmd rclone && return
  fi
  if have_cmd apk && attempt_install "apk" install_with_apk; then
    is_dry_run || have_cmd rclone && return
  fi
  if attempt_install "the official rclone installer" install_with_official_script; then
    is_dry_run || have_cmd rclone && return
  fi

  echo "Unable to install rclone automatically." >&2
  exit 1
}

check_fuse_runtime() {
  if [[ "${OPEN_ONEDRIVE_SKIP_FUSE_CHECK:-0}" == "1" ]]; then
    return
  fi

  if [[ ! -e /dev/fuse ]]; then
    echo "Warning: /dev/fuse is not available. open-onedrive needs FUSE to expose the OneDrive folder." >&2
  fi

  if have_cmd fusermount3 || have_cmd mount.fuse3; then
    return
  fi

  echo "Warning: fuse3 helpers were not found in PATH. Install fuse3 if the filesystem fails to start." >&2
}

release_base_url() {
  local repo="$1"
  local ref="$2"

  if [[ -n "${OPEN_ONEDRIVE_RELEASE_BASE_URL:-}" ]]; then
    printf '%s\n' "${OPEN_ONEDRIVE_RELEASE_BASE_URL}"
    return
  fi

  local effective_ref="$ref"
  if [[ -z "$effective_ref" ]]; then
    effective_ref="$OPEN_ONEDRIVE_STABLE_REF"
  fi

  if [[ -n "$effective_ref" ]]; then
    printf 'https://github.com/%s/releases/download/%s\n' "$repo" "$effective_ref"
    return
  fi

  printf 'https://github.com/%s/releases/latest/download\n' "$repo"
}

release_target_ref() {
  local ref="$1"
  local effective_ref="$ref"
  if [[ -z "$effective_ref" ]]; then
    effective_ref="$OPEN_ONEDRIVE_STABLE_REF"
  fi
  if [[ -n "$effective_ref" ]]; then
    printf '%s\n' "$effective_ref"
    return
  fi
  printf 'latest\n'
}

install_metadata_dir() {
  printf '%s\n' "${HOME:?HOME is not set}/.local/share/open-onedrive"
}

install_metadata_file() {
  printf '%s/install-metadata.env\n' "$(install_metadata_dir)"
}

read_metadata_value() {
  local file="$1"
  local key="$2"
  if [[ ! -f "$file" ]]; then
    return
  fi
  awk -F= -v key="$key" '$1 == key { print substr($0, index($0, "=") + 1); exit }' "$file"
}

installed_open_onedrive_exists() {
  local home_dir="${HOME:?HOME is not set}"
  [[ -x "${home_dir}/.local/bin/open-onedrive" || -x "${home_dir}/.local/bin/openonedrived" ]]
}

can_prompt_user() {
  [[ -e /dev/tty && -r /dev/tty && -w /dev/tty && ( -t 1 || -t 2 ) ]]
}

confirm_with_tty() {
  local prompt="$1"
  if assume_yes; then
    return 0
  fi
  if ! can_prompt_user; then
    return 0
  fi

  local answer
  while true; do
    printf '%s [y/N] ' "$prompt" > /dev/tty
    if ! IFS= read -r answer < /dev/tty; then
      return 1
    fi
    case "${answer}" in
      [Yy]|[Yy][Ee][Ss])
        return 0
        ;;
      ""|[Nn]|[Nn][Oo])
        return 1
        ;;
      *)
        printf 'Please answer y or n.\n' > /dev/tty
        ;;
    esac
  done
}

check_existing_installation() {
  local target_ref="$1"
  local target_mode="$2"
  if ! installed_open_onedrive_exists; then
    return 0
  fi

  local metadata_file
  metadata_file="$(install_metadata_file)"
  local installed_ref installed_mode prompt
  installed_ref="$(read_metadata_value "$metadata_file" "INSTALL_REF")"
  installed_mode="$(read_metadata_value "$metadata_file" "INSTALL_MODE")"

  if [[ -n "$installed_ref" ]]; then
    if [[ "$installed_ref" == "$target_ref" && "$installed_mode" == "$target_mode" ]]; then
      prompt="open-onedrive ${installed_ref} (${installed_mode}) is already installed. Reinstall it?"
    else
      prompt="open-onedrive ${installed_ref:-unknown} (${installed_mode:-unknown}) is already installed. Replace it with ${target_ref} (${target_mode})?"
    fi
  else
    prompt="An existing open-onedrive installation was found under ~/.local. Replace it with ${target_ref} (${target_mode})?"
  fi

  if is_dry_run; then
    echo "${prompt}"
    return 0
  fi

  if ! can_prompt_user && ! assume_yes; then
    echo "${prompt} Re-run with OPEN_ONEDRIVE_ASSUME_YES=1 to replace the existing installation non-interactively." >&2
    exit 1
  fi

  if confirm_with_tty "$prompt"; then
    return 0
  fi

  if can_prompt_user; then
    echo "Keeping the existing installation unchanged."
    exit 0
  fi

  echo "Existing installation detected; continuing without an interactive prompt." >&2
}

write_install_metadata() {
  local install_ref="$1"
  local install_mode="$2"
  local metadata_dir metadata_file
  metadata_dir="$(install_metadata_dir)"
  metadata_file="$(install_metadata_file)"

  if is_dry_run; then
    echo "Would write install metadata to ${metadata_file} (${install_ref}, ${install_mode})."
    return
  fi

  mkdir -p "$metadata_dir"
  cat > "$metadata_file" <<EOF
INSTALL_REF=${install_ref}
INSTALL_MODE=${install_mode}
INSTALLED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
EOF
}

write_launcher() {
  local path="$1"
  local bin_dir="$2"
  local libexec_dir="$3"
  local temp_path
  temp_path="$(mktemp "${path}.XXXXXX")"
  cat > "$temp_path" <<EOF
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
  if ! pgrep -u "\$launcher_user" -f "${libexec_dir}/open-onedrive-tray" >/dev/null 2>&1; then
    "${libexec_dir}/open-onedrive-tray" >/dev/null 2>&1 &
    disown || true
  fi
fi

exec "${libexec_dir}/open-onedrive-ui" "\$@"
EOF
  chmod +x "$temp_path"
  mv -f "$temp_path" "$path"
}

write_desktop_entry() {
  local path="$1"
  local bin_dir="$2"
  local temp_path
  temp_path="$(mktemp "${path}.XXXXXX")"
  cat > "$temp_path" <<EOF
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
  mv -f "$temp_path" "$path"
}

write_systemd_service() {
  local path="$1"
  local bin_dir="$2"
  local temp_path
  temp_path="$(mktemp "${path}.XXXXXX")"
  cat > "$temp_path" <<EOF
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
  mv -f "$temp_path" "$path"
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
  local emblem_dir="$prefix/share/icons/hicolor/scalable/emblems"
  local nautilus_extension_dir="$prefix/share/nautilus-python/extensions"
  local plugin_root="$prefix/lib/qt6/plugins/kf6"
  local action_plugin_dir="$plugin_root/kfileitemaction"
  local overlay_plugin_dir="$plugin_root/overlayicon"
  local service_dir="$home_dir/.config/systemd/user"

  mkdir -p \
    "$bin_dir" \
    "$libexec_dir" \
    "$app_dir" \
    "$icon_dir" \
    "$emblem_dir" \
    "$nautilus_extension_dir" \
    "$action_plugin_dir" \
    "$overlay_plugin_dir" \
    "$service_dir"

  stop_running_open_onedrive "$prefix"

  replace_installed_file "$extracted_root/openonedrived" "$bin_dir/openonedrived" 755
  replace_installed_file "$extracted_root/openonedrivectl" "$bin_dir/openonedrivectl" 755
  replace_installed_file "$extracted_root/open-onedrive-ui" "$libexec_dir/open-onedrive-ui" 755
  replace_installed_file "$extracted_root/open-onedrive-tray" "$libexec_dir/open-onedrive-tray" 755
  replace_installed_file "$extracted_root/openonedrive.py" "$nautilus_extension_dir/openonedrive.py" 755
  replace_installed_file "$extracted_root/libopen_onedrive_fileitemaction.so" "$action_plugin_dir/libopen_onedrive_fileitemaction.so"
  replace_installed_file "$extracted_root/libopen_onedrive_overlayicon.so" "$overlay_plugin_dir/libopen_onedrive_overlayicon.so"
  replace_installed_file "$extracted_root/io.github.smturtle2.OpenOneDrive.svg" "$icon_dir/io.github.smturtle2.OpenOneDrive.svg"
  replace_installed_file "$extracted_root/open-onedrive-online-only.svg" "$emblem_dir/open-onedrive-online-only.svg"
  replace_installed_file "$extracted_root/open-onedrive-local.svg" "$emblem_dir/open-onedrive-local.svg"
  replace_installed_file "$extracted_root/open-onedrive-pinned.svg" "$emblem_dir/open-onedrive-pinned.svg"
  replace_installed_file "$extracted_root/open-onedrive-syncing.svg" "$emblem_dir/open-onedrive-syncing.svg"
  replace_installed_file "$extracted_root/open-onedrive-attention.svg" "$emblem_dir/open-onedrive-attention.svg"

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
  if have_cmd gtk-update-icon-cache; then
    gtk-update-icon-cache -f -t "$prefix/share/icons/hicolor" >/dev/null 2>&1 || true
  fi
  if have_cmd kbuildsycoca6; then
    kbuildsycoca6 >/dev/null 2>&1 || true
  fi

  write_install_metadata "${OPEN_ONEDRIVE_INSTALL_REF_ACTUAL}" "${OPEN_ONEDRIVE_INSTALL_MODE_ACTUAL}"
}

install_from_release() {
  local repo="$1"
  local ref="$2"
  local temp_dir="$3"
  local asset_name="open-onedrive-linux-x86_64.tar.gz"
  local checksum_name="${asset_name}.sha256"
  local base_url
  base_url="$(release_base_url "$repo" "$ref")"

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

  ensure_rclone_installed
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
  write_install_metadata "${OPEN_ONEDRIVE_INSTALL_REF_ACTUAL}" "${OPEN_ONEDRIVE_INSTALL_MODE_ACTUAL}"
}

main() {
  local repo="${OPEN_ONEDRIVE_REPO:-smturtle2/open-onedrive}"
  local ref="${OPEN_ONEDRIVE_REF:-}"
  local mode="${OPEN_ONEDRIVE_INSTALL_MODE:-release}"
  if [[ "${OPEN_ONEDRIVE_BUILD_FROM_SOURCE:-0}" == "1" ]]; then
    mode="source"
  fi

  if [[ "$mode" == "release" ]]; then
    OPEN_ONEDRIVE_INSTALL_REF_ACTUAL="$(release_target_ref "$ref")"
  else
    OPEN_ONEDRIVE_INSTALL_REF_ACTUAL="source@${ref:-main}"
  fi
  OPEN_ONEDRIVE_INSTALL_MODE_ACTUAL="$mode"

  check_existing_installation "$OPEN_ONEDRIVE_INSTALL_REF_ACTUAL" "$OPEN_ONEDRIVE_INSTALL_MODE_ACTUAL"

  OPEN_ONEDRIVE_TMPDIR="$(mktemp -d "${TMPDIR:-/tmp}/open-onedrive-install.XXXXXX")"
  trap 'rm -rf "${OPEN_ONEDRIVE_TMPDIR:-}"' EXIT

  case "$mode" in
    release)
      install_from_release "$repo" "$ref" "$OPEN_ONEDRIVE_TMPDIR"
      ;;
    source)
      install_from_source "$repo" "$ref" "$OPEN_ONEDRIVE_TMPDIR" "$@"
      ;;
    *)
      echo "Unknown OPEN_ONEDRIVE_INSTALL_MODE: ${mode}" >&2
      exit 1
      ;;
  esac
}

main "$@"
