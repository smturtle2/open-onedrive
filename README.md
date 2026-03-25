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
  <a href="#highlights">Highlights</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#how-it-works">How It Works</a> ·
  <a href="#development">Development</a>
</p>

## Overview

`open-onedrive` is a Linux desktop wrapper around `rclone`, not a custom sync engine. It owns an app-specific `rclone.conf`, supervises the foreground mount process through a daemon, and exposes a Qt6/Kirigami shell plus a small D-Bus CLI for status and recovery tasks.

## Highlights

- interactive mount directory selection from setup and dashboard
- browser-based Microsoft sign-in delegated to `rclone`
- dedicated config at `~/.config/open-onedrive/rclone/rclone.conf`
- daemon-managed `rclone mount` with restart backoff and recent log capture
- Qt6/Kirigami desktop UI with lightweight Dolphin actions
- `openonedrivectl` CLI for status, logs, and manual mount control

## Requirements

- Linux desktop environment
- `rclone` as a hard runtime dependency
- first-class target: KDE Plasma 6 with Qt6/Kirigami
- current wrapper release targets OneDrive Personal

## Quick Start

Install from the repository:

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./scripts/install.sh
```

Launch the app and confirm the daemon:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

If `rclone` is missing, the installer first tries a supported system package manager and then falls back to the official `rclone` install script. That step may prompt for `sudo`.

## How It Works

- `openonedrived` owns runtime state, D-Bus methods, and mount supervision.
- `rclone` handles Microsoft auth, mount execution, and VFS cache behavior.
- the UI and `openonedrivectl` both talk to the daemon over the session bus.
- the app never touches `~/.config/rclone/rclone.conf`; it only uses its own config under XDG.

## Project Layout

| Path | Purpose |
| --- | --- |
| `crates/openonedrived` | daemon entrypoint and D-Bus surface |
| `crates/openonedrivectl` | debug CLI for daemon control and status |
| `crates/rclone-backend` | `rclone` discovery, config ownership, mount supervision, logs |
| `crates/config` | XDG paths, app config, mount path validation |
| `crates/ipc-types` | shared D-Bus status types |
| `crates/state` | persisted lightweight runtime state |
| `ui/` | Qt6/Kirigami shell |
| `integrations/` | Dolphin actions |
| `packaging/` | launcher, desktop entry, and user service templates |
| `xtask/` | bootstrap, build, test, and install automation |

## Non-goals

- no custom Microsoft OAuth, Graph delta sync, SQLite item index, or in-house FUSE/VFS engine
- no writes to the user's default `~/.config/rclone/rclone.conf`
- no per-file pin or evict, placeholder badges, or overlay state in this release
- no compatibility layer for legacy direct-engine state

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
