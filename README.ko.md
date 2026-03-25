<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="128">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  Linux에서 <code>rclone mount</code>를 감독하는 OneDrive 데스크톱 셸.
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="#quick-start">빠른 시작</a> ·
  <a href="#highlights">주요 기능</a> ·
  <a href="#architecture">아키텍처</a> ·
  <a href="#development">개발</a>
</p>

<p align="center">
  <img src="./assets/docs/dashboard-hero.svg" alt="open-onedrive dashboard preview" width="100%">
</p>

## Overview

`open-onedrive`는 자체 동기화 엔진이 아니라 `rclone` 위에 얹힌 Linux 데스크톱 wrapper입니다. 앱 전용 `rclone.conf`를 관리하고, foreground mount 프로세스를 daemon으로 감독하며, 상태 확인과 복구를 위한 Qt6/Kirigami UI와 작은 D-Bus CLI를 제공합니다.

## Quick Start

GitHub에서 한 줄로 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

저장소를 clone해서 설치:

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./install.sh
```

앱 실행과 daemon 확인:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

`rclone`이 없으면 installer가 먼저 지원되는 시스템 패키지 매니저를 시도하고, 실패하면 공식 `rclone` 설치 스크립트로 fallback 합니다. 이 단계에서 `sudo` 입력이 필요할 수 있습니다.

## Highlights

- `curl | bash` 또는 로컬 `./install.sh`로 바로 시작하는 설치 흐름
- `~/.config/open-onedrive/rclone/rclone.conf` 아래의 앱 전용 `rclone.conf`
- restart backoff와 최근 로그 수집이 포함된 daemon-managed `rclone mount`
- mount 오류 시에도 로그와 복구 액션을 유지하는 dashboard 흐름
- Qt6/Kirigami UI와 `openonedrivectl`를 통한 D-Bus 상태 조회 및 제어
- KDE Plasma용 가벼운 Dolphin 액션 플러그인

## What It Manages

- `openonedrived`가 runtime 상태, D-Bus 메서드, mount supervision, restart policy를 담당합니다.
- Microsoft 인증, mount 실행, VFS 캐시는 `rclone`이 처리합니다.
- UI와 `openonedrivectl`는 둘 다 세션 버스의 daemon과 통신합니다.
- wrapper는 사용자의 기본 `~/.config/rclone/rclone.conf`에 쓰지 않습니다.

## Preview

Dashboard는 운영 가시성에 초점을 둡니다. 현재 mount 상태, mount path, cache 크기, 복구 액션, 읽기 쉬운 로그를 한 화면에 유지합니다.

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture flow" width="100%">
</p>

## Architecture

| 계층 | 역할 |
| --- | --- |
| `install.sh` | GitHub 또는 로컬 checkout에서 부트스트랩 |
| `xtask` | build, install, desktop integration 자동화 |
| `openonedrived` | runtime state, D-Bus surface, mount supervision |
| `rclone-backend` | `rclone` 탐색, 설정 소유, 로그, 재시도 |
| `openonedrivectl` | daemon 메서드와 상태를 다루는 CLI |
| `ui/` | setup, dashboard, logs를 위한 Qt6/Kirigami 셸 |
| `integrations/` | Dolphin 액션 플러그인 |

## Config

`config.toml`은 의도적으로 작게 유지됩니다. 보통은 아래 정도만 다룹니다.

```toml
mount_path = "/home/you/OneDrive"
remote_name = "openonedrive"
cache_limit_gb = 10
auto_mount = true

# Optional manual overrides
# rclone_bin = "/usr/bin/rclone"
# custom_client_id = "..."
```

## Project Layout

| Path | 설명 |
| --- | --- |
| `crates/openonedrived` | daemon 진입점과 D-Bus 표면 |
| `crates/openonedrivectl` | daemon 제어와 상태 확인용 CLI |
| `crates/rclone-backend` | `rclone` 탐색, 설정 소유, mount 감독, 로그 |
| `crates/config` | XDG 경로, 앱 설정, mount path 검증 |
| `crates/ipc-types` | 공유 D-Bus 상태 타입 |
| `crates/state` | runtime 상태 저장 |
| `ui/` | Qt6/Kirigami 셸 |
| `integrations/` | Dolphin 액션 |
| `packaging/` | launcher, desktop entry, user service 템플릿 |
| `xtask/` | bootstrap, build, test, install 자동화 |

## Non-goals

- Microsoft OAuth 직접 구현, Graph delta sync, SQLite item index, 자체 FUSE/VFS 엔진 없음
- 사용자의 기본 `~/.config/rclone/rclone.conf`에 쓰지 않음
- 이번 릴리스에 per-file pin/evict, placeholder badge, overlay state 없음

legacy direct-engine 상태는 startup 시 삭제됩니다.

## Development

일상적인 개발 명령:

```bash
./scripts/dev.sh bootstrap
./scripts/dev.sh up
./scripts/dev.sh status
./scripts/dev.sh test
```

워크스페이스 작업:

```bash
cargo run -p xtask -- check
cargo run -p xtask -- build-ui
cargo run -p xtask -- build-integrations
cargo run -p xtask -- install
```

## License

MIT. 자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
