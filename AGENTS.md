# Repository Guidelines

## Project Structure & Module Organization
`src/` holds the app and core logic. Keep `main.rs` thin; it should only launch `slippy::ui_fltk::run()`. Preserve the layering described in `CLAUDE.md`: `diff_core` for pure diff logic, `app_state` for the UI-independent state machine, `diff_view` for rendered diff rows, `ui_fltk` for FLTK widgets and clipboard work, and `config` for persisted UI metadata only. Supporting material lives in `docs/`, packaging assets in `packaging/`, icons in `assets/icons/`, and GUI probes in `examples/`.

## Build, Test, and Development Commands
Use Cargo for the normal workflow:

```bash
cargo run                   # launch the desktop app
cargo run --features wayland
cargo test                  # run all unit tests
cargo test diff_core        # run a focused test subset
cargo fmt                   # format Rust sources
cargo clippy --all-targets --all-features
cargo build --release       # produce release binaries
```

## Coding Style & Naming Conventions
Follow standard Rust formatting with `cargo fmt` and keep Clippy-clean when practical. Use `snake_case` for functions/modules, `CamelCase` for types, and short, explicit names for diff/status concepts. Prefer pure logic in `diff_core`, `app_state`, and `diff_view`; keep FLTK, threading, and clipboard access inside `ui_fltk`. Do not add persistence for pasted text or diff output; config storage is limited to layout, theme, and font metadata.

## Testing Guidelines
Tests are primarily inline unit tests inside the source modules, with targeted probes in `examples/` and `tests/` when needed. Add tests next to the logic you change, especially for diff rendering, stale-request handling, config normalization, and privacy guarantees. Run `cargo test` before opening a PR; use focused commands such as `cargo test inline -- --nocapture` while iterating.

## Commit & Pull Request Guidelines
Recent history follows Conventional Commits such as `fix: restore linux release builds`, `feat: add Slippy app logo`, and `ci: add release automation`. Keep that format: `feat:`, `fix:`, `ci:`, `docs:`, `chore:`. PRs should explain the user-visible change, note any packaging or platform impact, link the relevant issue when applicable, and include screenshots or short recordings for UI changes. Mention the verification you ran (`cargo test`, manual GUI smoke checks, Wayland build, etc.).

## Architecture & Safety Notes
Do not break the stale-worker guard: old diff results must be ignored rather than canceling worker threads. Keep status strings, palette choices, and product behavior aligned with `DESIGN.md`, `IMPLEMENTATION_PLAN.md`, and `TODOS.md` before changing UX or scope.
