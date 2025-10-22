#!/usr/bin/env bash
set -euo pipefail
CMD=${1:-start}
TRACEFS=${2:-/sys/kernel/tracing}
if [[ "$CMD" == "start" ]]; then
  sudo sh -c "echo 1 > $TRACEFS/tracing_on"
  sudo sh -c "echo 1 > $TRACEFS/events/kvm/enable" || true
  if [[ -d "$TRACEFS/hyp" ]]; then
    sudo sh -c "echo 1 > $TRACEFS/hyp/events/enable" || true
  fi
  echo "tracefs kvm:* enabled"
elif [[ "$CMD" == "stop" ]]; then
  sudo sh -c "echo 0 > $TRACEFS/events/kvm/enable" || true
  if [[ -d "$TRACEFS/hyp" ]]; then
    sudo sh -c "echo 0 > $TRACEFS/hyp/events/enable" || true
  fi
  sudo sh -c "echo 0 > $TRACEFS/tracing_on"
  echo "tracefs disabled"
else
  echo "usage: $0 start|stop [tracefs_path]" >&2
  exit 2
fi
