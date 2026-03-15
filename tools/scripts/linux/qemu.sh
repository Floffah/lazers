#!/usr/bin/env bash

platform_find_ovmf_code() {
  local candidate

  shopt -s nullglob

  for candidate in \
    /usr/share/OVMF/OVMF_CODE.fd \
    /usr/share/OVMF/OVMF_CODE_4M.fd \
    /usr/share/ovmf/OVMF_CODE.fd \
    /usr/share/ovmf/OVMF_CODE_4M.fd \
    /usr/share/edk2/ovmf/OVMF_CODE.fd \
    /usr/share/edk2/ovmf/OVMF_CODE_4M.fd
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
    /usr/share/OVMF/OVMF_VARS.fd \
    /usr/share/OVMF/OVMF_VARS_4M.fd \
    /usr/share/ovmf/OVMF_VARS.fd \
    /usr/share/ovmf/OVMF_VARS_4M.fd \
    /usr/share/edk2/ovmf/OVMF_VARS.fd \
    /usr/share/edk2/ovmf/OVMF_VARS_4M.fd
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

  if command -v magick >/dev/null 2>&1; then
    magick "$ppm_path" "$png_path"
    return 0
  fi

  if command -v convert >/dev/null 2>&1; then
    convert "$ppm_path" "$png_path"
    return 0
  fi

  if command -v pnmtopng >/dev/null 2>&1; then
    pnmtopng "$ppm_path" > "$png_path"
    return 0
  fi

  echo "missing a PPM-to-PNG conversion tool (install ImageMagick or netpbm)" >&2
  exit 1
}
