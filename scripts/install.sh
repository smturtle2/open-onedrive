#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

if ! have_cmd cargo; then
  echo "Rust toolchain not found. Install cargo/rustc first, then rerun the bootstrap." >&2
  echo "Required build tools: cargo, cmake (or qt-cmake), ninja/make, pkg-config, qml, fuse3." >&2
  exit 1
fi

"$ROOT_DIR/scripts/install-rclone.sh"
if [[ ! -e /dev/fuse ]]; then
  echo "Warning: /dev/fuse is not available. open-onedrive needs FUSE 3 to expose the OneDrive root folder." >&2
fi
cargo run -p xtask -- install
