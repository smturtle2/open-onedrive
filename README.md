<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  A KDE-first Linux shell for OneDrive where <code>rclone</code> provides the mounted tree and file bytes, while the wrapper owns path state, device residency, Dolphin overlays, and the tray/dashboard experience.
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/release.yml"><img alt="Release" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/release.yml?label=release"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

<p align="center">
  <a href="./README.ko.md">한국어</a> ·
  <a href="#highlights">Highlights</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#configuration">Configuration</a> ·
  <a href="#how-it-works">How It Works</a> ·
  <a href="#development">Development</a>
</p>

<p align="center">
  <img src="./assets/docs/dashboard-hero.svg" alt="open-onedrive dashboard preview" width="100%">
</p>

## Overview

`open-onedrive` targets `KDE Plasma 6 + Dolphin` on Linux. The daemon owns an app-specific `rclone.conf`, supervises `rclone mount`, maintains a SQLite-backed path-state cache, and exposes that state to the dashboard, tray icon, CLI, Dolphin overlays, and Dolphin context actions.

`rclone` is still responsible for:

- mounting the OneDrive tree into the filesystem
- downloading file bytes on demand
- listing remote paths for cache refresh via `lsjson`

The wrapper is responsible for:

- keeping files pinned locally or returning them to online-only mode
- caching path state for fast Dolphin overlays
- driving a tray-resident desktop app and signal-driven dashboard
- keeping its runtime state under dedicated XDG paths instead of touching the user's default `~/.config/rclone/rclone.conf`

## Highlights

- `curl ... | bash` installs the latest GitHub release asset by default
- app-owned `rclone.conf` under XDG paths, isolated from `~/.config/rclone/rclone.conf`
- daemon-managed `rclone mount` with mount readiness checks, restart backoff, and recent log capture
- SQLite-backed path-state cache refreshed from `rclone lsjson`
- Dolphin overlay icons plus `Keep on this device` / `Make online-only` actions
- Qt6/Kirigami dashboard with quick per-file controls, sync pause/resume, and recent diagnostics
- KDE StatusNotifier tray item with close-to-tray behavior

## Quick Start

Install the latest release directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

Install a specific release tag:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_REF=v0.1.0 bash
```

Force the old source-build bootstrap path:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_BUILD_FROM_SOURCE=1 bash
```

What the release installer does:

- downloads the Linux `x86_64` release archive and its SHA256 file
- verifies the archive before extracting it
- installs binaries, KDE plugins, icon, launcher, and the user service into your home directory
- installs `rclone` automatically if it is missing
- enables `openonedrived.service` for the current user when `systemd --user` is available

Launch and verify:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

Typical first run:

1. Choose an empty mount directory such as `~/OneDrive`.
2. Finish the Microsoft browser sign-in started by `rclone`.
3. Open the mounted folder in Dolphin.
4. Use overlay icons or the context menu to keep files local or return them to online-only mode.
5. Close the window and keep the app running from the tray if desired.

CLI equivalents:

```bash
openonedrivectl keep-local ~/OneDrive/Documents/report.pdf
openonedrivectl make-online-only ~/OneDrive/Documents/report.pdf
openonedrivectl rescan
openonedrivectl path-states ~/OneDrive/Documents/report.pdf
```

## Requirements

- Linux `x86_64`
- runtime dependency: `rclone`
- first-class target: `KDE Plasma 6` with `Dolphin`
- release installer target: user-local install under `~/.local`
- source build path: Rust, CMake, Qt6 tooling, KF6 development packages, and a C++ compiler

## Configuration

The app stores its config under the XDG project directory, typically:

- `~/.config/open-onedrive/config.toml`
- `~/.config/open-onedrive/rclone/rclone.conf`
- `~/.local/state/open-onedrive/runtime-state.toml`
- `~/.local/state/open-onedrive/path-state.sqlite3`
- `~/.cache/open-onedrive/rclone/`

Example `config.toml`:

```toml
mount_path = "/home/you/OneDrive"
remote_name = "openonedrive"
cache_limit_gb = 10
auto_mount = true

# Optional overrides
# rclone_bin = "/usr/bin/rclone"
# custom_client_id = "your-microsoft-client-id"
```

Design guarantees:

- the wrapper never writes to `~/.config/rclone/rclone.conf`
- runtime state and path-state cache live under the app's own XDG surface area
- the dashboard, tray, Dolphin actions, and Dolphin overlays all resolve from the same daemon state
- `openonedrived --print-config` stays read-only when no config file exists

## UI Notes

- Setup focuses on picking an empty mount directory and starting the browser auth flow.
- Dashboard shows mount state, sync state, queue depth, cache size, pinned file count, last sync time, and recent diagnostics.
- Quick file controls in the dashboard let you apply residency changes without leaving the app.
- The tray icon mirrors mount/sync/error state and keeps the app resident after the window closes.
- Dolphin is both a browsing surface and a control surface: overlays show file residency, and context actions call the same daemon methods as the UI and CLI.

## How It Works

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture overview" width="100%">
</p>

- `openonedrived` owns runtime state, D-Bus methods, mount supervision, path-state caching, and residency policy.
- `rclone mount` provides the visible OneDrive tree in Dolphin and fetches file bytes on demand.
- `rclone lsjson` refreshes the SQLite path-state cache used by the tray, dashboard, CLI, and overlay plugin.
- Dolphin overlays query the daemon asynchronously and invalidate their local cache from daemon signals.
- Dolphin actions and `openonedrivectl` both call the daemon to hydrate or evict individual files.

## Project Layout

| Path | Purpose |
| --- | --- |
| `install.sh` | release-first `curl ... | bash` entrypoint |
| `scripts/install.sh` | developer source install path |
| `crates/openonedrived` | daemon entrypoint and D-Bus surface |
| `crates/openonedrivectl` | CLI for control, status, rescans, and path-state inspection |
| `crates/rclone-backend` | `rclone` discovery, mount supervision, cache policy, path-state cache, logs |
| `crates/config` | XDG paths, app config, mount path validation |
| `crates/ipc-types` | shared D-Bus status and path-state types |
| `crates/state` | persisted lightweight runtime state |
| `ui/` | Qt6/Kirigami shell and KDE tray integration |
| `integrations/` | Dolphin file actions and overlay plugin |
| `xtask/` | source-build automation used by developers |

## Non-goals

- no Windows Cloud Files placeholder parity or Finder-style virtual file APIs
- no GNOME/Nautilus support in this release
- no custom Microsoft OAuth stack
- no Graph delta sync engine or cross-desktop abstraction layer

## Development

Day-to-day repo commands:

```bash
./scripts/dev.sh bootstrap
./scripts/dev.sh up
./scripts/dev.sh status
./scripts/dev.sh test
```

Workspace tasks:

```bash
cargo run -p xtask -- check
cargo run -p xtask -- build-ui
cargo run -p xtask -- build-integrations
cargo run -p xtask -- install
```

## License

MIT. See [LICENSE](./LICENSE).
