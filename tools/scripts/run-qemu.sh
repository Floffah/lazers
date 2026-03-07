#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_PATH="$ROOT_DIR/build/lazers.img"
VARS_PATH="$ROOT_DIR/build/edk2-vars.fd"

find_ovmf_code() {
  local candidate

  for candidate in \
    /opt/homebrew/share/qemu/edk2-x86_64-code.fd \
    /usr/local/share/qemu/edk2-x86_64-code.fd \
    /opt/homebrew/Cellar/qemu/*/share/qemu/edk2-x86_64-code.fd
  do
    if [[ -f "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}

find_ovmf_vars_template() {
  local candidate

  for candidate in \
    /opt/homebrew/share/qemu/edk2-x86_64-vars.fd \
    /usr/local/share/qemu/edk2-x86_64-vars.fd \
    /opt/homebrew/Cellar/qemu/*/share/qemu/edk2-x86_64-vars.fd \
    /opt/homebrew/share/qemu/edk2-i386-vars.fd \
    /usr/local/share/qemu/edk2-i386-vars.fd \
    /opt/homebrew/Cellar/qemu/*/share/qemu/edk2-i386-vars.fd
  do
    if [[ -f "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}

if [[ ! -f "$IMAGE_PATH" ]]; then
  echo "missing disk image at $IMAGE_PATH" >&2
  exit 1
fi

OVMF_CODE="$(find_ovmf_code || true)"
OVMF_VARS_TEMPLATE="$(find_ovmf_vars_template || true)"
if [[ -z "$OVMF_CODE" ]]; then
  echo "unable to locate an OVMF/EDK2 x86_64 firmware image" >&2
  exit 1
fi
if [[ -z "$OVMF_VARS_TEMPLATE" ]]; then
  echo "unable to locate an EDK2 variable-store template" >&2
  exit 1
fi

cp "$OVMF_VARS_TEMPLATE" "$VARS_PATH"

exec qemu-system-x86_64 \
  -machine q35,accel=tcg \
  -cpu qemu64 \
  -m 256M \
  -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
  -drive if=pflash,format=raw,file="$VARS_PATH" \
  -device qemu-xhci,id=xhci \
  -drive if=none,id=usbstick,format=raw,file="$IMAGE_PATH" \
  -device usb-storage,bus=xhci.0,drive=usbstick,removable=true \
  -serial mon:stdio \
  -no-reboot \
  -no-shutdown
