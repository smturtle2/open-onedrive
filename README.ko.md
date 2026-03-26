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
- `Keep on this device`와 `Free up space`는 CLI, 우선 지원하는 Dolphin, 그리고 Nautilus에서 제공하고, 앱과 트레이는 설정 및 백그라운드 제어에 집중합니다
- 메인 창은 폴더 경로, daemon 상태, 핵심 sync 제어만 남긴 settings-first 표면입니다
- 세션 로그인 시 자동 시작되는 독립 tray helper가 있고, `Quit`는 창·tray·daemon을 함께 정상 종료합니다
- 일반 `~/.config/rclone/rclone.conf`와 분리된 app-owned `rclone.conf`를 사용합니다
- installer는 one-line 설치, 업그레이드 확인, checksum 검증, `rclone` bootstrap을 지원합니다

## 빠른 시작

현재 bootstrap 스크립트에 고정된 안정판 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | bash
```

같은 bootstrap 경로로 특정 release tag 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_REF=YOUR_TAG bash
```

같은 bootstrap 경로로 source 빌드:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_INSTALL_MODE=source bash
```

자동화 환경에서 upgrade 확인 생략:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_ASSUME_YES=1 bash
```

시스템을 바꾸지 않고 installer 동작만 미리 보기:

```bash
curl -fsSL https://raw.githubusercontent.com/smturtle2/open-onedrive/main/install.sh | env OPEN_ONEDRIVE_DRY_RUN=1 bash
```

bootstrap 스크립트는 `~/.local` 아래에 설치하고, `~/.local/share/open-onedrive/install-metadata.env`에 설치 메타데이터를 기록하며, launcher, user service, tray autostart entry, Dolphin 플러그인, Nautilus extension, 아이콘을 갱신합니다. `systemctl --user`를 사용할 수 있으면 `openonedrived.service`도 활성화합니다. release 모드에서는 `open-onedrive-linux-x86_64.tar.gz`를 내려받아 SHA256을 검증하고, 기존 설치를 확인한 뒤 필요 시 `rclone`을 자동 설치합니다. source 모드에서는 임시 source archive를 내려받아 `scripts/install.sh`를 실행합니다. 업그레이드 시에는 실행 중인 daemon, tray, UI를 중지한 뒤 파일을 교체하므로 active transfer가 끝난 뒤 진행해 주세요.

설치 레이아웃:

- `~/.local/bin`: `open-onedrive`, `openonedrived`, `openonedrivectl`, `openonedrive-rclone-worker`
- `~/.local/lib/open-onedrive`: 설정 창과 tray helper
- `~/.local/lib/qt6/plugins/kf6`: Dolphin action 및 overlay 플러그인
- `~/.local/share/nautilus-python/extensions/openonedrive.py`: Nautilus action 및 emblem
- `~/.config/systemd/user/openonedrived.service`와 `~/.config/autostart/io.github.smturtle2.OpenOneDriveTray.desktop`

주요 installer 환경 변수:

| 변수 | 용도 |
| --- | --- |
| `OPEN_ONEDRIVE_REF` | 설치할 release tag 또는 source archive ref입니다. |
| `OPEN_ONEDRIVE_INSTALL_MODE` | `release`(기본값) 또는 `source`입니다. |
| `OPEN_ONEDRIVE_BUILD_FROM_SOURCE` | `1`로 설정하면 `OPEN_ONEDRIVE_INSTALL_MODE=source`와 같은 호환용 별칭입니다. |
| `OPEN_ONEDRIVE_ASSUME_YES` | 기존 설치를 prompt 없이 다시 설치하거나 교체합니다. |
| `OPEN_ONEDRIVE_DRY_RUN` | 실제 변경 없이 실행될 명령과 prompt를 출력합니다. |
| `OPEN_ONEDRIVE_REPO` | 테스트용 fork 등 다른 GitHub repo를 지정합니다. |
| `OPEN_ONEDRIVE_RELEASE_BASE_URL` | mirror 또는 로컬 CI smoke test용 release asset base URL을 덮어씁니다. |
| `OPEN_ONEDRIVE_SKIP_FUSE_CHECK` | container나 CI에서 `/dev/fuse`, `fuse3` helper 경고를 건너뜁니다. |

실행과 확인:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
openonedrivectl shutdown
```

## 일상 사용

첫 실행:

1. 앱 창에서 `~/OneDrive` 같은 보이는 폴더를 고릅니다. 기존 파일이 들어 있는 폴더도 원격 우선으로 인수할 수 있고, 일치하는 파일은 cache로 옮기고 나머지는 폐기합니다.
2. `rclone`이 여는 브라우저 로그인 절차를 마칩니다.
3. 우선 Dolphin에서, 필요하면 Nautilus에서 보이는 폴더를 열고 online-only와 local 항목을 같은 트리에서 확인합니다.
4. 파일 탐색기 또는 CLI에서 `Keep on this device` 또는 `Free up space`를 사용합니다.

주요 화면:

- `Window`: 폴더 경로, 연결 또는 복구, 파일시스템 시작 또는 중지, sync 일시정지 또는 재개
- `Dolphin`: residency 액션과 overlay 상태를 다루는 우선 작업 표면
- `Nautilus`: action과 emblem을 제공하는 보조 작업 표면
- `Tray`: 창을 닫은 뒤에도 남고, 로그인 시 자동 시작되는 background 제어 표면
- tray의 `Quit`는 열려 있는 창을 닫고 daemon까지 정상 종료합니다
- `CLI`: 스크립트와 터미널에서 상태 확인과 residency 제어

파일 탐색기 통합:

- `Dolphin`을 overlay와 컨텍스트 액션의 우선 지원 대상으로 둡니다
- `Nautilus`도 action과 emblem을 계속 제공하지만 통합 표면은 더 좁습니다
- 우클릭 메뉴에서 `Keep on this device`, `Free up space`, retry 동작을 노출합니다
- overlay 상태로 online-only, local, syncing, attention을 구분합니다

## 동작 방식

- `rclone`은 인증, 원격 목록, 업로드/다운로드 primitive를 담당합니다
- `openonedrived`는 커스텀 sync 모델, metadata cache, path state, 직렬 action queue를 직접 소유합니다
- 모든 `rclone` 호출은 분리된 helper binary를 통해 실행되어 긴 refresh나 transfer가 메인 daemon 제어 경로를 막지 않게 합니다
- hydrate된 바이트는 숨김 backing 디렉터리에 저장되고 visible tree는 깔끔하게 유지됩니다
- Qt 셸은 settings-first 표면에 집중하고, tray helper, CLI, Dolphin 플러그인, Nautilus extension은 모두 같은 daemon 상태를 읽습니다

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
- `Personal Vault`는 OneDrive에는 보일 수 있지만 현재 `rclone`으로 하위 목록을 안정적으로 읽을 수 없어, open-onedrive는 백그라운드 스캔에서 이를 치명적 동기화 실패로 취급하지 않고 건너뜁니다.
- Dolphin overlay나 action이 보이지 않음: `kbuildsycoca6` 실행 후 Dolphin을 재시작하고 `~/.local/lib/qt6/plugins/kf6/` 설치를 확인합니다.
- Nautilus action이나 emblem이 보이지 않음: `nautilus-python` 설치 여부를 확인한 뒤 Nautilus를 재시작합니다.

## License

MIT. 자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
