#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

DAEMON_LOG_DIR="$ROOT_DIR/.cache"
DAEMON_LOG_FILE="$DAEMON_LOG_DIR/openonedrived.log"
DAEMON_PID_FILE="$DAEMON_LOG_DIR/openonedrived.pid"

bootstrap() {
  cargo run -p xtask -- bootstrap
  cargo build --workspace
  cargo run -p xtask -- build-ui
  cargo run -p xtask -- build-integrations
}

start_daemon() {
  mkdir -p "$DAEMON_LOG_DIR"

  if [[ -f "$DAEMON_PID_FILE" ]] && kill -0 "$(cat "$DAEMON_PID_FILE")" 2>/dev/null; then
    echo "daemon already running: $(cat "$DAEMON_PID_FILE")"
    return
  fi

  target/debug/openonedrived >"$DAEMON_LOG_FILE" 2>&1 &
  echo $! >"$DAEMON_PID_FILE"
  echo "daemon started: $!"
}

stop_daemon() {
  if [[ ! -f "$DAEMON_PID_FILE" ]]; then
    echo "daemon is not running"
    return
  fi

  local pid
  pid="$(cat "$DAEMON_PID_FILE")"
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid"
    wait "$pid" 2>/dev/null || true
    echo "daemon stopped: $pid"
  fi
  rm -f "$DAEMON_PID_FILE"
}

up() {
  start_daemon
  if [[ -x "$ROOT_DIR/build/ui/open-onedrive-ui" ]]; then
    "$ROOT_DIR/build/ui/open-onedrive-ui"
  else
    echo "UI binary not found. Run ./scripts/dev.sh bootstrap first."
    return 1
  fi
}

status() {
  target/debug/openonedrivectl status
}

test_all() {
  cargo test --workspace
}

usage() {
  cat <<'EOF'
Usage: ./scripts/dev.sh <command>

Commands:
  bootstrap   Verify tools and build daemon, UI, and integrations
  up          Start daemon in background and launch the UI
  daemon      Start only the daemon in foreground
  stop        Stop the background daemon
  status      Query daemon status over D-Bus
  test        Run cargo test --workspace
EOF
}

case "${1:-}" in
  bootstrap)
    bootstrap
    ;;
  up)
    up
    ;;
  daemon)
    exec target/debug/openonedrived
    ;;
  stop)
    stop_daemon
    ;;
  status)
    status
    ;;
  test)
    test_all
    ;;
  *)
    usage
    exit 1
    ;;
esac
