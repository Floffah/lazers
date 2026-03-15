#!/usr/bin/env bash

IMAGE_SECTOR_SIZE=512
GPT_RESERVED_END_SECTORS=33
DEFAULT_PARTITION_ALIGNMENT_SECTORS=2048
LINUX_STAGING_DIR=""

linux_cleanup_image_staging() {
  if [[ -n "${LINUX_STAGING_DIR:-}" ]]; then
    rm -rf "$LINUX_STAGING_DIR"
  fi
}

platform_build_image() {
  local image_size_bytes=""
  local esp_size_bytes=""
  local total_sectors=""
  local esp_start_sector=""
  local esp_sector_count=""
  local esp_end_sector=""
  local system_start_sector=""
  local system_end_sector=""
  local system_sector_count=""
  local system_size_bytes=""
  local esp_image=""
  local system_image=""

  require_command qemu-img
  require_command sgdisk
  require_command truncate
  require_command dd
  require_command mformat
  require_command mmd
  require_command mcopy
  require_command python3

  image_size_bytes="$(size_to_bytes "$IMAGE_SIZE")"
  esp_size_bytes="$(size_to_bytes "$ESP_SIZE")"
  total_sectors=$((image_size_bytes / IMAGE_SECTOR_SIZE))
  esp_start_sector="$DEFAULT_PARTITION_ALIGNMENT_SECTORS"
  esp_sector_count=$((esp_size_bytes / IMAGE_SECTOR_SIZE))
  esp_end_sector=$((esp_start_sector + esp_sector_count - 1))
  system_start_sector="$(align_sector $((esp_end_sector + 1)))"
  system_end_sector=$((total_sectors - GPT_RESERVED_END_SECTORS - 1))
  system_sector_count=$((system_end_sector - system_start_sector + 1))
  system_size_bytes=$((system_sector_count * IMAGE_SECTOR_SIZE))

  if (( system_sector_count <= 0 )); then
    echo "image layout does not leave room for the system partition" >&2
    exit 1
  fi

  mkdir -p "$BUILD_DIR"
  rm -f "$IMAGE_PATH"
  qemu-img create -f raw "$IMAGE_PATH" "$IMAGE_SIZE" >/dev/null

  sgdisk \
    --clear \
    --new=1:${esp_start_sector}:${esp_end_sector} \
    --typecode=1:ef00 \
    --change-name=1:${ESP_VOLUME_NAME} \
    --new=2:${system_start_sector}:${system_end_sector} \
    --typecode=2:0700 \
    --change-name=2:${SYSTEM_VOLUME_NAME} \
    "$IMAGE_PATH" >/dev/null

  LINUX_STAGING_DIR="$(mktemp -d "$BUILD_DIR/build-image-linux.XXXXXX")"
  trap linux_cleanup_image_staging EXIT

  esp_image="$LINUX_STAGING_DIR/esp.fat"
  system_image="$LINUX_STAGING_DIR/system.fat"

  truncate -s "$esp_size_bytes" "$esp_image"
  truncate -s "$system_size_bytes" "$system_image"

  mformat -i "$esp_image" -F -v "$ESP_VOLUME_NAME" ::
  mformat -i "$system_image" -F -v "$SYSTEM_VOLUME_NAME" ::

  mmd -i "$esp_image" ::/EFI
  mmd -i "$esp_image" ::/EFI/BOOT
  mmd -i "$esp_image" ::/lazers
  mcopy -i "$esp_image" "$LOADER_PATH" ::/EFI/BOOT/BOOTX64.EFI
  mcopy -i "$esp_image" "$KERNEL_PATH" ::/lazers/kernel.elf

  mmd -i "$system_image" ::/SYSTEM
  mmd -i "$system_image" ::/SYSTEM/BIN
  for package in "${USER_PACKAGES[@]}"; do
    mcopy -i "$system_image" "$ROOT_DIR/build/$package" "::/SYSTEM/BIN/${package^^}"
  done

  dd if="$esp_image" of="$IMAGE_PATH" bs="$IMAGE_SECTOR_SIZE" seek="$esp_start_sector" conv=notrunc status=none
  dd if="$system_image" of="$IMAGE_PATH" bs="$IMAGE_SECTOR_SIZE" seek="$system_start_sector" conv=notrunc status=none

  python3 \
    "$ROOT_DIR/tools/scripts/patch_gpt_layout.py" \
    "$IMAGE_PATH" \
    "$LOGICAL_ESP_PARTITION_NAME" \
    "$LOGICAL_SYSTEM_PARTITION_NAME"

  echo "created $IMAGE_PATH with runtime binaries under $LOGICAL_RUNTIME_BIN_DIR"
}

size_to_bytes() {
  local size="$1"

  case "$size" in
    *[mM])
      printf '%s\n' "$(( ${size%[mM]} * 1024 * 1024 ))"
      ;;
    *[kK])
      printf '%s\n' "$(( ${size%[kK]} * 1024 ))"
      ;;
    *[gG])
      printf '%s\n' "$(( ${size%[gG]} * 1024 * 1024 * 1024 ))"
      ;;
    *)
      printf '%s\n' "$size"
      ;;
  esac
}

align_sector() {
  local sector="$1"

  printf '%s\n' "$(( ((sector + DEFAULT_PARTITION_ALIGNMENT_SECTORS - 1) / DEFAULT_PARTITION_ALIGNMENT_SECTORS) * DEFAULT_PARTITION_ALIGNMENT_SECTORS ))"
}
