#!/usr/bin/env bash

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"
IMAGE_PATH="$BUILD_DIR/lazers.img"

find_ovmf_code() {
  local candidate

  shopt -s nullglob

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

  shopt -s nullglob

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

require_qemu_image() {
  if [[ ! -f "$IMAGE_PATH" ]]; then
    echo "missing disk image at $IMAGE_PATH" >&2
    exit 1
  fi
}

prepare_qemu_vars_copy() {
  local vars_path="$1"
  local ovmf_vars_template

  ovmf_vars_template="$(find_ovmf_vars_template || true)"
  if [[ -z "$ovmf_vars_template" ]]; then
    echo "unable to locate an EDK2 variable-store template" >&2
    exit 1
  fi

  mkdir -p "$(dirname "$vars_path")"
  cp "$ovmf_vars_template" "$vars_path"
}

qemu_base_args() {
  local ovmf_code
  local vars_path="$1"

  ovmf_code="$(find_ovmf_code || true)"
  if [[ -z "$ovmf_code" ]]; then
    echo "unable to locate an OVMF/EDK2 x86_64 firmware image" >&2
    exit 1
  fi

  QEMU_BASE_ARGS=(
    -machine q35,accel=tcg \
    -cpu qemu64 \
    -m 256M \
    -drive "if=pflash,format=raw,readonly=on,file=$ovmf_code" \
    -drive "if=pflash,format=raw,file=$vars_path" \
    -device ich9-ahci,id=ahci \
    -drive "if=none,id=systemdisk,format=raw,file=$IMAGE_PATH" \
    -device ide-hd,bus=ahci.0,drive=systemdisk \
    -no-reboot \
    -no-shutdown
  )
}
