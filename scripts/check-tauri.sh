#!/usr/bin/env bash
set -euo pipefail

CARGO=${CARGO:-cargo}

# Ensure Cargo is installed and provide the setup target when it is missing.
check_cargo() {
  if ! command -v "$CARGO" >/dev/null 2>&1; then
    echo "Cargo is required. Run 'make install-rust', then source ~/.cargo/env or open a new shell." >&2
    exit 1
  fi
}

# Ensure pkg-config is installed and provide the setup target when it is missing.
check_pkg_config() {
  if ! command -v pkg-config >/dev/null 2>&1; then
    echo "pkg-config is required. Run 'make install-linux-deps' or install the packages listed in README.md." >&2
    exit 1
  fi
}

# Check required Tauri pkg-config modules and report any missing libraries.
check_tauri_libraries() {
  local missing=""
  local package
  for package in gtk+-3.0 webkit2gtk-4.1 librsvg-2.0; do
    if ! pkg-config --exists "$package"; then
      missing+=" $package"
    fi
  done

  if [[ -n "$missing" ]]; then
    echo "Missing pkg-config libraries:$missing. Run 'make install-linux-deps' and retry." >&2
    exit 1
  fi
}

check_cargo
check_pkg_config
check_tauri_libraries
