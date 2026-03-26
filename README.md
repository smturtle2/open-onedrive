<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  <strong>OneDrive as a normal Linux folder.</strong><br/>
  Visible online-only files, on-demand hydration, per-file residency control, a simple dashboard, a minimal settings surface, and one daemon state shared by the app, tray, CLI, Dolphin, and Nautilus.
</p>

<p align="center">
  <a href="./README.ko.md">한국어</a> ·
  <a href="#highlights">Highlights</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#day-to-day">Day to Day</a> ·
  <a href="#how-it-works">How It Works</a> ·
  <a href="#development">Development</a>
</p>

<p align="center">
  <img src="./assets/docs/app-shell-screenshot.png" alt="open-onedrive showing the visible OneDrive folder workflow with dashboard and files surfaces" width="100%">
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/ci.yml?label=ci"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/release.yml"><img alt="Release" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/release.yml?label=release"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

> Stable releases target Linux `x86_64`. The project uses a custom FUSE filesystem for the visible OneDrive root. It does not use `rclone mount`.

## Overview

`open-onedrive` gives Linux a visible folder such as `~/OneDrive` and keeps online-only items visible before hydration.

The split is deliberate:

- `rclone` handles auth, remote listing, and upload/download primitives
- `openonedrived` owns the custom sync model, metadata cache, on-demand hydration, queueing, retry flow, and path state
- the Qt shell, separate tray helper, CLI, Dolphin plugins, and Nautilus extension all read the same daemon state

The result is a normal folder path for regular Linux apps, plus explicit `Keep on this device` and `Free up space` controls.

## Highlights

- visible online-only files and folders through a custom FUSE root
- on-demand downloads and queued uploads driven by the daemon, not by `rclone mount`
- per-file and per-folder residency control from the app, tray, CLI, Dolphin, and Nautilus
- simple Dashboard and Settings surfaces instead of a crowded control panel
- independent tray helper so the window can close without stopping background controls
- app-owned `rclone.conf` under XDG paths, isolated from your normal `~/.config/rclone/rclone.conf`
- one-line installer with checksum verification, existing-install upgrade checks, and automatic `rclone` install when missing

## Quick Start

Install the latest stable release:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

Install a pinned tag:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/YOUR_TAG/install.sh | bash
```

Build from source through the same bootstrap path:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_BUILD_FROM_SOURCE=1 bash
```

Skip interactive upgrade prompts in automation:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_ASSUME_YES=1 bash
```

The release installer:

- downloads the Linux release archive and SHA256 file
- detects an existing install and asks before interactive upgrades or reinstalls
- installs `rclone` automatically if it is missing
- installs the daemon, UI, tray helper, icon, launcher, user service, Dolphin integration, and Nautilus extension under `~/.local`
- refuses to replace an existing install non-interactively unless `OPEN_ONEDRIVE_ASSUME_YES=1` is set

Launch and verify:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

## Day to Day

First run:

1. Choose an empty visible folder such as `~/OneDrive` in `Settings`.
2. Finish the Microsoft browser sign-in started by `rclone`.
3. Start the filesystem if it is not already running.
4. Open `Files` to browse online-only and local items side by side.
5. Use `Keep on device` or `Free up space` from the app, tray, Dolphin, Nautilus, or CLI.

Main surfaces:

- `Dashboard`: compact status, queue, storage, and the next recommended action
- `Files`: the main workspace for residency changes and online-only visibility
- `Settings`: folder path, connect, repair, restart, and disconnect
- `Logs`: recent daemon and `rclone` output for recovery work
- `Tray`: independent helper that stays resident after the window closes

CLI equivalents:

```bash
openonedrivectl set-root-path ~/OneDrive
openonedrivectl start-filesystem
openonedrivectl keep-local ~/OneDrive/Documents/report.pdf
openonedrivectl make-online-only ~/OneDrive/Documents/report.pdf
openonedrivectl retry-transfer ~/OneDrive/Documents/report.pdf
openonedrivectl list-directory Docs
openonedrivectl refresh-directory Docs
openonedrivectl search-paths report --limit 20
openonedrivectl path-states ~/OneDrive/Documents/report.pdf
```

Project state lives under XDG paths, typically:

- `~/.config/open-onedrive/config.toml`
- `~/.config/open-onedrive/rclone/rclone.conf`
- `~/.local/share/open-onedrive/install-metadata.env`
- `~/.local/state/open-onedrive/runtime-state.toml`
- `~/.local/state/open-onedrive/path-state.sqlite3`

## Supported Scope

| Area | Status |
| --- | --- |
| OS / arch | Linux `x86_64` |
| Visible root | custom FUSE path managed by `openonedrived` |
| OneDrive backend | `rclone` auth, list, upload, and download primitives |
| Native file manager integration | `Dolphin` and `Nautilus` |
| UI surface | Qt shell plus separate tray helper |
| Stable installer target | user-local install under `~/.local` |

Current non-goals:

- `rclone mount`
- native integrations beyond `Dolphin` and `Nautilus`
- Windows Cloud Files placeholder parity
- custom Microsoft OAuth stack
- automatic cache eviction

## How It Works

- the daemon owns one serialized action queue so `rclone` work stays isolated from the UI process
- `rclone lsjson --hash` refreshes remote metadata without hydrating file bytes
- `rclone copyto` downloads cold files on first open and uploads dirty local writes
- path state is persisted so online-only, local, conflict, and error status stay visible across the shell, tray, CLI, and file-manager integrations
- hydrated bytes live in a hidden backing directory inside the visible root while the visible tree stays clean

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
- Dolphin actions or overlays are missing: run `kbuildsycoca6`, restart Dolphin, and verify plugins under `~/.local/lib/qt6/plugins/kf6/`.
- Nautilus actions or emblems are missing: confirm `nautilus-python` is installed, then restart Nautilus.
- sync is paused or degraded: on-demand opens still work, but dirty local writes stay queued until you resume sync.

## License

MIT. See [LICENSE](./LICENSE).
