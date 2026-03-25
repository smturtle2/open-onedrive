<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  A Linux desktop shell for OneDrive that supervises <code>rclone mount</code> instead of reinventing sync.
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

`open-onedrive` is a Linux desktop wrapper around `rclone`, not a custom sync engine. It owns an app-specific `rclone.conf`, supervises the foreground mount process through a daemon, exposes a Qt6/Kirigami shell plus a D-Bus CLI, and keeps recovery actions visible when the mount fails.

## Highlights

- one-line bootstrap with `curl ... | bash`
- app-owned `rclone.conf` under XDG paths, isolated from `~/.config/rclone/rclone.conf`
- daemon-managed `rclone mount` with restart backoff and recent log capture
- dashboard and logs that stay available during recovery
- Qt6/Kirigami shell plus `openonedrivectl` for diagnostics
- Dolphin integration for lightweight KDE desktop actions

## Quick Start

Install directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

Pin a specific branch or tag:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_REF=main bash
```

What the installer does:

- downloads the repository payload into a temporary directory
- installs `rclone` automatically if it is missing
- builds the Rust workspace, Qt shell, and KDE integrations
- installs the launcher, desktop entry, icon, and `openonedrived.service`

Launch and verify:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

Source-based local install still works:

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./scripts/install.sh
```

## Requirements

- Linux desktop environment
- `rclone` as a hard runtime dependency
- first-class target: KDE Plasma 6 with Qt6/Kirigami
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
- `openonedrived --print-config` stays read-only when no config file exists

## UI Notes

- Setup focuses on choosing a mount directory and launching the browser auth flow.
- Dashboard keeps mount controls, latest status, and diagnostics in one place.
- Logs remain reachable during recovery, so retry and inspection happen without dropping back to setup.

## How It Works

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture overview" width="100%">
</p>

- `openonedrived` owns runtime state, D-Bus methods, and mount supervision.
- `rclone` handles Microsoft auth, mount execution, and VFS cache behavior.
- the UI and `openonedrivectl` both talk to the daemon over the session bus.
- the app stays inside its own XDG-owned surface area instead of sharing the user's default `rclone` config.

## Project Layout

| Path | Purpose |
| --- | --- |
| `install.sh` | bootstrap entrypoint for `curl ... | bash` |
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
- no placeholder badges, per-file pin or evict, or overlay state in this release
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
