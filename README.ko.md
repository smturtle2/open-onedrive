<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  <code>rclone mount</code>는 원격 트리와 파일 바이트를 제공하고, 로컬 보존 정책은 래퍼가 직접 관리하는 Linux용 OneDrive 데스크톱 셸.
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="#주요-기능">주요 기능</a> ·
  <a href="#빠른-시작">빠른 시작</a> ·
  <a href="#설정">설정</a> ·
  <a href="#동작-방식">동작 방식</a> ·
  <a href="#개발">개발</a>
</p>

<p align="center">
  <img src="./assets/docs/dashboard-hero.svg" alt="open-onedrive dashboard preview" width="100%">
</p>

## 개요

`open-onedrive`는 Linux 데스크톱용 `rclone` 래퍼이지, 자체 동기화 엔진이 아닙니다. daemon이 앱 전용 `rclone.conf`, `rclone mount`, 로그, 상태, 파일 보존 정책을 관리하고, `rclone`은 마운트된 원격 트리와 파일 바이트를 가져오는 역할만 맡습니다. 어떤 파일을 장치에 계속 남길지, 어떤 파일을 다시 온라인 전용으로 돌릴지는 래퍼가 결정합니다.

## 주요 기능

- 저장소를 내려받아 로컬에서 바로 빌드하는 `curl ... | bash` 부트스트랩
- `~/.config/rclone/rclone.conf`와 분리된 앱 전용 `rclone.conf`
- restart backoff, 최근 로그 수집, 캐시 사용량 집계를 포함한 daemon-managed `rclone mount`
- Dolphin에서 바로 쓰는 파일별 `Keep on this device` / `Make online-only`
- 앱 전용 `rclone` VFS 캐시 위에서 동작하는 래퍼 주도 보존 정책
- 진단과 파일 보존 제어에 쓸 수 있는 Qt6/Kirigami UI와 `openonedrivectl`

## 빠른 시작

GitHub에서 바로 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

특정 태그나 브랜치 고정:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_REF=v0.1.0 bash
```

이 부트스트랩이 실제로 하는 일:

- 저장소 payload를 임시 디렉터리로 내려받음
- `rclone`이 없으면 자동 설치 시도
- Rust workspace, Qt UI, KDE 통합 플러그인을 로컬에서 소스 빌드
- launcher, desktop entry, icon, `openonedrived.service` 설치
- 설치가 끝나면 임시 checkout 삭제

`curl | bash` 흐름에 필요한 빌드 전제:

- `cargo`
- `cmake` 또는 `qt-cmake`
- `ninja` 또는 `make`
- `pkg-config`
- `qml`
- 배포판에 맞는 Qt6, KF6, Kirigami 개발 패키지

실행과 확인:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

처음 쓰는 흐름:

1. `~/OneDrive` 같은 빈 디렉터리를 마운트 위치로 선택합니다.
2. `rclone`이 연 브라우저에서 Microsoft 로그인을 마칩니다.
3. Dolphin에서 마운트된 폴더를 엽니다.
4. 파일을 우클릭해서 `Keep on this device` 또는 `Make online-only`를 고릅니다.

CLI에서도 같은 작업을 할 수 있습니다:

```bash
openonedrivectl keep-local ~/OneDrive/Documents/report.pdf
openonedrivectl make-online-only ~/OneDrive/Documents/report.pdf
```

## 요구 사항

- Linux 데스크톱 환경
- 런타임 의존성인 `rclone`
- 주 타깃 환경: KDE Plasma 6, Qt6/Kirigami, Dolphin
- 부트스트랩 설치를 위한 로컬 빌드 도구체인: Rust, CMake, Qt 툴링, C++ 컴파일러
- 현재 래퍼 흐름은 OneDrive Personal 기준

## 설정

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

- 래퍼는 `~/.config/rclone/rclone.conf`에 쓰지 않습니다.
- runtime state는 사용자 설정 파일과 분리해 저장합니다.
- pinned 상태는 사용자의 기본 `rclone` 표면이 아니라 래퍼 runtime state 안에 저장됩니다.
- `openonedrived --print-config`는 설정 파일이 없어도 읽기 전용으로 동작합니다.

## UI 흐름

- Setup은 빈 마운트 디렉터리 선택과 브라우저 인증 시작에 집중합니다.
- Dashboard는 mount 제어, 캐시 크기, pinned 파일 수, 진단 정보를 한 화면에 모읍니다.
- 오류가 나도 Logs 탭이 살아 있어서 setup으로 돌아가지 않고 복구할 수 있습니다.
- 파일별 제어는 Dolphin에서 합니다. 마운트된 항목을 우클릭해서 장치 유지 또는 온라인 전용 전환을 실행합니다.

## 동작 방식

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture overview" width="100%">
</p>

- `openonedrived`가 runtime 상태, D-Bus 메서드, mount supervision, 보존 정책을 관리합니다.
- `rclone mount`는 파일 탐색기에서 보이는 원격 트리를 제공하고, 필요할 때 파일 바이트를 가져옵니다.
- 래퍼는 pinned 파일 목록을 기록하고 앱 전용 VFS 캐시를 그 집합 기준으로 정리합니다.
- Dolphin 액션과 `openonedrivectl`는 둘 다 daemon에 요청해서 개별 파일을 hydrate 하거나 evict 합니다.
- 앱은 사용자의 기본 `rclone` 설정을 공유하지 않고, 자기 XDG 경로 안에서만 동작합니다.

## 프로젝트 구성

| 경로 | 역할 |
| --- | --- |
| `install.sh` | `curl ... | bash`용 bootstrap 진입점 |
| `crates/openonedrived` | daemon 진입점과 D-Bus 표면 |
| `crates/openonedrivectl` | daemon 제어, 상태 확인, 보존 정책용 CLI |
| `crates/rclone-backend` | `rclone` 탐색, 설정 소유, mount 감독, 캐시 정책, 로그 수집 |
| `crates/config` | XDG 경로, 앱 설정, mount path 검증 |
| `crates/ipc-types` | 공유 D-Bus 상태 타입 |
| `crates/state` | 경량 runtime 상태 저장 |
| `ui/` | Qt6/Kirigami 셸 |
| `integrations/` | Dolphin 파일 액션 |
| `packaging/` | launcher, desktop entry, user service 템플릿 |
| `xtask/` | bootstrap, build, test, install 자동화 |

## 비목표

- Microsoft OAuth 직접 구현, Graph delta sync, 자체 sync 엔진 없음
- 사용자의 기본 `~/.config/rclone/rclone.conf`에 쓰지 않음
- 이번 릴리스에 Finder 스타일 placeholder badge나 cloud overlay icon 없음
- legacy direct-engine 호환 레이어 없음

legacy direct-engine 상태는 startup 시 제거됩니다.

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

## 라이선스

MIT. 자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
