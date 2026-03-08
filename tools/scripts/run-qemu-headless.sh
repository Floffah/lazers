#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/qemu-common.sh"

VARS_PATH="$BUILD_DIR/edk2-vars-headless.fd"
MONITOR_PATH="$BUILD_DIR/qemu-monitor.sock"
SCREENSHOT_PPM="$BUILD_DIR/qemu-headless.ppm"
SCREENSHOT_PNG="$BUILD_DIR/qemu-headless.png"

require_qemu_image
prepare_qemu_vars_copy "$VARS_PATH"

rm -f "$MONITOR_PATH" "$SCREENSHOT_PPM" "$SCREENSHOT_PNG"

qemu_base_args "$VARS_PATH"

cleanup() {
  rm -f "$MONITOR_PATH" "$SCREENSHOT_PPM"
}

trap cleanup EXIT

qemu-system-x86_64 \
  "${QEMU_BASE_ARGS[@]}" \
  -monitor "unix:$MONITOR_PATH,server,nowait" \
  -display none \
  -serial none \
  -daemonize

for _ in $(seq 1 20); do
  if [[ -S "$MONITOR_PATH" ]]; then
    break
  fi
  sleep 1
done

if [[ ! -S "$MONITOR_PATH" ]]; then
  echo "timed out waiting for QEMU monitor socket" >&2
  exit 1
fi

sleep 4
printf 'screendump %s\nquit\n' "$SCREENSHOT_PPM" | nc -U "$MONITOR_PATH" >/dev/null
sips -s format png "$SCREENSHOT_PPM" --out "$SCREENSHOT_PNG" >/dev/null

echo "captured headless boot screenshot at $SCREENSHOT_PNG"
