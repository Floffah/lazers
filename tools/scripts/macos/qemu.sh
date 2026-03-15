#!/usr/bin/env bash

platform_find_ovmf_code() {
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

platform_find_ovmf_vars_template() {
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

platform_convert_ppm_to_png() {
  local ppm_path="$1"
  local png_path="$2"

  require_command sips
  sips -s format png "$ppm_path" --out "$png_path" >/dev/null
}
