<p align="center">
  <img src="./assets/open-onedrive.svg" alt="open-onedrive logo" width="112">
</p>

<h1 align="center">open-onedrive</h1>

<p align="center">
  <strong>OneDrive를 Linux의 평범한 폴더처럼.</strong><br/>
  online-only 파일 가시성, on-demand hydrate, 파일 단위 residency 제어, 단순한 설정 창, 그리고 앱·트레이·CLI·Dolphin·Nautilus가 공유하는 하나의 daemon 상태를 제공합니다.
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="#주요-특징">주요 특징</a> ·
  <a href="#빠른-시작">빠른 시작</a> ·
  <a href="#일상-사용">일상 사용</a> ·
  <a href="#개발">개발</a>
</p>

<p align="center">
  <img src="./assets/docs/app-shell-screenshot.png" alt="보이는 OneDrive 폴더를 관리하는 간결한 설정 및 상태 창을 보여주는 open-onedrive" width="100%">
</p>

<p align="center">
  <a href="https://kde.org/plasma-desktop/"><img alt="Platform" src="https://img.shields.io/badge/platform-KDE%20Plasma%206-1D99F3?logo=kdeplasma&logoColor=white"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/core-Rust-black?logo=rust"></a>
  <a href="https://www.qt.io/"><img alt="Qt6" src="https://img.shields.io/badge/ui-Qt%206-41CD52?logo=qt"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/ci.yml?label=ci"></a>
  <a href="https://github.com/smturtle2/open-onedrive/actions/workflows/release.yml"><img alt="Release" src="https://img.shields.io/github/actions/workflow/status/smturtle2/open-onedrive/release.yml?label=release"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

> 안정판은 `Linux x86_64`를 대상으로 하며, 보이는 OneDrive 루트는 `openonedrived`가 소유한 커스텀 FUSE 파일시스템으로 제공합니다. `rclone mount`는 사용하지 않습니다.

## 주요 특징

- hydrate 전에도 online-only 파일과 폴더를 계속 표시합니다
- `Keep on this device`와 `Free up space`는 CLI, Dolphin, Nautilus에서 제공하고, 앱과 트레이는 설정 및 백그라운드 제어에 집중합니다
- 메인 창은 폴더 경로, daemon 상태, 핵심 sync 제어만 남긴 settings-first 표면입니다
- 창을 닫아도 background 제어를 유지하는 독립 tray helper가 있습니다
- 일반 `~/.config/rclone/rclone.conf`와 분리된 app-owned `rclone.conf`를 사용합니다
- installer는 one-line 설치, 업그레이드 확인, checksum 검증, `rclone` bootstrap을 지원합니다

## 빠른 시작

최신 안정판 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

특정 tag 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/YOUR_TAG/install.sh | bash
```

같은 bootstrap 경로로 source 빌드:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_BUILD_FROM_SOURCE=1 bash
```

자동화 환경에서 upgrade 확인 생략:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_ASSUME_YES=1 bash
```

installer는 release payload 다운로드, SHA256 검증, 기존 설치 확인, `rclone` 자동 설치를 처리합니다. 업그레이드 시 실행 중인 daemon, tray, UI를 중지한 뒤 파일을 교체하고 user service를 다시 활성화하므로, active transfer가 끝난 뒤 진행하고 필요하면 앱 창을 다시 열어 주세요.

실행과 확인:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

## 일상 사용

첫 실행:

1. 앱 창에서 `~/OneDrive` 같은 빈 보이는 폴더를 고릅니다.
2. `rclone`이 여는 브라우저 로그인 절차를 마칩니다.
3. Dolphin 또는 Nautilus에서 보이는 폴더를 열고 online-only와 local 항목을 같은 트리에서 확인합니다.
4. 파일 탐색기 또는 CLI에서 `Keep on this device` 또는 `Free up space`를 사용합니다.

주요 화면:

- `Window`: 폴더 경로, 연결 또는 복구, 파일시스템 시작 또는 중지, sync 일시정지 또는 재개
- `Dolphin` / `Nautilus`: residency 액션과 overlay 상태를 다루는 메인 작업 표면
- `Tray`: 창을 닫은 뒤에도 남는 background 제어 표면
- `CLI`: 스크립트와 터미널에서 상태 확인과 residency 제어

파일 탐색기 통합:

- `Dolphin`을 overlay와 컨텍스트 액션의 우선 안정화 대상으로 둡니다
- `Nautilus`도 action과 emblem을 계속 제공합니다
- 우클릭 메뉴에서 `Keep on this device`, `Free up space`, retry 동작을 노출합니다
- overlay 상태로 online-only, local, syncing, attention을 구분합니다

## 동작 방식

- `rclone`은 인증, 원격 목록, 업로드/다운로드 primitive를 담당합니다
- `openonedrived`는 커스텀 sync 모델, metadata cache, path state, 직렬 action queue를 직접 소유합니다
- hydrate된 바이트는 숨김 backing 디렉터리에 저장되고 visible tree는 깔끔하게 유지됩니다
- Qt 셸, tray helper, CLI, Dolphin 플러그인, Nautilus extension은 모두 같은 daemon 상태를 읽습니다

## 개발

일상 명령:

```bash
./scripts/dev.sh bootstrap
./scripts/dev.sh up
./scripts/dev.sh test
```

`./scripts/dev.sh up`는 빠른 반복 작업을 위해 UI만 직접 실행합니다. 분리된 tray helper 경로까지 확인하려면 설치된 `open-onedrive` launcher를 사용하세요.

source 빌드 준비물:

- `cargo`가 포함된 Rust toolchain
- UI와 tray에 필요한 Qt 6 / KDE Frameworks 6 개발 패키지
- `cmake`, `ninja` 또는 `make`, `pkg-config`, `fuse3`

워크스페이스 작업:

```bash
cargo run -p xtask -- check
cargo run -p xtask -- build-ui
cargo run -p xtask -- build-integrations
```

## 트러블슈팅

- `Daemon not reachable on D-Bus`: `open-onedrive`를 한 번 실행하거나 `systemctl --user status openonedrived.service`를 확인합니다.
- 파일시스템 시작 실패: `/dev/fuse`와 `fusermount3` 또는 `mount.fuse3` 존재 여부를 확인합니다.
- Dolphin overlay나 action이 보이지 않음: `kbuildsycoca6` 실행 후 Dolphin을 재시작하고 `~/.local/lib/qt6/plugins/kf6/` 설치를 확인합니다.
- Nautilus action이나 emblem이 보이지 않음: `nautilus-python` 설치 여부를 확인한 뒤 Nautilus를 재시작합니다.

## License

MIT. 자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
