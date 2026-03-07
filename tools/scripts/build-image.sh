#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
IMAGE_PATH="$BUILD_DIR/lazers.img"
MOUNT_POINT="/Volumes/LAZERS"
LOADER_PATH="$ROOT_DIR/target/x86_64-unknown-uefi/release/uefi-loader.efi"
KERNEL_PATH="$ROOT_DIR/target/x86_64-unknown-none/release/lazers-kernel"
IMAGE_SIZE="64m"
VOLUME_NAME="LAZERS"

if [[ ! -f "$LOADER_PATH" ]]; then
  echo "missing loader binary at $LOADER_PATH" >&2
  exit 1
fi

if [[ ! -f "$KERNEL_PATH" ]]; then
  echo "missing kernel binary at $KERNEL_PATH" >&2
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

diskutil partitionDisk "$DEVICE" GPT FAT32 "$VOLUME_NAME" 100% >/dev/null

PARTITION="${DEVICE}s1"
if [[ ! -d "$MOUNT_POINT" ]]; then
  echo "expected mounted volume at $MOUNT_POINT" >&2
  exit 1
fi

mkdir -p "$MOUNT_POINT/EFI/BOOT" "$MOUNT_POINT/lazers"
cp "$LOADER_PATH" "$MOUNT_POINT/EFI/BOOT/BOOTX64.EFI"
cp "$KERNEL_PATH" "$MOUNT_POINT/lazers/kernel.elf"
sync
diskutil unmount "$PARTITION" >/dev/null
python3 "$ROOT_DIR/tools/scripts/patch_gpt_esp.py" "$IMAGE_PATH"

echo "created $IMAGE_PATH"
