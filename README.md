# open-onedrive

Windows OneDrive-like OSS client for Linux, built for KDE Plasma 6 and Wayland.

[![Platform](https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white)](https://kde.org/plasma-desktop/)
[![Rust](https://img.shields.io/badge/core-Rust-black?logo=rust)](https://www.rust-lang.org/)
[![Qt6](https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt)](https://www.qt.io/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[Korean README](./README.ko.md) · [Install](#install) · [Architecture](#architecture)

`open-onedrive` aims to feel like the Windows OneDrive desktop client on a modern Linux desktop:

- User-configurable OneDrive mount path
- Rust daemon with D-Bus control surface
- Placeholder-style FUSE filesystem
- Qt/Kirigami desktop shell
- Dolphin context actions and overlay plugins
- User-local install with desktop launcher and systemd user service

## Status

This repository now builds and installs in the current environment.

Working today:

- `cargo check --workspace`
- `cargo test --workspace`
- Microsoft browser login callback + token persistence
- Graph `drive/root/delta` indexing into the SQLite metadata store
- FUSE mount populated from real remote metadata
- On-demand file download when reading a mounted file
- `./scripts/install.sh` user-local install, app launcher, and systemd user service

Current scope of the implementation:

- Real Microsoft Graph metadata sync and periodic polling
- Read-only FUSE tree with on-demand file hydration and local cache
- D-Bus methods for login, mount path updates, pin/evict, status, and item lookup
- Qt shell wired to daemon status over D-Bus with live refresh
- Dolphin action and overlay plugins installable under `~/.local`

Still in progress:

- Writable sync and upload path
- Richer desktop polish around notifications, tray UX, and full error recovery

## Install

Install locally and register the app in one command:

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./scripts/install.sh
```

What this does:

- builds the Rust daemon and CLI
- builds the Qt desktop shell
- builds the Dolphin integration plugins
- installs everything into `~/.local`
- registers `open-onedrive` in the desktop app menu
- installs and enables `openonedrived.service` with `systemctl --user`

After install:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

For development:

```bash
./scripts/dev.sh bootstrap
./scripts/dev.sh up
./scripts/dev.sh install
```

## Architecture

The repo is split by responsibility:

- `crates/openonedrived`: daemon entrypoint, app lifecycle, D-Bus service, mount control
- `crates/openonedrivectl`: developer CLI for the daemon D-Bus interface
- `crates/config`: XDG paths, config load/save, mount path validation
- `crates/state`: SQLite metadata store for auth, delta cursors, and indexed items
- `crates/vfs`: FUSE filesystem snapshot layer and content provider hook
- `crates/auth`: Microsoft auth URL, PKCE, token exchange, token refresh
- `crates/graph`: Microsoft Graph delta/content client
- `ui/`: Qt6/Kirigami desktop shell
- `integrations/`: Dolphin context action and overlay plugins
- `packaging/`: desktop entry, launcher, and user service templates
- `xtask/`: build/bootstrap helpers

## Repository Commands

```bash
cargo run -p xtask -- bootstrap
cargo run -p xtask -- check
cargo run -p xtask -- test
cargo run -p xtask -- build-ui
cargo run -p xtask -- build-integrations
cargo run -p xtask -- install
```

## Goal

The long-term goal is not just “sync a folder.” It is to deliver a Linux-native client that gets close to the Windows OneDrive experience:

- background daemon
- tray/settings UI
- mount-path selection from setup UI
- Files On-Demand-like placeholder behavior
- Dolphin right-click actions
- state overlays
- one-command local install and app registration

If you are building on the same environment as this repo, the current codebase is ready to extend from here.
