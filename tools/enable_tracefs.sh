#!/usr/bin/env bash
set -euo pipefail

CMD=${1:-start}
TRACEFS=${2:-/sys/kernel/tracing}

if [[ ! -d "$TRACEFS" ]]; then
  echo "tracefs root not found: $TRACEFS" >&2
  exit 1
fi

start() {
  sudo sh -c "echo 1 > '$TRACEFS/tracing_on'" || true

  # KVM tracepoints
  if [[ -d "$TRACEFS/events/kvm" ]]; then
    sudo sh -c "echo 1 > '$TRACEFS/events/kvm/enable'" || true
  fi

  # Some arm64/pKVM setups expose hyp events separately
  if [[ -d "$TRACEFS/hyp/events" ]]; then
    sudo sh -c "echo 1 > '$TRACEFS/hyp/events/enable'" || true
  fi

  echo "tracefs enabled under $TRACEFS"
}

stop() {
  if [[ -d "$TRACEFS/events/kvm" ]]; then
    sudo sh -c "echo 0 > '$TRACEFS/events/kvm/enable'" || true
  fi
  if [[ -d "$TRACEFS/hyp/events" ]]; then
    sudo sh -c "echo 0 > '$TRACEFS/hyp/events/enable'" || true
  fi
  sudo sh -c "echo 0 > '$TRACEFS/tracing_on'" || true
  echo "tracefs disabled under $TRACEFS"
}

case "$CMD" in
  start) start ;;
  stop) stop ;;
  *) echo "usage: $0 start|stop [tracefs_path]" >&2; exit 2 ;;
esac
