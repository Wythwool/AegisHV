#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
usage: scripts/check-type1-host-text.sh [DISASSEMBLY]

Rejects x87, MMX, SIMD, and extended-state instructions in a textual host
.text disassembly. Reads standard input when DISASSEMBLY is omitted or '-'.
USAGE
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi
if [ "$#" -gt 1 ]; then
  usage
  exit 64
fi

input="${1:--}"
if [ "$input" = "-" ]; then
  disassembly="$(sed -n 'p')"
elif [ -f "$input" ]; then
  disassembly="$(sed -n 'p' "$input")"
else
  echo "type1 host text check: disassembly does not exist: $input" >&2
  exit 66
fi

instructions="$({
  printf '%s\n' "$disassembly" | awk '
    /^[[:xdigit:]]+ <[^>]+>:/ {
      context_switch = ($0 ~ /<aegishv_svm_enter>:/)
    }
    /^[[:space:]]*[[:xdigit:]]+:/ {
      sub(/^[[:space:]]*[^:]+:[[:space:]]*/, "")
      while ($0 ~ /^[[:xdigit:]][[:xdigit:]][[:space:]]+/) {
        sub(/^[[:xdigit:]][[:xdigit:]][[:space:]]+/, "")
      }
      if (context_switch && $0 ~ /^fx(save|rstor)64[[:space:]]/) next
      print
    }
  '
})"
if [ -z "$instructions" ]; then
  echo "type1 host text check: disassembly contains no instructions" >&2
  exit 70
fi

# Rust host code is integer-only. The SVM entry gate owns the four FXSAVE
# transfers; no other function may touch the guest's extended state.
forbidden='^[[:space:]]*f[[:alnum:]_.]*([[:space:]]|$)|^[[:space:]]*(wait|emms|vzero(all|upper)|xsave[a-z0-9]*|xrstor[a-z0-9]*|v?ldmxcsr|v?stmxcsr|xgetbv|xsetbv|rdpkru|wrpkru|ldtilecfg|sttilecfg|tilerelease)([[:space:]]|$)|%(st\([0-7]\)|(xmm|ymm|zmm|mm|tmm)[0-9]+|k[0-7]|bnd[0-3])([^[:alnum:]_]|$)'
violations="$(printf '%s\n' "$instructions" | grep -Ei "$forbidden" || true)"
if [ -n "$violations" ]; then
  first_violation="$(printf '%s\n' "$violations" | sed -n '1p')"
  echo "type1 host text check: FPU/SIMD state instruction found: $first_violation" >&2
  exit 70
fi
