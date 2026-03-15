#!/usr/bin/env bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="$ROOT_DIR/build"

detect_host_os() {
  case "$(uname -s)" in
    Darwin)
      printf 'macos\n'
      ;;
    Linux)
      printf 'linux\n'
      ;;
    *)
      return 1
      ;;
  esac
}

HOST_OS="${LAZERS_HOST_OS:-$(detect_host_os || true)}"

require_supported_host() {
  if [[ -z "$HOST_OS" ]]; then
    echo "unsupported host operating system: $(uname -s)" >&2
    exit 1
  fi
}

source_platform_script() {
  local relative_path="$1"
  local script_path

  require_supported_host
  script_path="$SCRIPT_DIR/$HOST_OS/$relative_path"
  if [[ ! -f "$script_path" ]]; then
    echo "missing platform script at $script_path" >&2
    exit 1
  fi

  # shellcheck source=/dev/null
  source "$script_path"
}

require_command() {
  local command_name="$1"

  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "missing required command: $command_name" >&2
    exit 1
  fi
}
