#!/usr/bin/env bash

MACOS_IMAGE_DEVICE=""
MACOS_ESP_PARTITION=""
MACOS_SYSTEM_PARTITION=""

platform_build_image() {
  cleanup() {
    set +e
    if [[ -n "$MACOS_ESP_PARTITION" ]]; then
      diskutil unmount "$MACOS_ESP_PARTITION" >/dev/null 2>&1 || true
    fi
    if [[ -n "$MACOS_SYSTEM_PARTITION" ]]; then
      diskutil unmount "$MACOS_SYSTEM_PARTITION" >/dev/null 2>&1 || true
    fi
    if [[ -n "$MACOS_IMAGE_DEVICE" ]]; then
      diskutil unmountDisk "$MACOS_IMAGE_DEVICE" >/dev/null 2>&1 || true
      hdiutil detach "$MACOS_IMAGE_DEVICE" >/dev/null 2>&1 || true
    fi
  }

  require_command qemu-img
  require_command hdiutil
  require_command diskutil
  require_command python3

  mkdir -p "$BUILD_DIR"
  rm -f "$IMAGE_PATH"
  qemu-img create -f raw "$IMAGE_PATH" "$IMAGE_SIZE" >/dev/null

  trap cleanup EXIT

  MACOS_IMAGE_DEVICE="$(
    hdiutil attach \
      -nomount \
      -imagekey diskimage-class=CRawDiskImage \
      "$IMAGE_PATH" | awk 'NR==1 { print $1 }'
  )"

  if [[ -z "$MACOS_IMAGE_DEVICE" ]]; then
    echo "failed to attach raw image" >&2
    exit 1
  fi

  diskutil partitionDisk \
    "$MACOS_IMAGE_DEVICE" \
    GPT \
    FAT32 "$ESP_VOLUME_NAME" "$ESP_SIZE" \
    FAT32 "$SYSTEM_VOLUME_NAME" "$SYSTEM_SIZE" >/dev/null

  MACOS_ESP_PARTITION="${MACOS_IMAGE_DEVICE}s1"
  MACOS_SYSTEM_PARTITION="${MACOS_IMAGE_DEVICE}s2"
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

  mkdir -p "$SYSTEM_MOUNT_POINT$STAGING_RUNTIME_BIN_DIR"
  for package in "${USER_PACKAGES[@]}"; do
    cp "$ROOT_DIR/build/$package" "$SYSTEM_MOUNT_POINT$STAGING_RUNTIME_BIN_DIR/${package^^}"
  done

  sync
  diskutil unmount "$MACOS_ESP_PARTITION" >/dev/null
  MACOS_ESP_PARTITION=""
  diskutil unmount "$MACOS_SYSTEM_PARTITION" >/dev/null
  MACOS_SYSTEM_PARTITION=""
  python3 \
    "$ROOT_DIR/tools/scripts/patch_gpt_layout.py" \
    "$IMAGE_PATH" \
    "$LOGICAL_ESP_PARTITION_NAME" \
    "$LOGICAL_SYSTEM_PARTITION_NAME"

  echo "created $IMAGE_PATH with runtime binaries under $LOGICAL_RUNTIME_BIN_DIR"
}
