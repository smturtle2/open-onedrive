<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  <strong>OneDrive as a normal Linux folder.</strong><br/>
  Visible online-only files, on-demand hydration, per-file residency control, a files-first shell, and one daemon state shared by the app, tray, CLI, Dolphin, and Nautilus.
</p>

<p align="center">
  <a href="./README.ko.md">한국어</a> ·
  <a href="#highlights">Highlights</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#everyday-use">Everyday Use</a> ·
  <a href="#development">Development</a>
</p>

<p align="center">
  <img src="./assets/docs/app-shell-screenshot.png" alt="open-onedrive showing the current dashboard, files view, and simple settings flow" width="100%">
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/ci.yml?label=ci"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/release.yml"><img alt="Release" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/release.yml?label=release"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

> Stable releases target Linux `x86_64`. The visible OneDrive root is provided by a custom FUSE filesystem owned by `openonedrived`. This project does not use `rclone mount`.

## Highlights

- online-only files and folders stay visible before hydration
- `Keep on this device` and `Free up space` work from the app, tray, CLI, Dolphin, and Nautilus
- `Files` is the main workspace; `Dashboard` stays compact and `Settings` stays intentionally small
- tray runs independently so background control survives after the window closes
- app-owned `rclone.conf` is isolated from your regular `~/.config/rclone/rclone.conf`
- the installer supports one-line install, upgrade checks, checksum verification, and `rclone` bootstrap when missing

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

The installer downloads the release payload, verifies SHA256, checks for an existing install before upgrading, and installs `rclone` automatically when it is missing.

Launch and verify:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

## Everyday Use

First run:

1. Open `Settings` and choose an empty visible folder such as `~/OneDrive`.
2. Finish the browser sign-in started by `rclone`.
3. Open `Files` and browse online-only and local items in the same folder tree.
4. Use `Keep on this device` or `Free up space` from the app, tray, Dolphin, Nautilus, or CLI.

Main surfaces:

- `Dashboard`: short status, queue summary, and the next action
- `Files`: browsing, search, and residency changes
- `Settings`: folder path, connect, repair, disconnect
- `Logs`: recent daemon and `rclone` output
- `Tray`: separate helper for background control

File manager integration:

- `Dolphin` is the primary stable target for overlays and context actions
- `Nautilus` remains available for actions and emblems
- right click actions expose `Keep on this device`, `Free up space`, and retry flows
- overlay states distinguish online-only, local, syncing, and attention states

## How It Works

- `rclone` handles auth, remote listing, and upload or download primitives
- `openonedrived` owns the custom sync model, metadata cache, path state, and serialized action queue
- hydrated bytes live in a hidden backing directory while the visible tree stays clean
- the Qt shell, tray helper, CLI, Dolphin plugin, and Nautilus extension all read the same daemon state

## Development

Day-to-day commands:

```bash
./scripts/dev.sh bootstrap
./scripts/dev.sh up
./scripts/dev.sh test
```

Workspace tasks:

```bash
cargo run -p xtask -- check
cargo run -p xtask -- build-ui
cargo run -p xtask -- build-integrations
```

## Troubleshooting

- `Daemon not reachable on D-Bus`: run `open-onedrive` once, or check `systemctl --user status openonedrived.service`.
- filesystem startup fails: confirm `/dev/fuse` exists and `fusermount3` or `mount.fuse3` is available in `PATH`.
- Dolphin overlays or actions are missing: run `kbuildsycoca6`, restart Dolphin, and verify the plugin install under `~/.local/lib/qt6/plugins/kf6/`.
- Nautilus actions or emblems are missing: confirm `nautilus-python` is installed, then restart Nautilus.

## License

MIT. See [LICENSE](./LICENSE).
