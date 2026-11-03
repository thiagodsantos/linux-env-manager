# Linux Environment Variable Manager — Implementation Plan

## 1. Goal

Build a Linux desktop app for viewing, adding, editing, deleting, and importing user-managed environment variables. Importing a `.env` file must add new keys and update existing keys. Deleting a variable requires an explicit confirmation dialog.

The app must be built with Tauri v2, React, TypeScript, Vite, Tailwind CSS, and PNPM. It must include unit tests with greater than 80% coverage, a Makefile, a README, and a `.gitignore`.

## 2. Platform and persistence decision

Linux environment variables belong to processes: a desktop app cannot change the environment of already-running shells or other processes. Persisting variables also does not, by itself, make them available in future shells.

For the MVP:

- Manage a **user-scoped** set of variables, not system-wide variables.
- Store the canonical data in an app-owned file under Tauri's per-user application configuration directory. Use a documented, versioned JSON format encrypted with authenticated encryption; keep its key in the user's Secret Service-compatible desktop keyring, never beside the store. Migrate existing plaintext version-1 data on first successful load. Do not edit `/etc/environment`, arbitrary paths, or shell startup files implicitly.
- Make it clear in the UI and README that saving changes updates the manager's persistent store, not the current process environment.
- Loading values into future shells is a separate, **opt-in** integration. The first release supports Bash and Zsh, configured independently in the app. Enabling adds a narrowly scoped managed block to only `~/.bashrc` or `~/.zshrc`; disabling removes only that block and preserves unrelated content. The block invokes the app's export mode, which decrypts the app-owned JSON store and emits safely quoted exports; it never sources imported contents or writes plaintext values into startup files. Changes apply to new shell sessions (or after manually sourcing the RC file), not already-running processes.

This boundary avoids implying that a GUI can update every Linux process and prevents accidental edits to system or user shell configuration.

## 3. Proposed architecture

### Frontend

- React + TypeScript screens and reusable UI components.
- Vite for development and bundling; Tailwind CSS for styling.
- A small typed API module wraps Tauri `invoke` calls. Components do not call native commands directly.
- UI state covers loading, empty, populated, validation, import preview, and recoverable error states.

### Frontend visual design and component structure

- Use the Coolors palette [`#05668D`, `#427AA1`, `#EBF2FA`, `#679436`, `#A5BE00`](https://coolors.co/palette/05668d-427aa1-ebf2fa-679436-a5be00). Define named design tokens and expose them through the Tailwind theme; use semantic tokens for surfaces, text, actions, and feedback rather than scattering raw hex values through components.
- Use Inter as the primary UI font, sourced from [Google Fonts](https://fonts.googleapis.com/css2?family=Inter:ital,opsz,wght@0,14..32,100..900;1,14..32,100..900), with a system sans-serif fallback. Apply it consistently through the global theme.
- Meet accessible contrast requirements for text, controls, and focus indicators; verify actual foreground/background pairings rather than assuming every palette combination is readable.
- Organize reusable frontend components using Atomic Design: **atoms** for primitives, **molecules** for small combinations, **organisms** for larger interface sections, **templates** for page layouts, and **pages** for route-level composition.
- Keep this taxonomy lightweight and pragmatic: compose larger components from smaller ones where useful, but avoid creating a component or abstraction for every element. Use the configured path alias for all component imports.

### Tauri / Rust backend

- Tauri v2 commands form the narrow frontend/native boundary: list variables, create or update a variable, delete a variable, and import/preview a `.env` file.
- A domain/service layer owns validation, uniqueness, import merge rules, and persistence. Keep filesystem operations behind the storage boundary so behavior can be tested without a GUI.
- A file-backed repository reads and writes only the app-owned store for normal CRUD. The import flow accepts a user-selected file through a native file picker rather than accepting arbitrary paths from untrusted frontend input.
- Persist safely: validate before mutation, encrypt the complete JSON payload with AES-256-GCM using a random key stored in Secret Service, write a temporary file and atomically replace the ciphertext store, preserve restrictive user-only permissions where supported, and surface actionable, value-free errors.
- Do not log variable values or expose secrets in error messages. Never execute imported file contents as shell code.

Use SOLID and DRY pragmatically: isolate the domain rules and persistence/native boundary, but avoid speculative plugin systems, excessive interfaces, or abstractions with only one unnecessary use. Keep components and services cohesive and implement only the agreed MVP.

## 4. User stories and acceptance criteria

1. **View variables**
   - On launch, show the saved user variables with key and value.
   - Provide clear loading, empty, and error states.
   - Values should be masked by default with an explicit reveal/hide control, so secrets are not immediately exposed on screen.

2. **Add a variable**
   - A form accepts a key and value and reports validation errors inline.
   - Keys must be non-empty and follow a documented environment-variable naming rule (recommend `^[A-Za-z_][A-Za-z0-9_]*$`).
   - Reject duplicate keys on create, or offer an explicit update path; do not silently overwrite from the add form.

3. **Edit a variable**
   - The user can change a saved value and, if supported, its key.
   - Validate the new key and prevent collisions with another variable.
   - Saving persists the change and refreshes the list; failures leave the previous saved value intact.

4. **Delete a variable**
   - Each row has a delete action.
   - Show a confirmation modal identifying the key before deletion. Cancel leaves data unchanged; confirm deletes and refreshes the list.

5. **Import a `.env` file**
   - The user chooses a file using a native picker; the app parses it as data and never sources or executes it.
   - Show a preview/count of new keys, keys that will be updated, and invalid or unsupported lines before applying changes.
   - Applying the import updates matching keys and adds new keys in one validated operation. A parse or write failure must not leave a partially applied import.
   - Define and document supported syntax. At minimum support blank lines, comments, `KEY=value`, and values containing `=`. Decide explicitly how quoted values, `export KEY=...`, interpolation, duplicate keys in one file, and malformed lines behave; do not claim full shell compatibility unless implemented.

## 5. Technology and repository setup

- Install the latest stable Node.js release available when implementation begins; record the chosen version and use a repository version pin such as `.nvmrc` (or the team's equivalent).
- Enable Corepack and use the latest stable PNPM supported by that Node release. Commit `pnpm-lock.yaml`; declare the package manager and Node engine in `package.json`.
- Scaffold using the official Tauri v2 create-project flow, selecting React, TypeScript, and Vite. Record required Linux system packages and setup steps in the README.
- Add Tailwind CSS using its current Vite-compatible setup. Define the approved palette as theme/design tokens and configure Inter as the global UI font. Pin dependencies through the lockfile rather than relying on floating installs.
- Configure a single path alias (recommend `@/*` → `src/*`) consistently in TypeScript and Vite. Use it for all internal frontend imports; do not use relative imports.
- Add a root `Makefile` with discoverable targets such as `install`, `dev`, `build`, `test`, `test-coverage`, `lint`, `typecheck`, and `check`. Targets should delegate to PNPM/Cargo and avoid duplicating package scripts.
- Add `.gitignore` entries for Node dependencies/build output, Rust `target`, Tauri build output, local environment files/secrets, and editor/OS artifacts. Do not ignore lockfiles.

### Project-local Zed agent skills

Provide reusable, focused instructions under `.agents/skills/` so Zed can apply repository-specific guidance to frontend and backend work:

- `.agents/skills/tauri-backend/SKILL.md` — Tauri v2 and Rust responsibilities, safe user-scoped persistence, CRUD commands, `.env` parsing/import behavior, security boundaries, and backend tests.
- `.agents/skills/react-frontend/SKILL.md` — React/TypeScript/Vite/Tailwind conventions, alias-only internal imports, accessible CRUD and import workflows, typed Tauri API boundary, and UI tests.
- Treat these as role-specific skills for coding agents, not separate autonomous processes. Each skill should direct the agent to read this plan, inspect current project conventions, stay within its frontend or backend scope, and run the relevant quality checks.
- Keep skill guidance consistent with this plan when requirements change; skills should reinforce the plan rather than introduce conflicting behavior.

## 6. Testing and quality gates

### Frontend

- Use Vitest with React Testing Library and user-event for unit/component tests.
- Test forms and validation, masking/reveal behavior, edit and delete interactions, confirmation cancel/confirm, import preview and merge feedback, and loading/error/empty states.
- Mock the typed Tauri API at the boundary; do not mock component internals.

### Rust backend

- Use Rust unit tests for key validation, `.env` parsing, duplicate handling, add/update merge behavior, delete behavior, and storage failure/atomicity cases where practical.
- Keep parsing and merge logic independent of Tauri command handlers so it can be tested directly.

### Coverage

- Configure frontend coverage reporting and enforce **more than 80%** for statements, branches, functions, and lines (threshold: 81% to avoid ambiguity at exactly 80%).
- Include backend tests in the normal test command. Report frontend and Rust coverage separately unless a combined metric is deliberately configured; do not imply Rust coverage is covered by a frontend-only number.
- Add CI or a documented local quality command that runs formatting, linting, type checking, tests, and coverage. Do not lower thresholds to make an incomplete test suite pass.

## 7. Delivery phases

1. **Confirm scope and safety** — Keep persistence user-scoped; Bash/Zsh integration is approved but remains explicit opt-in. Define import syntax and malformed-line policy.
2. **Scaffold** — Create the Tauri v2 React/TypeScript/Vite project, establish latest stable Node/PNPM versions, add aliases, Tailwind, scripts, Makefile, `.gitignore`, initial README, and the project-local backend and frontend Zed skills.
3. **Domain and storage** — Implement and test validation, canonical persistence, safe atomic writes, CRUD, and `.env` parsing/merge logic in Rust.
4. **Native API** — Add typed Tauri commands and native file selection for import; configure the minimum permissions/capabilities required.
5. **Frontend workflows** — Establish the palette tokens and Inter typography, organize reusable components with a pragmatic Atomic Design structure, then build the variable list, add/edit forms, masked values, delete confirmation, import preview/apply flow, and error states.
6. **Verification and documentation** — Reach the enforced coverage threshold, run all quality gates, verify a Linux development/build workflow, and document installation, usage, persistence boundaries, import syntax, and limitations.
7. **Opt-in shell integration** — Support Bash and Zsh using reversible managed blocks in the user's `.bashrc` and `.zshrc`; test quoting and block removal. Keep it separate from normal CRUD and document that it only affects future shells.

## 8. Definition of done

- CRUD and `.env` import behavior meet the acceptance criteria, including update-existing/add-new semantics and confirmed deletion.
- No frontend internal relative imports remain; alias configuration works in Vite, TypeScript, tests, and linting.
- `make check` (or its documented equivalent) passes, and frontend coverage thresholds are enforced above 80% in every configured category.
- Linux setup, run, build, test, import semantics, data location, opt-in Bash/Zsh shell integration, and process-environment limitations are documented in `README.md`.
- The frontend consistently uses the specified Coolors palette (`#05668D`, `#427AA1`, `#EBF2FA`, `#679436`, `#A5BE00`) and Inter font, with accessible contrast, and organizes components using the documented Atomic Design levels.
- No secret values are logged, the JSON store contains authenticated ciphertext rather than variable values, imported contents are never executed, and shell/system configuration is not modified without explicit user opt-in. Shell integration changes only its marked user-level Bash/Zsh block and is removable.
- Project-local Zed skills exist at `.agents/skills/tauri-backend/SKILL.md` and `.agents/skills/react-frontend/SKILL.md`, and their instructions agree with this plan.
