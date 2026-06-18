# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Slippy is a native desktop text-diff utility for comparing two pasted snippets (paste left / paste right / compare / copy the unified diff). Rust + `fltk-rs` GUI. v1 is paste-first and deliberately narrow — `DESIGN.md` is authoritative for product direction, layout, and color tokens; `IMPLEMENTATION_PLAN.md` is authoritative for the full behavioral contract, state rules, status tables, and test plan. `TODOS.md` tracks explicitly out-of-scope items. Consult these before changing behavior, colors, or status strings.

## Commands

```bash
cargo run                            # run the app (bundled FLTK, default)
cargo test                           # run all tests
cargo test diff_core                 # run one module's tests
cargo test inline -- --nocapture     # run tests whose name contains "inline", show println output
cargo build --release                # release build
cargo run --features wayland         # optional Wayland build
```

`cargo fmt` / `cargo clippy` are available (standard Rust); there is no project-specific lint config.

## Architecture

`main.rs` only calls `slippy::ui_fltk::run()`; `lib.rs` declares the four modules. The codebase is intentionally layered, and the **dependency rules between layers are the most important invariant to preserve when editing**:

- `diff_core` — pure diff logic (`similar`-based unified diff, line classification, inline replacement pairing, auto-diff thresholds). **Must not** depend on FLTK, clipboard (`arboard`), `config`, or threading.
- `app_state` — the UI-independent state machine (text, dirty/stale flags, monotonic request ids, status transitions). **Must not** depend on FLTK or `arboard`. Clipboard failures reach it only as status strings.
- `ui_fltk` — owns all FLTK widgets, styling/coloring, clipboard integration, worker-thread spawning, debounce timers, and keyboard shortcuts. The only module allowed to touch `arboard` and FLTK.
- `config` — persists only layout/theme/font metadata. **Must never** serialize pasted text or diff output (a privacy invariant, asserted in tests).

`diff_core` and `app_state` are pure and fully unit-tested; `ui_fltk` is the integration boundary and is verified by the manual GUI smoke checklist in `IMPLEMENTATION_PLAN.md`.

### Concurrency: the stale-worker guard

Diffing runs on a freshly spawned thread per request; there is **no cancellation**. Correctness depends on ignoring stale results, not killing workers:

1. Each `create_*_request` bumps a monotonic `latest_request_id` and snapshots the current text into a `DiffRequest`.
2. Any edit/paste after a request starts sets `dirty_since_latest_request = true`.
3. `apply_result(DiffResult)` applies **only if** the result's id equals `latest_request_id` **and** no edit happened since the request started; otherwise it returns `IgnoredStaleRequest` / `IgnoredBecauseDirty` and the previous diff is kept.

Results cross back to the UI thread through a single FLTK channel (`UiMessage::DiffReady`). Shared state is `Rc<RefCell<AppState>>` paired with `Rc<RefCell<UiHandles>>`. Do not weaken this guard — "stale worker output overwrites a newer edit" is an explicit acceptance failure.

### Auto-diff vs manual Compare

`should_auto_diff` returns false (skips debounced auto-diff, status → "Large input - press Compare to update.") when combined input exceeds 256 KiB **or** 8,000 lines. Manual Compare (`create_manual_request`) always bypasses the thresholds. Debounce is 300ms (`DEBOUNCE_MS`).

### Diff output contract (`diff_core`)

- Equal text → exactly `No differences\n`.
- Output always ends with exactly one trailing newline.
- Display consumes `Vec<DiffOp>` directly; copy renders standard unified text via `render_unified_diff`.
- Similarity-weighted banded alignment (fuzzy LCS); inline fragments get background colors via FLTK `StyleTableEntryExt` — no text brackets, no `@@` in display.

### Config & privacy

Layout/theme/font only, via the `directories` crate with app identity `dev.wwwhynot3.slippy` / `Slippy`. `vertical_split` defaults to 0.45, clamped 0.30–0.70. Invalid or missing config falls back to defaults and reports status; save errors never crash the app. Config APIs accept injected paths for testing (`tempfile` is a dev-dependency).

## Conventions worth keeping

- Modules are developed test-first; keep pure logic in `diff_core`/`app_state` so it stays unit-testable and push FLTK/clipboard concerns up into `ui_fltk`.
- When an `fltk-rs` release changes the Wayland feature spelling, update only the feature mapping in `Cargo.toml` (`wayland = ["fltk/use-wayland"]`) — do not chase it in code.
- Palette tokens and status-string tables live in `DESIGN.md` / `IMPLEMENTATION_PLAN.md`; keep code constants in sync with those tables rather than introducing ad-hoc colors or status text.
- All diff thresholds/ratios live in `diff_core::DiffOptions::default()`; `config::DiffOverrides` carries optional overrides; `ui_fltk::diff_options_from_config` bridges them.
