<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  Stable <strong>v1.0.0</strong> for <strong>KDE Plasma 6 + Dolphin</strong>: a Linux OneDrive client that exposes a real local root folder, hydrates files on demand, lets you keep files on this device or return them to online-only, and keeps the daemon, tray, CLI, and Dolphin in sync.
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
  <a href="#supported-scope">Supported Scope</a> ·
  <a href="#how-it-works">How It Works</a> ·
  <a href="#development">Development</a>
</p>

<p align="center">
  <img src="./assets/docs/dashboard-hero.svg" alt="open-onedrive overview shell, logs, explorer actions, and tray" width="100%">
</p>

> `v1.0.0` is the first stable release line. The scope is intentionally narrow: Linux `x86_64`, `KDE Plasma 6`, and `Dolphin`. The goal is a reliable local-first OneDrive experience, not broad desktop coverage.

## Overview

`open-onedrive` gives Linux a visible OneDrive root such as `~/OneDrive` without using `rclone mount`.

Instead:

- `rclone` handles auth, remote listing, and upload/download primitives
- `openonedrived` owns the custom FUSE filesystem, on-demand hydration, upload queue, path-state cache, conflicts, and retry flow
- the Qt/Kirigami shell, tray, CLI, and Dolphin plugins all read the same daemon state

The result is a normal local path for regular Linux apps, with explicit per-file residency controls.

## Highlights

- visible root folder backed by a custom FUSE filesystem
- on-demand hydration for normal Linux apps, not only KDE apps
- per-file `Keep on this device` and `Make online-only`
- app-owned `rclone.conf` under XDG paths, isolated from `~/.config/rclone/rclone.conf`
- Dolphin overlays and file actions for residency control inside the visible root
- tray + overview shell + logs page + CLI, all backed by the same daemon state
- stable one-line installer with release archive verification and staged release smoke tests

## Supported Scope

| Area | Status |
| --- | --- |
| OS / arch | Linux `x86_64` |
| Desktop | `KDE Plasma 6` |
| File manager integration | `Dolphin` |
| OneDrive backend | `rclone` auth/list/upload/download primitives |
| Local filesystem model | custom FUSE mount owned by `openonedrived` |
| Stable installer target | user-local install under `~/.local` |

Non-goals for `v1.0.0`:

- `rclone mount`
- GNOME / Nautilus support
- KIO-only browsing
- Windows Cloud Files placeholder parity
- custom Microsoft OAuth stack
- automatic cache eviction

## Quick Start

Install the latest stable release:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

Install an exact tag with a fully pinned bootstrap path:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/v1.0.0/install.sh | bash
```

Install from source instead of release artifacts:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_BUILD_FROM_SOURCE=1 bash
```

What the release installer does:

- downloads the Linux release archive and SHA256 file
- verifies the archive before extracting it
- installs binaries, KDE plugins, icon, launcher, and user service into your home directory
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

1. Choose an empty visible root such as `~/OneDrive`.
2. Finish the Microsoft browser sign-in flow started by `rclone`.
3. Start the filesystem if it is not already running.
4. Open the visible root from Dolphin, a terminal, VS Code, LibreOffice, or another regular app.
5. Keep selected files local or return them to online-only mode from the overview shell, tray, CLI, or Dolphin actions.

## Day-to-Day Controls

CLI equivalents:

```bash
openonedrivectl set-root-path ~/OneDrive
openonedrivectl start-filesystem
openonedrivectl keep-local ~/OneDrive/Documents/report.pdf
openonedrivectl make-online-only ~/OneDrive/Documents/report.pdf
openonedrivectl retry-transfer ~/OneDrive/Documents/report.pdf
openonedrivectl path-states ~/OneDrive/Documents/report.pdf
```

Recovery surfaces:

- the overview shell keeps setup, control, and logs reachable even when the daemon needs attention
- tray notifications are reserved for actionable background errors
- Dolphin overlays invalidate from daemon signals rather than using a disconnected local cache

## Configuration

The app stores its own state under XDG project paths, typically:

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
# cache_limit_gb is reserved in v1.0.0 and is not enforced yet
```

Design guarantees:

- the wrapper never writes to `~/.config/rclone/rclone.conf`
- hydrated bytes live in the hidden backing directory inside the visible root
- the daemon, tray, CLI, and Dolphin integrations resolve from the same path-state view
- disconnecting removes only app-owned local state and backing bytes, not your online files in OneDrive

## How It Works

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture overview" width="100%">
</p>

- `openonedrived` owns runtime state, D-Bus methods, the custom FUSE mount, queueing, conflicts, and residency policy
- `rclone lsjson --hash` refreshes remote metadata and revision tokens
- `rclone copyto` downloads cold files on first open and uploads dirty local writes
- the hidden backing directory stores hydrated bytes while the visible root stays clean
- Dolphin overlays and actions operate on the visible root and ignore the hidden backing directory

## Why Not `rclone mount`?

Because this project needs wrapper-owned behavior that survives outside `rclone` itself:

- explicit per-file residency state
- unified daemon state for UI, tray, CLI, and Dolphin
- local retry and conflict handling around a visible root
- Linux app compatibility through a normal folder path, not a special browsing surface

## Development

Day-to-day commands:

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

## Troubleshooting

- `Daemon not reachable on D-Bus`: run `open-onedrive` once, or check `systemctl --user status openonedrived.service`.
- filesystem startup fails: confirm `/dev/fuse` exists and `fusermount3` or `mount.fuse3` is available in `PATH`.
- Dolphin actions or overlays are missing: run `kbuildsycoca6`, restart Dolphin, and verify the plugins under `~/.local/lib/qt6/plugins/kf6/`.
- sync is paused or degraded: on-demand reads still work, but dirty local writes stay queued until you resume sync.

## License

MIT. See [LICENSE](./LICENSE).
