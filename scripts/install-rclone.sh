#!/usr/bin/env bash
set -euo pipefail

have_cmd() {
  command -v "$1" >/dev/null 2>&1
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

main() {
  if have_cmd rclone; then
    local current_version
    current_version="$(rclone version 2>/dev/null | head -n1 | sed 's/^rclone //')"
    if [[ "${OPEN_ONEDRIVE_UPGRADE_RCLONE:-0}" != "1" ]]; then
      if [[ -n "$current_version" ]]; then
        echo "Using existing rclone ${current_version}."
      else
        echo "Using existing rclone installation."
      fi
      exit 0
    fi

    if is_dry_run; then
      echo "Would upgrade existing rclone ${current_version:-unknown version}."
    elif ! can_prompt_user && ! assume_yes; then
      echo "rclone ${current_version:-unknown version} is already installed. Continuing with a non-interactive upgrade."
    elif ! confirm_with_tty "rclone ${current_version:-unknown version} is already installed. Upgrade it anyway?"; then
      echo "Keeping the existing rclone installation."
      exit 0
    fi
  fi

  if have_cmd rclone && [[ "${OPEN_ONEDRIVE_UPGRADE_RCLONE:-0}" != "1" ]]; then
    exit 0
  fi

  if have_cmd apt-get && attempt_install "apt-get" install_with_apt; then
    if is_dry_run || have_cmd rclone; then
      exit 0
    fi
  fi

  if have_cmd dnf && attempt_install "dnf" install_with_dnf; then
    if is_dry_run || have_cmd rclone; then
      exit 0
    fi
  fi

  if have_cmd pacman && attempt_install "pacman" install_with_pacman; then
    if is_dry_run || have_cmd rclone; then
      exit 0
    fi
  fi

  if have_cmd zypper && attempt_install "zypper" install_with_zypper; then
    if is_dry_run || have_cmd rclone; then
      exit 0
    fi
  fi

  if have_cmd apk && attempt_install "apk" install_with_apk; then
    if is_dry_run || have_cmd rclone; then
      exit 0
    fi
  fi

  if attempt_install "the official rclone installer" install_with_official_script; then
    if is_dry_run || have_cmd rclone; then
      exit 0
    fi
  fi

  echo "Unable to install rclone automatically." >&2
  exit 1
}

main "$@"
