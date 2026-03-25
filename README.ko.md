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
  <a href="#overview">개요</a> ·
  <a href="#highlights">주요 기능</a> ·
  <a href="#quick-start">빠른 시작</a> ·
  <a href="#how-it-works">동작 방식</a> ·
  <a href="#development">개발</a>
</p>

<a id="overview"></a>
## 개요

`open-onedrive`는 자체 동기화 엔진이 아니라 `rclone` 위에 얹힌 Linux 데스크톱 wrapper입니다. 앱 전용 `rclone.conf`를 관리하고, foreground mount 프로세스를 daemon으로 감독하며, 상태 확인과 복구를 위한 Qt6/Kirigami UI와 작은 D-Bus CLI를 제공합니다.

<a id="highlights"></a>
## 주요 기능

- setup과 dashboard에서 모두 가능한 대화형 mount directory 선택
- `rclone`에 위임된 Microsoft 브라우저 로그인
- `~/.config/open-onedrive/rclone/rclone.conf` 아래의 앱 전용 설정
- restart backoff와 최근 로그 수집이 포함된 daemon-managed `rclone mount`
- 가벼운 Dolphin 액션이 포함된 Qt6/Kirigami 데스크톱 UI
- 상태, 로그, 수동 mount 제어를 위한 `openonedrivectl` CLI

<a id="requirements"></a>
## 요구 사항

- Linux 데스크톱 환경
- 필수 런타임 의존성인 `rclone`
- 주 타깃 환경: KDE Plasma 6 + Qt6/Kirigami
- 현재 wrapper 릴리스는 OneDrive Personal 기준

<a id="quick-start"></a>
## 빠른 시작

저장소에서 설치:

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./scripts/install.sh
```

앱 실행과 daemon 확인:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

`rclone`이 없으면 installer가 먼저 지원되는 시스템 패키지 매니저를 시도하고, 실패하면 공식 `rclone` 설치 스크립트로 fallback 합니다. 이 단계에서 `sudo` 입력이 필요할 수 있습니다.

<a id="how-it-works"></a>
## 동작 방식

- `openonedrived`가 runtime 상태, D-Bus 메서드, mount supervision을 담당합니다.
- Microsoft 인증, mount 실행, VFS 캐시는 `rclone`이 처리합니다.
- UI와 `openonedrivectl`는 둘 다 세션 버스의 daemon과 통신합니다.
- 앱은 `~/.config/rclone/rclone.conf`를 건드리지 않고, XDG 아래의 전용 설정만 사용합니다.

<a id="project-layout"></a>
## 프로젝트 구성

| Path | 설명 |
| --- | --- |
| `crates/openonedrived` | daemon 진입점과 D-Bus 표면 |
| `crates/openonedrivectl` | daemon 제어와 상태 확인용 디버그 CLI |
| `crates/rclone-backend` | `rclone` 탐색, 설정 소유, mount 감독, 로그 수집 |
| `crates/config` | XDG 경로, 앱 설정, mount path 검증 |
| `crates/ipc-types` | 공유 D-Bus 상태 타입 |
| `crates/state` | 경량 runtime 상태 저장 |
| `ui/` | Qt6/Kirigami 셸 |
| `integrations/` | Dolphin 액션 |
| `packaging/` | launcher, desktop entry, user service 템플릿 |
| `xtask/` | bootstrap, build, test, install 자동화 |

<a id="non-goals"></a>
## 비목표

- Microsoft OAuth 직접 구현, Graph delta sync, SQLite item index, 자체 FUSE/VFS 엔진 없음
- 사용자의 기본 `~/.config/rclone/rclone.conf`에 쓰지 않음
- 이번 릴리스에 per-file pin/evict, placeholder badge, overlay state 없음
- legacy direct-engine 상태와의 호환 레이어 없음

legacy direct-engine 상태는 startup 시 삭제됩니다.

<a id="development"></a>
## 개발

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

<a id="license"></a>
## 라이선스

MIT. 자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
