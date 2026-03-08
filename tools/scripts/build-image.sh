#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
IMAGE_PATH="$BUILD_DIR/lazers.img"
ESP_MOUNT_POINT="/Volumes/LAZERSESP"
SYSTEM_MOUNT_POINT="/Volumes/LAZERSSYS"
LOADER_PATH="$ROOT_DIR/target/x86_64-unknown-uefi/release/uefi-loader.efi"
KERNEL_PATH="$ROOT_DIR/target/x86_64-unknown-none/release/kernel"
USER_ECHO_PATH="$ROOT_DIR/build/echo"
USER_LASH_PATH="$ROOT_DIR/build/lash"
USER_LS_PATH="$ROOT_DIR/build/ls"
IMAGE_SIZE="256m"
ESP_VOLUME_NAME="LAZERSESP"
SYSTEM_VOLUME_NAME="LAZERSSYS"
ESP_SIZE="64m"
SYSTEM_SIZE="R"

if [[ ! -f "$LOADER_PATH" ]]; then
  echo "missing loader binary at $LOADER_PATH" >&2
  exit 1
fi

if [[ ! -f "$KERNEL_PATH" ]]; then
  echo "missing kernel binary at $KERNEL_PATH" >&2
  exit 1
fi

if [[ ! -f "$USER_ECHO_PATH" ]]; then
  echo "missing user binary at $USER_ECHO_PATH" >&2
  exit 1
fi

if [[ ! -f "$USER_LASH_PATH" ]]; then
  echo "missing user binary at $USER_LASH_PATH" >&2
  exit 1
fi

if [[ ! -f "$USER_LS_PATH" ]]; then
  echo "missing user binary at $USER_LS_PATH" >&2
  exit 1
fi

mkdir -p "$BUILD_DIR"
rm -f "$IMAGE_PATH"
qemu-img create -f raw "$IMAGE_PATH" "$IMAGE_SIZE" >/dev/null

DEVICE=""

cleanup() {
  set +e
  if [[ -n "$DEVICE" ]]; then
    diskutil unmountDisk "$DEVICE" >/dev/null 2>&1 || true
    hdiutil detach "$DEVICE" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT

DEVICE="$(
  hdiutil attach \
    -nomount \
    -imagekey diskimage-class=CRawDiskImage \
    "$IMAGE_PATH" | awk 'NR==1 { print $1 }'
)"

if [[ -z "$DEVICE" ]]; then
  echo "failed to attach raw image" >&2
  exit 1
fi

diskutil partitionDisk \
  "$DEVICE" \
  GPT \
  FAT32 "$ESP_VOLUME_NAME" "$ESP_SIZE" \
  FAT32 "$SYSTEM_VOLUME_NAME" "$SYSTEM_SIZE" >/dev/null

ESP_PARTITION="${DEVICE}s1"
SYSTEM_PARTITION="${DEVICE}s2"
if [[ ! -d "$ESP_MOUNT_POINT" ]]; then
  echo "expected mounted ESP volume at $ESP_MOUNT_POINT" >&2
  exit 1
fi

if [[ ! -d "$SYSTEM_MOUNT_POINT" ]]; then
  echo "expected mounted system volume at $SYSTEM_MOUNT_POINT" >&2
  exit 1
fi

mkdir -p "$ESP_MOUNT_POINT/EFI/BOOT" "$ESP_MOUNT_POINT/lazers"
cp "$LOADER_PATH" "$ESP_MOUNT_POINT/EFI/BOOT/BOOTX64.EFI"
cp "$KERNEL_PATH" "$ESP_MOUNT_POINT/lazers/kernel.elf"

mkdir -p "$SYSTEM_MOUNT_POINT/BIN"
cp "$USER_ECHO_PATH" "$SYSTEM_MOUNT_POINT/BIN/ECHO"
cp "$USER_LASH_PATH" "$SYSTEM_MOUNT_POINT/BIN/LASH"
cp "$USER_LS_PATH" "$SYSTEM_MOUNT_POINT/BIN/LS"
sync
diskutil unmount "$ESP_PARTITION" >/dev/null
diskutil unmount "$SYSTEM_PARTITION" >/dev/null
python3 "$ROOT_DIR/tools/scripts/patch_gpt_layout.py" "$IMAGE_PATH"

echo "created $IMAGE_PATH"
