<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  <strong>KDE Plasma 6 + Dolphin</strong> 전용 정식 안정판 <strong>v1.0.1</strong>. 일반 로컬 폴더처럼 보이는 OneDrive 루트, on-demand hydrate, 파일별 장치 유지 / online-only 전환, tray, CLI, Dolphin 통합을 한 daemon 상태 위에 올린 Linux OneDrive 클라이언트입니다.
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/ci.yml?label=ci"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/release.yml"><img alt="Release" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/release.yml?label=release"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="#주요-특징">주요 특징</a> ·
  <a href="#빠른-시작">빠른 시작</a> ·
  <a href="#지원-범위">지원 범위</a> ·
  <a href="#동작-방식">동작 방식</a> ·
  <a href="#개발">개발</a>
</p>

<p align="center">
  <img src="./assets/docs/dashboard-hero.svg" alt="open-onedrive overview shell, logs, explorer actions, and tray" width="100%">
</p>

> `v1.0.1`은 현재 안정판 라인입니다. 범위는 의도적으로 좁게 유지합니다. 공식 지원은 `Linux x86_64`, `KDE Plasma 6`, `Dolphin`에 한정합니다.

## 개요

`open-onedrive`는 `~/OneDrive` 같은 보이는 폴더를 제공하지만 `rclone mount`는 사용하지 않습니다.

대신:

- `rclone`은 인증, 원격 목록, 업로드/다운로드 primitive만 담당합니다
- `openonedrived`는 커스텀 FUSE 파일시스템, on-demand hydrate, 업로드 큐, path-state cache, conflict, retry 흐름을 직접 소유합니다
- Qt/Kirigami 셸, tray, CLI, Dolphin 플러그인은 모두 같은 daemon 상태를 읽습니다

즉, 일반 Linux 앱에는 평범한 로컬 경로처럼 보이면서도, 파일별 residency 제어는 wrapper가 직접 책임집니다.

## 주요 특징

- 커스텀 FUSE 위에 올린 보이는 OneDrive 루트 폴더
- 일반 Linux 앱에서도 동작하는 on-demand hydrate
- 파일별 `Keep on this device` / `Make online-only`
- `~/.config/rclone/rclone.conf`와 분리된 app-owned `rclone.conf`
- Dolphin overlay와 context action을 통한 탐색기 안 residency 제어
- tray + overview shell + logs page + CLI가 하나의 daemon 상태를 공유
- release archive 검증과 smoke test가 포함된 `curl ... | bash` 설치 경로

## 지원 범위

| 영역 | 상태 |
| --- | --- |
| OS / 아키텍처 | Linux `x86_64` |
| 데스크톱 | `KDE Plasma 6` |
| 파일 관리자 통합 | `Dolphin` |
| OneDrive backend | `rclone` auth/list/upload/download primitive |
| 로컬 파일시스템 모델 | `openonedrived`가 소유하는 커스텀 FUSE mount |
| 안정판 설치 경로 | `~/.local` 사용자 로컬 설치 |

`v1.0.1`의 비목표:

- `rclone mount`
- GNOME / Nautilus 지원
- KIO 전용 탐색
- Windows Cloud Files 수준의 placeholder parity
- custom Microsoft OAuth stack
- 자동 cache eviction

## 빠른 시작

최신 안정판 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

정확한 tag로 완전히 고정된 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/v1.0.1/install.sh | bash
```

release artifact 대신 source 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_BUILD_FROM_SOURCE=1 bash
```

release installer가 하는 일:

- Linux release archive와 SHA256 파일 다운로드
- checksum 검증 후 압축 해제
- 바이너리, KDE plugin, icon, launcher, user service를 홈 디렉터리에 설치
- `rclone`이 없으면 자동 설치 시도
- FUSE 3 런타임이 없으면 경고 출력
- `systemd --user`가 있으면 `openonedrived.service` 활성화

실행과 확인:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

첫 실행 흐름:

1. `~/OneDrive` 같은 빈 루트 폴더를 고릅니다.
2. `rclone`이 시작한 Microsoft 브라우저 로그인 과정을 끝냅니다.
3. 필요하면 파일시스템을 시작합니다.
4. Dolphin, 터미널, VS Code, LibreOffice 같은 일반 앱에서 루트 폴더를 엽니다.
5. overview shell, tray, CLI, Dolphin action으로 파일을 로컬 유지하거나 다시 online-only로 되돌립니다.

## 일상 제어

CLI 예시:

```bash
openonedrivectl set-root-path ~/OneDrive
openonedrivectl start-filesystem
openonedrivectl keep-local ~/OneDrive/Documents/report.pdf
openonedrivectl make-online-only ~/OneDrive/Documents/report.pdf
openonedrivectl retry-transfer ~/OneDrive/Documents/report.pdf
openonedrivectl path-states ~/OneDrive/Documents/report.pdf
```

복구 표면:

- overview shell은 daemon이 불안정해도 setup, 제어, logs 진입점을 같이 유지합니다
- tray 알림은 백그라운드의 actionable error 중심으로만 보냅니다
- Dolphin overlay는 daemon signal로 cache를 무효화해 local-only 추정치에 의존하지 않습니다

## 설정

앱은 XDG 경로 아래에 자체 상태를 저장합니다:

- `~/.config/open-onedrive/config.toml`
- `~/.config/open-onedrive/rclone/rclone.conf`
- `~/.local/state/open-onedrive/runtime-state.toml`
- `~/.local/state/open-onedrive/path-state.sqlite3`

예시 `config.toml`:

```toml
root_path = "/home/you/OneDrive"
remote_name = "openonedrive"
cache_limit_gb = 10
auto_start_filesystem = true
backing_dir_name = ".openonedrive-cache"

# Optional overrides
# rclone_bin = "/usr/bin/rclone"
# custom_client_id = "your-microsoft-client-id"
# cache_limit_gb 는 v1.0.1에서 예약만 되어 있고 아직 강제되지 않습니다
```

보장 사항:

- wrapper는 `~/.config/rclone/rclone.conf`를 수정하지 않습니다
- hydrate된 바이트는 보이는 루트 안의 숨김 backing 디렉터리에 저장됩니다
- daemon, tray, CLI, Dolphin 통합은 모두 같은 path-state view를 읽습니다
- disconnect는 OneDrive 온라인 파일이 아니라 app-owned 로컬 상태와 backing byte만 지웁니다

## 동작 방식

<p align="center">
  <img src="./assets/docs/flow-overview.svg" alt="open-onedrive architecture overview" width="100%">
</p>

- `openonedrived`가 runtime state, D-Bus method, 커스텀 FUSE mount, queue, conflict, residency policy를 소유합니다
- `rclone lsjson --hash`가 원격 메타데이터와 revision token을 새로 읽습니다
- `rclone copyto`가 첫 open에서 cold file을 내려받고 dirty local write를 업로드합니다
- 숨김 backing 디렉터리가 hydrate byte를 보관하고, visible root는 깔끔하게 유지됩니다
- Dolphin overlay와 action은 visible root만 대상으로 하고 숨김 backing 디렉터리는 무시합니다

## 왜 `rclone mount`가 아닌가?

이 프로젝트가 wrapper 쪽에서 직접 책임져야 하는 동작이 있기 때문입니다:

- 파일별 residency 상태
- UI, tray, CLI, Dolphin이 공유하는 daemon 상태
- visible root 중심의 local retry / conflict 처리
- 일반 Linux 앱이 특수 브라우징 표면이 아닌 평범한 폴더 경로로 접근하는 모델

## 개발

일상 명령:

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

## 트러블슈팅

- `Daemon not reachable on D-Bus`: `open-onedrive`를 한 번 실행하거나 `systemctl --user status openonedrived.service`를 확인합니다.
- 파일시스템 시작 실패: `/dev/fuse` 존재 여부와 `fusermount3` 또는 `mount.fuse3`가 `PATH`에 있는지 확인합니다.
- Dolphin action/overlay가 보이지 않음: `kbuildsycoca6` 실행 후 Dolphin을 재시작하고, `~/.local/lib/qt6/plugins/kf6/` 아래 plugin이 설치되었는지 확인합니다.
- sync가 paused 또는 degraded 상태: on-demand read는 계속 동작하지만 dirty write는 resume 전까지 큐에 남습니다.

## License

MIT. 자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
