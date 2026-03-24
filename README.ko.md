# open-onedrive

KDE Plasma 6 + Wayland 환경에서 Windows OneDrive 앱과 비슷한 경험을 목표로 하는 오픈소스 OneDrive 클라이언트입니다.

[English README](./README.md) · [빠른 시작](#빠른-시작) · [구조](#구조)

현재 구현된 것:

- 사용자가 바꿀 수 있는 OneDrive 마운트 경로
- Rust daemon + D-Bus 제어면
- placeholder 스타일 FUSE 파일시스템 골격
- Qt/Kirigami 데스크톱 셸
- Dolphin 우클릭 액션 / overlay 플러그인 골격

## 현재 상태

이 저장소는 현재 환경에서 빌드됩니다.

검증 완료:

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo run -p xtask -- bootstrap`
- `cargo run -p xtask -- build-ui`
- `cargo run -p xtask -- build-integrations`
- `openonedrived` 와 `openonedrivectl status` D-Bus round-trip

현재 범위:

- SQLite demo metadata 기반의 read-only placeholder FUSE 트리
- 로그인 bootstrap, mount path 변경, pin/evict, status, item lookup D-Bus 메서드
- daemon 상태를 읽는 Qt 셸
- 빌드 가능한 Dolphin integration scaffold

진행 중:

- 실제 Microsoft Graph 동기화 엔진
- 실제 hydrate/download/upload 경로
- 실제 클라우드 상태 기반 overlay

## 빠른 시작

개발용 설치/실행은 두 줄로 끝납니다.

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./scripts/dev.sh bootstrap
./scripts/dev.sh up
```

의미:

- `bootstrap`: 도구 확인, Rust workspace 빌드, UI 빌드, KDE integration 빌드
- `up`: daemon을 백그라운드로 띄우고 데스크톱 UI를 실행

추가 명령:

```bash
./scripts/dev.sh status
./scripts/dev.sh test
./scripts/dev.sh daemon
```

## 구조

- `crates/openonedrived`: daemon entrypoint, 앱 lifecycle, D-Bus 서비스, mount 제어
- `crates/openonedrivectl`: daemon D-Bus용 CLI
- `crates/config`: XDG 경로, config load/save, mount path validation
- `crates/state`: SQLite metadata store
- `crates/vfs`: FUSE snapshot 레이어
- `crates/auth`: Microsoft auth URL + PKCE bootstrap
- `crates/graph`: Microsoft Graph client scaffold
- `ui/`: Qt6/Kirigami UI
- `integrations/`: Dolphin action / overlay plugin
- `xtask/`: bootstrap / build helper

## 목표

이 프로젝트의 목표는 단순한 “동기화 폴더”가 아닙니다. Linux에서 다음을 갖춘 OneDrive-like client를 만드는 것입니다.

- 백그라운드 daemon
- tray/settings UI
- 초기 UI에서 mount path 선택
- Files On-Demand에 가까운 placeholder 경험
- Dolphin 우클릭 액션
- 상태 overlay

