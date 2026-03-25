# open-onedrive

OneDrive desktop shell for Linux that supervises `rclone mount` instead of implementing its own sync engine.

[![Platform](https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white)](https://kde.org/plasma-desktop/)
[![Rust](https://img.shields.io/badge/core-Rust-black?logo=rust)](https://www.rust-lang.org/)
[![Qt6](https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt)](https://www.qt.io/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[Korean README](./README.ko.md) · [Install](#install) · [Architecture](#architecture)

## What It Is

`open-onedrive` now acts as a wrapper around `rclone`:

- user-selectable host mount path
- browser-based Microsoft sign-in delegated to `rclone`
- app-owned `rclone.conf` under XDG config
- daemon-managed `rclone mount` foreground child with restart backoff
- Qt/Kirigami shell for setup, dashboard, and recent logs
- lightweight Dolphin mount actions

This is a breaking product pivot. The old direct Microsoft OAuth, Graph delta sync, SQLite item index, and custom FUSE/VFS engine are no longer the product model.

## Current Scope

- first wrapper release targets OneDrive Personal
- `rclone` is a hard runtime dependency
- the app never touches `~/.config/rclone/rclone.conf`
- mount caching is handled through `rclone` VFS cache
- per-file pin/evict, overlay state, and placeholder badges are out of scope for this release

Legacy direct-engine state is discarded on startup. Old config compatibility is not preserved.

## Install

If `rclone` is missing, the installer now tries to install it automatically with a supported system package manager first, then falls back to the official `rclone` install script. This may prompt for `sudo`.

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./scripts/install.sh
```

After install:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

The daemon stores its dedicated remote configuration at:

```text
~/.config/open-onedrive/rclone/rclone.conf
```

## Architecture

- `crates/openonedrived`: daemon entrypoint and D-Bus surface
- `crates/openonedrivectl`: debug CLI for the daemon D-Bus interface
- `crates/config`: XDG paths, wrapper config, mount path validation
- `crates/ipc-types`: shared D-Bus status types
- `crates/rclone-backend`: rclone binary discovery, config ownership, mount supervision, log capture
- `crates/state`: persisted lightweight runtime state
- `ui/`: Qt6/Kirigami shell
- `integrations/`: Dolphin mount actions
- `packaging/`: desktop entry, launcher, and user service templates
- `xtask/`: build, install, and dependency checks

## Repository Commands

```bash
cargo run -p xtask -- bootstrap
cargo run -p xtask -- check
cargo run -p xtask -- test
cargo run -p xtask -- build-ui
cargo run -p xtask -- build-integrations
cargo run -p xtask -- install
```
