<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  자체 동기화 엔진 대신 <code>rclone mount</code>를 감독하는 Linux용 OneDrive 데스크톱 셸.
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="#highlights">주요 기능</a> ·
  <a href="#quick-start">빠른 시작</a> ·
  <a href="#configuration">설정</a> ·
  <a href="#how-it-works">동작 방식</a> ·
  <a href="#development">개발</a>
</p>

<p align="center">
  <img src="./assets/docs/dashboard-hero.svg" alt="open-onedrive dashboard preview" width="100%">
</p>

## Overview

`open-onedrive`는 Linux 데스크톱용 `rclone` wrapper입니다. 앱 전용 `rclone.conf`를 관리하고, foreground mount 프로세스를 daemon으로 감독하며, Qt6/Kirigami UI와 D-Bus CLI를 통해 상태 확인과 복구를 제공합니다. mount 오류가 발생해도 dashboard와 logs 화면을 계속 유지하는 쪽에 초점을 맞췄습니다.

## Highlights

- `curl ... | bash` 한 줄로 설치
- `~/.config/rclone/rclone.conf`와 분리된 앱 전용 `rclone.conf`
- restart backoff와 최근 로그 수집이 포함된 daemon-managed `rclone mount`
- 오류 상태에서도 유지되는 dashboard와 logs 기반 복구 흐름
- Qt6/Kirigami UI와 `openonedrivectl` CLI
- KDE Dolphin용 가벼운 연동 제공

## Quick Start

GitHub에서 바로 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

특정 브랜치나 태그 고정:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_REF=main bash
```

설치 스크립트가 하는 일:

- 저장소 payload를 임시 디렉터리로 내려받음
- `rclone`이 없으면 자동 설치 시도
- Rust workspace, Qt UI, KDE integration 빌드
- launcher, desktop entry, icon, `openonedrived.service` 설치
- 설치가 끝나면 임시 checkout 삭제

실행과 확인:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

## Requirements

- Linux 데스크톱 환경
- 필수 런타임 의존성인 `rclone`
- 주 타깃 환경: KDE Plasma 6 + Qt6/Kirigami
- 현재 wrapper 흐름은 OneDrive Personal 기준

## Configuration

앱 설정은 보통 다음 XDG 경로 아래에 저장됩니다.

- `~/.config/open-onedrive/config.toml`
- `~/.config/open-onedrive/rclone/rclone.conf`
- `~/.local/state/open-onedrive/runtime-state.toml`
- `~/.cache/open-onedrive/rclone/`

예시 `config.toml`:

```toml
mount_path = "/home/you/OneDrive"
remote_name = "openonedrive"
cache_limit_gb = 10
auto_mount = true

# Optional overrides
# rclone_bin = "/usr/bin/rclone"
# custom_client_id = "your-microsoft-client-id"
```

설계상 보장하는 점:

- wrapper는 `~/.config/rclone/rclone.conf`에 쓰지 않음
- runtime state는 사용자 설정 파일과 분리해 저장
- `openonedrived --print-config`는 설정 파일이 없어도 읽기 전용으로 동작

## UI Notes

- Setup은 mount 디렉터리 선택과 브라우저 인증 시작에 집중합니다.
- Dashboard는 mount 제어, 최신 상태, 진단 정보를 한 곳에 모읍니다.
- 오류가 나도 Logs 탭이 남아 있어서 setup으로 되돌아가지 않고 복구할 수 있습니다.

## How It Works

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture overview" width="100%">
</p>

- `openonedrived`가 runtime 상태, D-Bus 메서드, mount supervision을 담당합니다.
- Microsoft 인증, mount 실행, VFS cache는 `rclone`이 처리합니다.
- UI와 `openonedrivectl`는 둘 다 세션 버스의 daemon과 통신합니다.
- 앱은 사용자의 기본 `rclone` 설정을 공유하지 않고, 자기 XDG 경로 안에서만 동작합니다.

## Project Layout

| Path | 설명 |
| --- | --- |
| `install.sh` | `curl ... | bash`용 bootstrap 진입점 |
| `crates/openonedrived` | daemon 진입점과 D-Bus 표면 |
| `crates/openonedrivectl` | daemon 제어와 상태 확인용 CLI |
| `crates/rclone-backend` | `rclone` 탐색, 설정 소유, mount 감독, 로그 수집 |
| `crates/config` | XDG 경로, 앱 설정, mount path 검증 |
| `crates/ipc-types` | 공유 D-Bus 상태 타입 |
| `crates/state` | 경량 runtime 상태 저장 |
| `ui/` | Qt6/Kirigami 셸 |
| `integrations/` | Dolphin 액션 |
| `packaging/` | launcher, desktop entry, user service 템플릿 |
| `xtask/` | bootstrap, build, test, install 자동화 |

## Non-goals

- Microsoft OAuth 직접 구현, Graph delta sync, SQLite item index, 자체 FUSE/VFS 엔진 없음
- 사용자의 기본 `~/.config/rclone/rclone.conf`에 쓰지 않음
- 이번 릴리스에 placeholder badge, per-file pin/evict, overlay state 없음
- legacy direct-engine 호환 레이어 없음

legacy direct-engine 상태는 startup 시 제거됩니다.

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
