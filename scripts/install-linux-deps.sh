#!/usr/bin/env bash
set -euo pipefail

# Install the Linux Tauri development packages using the detected package manager.
install_linux_deps() {
  if command -v dnf >/dev/null 2>&1; then
    sudo dnf install pkgconf-pkg-config gtk3-devel webkit2gtk4.1-devel openssl-devel curl wget file libxdo-devel librsvg2-devel libayatana-appindicator-gtk3
  elif command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update
    sudo apt-get install build-essential pkg-config curl wget file libgtk-3-dev libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev libwebkit2gtk-4.1-dev
  else
    echo "Unsupported package manager. Install the Linux Tauri packages listed in README.md manually." >&2
    exit 1
  fi
}

install_linux_deps
