#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"
source "$SCRIPT_DIR/user-packages.sh"

IMAGE_NAME="${LAZERS_IMAGE_NAME:-lazers.img}"
IMAGE_PATH="$BUILD_DIR/$IMAGE_NAME"
LOADER_PATH="$ROOT_DIR/target/x86_64-unknown-uefi/release/uefi-loader.efi"
KERNEL_PATH="$ROOT_DIR/target/x86_64-unknown-none/release/kernel"
IMAGE_SIZE="256m"
ESP_SIZE="64m"
SYSTEM_SIZE="R"

LOGICAL_ESP_PARTITION_NAME="LAZERS-ESP"
LOGICAL_SYSTEM_PARTITION_NAME="LAZERS-SYSTEM"
LOGICAL_RUNTIME_BIN_DIR="/system/bin"

ESP_VOLUME_NAME="LAZERSESP"
SYSTEM_VOLUME_NAME="LAZERSSYS"
ESP_MOUNT_POINT="/Volumes/$ESP_VOLUME_NAME"
SYSTEM_MOUNT_POINT="/Volumes/$SYSTEM_VOLUME_NAME"
STAGING_RUNTIME_BIN_DIR="/SYSTEM/BIN"
LOWERCASE_ESP_DIR="/lazers"

if [[ ! -f "$LOADER_PATH" ]]; then
  echo "missing loader binary at $LOADER_PATH" >&2
  exit 1
fi

if [[ ! -f "$KERNEL_PATH" ]]; then
  echo "missing kernel binary at $KERNEL_PATH" >&2
  exit 1
fi

load_user_packages_from_env

for package in "${USER_PACKAGES[@]}"; do
  user_binary_path="$ROOT_DIR/build/$package"
  if [[ ! -f "$user_binary_path" ]]; then
    echo "missing user binary at $user_binary_path" >&2
    exit 1
  fi
done

source_platform_script "build-image.sh"
platform_build_image
