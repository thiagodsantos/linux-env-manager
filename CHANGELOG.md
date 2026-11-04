# Changelog

Notable changes to Linux Environment Manager are documented here. This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and uses semantic versioning.

## [0.5.0] - 2026-11-03

### Added

- Named environment profiles with descriptions and colors, plus profile-to-profile variable import when creating a profile.
- A shared General profile whose variables are inherited by custom profiles unless locally overridden.
- Profile selection and application controls, plus a profile-actions dropdown for `.env` import, clearing profile variables, and deleting custom profiles.
- Confirmation before profile deletion. General is protected, and deleting the active profile falls back to General.
- A Mermaid app-flow diagram in the README.

### Changed

- Existing single-environment stores migrate into the General profile.
- Opt-in Bash/Zsh exports use General variables plus overrides from the applied profile for future shell sessions.

## [0.4.0] - 2026-11-03

### Added

- Authenticated AES-256-GCM encryption for the saved variable store, with the encryption key held in the desktop keyring.
- Confirmation before clearing the saved environment.

### Changed

- Legacy plaintext stores are migrated to encrypted storage on successful load.

## [0.3.0] - 2026-11-03

### Added

- The implementation plan and project-local frontend and backend guidance for future development.
- A focused Rust backend structure separating commands, domain rules, models, shell integration, state, and storage.
- Frontend palette tokens and Rust formatting in the project workflow.

## [0.2.0] - 2026-11-03

### Added

- Continuous integration for frontend quality checks, Rust tests, and Linux Tauri builds.
- Linux release workflows for AppImage, Debian, and RPM packages.
- Conventional Commit-based versioning and automated GitHub Releases.

### Changed

- Documented the Python runtime requirement for the original shell-integration export flow.

## [0.1.0] - 2026-11-03

### Added

- Initial React and Tauri desktop app for user-scoped environment-variable management.
- Variable create, edit, confirmed delete, and `.env` import/preview workflows.
- Atomic JSON persistence in the user's application configuration directory.
- Optional, reversible Bash and Zsh startup-file integration.
- Frontend and Rust tests, linting, type checking, and coverage checks.
