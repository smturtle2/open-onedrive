<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  <strong>OneDrive as a normal Linux folder.</strong><br/>
  Visible online-only files, on-demand hydration, per-file and folder residency, an independent tray helper, and one daemon state shared by the shell, CLI, Dolphin, and Nautilus.
</p>

<p align="center">
  <a href="./README.ko.md">한국어</a> ·
  <a href="#highlights">Highlights</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#operator-surfaces">Operator Surfaces</a> ·
  <a href="#supported-scope">Supported Scope</a> ·
  <a href="#how-it-works">How It Works</a> ·
  <a href="#development">Development</a>
</p>

<p align="center">
  <img src="./assets/docs/app-shell-screenshot.png" alt="open-onedrive shell showing the Explorer empty-state guidance and left-rail workspace" width="100%">
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/ci.yml?label=ci"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/release.yml"><img alt="Release" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/release.yml?label=release"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

> Stable releases target Linux `x86_64`. Generic browse works through the custom FUSE path in terminals, editors, and regular Linux apps, while native file-manager actions are provided for `Dolphin` and `Nautilus`.

## Overview

`open-onedrive` gives Linux a visible OneDrive root such as `~/OneDrive` without using `rclone mount`.

Instead:

- `rclone` handles auth, remote listing, and upload/download primitives
- `openonedrived` owns the custom FUSE filesystem, metadata-backed online-only visibility, on-demand hydration, a serialized action queue, path-state cache, conflicts, and retry flow
- the Qt/Kirigami shell, independent tray helper, CLI, Dolphin plugins, and Nautilus extension all read the same daemon state

The result is a normal local path for regular Linux apps, with explicit file and folder residency controls.

## Highlights

- visible root folder backed by a custom FUSE filesystem
- online-only files and folders stay visible through metadata refreshes before hydration
- on-demand hydration for normal Linux apps, not only one desktop environment
- per-file and folder `Keep on this device` / `Make online-only`
- left-rail shell with dedicated Files, Activity, Setup, and Logs surfaces
- compact runtime inspector for queue depth, active work, backing usage, pinned files, and last sync state
- searchable in-app Files page with debounced whole-tree search, residency filters, explicit empty/error states, bulk actions, and row-level context menus
- structured logs with level, source, time, and a pinned latest issue for recovery work
- root-path moves carry the hidden hydrated backing store to the new root when it is safe to do so
- app-owned `rclone.conf` under XDG paths, isolated from `~/.config/rclone/rclone.conf`
- Dolphin overlays and file actions plus a Nautilus extension for residency control inside the visible root
- tray persistence through a separate helper process, with the daemon staying up even when the main window closes
- stable one-line installer with checksum-verified release archives, existing-install upgrade checks, fail-closed noninteractive upgrades, and release-workflow smoke tests for launcher and integration paths

## Operator Surfaces

- `Files`: the primary workspace for browsing online-only and local items together, filtering residency, and running `Keep on device`, `Free up space`, `Retry transfer`, and `Open/Browse` directly from rows or the selection bar
- `Activity`: a compact summary of queue depth, sync state, cache usage, and the next operator shortcut
- `Setup`: first-run connection, root-path edits, remote repair, and clean disconnect stay together
- `Logs`: search structured daemon and `rclone` output, switch between All / Attention / Transfers / Errors, and copy filtered recovery context
- `Tray`: a separate helper process keeps controls resident and can reopen the main window without depending on the window process staying alive
- `Dolphin` / `Nautilus`: native file-manager actions expose residency state from the visible root itself

## Supported Scope

| Area | Status |
| --- | --- |
| OS / arch | Linux `x86_64` |
| Generic browse surface | custom FUSE path for terminals, editors, office apps, and Linux file managers |
| Native file manager integration | `Dolphin` and `Nautilus` |
| UI surface | Qt/Kirigami shell plus separate tray helper |
| OneDrive backend | `rclone` auth/list/upload/download primitives |
| Local filesystem model | custom FUSE mount owned by `openonedrived` |
| Stable installer target | user-local install under `~/.local` |

Non-goals for the current stable line:

- `rclone mount`
- native integrations beyond `Dolphin` and `Nautilus`
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
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/YOUR_TAG/install.sh | bash
```

Install from source instead of release artifacts:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_BUILD_FROM_SOURCE=1 bash
```

Skip interactive upgrade prompts in automation:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_ASSUME_YES=1 bash
```

What the release installer does:

- downloads the Linux release archive and SHA256 file
- checks whether an existing install is present and prompts before interactive upgrades or reinstalls
- verifies the archive before extracting it
- installs binaries, the tray helper, file-manager integrations, icon, launcher, and user service into your home directory
- installs `rclone` automatically if it is missing
- warns when FUSE 3 runtime support is missing
- enables `openonedrived.service` for the current user when `systemd --user` is available
- writes install metadata under `~/.local/share/open-onedrive/install-metadata.env` for later upgrade checks
- refuses to replace an existing install non-interactively unless `OPEN_ONEDRIVE_ASSUME_YES=1` is set

Launch and verify:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

Typical first run:

1. Choose an empty visible root such as `~/OneDrive`.
2. Finish the Microsoft browser sign-in flow started by `rclone`.
3. Use the left-rail workspace shell to move between Files, Activity, Setup, and Logs while the current status stays visible.
4. Start the filesystem if it is not already running.
5. Open the visible root from Dolphin, Nautilus, a terminal, VS Code, LibreOffice, or another regular app.
6. Keep selected files local or return them to online-only mode from Files, the tray, the CLI, Dolphin actions, or Nautilus actions.

## Day-to-Day Controls

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

Recovery surfaces:

- the left-rail shell keeps Setup and Logs one click away while still surfacing the recommended next view
- the Files page exposes searchable path-state data with explicit unavailable/error/empty states plus bulk and row-level residency actions
- the logs page supports quick search plus filtered recovery work around structured daemon and `rclone` output
- tray notifications are reserved for actionable background errors, while the separate tray helper stays alive after the window closes
- Dolphin overlays and the Nautilus extension invalidate from daemon signals rather than using disconnected local caches

## Configuration

The app stores its own state under XDG project paths, typically:

- `~/.config/open-onedrive/config.toml`
- `~/.config/open-onedrive/rclone/rclone.conf`
- `~/.local/share/open-onedrive/install-metadata.env`
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
# cache_limit_gb is currently informational only; cache eviction stays manual
```

Design guarantees:

- the wrapper never writes to `~/.config/rclone/rclone.conf`
- hydrated bytes live in the hidden backing directory inside the visible root
- moving the visible root carries that hidden backing directory to the new root when the destination is safe
- the daemon, tray, CLI, Dolphin, and Nautilus integrations resolve from the same path-state view
- disconnecting removes only app-owned local state and backing bytes, not your online files in OneDrive

## How It Works
- `openonedrived` owns runtime state, D-Bus methods, the custom FUSE mount, one serialized action queue, conflicts, and residency policy
- `rclone lsjson --hash` refreshes remote metadata and revision tokens without hydrating file contents
- `rclone copyto` downloads cold files on first open and uploads dirty local writes
- targeted directory refreshes keep Files, Logs, Tray, Dolphin, and Nautilus in sync without depending only on full rescans
- the hidden backing directory stores hydrated bytes while the visible root stays clean
- Dolphin overlays, Nautilus emblems, and file actions operate on the visible root and ignore the hidden backing directory

## Why Not `rclone mount`?

Because this project needs wrapper-owned behavior that survives outside `rclone` itself:

- explicit per-file residency state
- unified daemon state for UI, tray, CLI, Dolphin, and Nautilus
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
- Nautilus actions or emblems are missing: confirm `nautilus-python` is installed, then restart Nautilus so it reloads `~/.local/share/nautilus-python/extensions/openonedrive.py`.
- sync is paused or degraded: on-demand reads still work, but dirty local writes stay queued until you resume sync.

## License

MIT. See [LICENSE](./LICENSE).
