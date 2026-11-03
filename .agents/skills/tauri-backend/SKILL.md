---
name: tauri-backend
description: Implement or review the Linux environment manager's Tauri v2 and Rust backend, including safe variable persistence, CRUD commands, .env parsing/import, and backend tests.
---

# Tauri and Rust Backend

Use this skill for backend implementation or review in this repository. Read `PLAN.md` first and inspect the existing Rust/Tauri structure before changing code. Follow the project's actual conventions and dependency versions rather than assuming scaffold filenames or APIs.

## Responsibilities

- Own the native/domain boundary: variable validation, CRUD rules, persistent storage, `.env` parsing and import merge behavior, Tauri commands, and backend tests.
- Keep commands thin. Put reusable domain behavior in testable Rust functions/services, and keep filesystem access behind a focused storage boundary.
- Preserve the documented MVP boundary: variables are user-scoped app data. Do not claim that saving changes an already-running process environment.

## Safety and behavior

- Store data only in the app-owned per-user configuration location resolved through supported Tauri APIs. Encrypt the complete JSON payload using authenticated encryption; keep the key in the user's Secret Service-compatible desktop keyring, never adjacent to the data file. Migrate legacy plaintext stores safely. Do not edit `/etc/environment`, arbitrary paths, or shell startup files as part of normal CRUD.
- Shell integration is separately opt-in and supports Bash/Zsh only. Add/remove only the clearly marked managed block in the selected user's `.bashrc` or `.zshrc`; preserve unrelated content. The block invokes the app's export mode to decrypt the encrypted store using the desktop keyring and emit validated, shell-quoted exports; never put plaintext values in startup files or source imported contents. Explain that new sessions (or manual re-sourcing) are required and already-running processes are unchanged.
- Use native file selection for user-chosen imports. Do not trust arbitrary frontend-supplied paths.
- Parse `.env` contents as data; never source, evaluate, or execute them. Do not log variable values or include them in error messages.
- Validate keys using the naming rule documented in `PLAN.md`. Reject invalid keys and handle duplicate keys deterministically.
- Import is an add-or-update merge: incoming keys replace existing values, new keys are added. Validate and apply the entire import atomically; malformed input or write failure must not leave a partial update.
- Define parser behavior for comments, blank lines, `KEY=value`, values containing `=`, quotes, `export`, duplicate keys, interpolation, and malformed lines. Keep documentation aligned with actual support; do not imply full shell syntax compatibility.
- Write encrypted persistent data safely using a temporary file and atomic replacement where supported. Use restrictive per-user permissions when supported and preserve the old data if a write fails.
- Return actionable, value-free errors. Avoid panics for routine invalid input or I/O failures.
- Request only the Tauri capabilities and permissions needed for the implemented commands and file picker.

## Design and tests

- Apply SOLID/DRY pragmatically: isolate domain logic and I/O seams, but avoid speculative abstractions or unnecessary dependencies.
- Unit-test key validation, parser behavior, add/update/delete rules, import merge semantics, relevant persistence failure cases, and shell quoting/managed-block insertion and removal.
- Keep parsing and merge logic independently testable without launching the Tauri app.
- Run relevant Rust formatting, tests, and project quality targets after changes. Report commands and failures accurately; do not claim unrun checks passed.
- Update `README.md` when implementation changes the documented data location, import syntax, setup, or environment-variable behavior.
