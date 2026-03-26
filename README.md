<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  <strong>OneDrive as a normal Linux folder.</strong><br/>
  Visible online-only files, on-demand hydration, per-file residency control, a simple settings window, and one daemon state shared by the app, tray, CLI, Dolphin, and Nautilus.
</p>

<p align="center">
  <a href="./README.ko.md">한국어</a> ·
  <a href="#highlights">Highlights</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#everyday-use">Everyday Use</a> ·
  <a href="#development">Development</a>
</p>

<p align="center">
  <img src="./assets/docs/app-shell-screenshot.png" alt="open-onedrive showing the compact settings and status window for the visible OneDrive folder" width="100%">
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
- `Keep on this device` and `Free up space` work from the CLI, Dolphin first, and Nautilus while the app and tray stay focused on setup plus background control
- the main window stays settings-first: folder path, daemon state, and essential sync controls only
- tray autostarts with your session, runs independently, and `Quit` shuts down the window, tray, and daemon together
- app-owned `rclone.conf` is isolated from your regular `~/.config/rclone/rclone.conf`
- the installer supports one-line install, upgrade checks, checksum verification, and `rclone` bootstrap when missing

## Quick Start

Install the stable release pinned by the current bootstrap script:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

Install a specific release tag through the same bootstrap path:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_REF=YOUR_TAG bash
```

Build from source through the same bootstrap path:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_INSTALL_MODE=source bash
```

Skip interactive upgrade prompts in automation:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_ASSUME_YES=1 bash
```

Preview the installer without changing the system:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_DRY_RUN=1 bash
```

The bootstrap script installs into `~/.local`, writes install metadata to `~/.local/share/open-onedrive/install-metadata.env`, refreshes the launcher, user service, tray autostart entry, Dolphin plugins, Nautilus extension, and icons, and enables `openonedrived.service` when `systemctl --user` is available. Release mode downloads `open-onedrive-linux-x86_64.tar.gz`, verifies SHA256, checks for an existing install before replacing it, and installs `rclone` automatically when it is missing. Source mode downloads a temporary source archive and runs `scripts/install.sh`. Upgrades stop the running daemon, tray, and UI before replacing files, so let active transfers finish first.

Installer layout:

- `~/.local/bin`: `open-onedrive`, `openonedrived`, `openonedrivectl`, `openonedrive-rclone-worker`
- `~/.local/lib/open-onedrive`: settings window and tray helper
- `~/.local/lib/qt6/plugins/kf6`: Dolphin action and overlay plugins
- `~/.local/share/nautilus-python/extensions/openonedrive.py`: Nautilus actions and emblems
- `~/.config/systemd/user/openonedrived.service` and `~/.config/autostart/io.github.smturtle2.OpenOneDriveTray.desktop`

Key installer environment variables:

| Variable | Purpose |
| --- | --- |
| `OPEN_ONEDRIVE_REF` | Release tag or source archive ref to install. |
| `OPEN_ONEDRIVE_INSTALL_MODE` | `release` (default) or `source`. |
| `OPEN_ONEDRIVE_BUILD_FROM_SOURCE` | Compatibility alias for `OPEN_ONEDRIVE_INSTALL_MODE=source` when set to `1`. |
| `OPEN_ONEDRIVE_ASSUME_YES` | Reinstall or replace an existing install without prompting. |
| `OPEN_ONEDRIVE_DRY_RUN` | Print commands and prompts without mutating the system. |
| `OPEN_ONEDRIVE_REPO` | Override the GitHub repo, useful for testing a fork. |
| `OPEN_ONEDRIVE_RELEASE_BASE_URL` | Override release asset downloads for mirrors or local CI smoke tests. |
| `OPEN_ONEDRIVE_SKIP_FUSE_CHECK` | Skip `/dev/fuse` and `fuse3` helper warnings in containers or CI. |

Launch and verify:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
openonedrivectl shutdown
```

## Everyday Use

First run:

1. Open the app window and choose a visible folder such as `~/OneDrive`. Existing populated folders can also be adopted with remote metadata taking priority, matching files kept as cache, and the rest discarded.
2. Finish the browser sign-in started by `rclone`.
3. Open the visible folder in Dolphin first, or Nautilus when needed, and browse online-only and local items in the same tree.
4. Use `Keep on this device` or `Free up space` from the file manager or CLI.

Main surfaces:

- `Window`: folder path, connect or repair, filesystem start or stop, pause or resume sync
- `Dolphin`: the primary workspace for residency actions and overlay states
- `Nautilus`: a shipped secondary workspace for actions and emblems
- `Tray`: separate helper for background control after the window closes
- `Quit` from the tray closes any open window and stops the daemon cleanly
- `CLI`: status checks and residency actions from scripts or terminals

File manager integration:

- `Dolphin` is the primary supported target for overlays and context actions
- `Nautilus` remains shipped for actions and emblems, with a narrower integration surface
- right click actions expose `Keep on this device`, `Free up space`, and retry flows
- overlay states distinguish online-only, local, syncing, and attention states

## How It Works

- `rclone` handles auth, remote listing, and upload or download primitives
- `openonedrived` owns the custom sync model, metadata cache, path state, and serialized action queue
- every `rclone` invocation runs through an isolated helper binary so long refreshes or transfers do not block the main daemon control path
- hydrated bytes live in a hidden backing directory while the visible tree stays clean
- the Qt shell stays settings-first, while the tray helper, CLI, Dolphin plugin, and Nautilus extension all read the same daemon state

## Development

Day-to-day commands:

```bash
./scripts/dev.sh bootstrap
./scripts/dev.sh up
./scripts/dev.sh test
```

`./scripts/dev.sh up` launches the UI directly for fast iteration. Use the installed `open-onedrive` launcher when you need to verify the separate tray helper path.

Source build prerequisites:

- Rust toolchain with `cargo`
- Qt 6 and KDE Frameworks 6 development packages used by the UI and tray
- `cmake`, `ninja` or `make`, `pkg-config`, and `fuse3`

Workspace tasks:

```bash
cargo run -p xtask -- check
cargo run -p xtask -- build-ui
cargo run -p xtask -- build-integrations
```

## Troubleshooting

- `Daemon not reachable on D-Bus`: run `open-onedrive` once, or check `systemctl --user status openonedrived.service`.
- filesystem startup fails: confirm `/dev/fuse` exists and `fusermount3` or `mount.fuse3` is available in `PATH`.
- `Personal Vault` may appear in OneDrive but is not currently listable through `rclone`; open-onedrive skips it during background scans instead of treating it as a fatal sync failure.
- Dolphin overlays or actions are missing: run `kbuildsycoca6`, restart Dolphin, and verify the plugin install under `~/.local/lib/qt6/plugins/kf6/`.
- Nautilus actions or emblems are missing: confirm `nautilus-python` is installed, then restart Nautilus.

## License

MIT. See [LICENSE](./LICENSE).
