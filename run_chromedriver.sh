#!/usr/bin/env bash
set -euo pipefail

PORT=9515
LOG_FILE=/tmp/chromedriver.log

if ! lsof -i :"$PORT" >/dev/null 2>&1; then
  echo "Starting chromedriver on port $PORT..."
  chromedriver --port="$PORT" >"$LOG_FILE" 2>&1 &

  for _ in {1..20}; do
    if lsof -i :"$PORT" >/dev/null 2>&1; then
      echo "Chromedriver started on port $PORT"
      break
    fi
    sleep 0.5
  done

  if ! lsof -i :"$PORT" >/dev/null 2>&1; then
    echo "Failed to start chromedriver on port $PORT"
    echo "See log: $LOG_FILE"
    exit 1
  fi
else
  echo "Chromedriver already running on port $PORT"
fi

exec "$(dirname "$0")/my_spider_bin"
