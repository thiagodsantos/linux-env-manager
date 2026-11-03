---
name: react-frontend
description: Implement or review the Linux environment manager's React, TypeScript, Vite, and Tailwind frontend, including accessible variable CRUD, import preview, Tauri API integration, and UI tests.
---

# React and TypeScript Frontend

Use this skill for frontend implementation or review in this repository. Read `PLAN.md` first, inspect the existing app structure and design conventions, and follow the project's installed versions and scripts.

## Responsibilities

- Build the variable list and add/edit/delete/import workflows described in `PLAN.md`.
- Keep UI components focused on presentation and interaction. Put calls to native functionality behind a small typed API/service module; do not invoke Tauri commands directly from many components.
- Represent loading, empty, success, validation, and recoverable error states explicitly.
- Clearly present the manager's persistence boundary: saving does not alter the environment of already-running Linux processes. Bash/Zsh support is a separate explicit opt-in for new shell sessions.

## Frontend conventions

- Use TypeScript with strict, explicit types for variables, import previews, command inputs, and results. Avoid `any` and unsafe casts unless there is a documented reason.
- Use the configured path alias (planned as `@/*` → `src/*`) for every internal import. Never use relative imports, including in tests. Keep TypeScript, Vite, test, and lint configuration aligned with the alias.
- Use React, Vite, and Tailwind CSS already configured by the project. Prefer existing components and dependencies over adding new ones.
- Make forms and dialogs keyboard-accessible, label inputs, associate validation messages with fields, and provide visible focus states.
- Mask variable values by default and provide an intentional reveal/hide control. Avoid exposing secret values in logs, toasts, error text, or unnecessary DOM content.
- Deletion must require a confirmation modal identifying the variable key. Cancel must not call the delete API; confirm should call it once and handle errors without falsely removing the row.
- Import must show a preview that distinguishes new keys, updated keys, and invalid/unsupported lines before applying. Reflect the backend's actual parser semantics; do not duplicate or silently diverge from backend validation/merge rules.
- Do not silently overwrite a duplicate key from the add form. Editing must show validation errors and prevent collisions.
- Shell integration controls are opt-in per supported shell (Bash/Zsh); explain the managed `.bashrc`/`.zshrc` block, disable/removal behavior, and that existing shells require re-sourcing. Keep calls behind the typed API.

## Visual design and component architecture

- Follow the palette in `PLAN.md`: `#780000`, `#C1121F`, `#FDF0D5`, `#003049`, and `#669BBC`. Define named design tokens and expose them through Tailwind; prefer semantic tokens over scattering raw hex values through components.
- Use Inter as the global UI font from [Google Fonts](https://fonts.googleapis.com/css2?family=Inter:ital,opsz,wght@0,14..32,100..900;1,14..32,100..900), with a system sans-serif fallback.
- Verify accessible contrast for actual text, control, and focus-indicator color pairings; do not assume every palette combination is readable.
- Organize reusable UI with a pragmatic Atomic Design structure: atoms for primitives, molecules for small combinations, organisms for larger interface sections, templates for page layouts, and pages for route-level composition.
- Keep the taxonomy lightweight. Compose components where it helps reuse and consistency, but do not create a component or abstraction for every element. Use the configured path alias for imports throughout the structure.

## Testing and quality

- Use the project's configured Vitest and React Testing Library setup. Test user-visible behavior with accessible queries and user interactions rather than component internals.
- Mock the typed native API at the boundary; do not mock component internals. Cover create/edit validation, duplicate handling, reveal/hide, delete confirmation cancel/confirm, import preview/apply feedback, shell integration enable/disable, and loading/error/empty states.
- Maintain configured coverage thresholds above 80% for statements, branches, functions, and lines; add meaningful tests rather than weakening thresholds.
- Run focused frontend tests, type checking, linting, and relevant Makefile quality targets after changes. State clearly which checks were run and whether they passed.
- Update frontend-facing documentation when user flows or limitations change.
