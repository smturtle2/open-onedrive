# open-onedrive

`rclone mount`를 직접 감독하는 Linux용 OneDrive 데스크톱 셸입니다. 더 이상 자체 OneDrive 동기화 엔진을 구현하지 않습니다.

[English README](./README.md) · [설치](#설치) · [아키텍처](#아키텍처)

## 개요

`open-onedrive`는 이제 `rclone` wrapper입니다.

- 사용자가 지정한 호스트 마운트 경로
- `rclone`에 위임된 Microsoft 브라우저 로그인
- XDG 아래 앱 전용 `rclone.conf`
- foreground `rclone mount` child를 감독하는 daemon
- setup/dashboard/logs로 단순화한 Qt/Kirigami UI
- 마운트 단위만 남긴 Dolphin 액션

기존의 Microsoft OAuth 구현, Graph delta sync, SQLite item index, 자체 FUSE/VFS 엔진은 제품 정의에서 제거되었습니다.

## 현재 범위

- 첫 wrapper 릴리스는 OneDrive Personal 우선
- `rclone`은 필수 런타임 의존성
- 앱은 `~/.config/rclone/rclone.conf`를 절대 건드리지 않음
- lazy download와 캐시는 `rclone` VFS 캐시에 위임
- per-file pin/evict, overlay 상태, placeholder 배지는 이번 릴리스 범위 밖

startup 시 legacy direct-engine 상태는 삭제합니다. 기존 설정 호환성은 유지하지 않습니다.

## 설치

`rclone`이 없으면 installer가 먼저 시스템 패키지 매니저로 자동 설치를 시도하고, 실패하면 공식 `rclone` 설치 스크립트로 fallback 합니다. 이 과정에서 `sudo` 입력이 필요할 수 있습니다.

```bash
git clone https://github.com/smturtle2/open-onedrive.git
cd open-onedrive
./scripts/install.sh
```

설치 후:

```bash
open-onedrive
systemctl --user status openonedrived.service
openonedrivectl status
```

앱 전용 rclone 설정 파일 경로:

```text
~/.config/open-onedrive/rclone/rclone.conf
```

## 아키텍처

- `crates/openonedrived`: daemon 진입점과 D-Bus 표면
- `crates/openonedrivectl`: daemon용 디버그 CLI
- `crates/config`: XDG 경로, wrapper 설정, mount path 검증
- `crates/ipc-types`: 공유 상태 타입
- `crates/rclone-backend`: rclone 탐색, config 소유, mount 감독, 로그 수집
- `crates/state`: 경량 runtime 상태 저장
- `ui/`: Qt6/Kirigami UI
- `integrations/`: Dolphin mount 액션
- `packaging/`: desktop entry, launcher, user service 템플릿
- `xtask/`: 빌드/설치/의존성 체크
