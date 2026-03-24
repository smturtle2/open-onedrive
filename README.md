# open-onedrive

Windows OneDrive-like OSS client for Linux, built for KDE Plasma 6 and Wayland.

[![Platform](https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white)](https://kde.org/plasma-desktop/)
[![Rust](https://img.shields.io/badge/core-Rust-black?logo=rust)](https://www.rust-lang.org/)
[![Qt6](https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt)](https://www.qt.io/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[Korean README](./README.ko.md) · [Quick Start](#quick-start) · [Architecture](#architecture)

`open-onedrive` aims to feel like the Windows OneDrive desktop client on a modern Linux desktop:

- User-configurable OneDrive mount path
- Rust daemon with D-Bus control surface
- Placeholder-style FUSE filesystem
- Qt/Kirigami desktop shell
- Dolphin context actions and overlay plugins

## Status

This repository already builds in the current environment.

Working today:

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo run -p xtask -- bootstrap`
- `cargo run -p xtask -- build-ui`
- `cargo run -p xtask -- build-integrations`
- `openonedrived` + `openonedrivectl status` D-Bus round-trip

Current scope of the implementation:

- Read-only placeholder FUSE tree seeded from SQLite demo metadata
- D-Bus methods for login bootstrap, mount path updates, pin/evict, status, and item lookup
- Qt shell wired to daemon status over D-Bus
- Dolphin action and overlay plugins scaffolded and buildable

Still in progress:

- Real Microsoft Graph sync engine
- Real hydrate/download/upload path
- Live overlay state backed by real cloud metadata

## Quick Start

Development setup is intentionally two commands:

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./scripts/dev.sh bootstrap
./scripts/dev.sh up
```

What those commands do:

- `bootstrap`: verifies tools, builds the Rust workspace, builds the Qt shell, builds KDE integrations
- `up`: starts the daemon in the background and launches the desktop UI

Useful follow-ups:

```bash
./scripts/dev.sh status
./scripts/dev.sh test
./scripts/dev.sh daemon
```

## Architecture

The repo is split by responsibility:

- `crates/openonedrived`: daemon entrypoint, app lifecycle, D-Bus service, mount control
- `crates/openonedrivectl`: developer CLI for the daemon D-Bus interface
- `crates/config`: XDG paths, config load/save, mount path validation
- `crates/state`: SQLite metadata store and bootstrap placeholder tree
- `crates/vfs`: FUSE filesystem snapshot layer
- `crates/auth`: Microsoft auth URL + PKCE bootstrap
- `crates/graph`: Microsoft Graph client scaffolding
- `ui/`: Qt6/Kirigami desktop shell
- `integrations/`: Dolphin context action and overlay plugins
- `xtask/`: build/bootstrap helpers

## Repository Commands

```bash
cargo run -p xtask -- bootstrap
cargo run -p xtask -- check
cargo run -p xtask -- test
cargo run -p xtask -- build-ui
cargo run -p xtask -- build-integrations
```

## Goal

The long-term goal is not just “sync a folder.” It is to deliver a Linux-native client that gets close to the Windows OneDrive experience:

- background daemon
- tray/settings UI
- mount-path selection from setup UI
- Files On-Demand-like placeholder behavior
- Dolphin right-click actions
- state overlays

If you are building on the same environment as this repo, the current codebase is ready to extend from here.

