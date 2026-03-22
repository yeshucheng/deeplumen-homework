#!/usr/bin/env bash
set -euo pipefail

PORT="${PORT:-9515}"
HOST="${HOST:-127.0.0.1}"
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHROMEDRIVER_LOG="${PROJECT_DIR}/chromedriver.log"
STARTED_BY_SCRIPT=0
CHROMEDRIVER_PID=""

log() {
  echo "[$(date '+%H:%M:%S')] $*"
}

have_cmd() {
  command -v "$1" >/dev/null 2>&1
}

detect_os() {
  case "$(uname -s)" in
    Darwin) echo "macos" ;;
    Linux) echo "linux" ;;
    *) echo "unknown" ;;
  esac
}

cleanup() {
  if [[ "$STARTED_BY_SCRIPT" -eq 1 && -n "$CHROMEDRIVER_PID" ]]; then
    if kill -0 "$CHROMEDRIVER_PID" >/dev/null 2>&1; then
      log "stopping chromedriver (pid=$CHROMEDRIVER_PID)"
      kill "$CHROMEDRIVER_PID" >/dev/null 2>&1 || true
    fi
  fi
}
trap cleanup EXIT

install_chromedriver_macos() {
  if ! have_cmd brew; then
    echo "[ERROR] Homebrew 未安装，请先安装 Homebrew。"
    exit 1
  fi

  log "installing chromedriver on macOS via Homebrew..."
  brew install --cask chromedriver
}

install_chromedriver_linux() {
  log "installing chromedriver on Linux..."

  if have_cmd apt-get; then
    sudo apt-get update
    sudo apt-get install -y chromium-driver || sudo apt-get install -y chromium-chromedriver
    return 0
  fi

  if have_cmd dnf; then
    sudo dnf install -y chromedriver || sudo dnf install -y chromium-chromedriver
    return 0
  fi

  if have_cmd yum; then
    sudo yum install -y chromedriver || sudo yum install -y chromium-chromedriver
    return 0
  fi

  if have_cmd pacman; then
    sudo pacman -Sy --noconfirm chromedriver
    return 0
  fi

  if have_cmd zypper; then
    sudo zypper install -y chromedriver
    return 0
  fi

  echo "[ERROR] 未识别的 Linux 包管理器，请手动安装 chromedriver。"
  exit 1
}

ensure_chromedriver_installed() {
  if have_cmd chromedriver; then
    log "chromedriver already installed: $(command -v chromedriver)"
    chromedriver --version || true
    return 0
  fi

  case "$(detect_os)" in
    macos) install_chromedriver_macos ;;
    linux) install_chromedriver_linux ;;
    *)
      echo "[ERROR] 不支持的系统: $(uname -s)"
      exit 1
      ;;
  esac

  if ! have_cmd chromedriver; then
    echo "[ERROR] chromedriver 安装后仍不可用。"
    exit 1
  fi

  log "chromedriver installed: $(command -v chromedriver)"
  chromedriver --version || true
}

port_in_use() {
  if have_cmd lsof; then
    lsof -i TCP:"$1" -sTCP:LISTEN >/dev/null 2>&1
    return
  fi

  if have_cmd ss; then
    ss -ltn "( sport = :$1 )" | grep -q ":$1"
    return
  fi

  if have_cmd netstat; then
    netstat -ltn 2>/dev/null | grep -q ":$1 "
    return
  fi

  return 1
}

chromedriver_pids_on_port() {
  if have_cmd lsof; then
    lsof -ti TCP:"$1" -sTCP:LISTEN 2>/dev/null || true
    return
  fi

  if have_cmd ss; then
    ss -ltnp "( sport = :$1 )" 2>/dev/null | sed -n 's/.*pid=\([0-9]\+\).*/\1/p' | sort -u || true
    return
  fi

  return 0
}

show_port_process() {
  if have_cmd lsof; then
    lsof -i TCP:"$1" -sTCP:LISTEN || true
    return
  fi

  if have_cmd ss; then
    ss -ltnp "( sport = :$1 )" || true
    return
  fi

  if have_cmd netstat; then
    netstat -ltnp 2>/dev/null | grep ":$1 " || true
    return
  fi
}

is_chromedriver_healthy() {
  if ! have_cmd curl; then
    return 1
  fi

  local status_url="http://${HOST}:${PORT}/status"

  # 只要能返回包含 ready/value 的内容，就认为基本健康
  local body
  body="$(curl -fsS --max-time 2 "$status_url" 2>/dev/null || true)"

  if [[ -z "$body" ]]; then
    return 1
  fi

  echo "$body" | grep -Eq '"ready"|\"value\"|\"message\"' || return 1
  return 0
}

kill_existing_driver_on_port() {
  local pids
  pids="$(chromedriver_pids_on_port "$PORT")"

  if [[ -z "$pids" ]]; then
    return 0
  fi

  log "killing stale process(es) on port $PORT: $pids"
  for pid in $pids; do
    kill "$pid" >/dev/null 2>&1 || true
  done

  sleep 1
}

start_chromedriver() {
  log "starting chromedriver on port $PORT ..."
  nohup chromedriver --port="$PORT" >"$CHROMEDRIVER_LOG" 2>&1 &
  CHROMEDRIVER_PID=$!
  STARTED_BY_SCRIPT=1

  sleep 2

  if ! kill -0 "$CHROMEDRIVER_PID" >/dev/null 2>&1; then
    echo "[ERROR] chromedriver 启动失败。"
    echo "[INFO] chromedriver log:"
    cat "$CHROMEDRIVER_LOG" || true
    exit 1
  fi

  if ! is_chromedriver_healthy; then
    echo "[ERROR] chromedriver 已启动但健康检查失败。"
    echo "[INFO] chromedriver log:"
    cat "$CHROMEDRIVER_LOG" || true
    exit 1
  fi

  log "chromedriver started, pid=$CHROMEDRIVER_PID"
  log "chromedriver log: $CHROMEDRIVER_LOG"
}

ensure_running_healthy_chromedriver() {
  if port_in_use "$PORT"; then
    log "port $PORT is already in use"
    show_port_process "$PORT"

    if is_chromedriver_healthy; then
      log "existing chromedriver is healthy, reuse it"
      return 0
    fi

    log "existing process on port $PORT is not healthy, restarting it"
    kill_existing_driver_on_port
  fi

  start_chromedriver
}

run_cargo() {
  cd "$PROJECT_DIR"
  log "running cargo run ..."
  cargo run
}

main() {
  log "project dir: $PROJECT_DIR"
  log "detected os: $(detect_os)"

  ensure_chromedriver_installed
  ensure_running_healthy_chromedriver
  run_cargo
}

main "$@"
