<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  <code>rclone</code>가 마운트된 트리와 파일 바이트를 담당하고, wrapper가 경로 상태, 장치 보존 정책, Dolphin 오버레이, tray/dashboard 경험을 담당하는 KDE 중심 OneDrive 셸입니다.
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/release.yml"><img alt="Release" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/release.yml?label=release"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="#주요-특징">주요 특징</a> ·
  <a href="#빠른-시작">빠른 시작</a> ·
  <a href="#설정">설정</a> ·
  <a href="#동작-방식">동작 방식</a> ·
  <a href="#개발">개발</a>
</p>

<p align="center">
  <img src="./assets/docs/dashboard-hero.svg" alt="open-onedrive dashboard preview" width="100%">
</p>

## 개요

`open-onedrive`는 `KDE Plasma 6 + Dolphin`을 1차 타깃으로 하는 Linux용 OneDrive 셸입니다. daemon은 앱 전용 `rclone.conf`를 소유하고, `rclone mount`를 감독하며, SQLite 기반 path-state cache를 유지하고, 그 상태를 dashboard, tray, CLI, Dolphin overlay, Dolphin context action에 제공합니다.

`rclone`이 맡는 일:

- OneDrive 트리를 파일시스템에 마운트
- 필요할 때 파일 바이트 다운로드
- `lsjson`으로 원격 경로 목록 제공

wrapper가 맡는 일:

- 파일을 장치에 고정하거나 다시 online-only로 되돌리기
- Dolphin overlay에 필요한 path-state cache 유지
- tray 상주 앱과 signal 기반 dashboard 제어
- 사용자 기본 `~/.config/rclone/rclone.conf`를 건드리지 않고 전용 XDG 경로 사용

## 주요 특징

- 기본 `curl ... | bash`가 최신 GitHub release asset을 설치
- `~/.config/rclone/rclone.conf`와 분리된 app-owned `rclone.conf`
- mount 준비 확인, 재시도 backoff, 최근 로그 캡처를 포함한 daemon-managed `rclone mount`
- `rclone lsjson`으로 갱신되는 SQLite path-state cache
- Dolphin overlay icon + `Keep on this device` / `Make online-only` 액션
- 빠른 파일 제어, sync pause/resume, 진단 로그를 갖춘 Qt6/Kirigami dashboard
- close-to-tray 동작을 지원하는 KDE StatusNotifier tray icon

## 빠른 시작

최신 release 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

특정 release tag 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_REF=v0.1.0 bash
```

예전처럼 소스 빌드 bootstrap 강제:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_BUILD_FROM_SOURCE=1 bash
```

release installer가 하는 일:

- Linux `x86_64` release archive와 SHA256 파일 다운로드
- archive 검증 후 압축 해제
- 바이너리, KDE plugin, icon, launcher, user service를 홈 디렉터리에 설치
- `rclone`이 없으면 자동 설치 시도
- `systemd --user`가 있으면 `openonedrived.service` 활성화

실행과 확인:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

첫 실행 흐름:

1. `~/OneDrive` 같은 빈 디렉터리를 mount 경로로 고릅니다.
2. `rclone`이 시작한 Microsoft 브라우저 로그인 과정을 끝냅니다.
3. Dolphin에서 마운트된 폴더를 엽니다.
4. overlay icon이나 context menu로 파일을 로컬 유지 또는 online-only로 전환합니다.
5. 창을 닫아도 tray에서 앱을 계속 실행할 수 있습니다.

CLI 예시:

```bash
openonedrivectl keep-local ~/OneDrive/Documents/report.pdf
openonedrivectl make-online-only ~/OneDrive/Documents/report.pdf
openonedrivectl rescan
openonedrivectl path-states ~/OneDrive/Documents/report.pdf
```

## 요구 사항

- Linux `x86_64`
- 런타임 의존성: `rclone`
- 1차 타깃: `KDE Plasma 6` + `Dolphin`
- release installer 기준 사용자 로컬 설치 경로: `~/.local`
- source build 경로: Rust, CMake, Qt6 tooling, KF6 development package, C++ compiler

## 설정

앱은 보통 다음 XDG 경로를 사용합니다:

- `~/.config/open-onedrive/config.toml`
- `~/.config/open-onedrive/rclone/rclone.conf`
- `~/.local/state/open-onedrive/runtime-state.toml`
- `~/.local/state/open-onedrive/path-state.sqlite3`
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

보장 사항:

- wrapper는 `~/.config/rclone/rclone.conf`를 절대 수정하지 않습니다
- runtime state와 path-state cache는 앱 전용 XDG 영역에 저장됩니다
- dashboard, tray, Dolphin action, Dolphin overlay는 모두 같은 daemon 상태를 봅니다
- `openonedrived --print-config`는 config 파일이 없어도 읽기 전용입니다

## UI 메모

- Setup 화면은 빈 mount 디렉터리 선택과 브라우저 인증 흐름 시작에 집중합니다.
- Dashboard는 mount 상태, sync 상태, queue depth, cache 크기, pinned file 수, 마지막 sync 시각, 최근 진단 로그를 보여줍니다.
- Dashboard의 quick file control로 앱 안에서 바로 residency 변경을 보낼 수 있습니다.
- Tray icon은 mount/sync/error 상태를 반영하고, 창을 닫은 뒤에도 앱을 상주시킵니다.
- Dolphin은 탐색 화면이자 제어 화면입니다. overlay는 파일 상태를 표시하고, context action은 UI/CLI와 같은 daemon 메서드를 호출합니다.

## 동작 방식

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture overview" width="100%">
</p>

- `openonedrived`가 runtime state, D-Bus method, mount supervision, path-state cache, residency policy를 소유합니다.
- `rclone mount`는 Dolphin에 보이는 OneDrive 트리를 제공하고 필요 시 파일 바이트를 가져옵니다.
- `rclone lsjson`이 SQLite path-state cache를 갱신하고, tray/dashboard/CLI/overlay가 그 상태를 읽습니다.
- Dolphin overlay plugin은 daemon을 비동기로 조회하고, daemon signal로 자체 캐시를 무효화합니다.
- Dolphin action과 `openonedrivectl`는 모두 daemon을 호출해 개별 파일을 hydrate 하거나 evict 합니다.

## 프로젝트 구성

| Path | 역할 |
| --- | --- |
| `install.sh` | release 우선 `curl ... | bash` 진입점 |
| `scripts/install.sh` | 개발자용 source install 경로 |
| `crates/openonedrived` | daemon 진입점과 D-Bus 표면 |
| `crates/openonedrivectl` | 제어, 상태, rescan, path-state 조회용 CLI |
| `crates/rclone-backend` | `rclone` 탐색, mount 감독, cache 정책, path-state cache, 로그 |
| `crates/config` | XDG 경로, 앱 설정, mount 경로 검증 |
| `crates/ipc-types` | 공용 D-Bus 상태 및 path-state 타입 |
| `crates/state` | 가벼운 runtime state 영속화 |
| `ui/` | Qt6/Kirigami 셸과 KDE tray 통합 |
| `integrations/` | Dolphin file action과 overlay plugin |
| `xtask/` | 개발자용 source build 자동화 |

## 비목표

- Windows Cloud Files 수준의 placeholder parity나 Finder 스타일 가상 파일 API
- 이번 릴리스에서 GNOME/Nautilus 지원
- custom Microsoft OAuth stack
- Graph delta sync engine이나 cross-desktop abstraction layer

## 개발

일상적인 repo 명령:

```bash
./scripts/dev.sh bootstrap
./scripts/dev.sh up
./scripts/dev.sh status
./scripts/dev.sh test
```

workspace 작업:

```bash
cargo run -p xtask -- check
cargo run -p xtask -- build-ui
cargo run -p xtask -- build-integrations
cargo run -p xtask -- install
```

## 라이선스

MIT. 자세한 내용은 [LICENSE](./LICENSE).
