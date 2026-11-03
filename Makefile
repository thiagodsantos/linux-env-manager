PNPM ?= pnpm
CARGO ?= cargo
CURL ?= curl

.PHONY: install install-rust install-linux-deps dev build test test-coverage lint typecheck check check-tauri clean

install-rust:
	@command -v "$(CURL)" >/dev/null 2>&1 || { echo "curl is required to install Rust. Install curl and retry."; exit 1; }
	$(CURL) --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
	@echo 'Rust stable and Cargo are installed. Run: . "$$HOME/.cargo/env" (or open a new shell).'

install-linux-deps:
	@if command -v dnf >/dev/null 2>&1; then \
		sudo dnf install pkgconf-pkg-config gtk3-devel webkit2gtk4.1-devel openssl-devel curl wget file python3 libxdo-devel librsvg2-devel libayatana-appindicator-gtk3; \
	elif command -v apt-get >/dev/null 2>&1; then \
		sudo apt-get update && sudo apt-get install build-essential pkg-config curl wget file python3 libgtk-3-dev libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev libwebkit2gtk-4.1-dev; \
	else \
		echo "Unsupported package manager. Install the Linux Tauri packages listed in README.md manually."; exit 1; \
	fi

check-tauri:
	@command -v "$(CARGO)" >/dev/null 2>&1 || { echo "Cargo is required. Run 'make install-rust', then source ~/.cargo/env or open a new shell."; exit 1; }
	@command -v pkg-config >/dev/null 2>&1 || { echo "pkg-config is required. Run 'make install-linux-deps' or install the packages listed in README.md."; exit 1; }
	@missing=""; for package in gtk+-3.0 webkit2gtk-4.1 librsvg-2.0; do \
		pkg-config --exists "$$package" || missing="$$missing $$package"; \
	done; \
	if [ -n "$$missing" ]; then \
		echo "Missing pkg-config libraries:$$missing. Run 'make install-linux-deps' and retry."; exit 1; \
	fi

install:
	$(PNPM) install --frozen-lockfile

dev: check-tauri
	$(PNPM) tauri dev

build: check-tauri
	$(PNPM) tauri build

test: check-tauri
	$(PNPM) test
	$(CARGO) test --manifest-path src-tauri/Cargo.toml

test-coverage:
	$(PNPM) test:coverage

lint:
	$(PNPM) lint

typecheck:
	$(PNPM) typecheck

check: lint typecheck test test-coverage
	$(CARGO) fmt --manifest-path src-tauri/Cargo.toml --all -- --check

clean:
	rm -rf dist coverage src-tauri/target
