<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  Stable 0.1.2 for KDE Plasma 6 and Dolphin: a Linux OneDrive client with a real local root folder, transparent on-demand hydration, per-file keep-local or online-only control, Dolphin overlays, and a tray/dashboard shell.
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/ci.yml?label=ci"></a>
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

> `v0.1.2` is the first stable, supported release line for `KDE Plasma 6 + Dolphin` on Linux. The release focuses on startup freshness, immediate state propagation after local file operations, and a simpler user-local install flow.

## Overview

`open-onedrive` targets `KDE Plasma 6 + Dolphin` on Linux and exposes OneDrive through a custom FUSE filesystem rooted at a normal folder such as `~/OneDrive`.

It does **not** use `rclone mount`.

Instead:

- `rclone` handles remote auth, directory listing, and file transfer primitives
- the daemon owns the on-demand filesystem, transfer queue, path-state cache, conflicts, tray state, and Dolphin integration
- hydrated bytes live in an app-managed hidden backing directory inside the visible root, defaulting to `.openonedrive-cache`

This gives regular applications a normal local path while still supporting Windows-style per-file availability controls.

## Highlights

- visible root folder like `~/OneDrive` backed by a custom FUSE filesystem
- transparent on-demand hydration for normal Linux apps, not only KDE apps
- per-file `Keep on this device` and `Make online-only`
- app-owned `rclone.conf` under XDG paths, isolated from `~/.config/rclone/rclone.conf`
- SQLite-backed path-state cache plus queued upload and download operations
- Dolphin overlay icons and context actions
- Qt6/Kirigami dashboard with filesystem state, queue depth, conflicts, logs, and tray controls
- stable `curl ... | bash` installer and GitHub Release artifacts

## Quick Start

Install the latest release directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

Install a specific tag:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_REF=v0.1.2 bash
```

Force the source-build bootstrap path:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_BUILD_FROM_SOURCE=1 bash
```

What the release installer does:

- downloads the Linux `x86_64` release archive and SHA256 file
- verifies the archive before extracting it
- installs binaries, KDE plugins, icon, launcher, and the user service into your home directory
- installs `rclone` automatically if it is missing
- warns when FUSE 3 runtime support is missing
- enables `openonedrived.service` for the current user when `systemd --user` is available

Launch and verify:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

Typical first run:

1. Choose a root folder such as `~/OneDrive`.
2. Finish the Microsoft browser sign-in started by `rclone`.
3. Start the filesystem from the app if it is not already running.
4. Open the visible root folder in Dolphin, a terminal, LibreOffice, VS Code, or another normal app.
5. Keep selected files local or return them to online-only mode from the dashboard, tray, CLI, or Dolphin actions.

CLI equivalents:

```bash
openonedrivectl set-root-path ~/OneDrive
openonedrivectl start-filesystem
openonedrivectl keep-local ~/OneDrive/Documents/report.pdf
openonedrivectl make-online-only ~/OneDrive/Documents/report.pdf
openonedrivectl retry-transfer ~/OneDrive/Documents/report.pdf
openonedrivectl path-states ~/OneDrive/Documents/report.pdf
```

## Requirements

- Linux `x86_64`
- `rclone`
- FUSE 3 runtime support with `/dev/fuse`
- first-class target: `KDE Plasma 6` with `Dolphin`
- release installer target: user-local install under `~/.local`
- source build path: Rust, CMake, Qt6 tooling, KF6 development packages, fuse3 development packages, and a C++ compiler
- supported file-manager integration in this release: `Dolphin`

## Configuration

The app stores its config under the XDG project directory, typically:

- `~/.config/open-onedrive/config.toml`
- `~/.config/open-onedrive/rclone/rclone.conf`
- `~/.local/state/open-onedrive/runtime-state.toml`
- `~/.local/state/open-onedrive/path-state.sqlite3`

Example `config.toml`:

```toml
root_path = "/home/you/OneDrive"
remote_name = "openonedrive"
cache_limit_gb = 10
auto_start_filesystem = true
backing_dir_name = ".openonedrive-cache"

# Optional overrides
# rclone_bin = "/usr/bin/rclone"
# custom_client_id = "your-microsoft-client-id"
# cache_limit_gb is reserved in 0.1.2 and is not enforced yet
```

Design guarantees:

- the wrapper never writes to `~/.config/rclone/rclone.conf`
- the visible root folder is for normal app access
- hydrated bytes are stored in the hidden backing directory, not directly in the daemon state directory
- the dashboard, tray, CLI, and Dolphin integrations resolve from the same daemon state
- the hidden backing directory is implementation detail and should not be edited by hand
- this stable release supports `KDE Plasma 6 + Dolphin` only

## UI Notes

- Setup focuses on picking a visible root folder and starting the browser auth flow.
- Dashboard shows connection state, filesystem state, sync state, pending transfers, conflicts, backing storage usage, pinned file count, and recent diagnostics.
- The tray mirrors filesystem, transfer, and error state and keeps the app resident after the window closes.
- Dolphin overlays and actions operate on the visible root path while ignoring the hidden backing directory.

## How It Works

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture overview" width="100%">
</p>

- `openonedrived` owns runtime state, D-Bus methods, the custom FUSE filesystem, queueing, conflicts, and residency policy.
- the visible root folder is mounted by the daemon itself, not by `rclone mount`
- `rclone lsjson` refreshes remote metadata
- `rclone copyto` downloads and uploads file content on demand
- a hidden backing directory keeps hydrated and pinned bytes on disk
- Dolphin overlays query daemon state and invalidate local caches from daemon signals

## Project Layout

| Path | Purpose |
| --- | --- |
| `install.sh` | release-first `curl ... | bash` entrypoint |
| `scripts/install.sh` | developer source install path |
| `crates/openonedrived` | daemon entrypoint and D-Bus surface |
| `crates/openonedrivectl` | CLI for control, status, rescans, and path-state inspection |
| `crates/rclone-backend` | custom FUSE sync engine, transfer queue, path-state cache, logs, and `rclone` primitives |
| `crates/config` | XDG paths, app config, and visible-root validation |
| `crates/ipc-types` | shared D-Bus status and path-state types |
| `crates/state` | persisted lightweight runtime state |
| `ui/` | Qt6/Kirigami shell and KDE tray integration |
| `integrations/` | Dolphin file actions and overlay plugin |
| `xtask/` | source-build automation used by developers |

## Non-goals

- `rclone mount`
- KIO-only browsing
- Windows Cloud Files placeholder parity
- GNOME/Nautilus support in this release
- custom Microsoft OAuth stack
- automatic cache eviction in `v0.1.2`

## Troubleshooting

- `Daemon not reachable on D-Bus`: start the app once with `open-onedrive`, or check `systemctl --user status openonedrived.service`.
- FUSE startup failures: confirm `/dev/fuse` exists and `fusermount3` or `mount.fuse3` is available in `PATH`.
- Dolphin actions or overlays missing after install: run `kbuildsycoca6`, restart Dolphin, and verify the plugins were copied under `~/.local/lib/qt6/plugins/kf6/`.
- Sync paused: on-demand reads still work, but dirty local writes stay queued until you resume sync from the dashboard, tray, or CLI.

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
