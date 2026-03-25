<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="128">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  OneDrive desktop shell for Linux that supervises <code>rclone mount</code>.
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

<p align="center">
  <a href="./README.ko.md">한국어</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#highlights">Highlights</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#development">Development</a>
</p>

<p align="center">
  <img src="./assets/docs/dashboard-hero.svg" alt="open-onedrive dashboard preview" width="100%">
</p>

## Overview

`open-onedrive` is a Linux desktop wrapper around `rclone`, not a custom sync engine. It owns an app-specific `rclone.conf`, supervises the foreground mount process through a daemon, and exposes a Qt6/Kirigami shell plus a small D-Bus CLI for status, recovery, and diagnostics.

## Quick Start

Install from GitHub with one command:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

Install from a cloned checkout:

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./install.sh
```

Launch and confirm the daemon:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

If `rclone` is missing, the installer first tries a supported system package manager and then falls back to the official `rclone` install script. That step may prompt for `sudo`.

## Highlights

- one-command bootstrap with `curl | bash` or local `./install.sh`
- app-owned `rclone.conf` under `~/.config/open-onedrive/rclone/rclone.conf`
- daemon-managed `rclone mount` with restart backoff and recent log capture
- dashboard recovery flow that keeps logs visible when mount errors occur
- Qt6/Kirigami UI plus `openonedrivectl` for D-Bus status and control
- lightweight Dolphin action plugin for KDE Plasma

## What It Manages

- `openonedrived` owns runtime state, D-Bus methods, mount supervision, and restart policy.
- `rclone` owns Microsoft sign-in, mount execution, and VFS cache behavior.
- the UI and `openonedrivectl` talk to the daemon over the session bus.
- the wrapper never writes to the user's default `~/.config/rclone/rclone.conf`.

## Preview

The dashboard is built for operational clarity: current mount state, mount path, cache size, quick recovery actions, and readable logs stay in one place.

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture flow" width="100%">
</p>

## Architecture

| Layer | Responsibility |
| --- | --- |
| `install.sh` | bootstrap from GitHub or a local checkout |
| `xtask` | build, install, and desktop integration automation |
| `openonedrived` | runtime state, D-Bus surface, mount supervision |
| `rclone-backend` | `rclone` discovery, config ownership, logs, retries |
| `openonedrivectl` | CLI access to daemon methods and status |
| `ui/` | Qt6/Kirigami shell for setup, dashboard, and logs |
| `integrations/` | Dolphin action plugin |

## Config

`config.toml` is intentionally small. Typical fields:

```toml
mount_path = "/home/you/OneDrive"
remote_name = "openonedrive"
cache_limit_gb = 10
auto_mount = true

# Optional manual overrides
# rclone_bin = "/usr/bin/rclone"
# custom_client_id = "..."
```

## Project Layout

| Path | Purpose |
| --- | --- |
| `crates/openonedrived` | daemon entrypoint and D-Bus surface |
| `crates/openonedrivectl` | debug CLI for daemon control and status |
| `crates/rclone-backend` | `rclone` discovery, config ownership, mount supervision, logs |
| `crates/config` | XDG paths, app config, mount path validation |
| `crates/ipc-types` | shared D-Bus status types |
| `crates/state` | persisted runtime state |
| `ui/` | Qt6/Kirigami shell |
| `integrations/` | Dolphin actions |
| `packaging/` | launcher, desktop entry, and user service templates |
| `xtask/` | bootstrap, build, test, and install automation |

## Non-goals

- no custom Microsoft OAuth, Graph delta sync, SQLite item index, or in-house FUSE/VFS engine
- no writes to the user's default `~/.config/rclone/rclone.conf`
- no per-file pin or evict, placeholder badges, or overlay state in this release

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
