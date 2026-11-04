#!/usr/bin/env bash
set -euo pipefail

CURL=${CURL:-curl}

# Verify curl is available before downloading the Rust installer.
check_curl() {
  if ! command -v "$CURL" >/dev/null 2>&1; then
    echo "curl is required to install Rust. Install curl and retry." >&2
    exit 1
  fi
}

# Install the minimal stable Rust toolchain through the official rustup installer.
install_rust() {
  "$CURL" --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable
  echo 'Rust stable and Cargo are installed. Run: . "$HOME/.cargo/env" (or open a new shell).'
}

check_curl
install_rust
