#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/qemu-common.sh"

VARS_PATH="$BUILD_DIR/edk2-vars.fd"

require_qemu_image
prepare_qemu_vars_copy "$VARS_PATH"

qemu_base_args "$VARS_PATH"

exec qemu-system-x86_64 \
  "${QEMU_BASE_ARGS[@]}" \
  -serial mon:stdio \
