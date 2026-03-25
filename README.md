<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  A Linux desktop shell for OneDrive where <code>rclone mount</code> supplies the remote tree and file bytes, while the wrapper owns device residency.
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
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

`open-onedrive` is a Linux desktop wrapper around `rclone`, not a sync engine. The daemon owns an app-specific `rclone.conf`, supervises `rclone mount`, and keeps the file manager, dashboard, CLI, and cache policy aligned. `rclone` is used for the mounted directory tree and file bytes. The wrapper decides which files stay on the device and which ones return to online-only mode.

## Highlights

- `curl ... | bash` bootstrap that downloads the repo and runs a local source build
- app-owned `rclone.conf` under XDG paths, isolated from `~/.config/rclone/rclone.conf`
- daemon-managed `rclone mount` with restart backoff, recent log capture, and cache accounting
- per-file `Keep on this device` / `Make online-only` actions from Dolphin
- wrapper-managed file residency on top of the app-owned `rclone` VFS cache
- Qt6/Kirigami shell plus `openonedrivectl` for diagnostics and file residency actions

## Quick Start

Install directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

Pin a specific tag or branch:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_REF=v0.1.0 bash
```

What the bootstrap actually does:

- downloads the repository payload into a temporary directory
- installs `rclone` automatically if it is missing
- runs a local source build for the Rust workspace, Qt shell, and KDE integration plugin
- installs the launcher, desktop entry, icon, and `openonedrived.service`
- removes the temporary checkout after installation finishes

Build prerequisites for the `curl | bash` flow:

- `cargo`
- `cmake` or `qt-cmake`
- `ninja` or `make`
- `pkg-config`
- `qml`
- Qt6 and KF6/Kirigami development packages suitable for your distro

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
4. Right-click files and use `Keep on this device` or `Make online-only`.

CLI equivalents:

```bash
openonedrivectl keep-local ~/OneDrive/Documents/report.pdf
openonedrivectl make-online-only ~/OneDrive/Documents/report.pdf
```

## Requirements

- Linux desktop environment
- `rclone` as a runtime dependency
- first-class target: KDE Plasma 6 with Qt6/Kirigami and Dolphin
- local build toolchain for the bootstrap flow: Rust, CMake, Qt tooling, and a C++ compiler
- current wrapper flow targets OneDrive Personal

## Configuration

The app stores its config under the XDG project directory, typically:

- `~/.config/open-onedrive/config.toml`
- `~/.config/open-onedrive/rclone/rclone.conf`
- `~/.local/state/open-onedrive/runtime-state.toml`
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
- runtime state is persisted separately from the user-facing config
- pinned residency state lives with the wrapper runtime state, not in the user's default `rclone` surface area
- `openonedrived --print-config` stays read-only when no config file exists

## UI Notes

- Setup focuses on picking an empty mount directory and starting the browser auth flow.
- Dashboard keeps mount controls, cache size, pinned file count, and diagnostics in one place.
- Logs stay available during recovery, so retry and inspection do not drop back to setup.
- Dolphin is the per-file control surface: right-click mounted items to keep them on the device or return them to online-only mode.

## How It Works

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture overview" width="100%">
</p>

- `openonedrived` owns runtime state, D-Bus methods, mount supervision, and residency policy.
- `rclone mount` provides the visible remote tree in the file manager and fetches file bytes on demand.
- the wrapper records pinned files and prunes the app-owned VFS cache back to that pinned set.
- Dolphin actions and `openonedrivectl` both call the daemon to hydrate or evict individual files.
- the app stays inside its own XDG-owned surface area instead of sharing the user's default `rclone` config.

## Project Layout

| Path | Purpose |
| --- | --- |
| `install.sh` | bootstrap entrypoint for `curl ... | bash` |
| `crates/openonedrived` | daemon entrypoint and D-Bus surface |
| `crates/openonedrivectl` | debug CLI for daemon control, status, and residency actions |
| `crates/rclone-backend` | `rclone` discovery, config ownership, mount supervision, cache policy, logs |
| `crates/config` | XDG paths, app config, mount path validation |
| `crates/ipc-types` | shared D-Bus status types |
| `crates/state` | persisted lightweight runtime state |
| `ui/` | Qt6/Kirigami shell |
| `integrations/` | Dolphin file actions |
| `packaging/` | launcher, desktop entry, and user service templates |
| `xtask/` | bootstrap, build, test, and install automation |

## Non-goals

- no custom Microsoft OAuth, Graph delta sync, or in-house sync engine
- no writes to the user's default `~/.config/rclone/rclone.conf`
- no Finder-style placeholder badges or cloud overlay icons in this release
- no legacy direct-engine compatibility layer

Legacy direct-engine state is discarded on startup.

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
