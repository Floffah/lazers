#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/qemu-common.sh"

VARS_PATH="$BUILD_DIR/edk2-vars-selftest.fd"
SERIAL_LOG="$BUILD_DIR/selftest-serial.log"

require_qemu_image
prepare_qemu_vars_copy "$VARS_PATH"

mkdir -p "$BUILD_DIR"
rm -f "$SERIAL_LOG"

qemu_base_args "$VARS_PATH"

qemu-system-x86_64 \
  "${QEMU_BASE_ARGS[@]}" \
  -display none \
  -monitor none \
  -serial stdio 2>&1 | tee "$SERIAL_LOG"

summary="$(grep -E 'selftest: [0-9]+ passed, [0-9]+ failed' "$SERIAL_LOG" | tail -n 1 | tr -d '\r' || true)"
if [[ -z "$summary" ]]; then
  echo "selftest host check failed: missing final selftest summary" >&2
  exit 1
fi

if [[ "$summary" =~ selftest:\ ([0-9]+)\ passed,\ ([0-9]+)\ failed$ ]]; then
  failed="${BASH_REMATCH[2]}"
  if [[ "$failed" == "0" ]]; then
    exit 0
  fi
fi

echo "selftest host check failed: $summary" >&2
exit 1
