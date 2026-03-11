#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
IMAGE_NAME="${LAZERS_IMAGE_NAME:-lazers.img}"
IMAGE_PATH="$BUILD_DIR/$IMAGE_NAME"
ESP_MOUNT_POINT="/Volumes/LAZERSESP"
SYSTEM_MOUNT_POINT="/Volumes/LAZERSSYS"
LOADER_PATH="$ROOT_DIR/target/x86_64-unknown-uefi/release/uefi-loader.efi"
KERNEL_PATH="$ROOT_DIR/target/x86_64-unknown-none/release/kernel"
IMAGE_SIZE="256m"
ESP_VOLUME_NAME="LAZERSESP"
SYSTEM_VOLUME_NAME="LAZERSSYS"
ESP_SIZE="64m"
SYSTEM_SIZE="R"
USER_PACKAGES=()

if [[ ! -f "$LOADER_PATH" ]]; then
  echo "missing loader binary at $LOADER_PATH" >&2
  exit 1
fi

if [[ ! -f "$KERNEL_PATH" ]]; then
  echo "missing kernel binary at $KERNEL_PATH" >&2
  exit 1
fi

while IFS= read -r package; do
  USER_PACKAGES+=("$package")
done < <(find "$ROOT_DIR/user" -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort)

if [[ ${#USER_PACKAGES[@]} -eq 0 ]]; then
  echo "no user packages found under $ROOT_DIR/user" >&2
  exit 1
fi

for package in "${USER_PACKAGES[@]}"; do
  user_binary_path="$ROOT_DIR/build/$package"
  if [[ ! -f "$user_binary_path" ]]; then
    echo "missing user binary at $user_binary_path" >&2
    exit 1
  fi
done

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
for package in "${USER_PACKAGES[@]}"; do
  cp "$ROOT_DIR/build/$package" "$SYSTEM_MOUNT_POINT/BIN/${package^^}"
done
sync
diskutil unmount "$ESP_PARTITION" >/dev/null
diskutil unmount "$SYSTEM_PARTITION" >/dev/null
python3 "$ROOT_DIR/tools/scripts/patch_gpt_layout.py" "$IMAGE_PATH"

echo "created $IMAGE_PATH"
