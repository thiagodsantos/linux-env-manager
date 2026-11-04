PNPM ?= pnpm
CARGO ?= cargo
CURL ?= curl

.PHONY: install install-rust install-linux-deps dev build test test-coverage lint typecheck check check-tauri clean

# Install the stable Rust toolchain and Cargo.
install-rust:
	CURL="$(CURL)" ./scripts/install-rust.sh

# Install the system packages required to build the Tauri application on Linux.
install-linux-deps:
	./scripts/install-linux-deps.sh

# Verify that Rust and the required Linux Tauri libraries are available.
check-tauri:
	CARGO="$(CARGO)" ./scripts/check-tauri.sh

# Install JavaScript dependencies from the frozen pnpm lockfile.
install:
	$(PNPM) install --frozen-lockfile

# Start the Tauri application in development mode after checking prerequisites.
dev: check-tauri
	$(PNPM) tauri dev

# Build the Tauri application after checking prerequisites.
build: check-tauri
	$(PNPM) tauri build

# Run the frontend and Rust test suites after checking prerequisites.
test: check-tauri
	$(PNPM) test
	$(CARGO) test --manifest-path src-tauri/Cargo.toml

# Run the frontend test suite with coverage reporting.
test-coverage:
	$(PNPM) test:coverage

# Run the frontend linter.
lint:
	$(PNPM) lint

# Check frontend TypeScript types.
typecheck:
	$(PNPM) typecheck

# Run linting, type checks, tests, coverage, and Rust formatting checks.
check: lint typecheck test test-coverage
	$(CARGO) fmt --manifest-path src-tauri/Cargo.toml --all -- --check

# Remove generated frontend and Rust build artifacts and coverage output.
clean:
	rm -rf dist coverage src-tauri/target
