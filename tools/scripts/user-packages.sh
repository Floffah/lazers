#!/usr/bin/env bash

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

USER_DIR="$ROOT_DIR/user"

list_user_packages() {
  find "$USER_DIR" -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort
}

load_user_packages() {
  mapfile -t USER_PACKAGES < <(list_user_packages)
  if [[ ${#USER_PACKAGES[@]} -eq 0 ]]; then
    echo "no user packages found under $USER_DIR" >&2
    return 1
  fi
}

load_user_packages_from_env() {
  if [[ -z "${LAZERS_USER_PACKAGES:-}" ]]; then
    echo "missing LAZERS_USER_PACKAGES for image assembly" >&2
    return 1
  fi

  USER_PACKAGES=()
  while IFS= read -r package; do
    if [[ -n "$package" ]]; then
      USER_PACKAGES+=("$package")
    fi
  done <<< "$LAZERS_USER_PACKAGES"

  if [[ ${#USER_PACKAGES[@]} -eq 0 ]]; then
    echo "LAZERS_USER_PACKAGES did not contain any package names" >&2
    return 1
  fi
}
