<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  <strong>OneDrive를 Linux의 평범한 폴더처럼.</strong><br/>
  online-only 파일 가시성, on-demand hydrate, 파일 및 폴더 단위 residency, 독립 tray helper, 그리고 셸·CLI·Dolphin·Nautilus가 공유하는 하나의 daemon 상태를 제공합니다.
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="#주요-특징">주요 특징</a> ·
  <a href="#빠른-시작">빠른 시작</a> ·
  <a href="#운영-표면">운영 표면</a> ·
  <a href="#지원-범위">지원 범위</a> ·
  <a href="#동작-방식">동작 방식</a> ·
  <a href="#개발">개발</a>
</p>

<p align="center">
  <img src="./assets/docs/app-shell-screenshot.png" alt="Explorer 빈 상태 안내와 좌측 워크스페이스 레일을 보여 주는 open-onedrive 셸" width="100%">
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/ci.yml?label=ci"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/release.yml"><img alt="Release" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/release.yml?label=release"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

> 안정판은 `Linux x86_64`를 대상으로 합니다. 범용 탐색은 커스텀 FUSE 경로를 통해 터미널, 에디터, 일반 Linux 앱에서 동작하고, native 파일 관리자 액션은 `Dolphin`과 `Nautilus`에 제공합니다.

## 개요

`open-onedrive`는 `~/OneDrive` 같은 보이는 폴더를 제공하지만 `rclone mount`는 사용하지 않습니다.

대신:

- `rclone`은 인증, 원격 목록, 업로드/다운로드 primitive만 담당합니다
- `openonedrived`는 커스텀 FUSE 파일시스템, metadata 기반 online-only 가시성, on-demand hydrate, 직렬 action queue, path-state cache, conflict, retry 흐름을 직접 소유합니다
- Qt/Kirigami 셸, 독립 tray helper, CLI, Dolphin 플러그인, Nautilus extension은 모두 같은 daemon 상태를 읽습니다

즉, 일반 Linux 앱에는 평범한 로컬 경로처럼 보이면서도, 파일과 폴더 residency 제어는 wrapper가 직접 책임집니다.

## 주요 특징

- 커스텀 FUSE 위에 올린 보이는 OneDrive 루트 폴더
- hydrate 전에 metadata refresh로 online-only 파일과 폴더를 계속 보이게 유지
- 일반 Linux 앱 전반에서 동작하는 on-demand hydrate
- 파일 및 폴더 단위 `Keep on this device` / `Make online-only`
- 좌측 레일 기반 Files, Activity, Setup, Logs 셸
- 큐 깊이, active 작업, backing 사용량, pinned 파일 수, 마지막 동기화 상태를 한 번에 보는 compact runtime inspector
- 경로를 직접 입력하지 않아도 되는 debounced 전체 검색, residency 필터, 빈 결과/오류 구분, bulk action, row context menu를 갖춘 Files 화면
- level, source, 시간, 최신 문제 고정을 포함한 structured logs 화면
- 루트 경로를 바꿀 때 안전하면 숨김 hydrated cache도 같이 옮기는 흐름
- `~/.config/rclone/rclone.conf`와 분리된 app-owned `rclone.conf`
- Dolphin overlay와 context action, Nautilus extension을 통한 탐색기 안 residency 제어
- main window와 분리된 tray helper로 창을 닫아도 백그라운드 제어면 유지
- checksum 검증, 기존 설치 업그레이드 확인, non-interactive fail-closed 보호, launcher/integration smoke test를 포함한 `curl ... | bash` 설치 경로

## 운영 표면

- `Files`: online-only와 local 항목을 같은 목록에서 보고, residency 필터를 적용하고, `Keep on device`, `Free up space`, `Retry transfer`, `Open/Browse`를 행 또는 선택 바에서 바로 실행합니다
- `Activity`: 큐 깊이, sync 상태, cache 사용량, 다음 바로가기만 간결하게 보여 줍니다
- `Setup`: 첫 연결, root path 변경, remote repair, clean disconnect를 같이 다룹니다
- `Logs`: 구조화된 daemon 및 `rclone` 출력을 검색하고, `All / Attention / Transfers / Errors` 필터로 좁히고, 필터된 로그를 복사해 복구 작업을 돕습니다
- `Tray`: 독립 helper 프로세스가 창 종료 후에도 제어면을 유지하고, 필요하면 메인 창을 다시 엽니다
- `Dolphin` / `Nautilus`: visible root에서 바로 per-file residency를 native action과 상태 표시로 노출합니다

## 지원 범위

| 영역 | 상태 |
| --- | --- |
| OS / 아키텍처 | Linux `x86_64` |
| 범용 탐색 표면 | 터미널, 에디터, 오피스 앱, Linux 파일 관리자에서 보이는 커스텀 FUSE 경로 |
| native 파일 관리자 통합 | `Dolphin`과 `Nautilus` |
| UI 표면 | Qt/Kirigami 셸 + 분리된 tray helper |
| OneDrive backend | `rclone` auth/list/upload/download primitive |
| 로컬 파일시스템 모델 | `openonedrived`가 소유하는 커스텀 FUSE mount |
| 안정판 설치 경로 | `~/.local` 사용자 로컬 설치 |

현재 안정판의 비목표:

- `rclone mount`
- `Dolphin` / `Nautilus`를 넘는 native 통합 확장
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
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/YOUR_TAG/install.sh | bash
```

release artifact 대신 source 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_BUILD_FROM_SOURCE=1 bash
```

자동화 환경에서 interactive upgrade prompt 생략:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_ASSUME_YES=1 bash
```

release installer가 하는 일:

- Linux release archive와 SHA256 파일 다운로드
- 기존 설치가 있으면 interactive upgrade / reinstall 여부 확인
- checksum 검증 후 압축 해제
- 바이너리, tray helper, 파일 관리자 통합, icon, launcher, user service를 홈 디렉터리에 설치
- `rclone`이 없으면 자동 설치 시도
- FUSE 3 런타임이 없으면 경고 출력
- `systemd --user`가 있으면 `openonedrived.service` 활성화
- 이후 업그레이드 비교를 위해 `~/.local/share/open-onedrive/install-metadata.env`에 설치 메타데이터 기록
- `OPEN_ONEDRIVE_ASSUME_YES=1` 없이 non-interactive 환경에서 기존 설치를 덮어쓰지 않음

실행과 확인:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

첫 실행 흐름:

1. `~/OneDrive` 같은 빈 루트 폴더를 고릅니다.
2. `rclone`이 시작한 Microsoft 브라우저 로그인 과정을 끝냅니다.
3. 좌측 레일 셸에서 Files, Activity, Setup, Logs를 오가며 현재 상태를 계속 확인합니다.
4. 필요하면 파일시스템을 시작합니다.
5. Dolphin, Nautilus, 터미널, VS Code, LibreOffice 같은 일반 앱에서 루트 폴더를 엽니다.
6. Files, tray, CLI, Dolphin action, Nautilus action으로 파일을 로컬 유지하거나 다시 online-only로 되돌립니다.

## 일상 제어

CLI 예시:

```bash
openonedrivectl set-root-path ~/OneDrive
openonedrivectl start-filesystem
openonedrivectl keep-local ~/OneDrive/Documents/report.pdf
openonedrivectl make-online-only ~/OneDrive/Documents/report.pdf
openonedrivectl retry-transfer ~/OneDrive/Documents/report.pdf
openonedrivectl list-directory Docs
openonedrivectl refresh-directory Docs
openonedrivectl search-paths report --limit 20
openonedrivectl path-states ~/OneDrive/Documents/report.pdf
```

복구 표면:

- 좌측 레일 셸은 Setup과 Logs를 항상 한 번에 열 수 있게 두고, 다음 권장 화면만 따로 보여줍니다
- Files 페이지는 unavailable / error / empty 상태를 분리한 searchable path-state view와 bulk / row-level residency action을 제공합니다
- logs 페이지는 structured daemon / `rclone` 출력에 검색과 필터를 적용해 복구 맥락을 좁혀볼 수 있습니다
- tray 알림은 백그라운드의 actionable error 중심으로만 보내고, 분리된 tray helper는 창 종료 후에도 남습니다
- Dolphin overlay와 Nautilus extension은 daemon signal로 cache를 무효화해 local-only 추정치에 의존하지 않습니다

## 설정

앱은 XDG 경로 아래에 자체 상태를 저장합니다:

- `~/.config/open-onedrive/config.toml`
- `~/.config/open-onedrive/rclone/rclone.conf`
- `~/.local/share/open-onedrive/install-metadata.env`
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
# cache_limit_gb 는 현재 정보용 값이며, cache eviction은 수동 동작만 지원합니다
```

보장 사항:

- wrapper는 `~/.config/rclone/rclone.conf`를 수정하지 않습니다
- hydrate된 바이트는 보이는 루트 안의 숨김 backing 디렉터리에 저장됩니다
- visible root를 옮길 때 목적지가 안전하면 그 숨김 backing 디렉터리도 같이 이동합니다
- daemon, tray, CLI, Dolphin, Nautilus 통합은 모두 같은 path-state view를 읽습니다
- disconnect는 OneDrive 온라인 파일이 아니라 app-owned 로컬 상태와 backing byte만 지웁니다

## 동작 방식
- `openonedrived`가 runtime state, D-Bus method, 커스텀 FUSE mount, 단일 직렬 action queue, conflict, residency policy를 소유합니다
- `rclone lsjson --hash`가 파일 바이트를 hydrate하지 않고 원격 메타데이터와 revision token을 새로 읽습니다
- `rclone copyto`가 첫 open에서 cold file을 내려받고 dirty local write를 업로드합니다
- targeted directory refresh가 Files, Logs, Tray, Dolphin, Nautilus 상태를 full rescan에만 의존하지 않고 갱신합니다
- 숨김 backing 디렉터리가 hydrate byte를 보관하고, visible root는 깔끔하게 유지됩니다
- Dolphin overlay, Nautilus emblem, 파일 관리자 action은 visible root만 대상으로 하고 숨김 backing 디렉터리는 무시합니다

## 왜 `rclone mount`가 아닌가?

이 프로젝트가 wrapper 쪽에서 직접 책임져야 하는 동작이 있기 때문입니다:

- 파일별 residency 상태
- UI, tray, CLI, Dolphin, Nautilus가 공유하는 daemon 상태
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
- Nautilus action/emblem이 보이지 않음: `nautilus-python` 설치 여부를 확인한 뒤, `~/.local/share/nautilus-python/extensions/openonedrive.py`를 Nautilus가 다시 읽도록 Nautilus를 재시작합니다.
- sync가 paused 또는 degraded 상태: on-demand read는 계속 동작하지만 dirty write는 resume 전까지 큐에 남습니다.

## License

MIT. 자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
