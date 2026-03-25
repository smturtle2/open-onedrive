# open-onedrive

KDE Plasma 6 + Wayland 환경에서 Windows OneDrive 앱과 비슷한 경험을 목표로 하는 오픈소스 OneDrive 클라이언트입니다.

[English README](./README.md) · [설치](#설치) · [구조](#구조)

현재 구현된 것:

- 사용자가 바꿀 수 있는 OneDrive 마운트 경로
- Rust daemon + D-Bus 제어면
- 실제 Graph 메타데이터를 읽는 FUSE 파일시스템
- Qt/Kirigami 데스크톱 셸
- Dolphin 우클릭 액션 / overlay 플러그인
- `~/.local` 설치, 앱 메뉴 등록, systemd user service

## 현재 상태

이 저장소는 현재 환경에서 빌드됩니다.

검증 완료:

- `cargo check --workspace`
- `cargo test --workspace`
- Microsoft 로그인 콜백 + 토큰 저장
- Graph `drive/root/delta` 메타데이터 동기화
- mount 안 파일 읽기 시 실제 다운로드
- `./scripts/install.sh` 로 로컬 설치 + 앱 등록 + user service

현재 범위:

- 실제 OneDrive 메타데이터를 SQLite에 인덱싱
- read-only FUSE + on-demand hydrate + 캐시
- mount path 변경, pin/evict, status, item lookup D-Bus 메서드
- 상태를 주기적으로 새로고침하는 Qt 셸
- `~/.local`에 설치 가능한 Dolphin integration

진행 중:

- 쓰기 동기화와 업로드 경로
- 더 완성된 tray/notification UX
- 에러 복구와 동기화 안정성 강화

## 설치

로컬 설치와 앱 등록은 한 줄입니다.

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./scripts/install.sh
```

이 명령이 하는 일:

- Rust daemon/CLI 빌드
- Qt UI 빌드
- Dolphin plugin 빌드
- `~/.local` 설치
- 앱 메뉴 등록
- `openonedrived.service` 를 `systemctl --user` 로 enable/start

설치 후 실행:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

개발용 명령:

```bash
./scripts/dev.sh bootstrap
./scripts/dev.sh up
./scripts/dev.sh install
```

## 구조

- `crates/openonedrived`: daemon entrypoint, 앱 lifecycle, D-Bus 서비스, mount 제어
- `crates/openonedrivectl`: daemon D-Bus용 CLI
- `crates/config`: XDG 경로, config load/save, mount path validation
- `crates/state`: auth, delta cursor, item index를 저장하는 SQLite metadata store
- `crates/vfs`: FUSE snapshot 레이어와 content provider hook
- `crates/auth`: Microsoft auth URL, PKCE, 토큰 교환, 토큰 갱신
- `crates/graph`: Microsoft Graph delta/content client
- `ui/`: Qt6/Kirigami UI
- `integrations/`: Dolphin action / overlay plugin
- `packaging/`: desktop entry, launcher, user service 템플릿
- `xtask/`: bootstrap / build helper

## 목표

이 프로젝트의 목표는 단순한 “동기화 폴더”가 아닙니다. Linux에서 다음을 갖춘 OneDrive-like client를 만드는 것입니다.

- 백그라운드 daemon
- tray/settings UI
- 초기 UI에서 mount path 선택
- Files On-Demand에 가까운 placeholder 경험
- Dolphin 우클릭 액션
- 상태 overlay
- 간단한 설치와 앱 등록
