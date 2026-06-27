# Custom Diff Canvas And Pin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the diff text display with a drawn IDE-like canvas, add matching input gutters, and add a topmost-window pin toggle.

**Architecture:** Keep diff computation and `diff_view.rs` unchanged. Add UI-only rendering helpers and FLTK draw callbacks in `src/ui_fltk.rs`; keep `TextEditor` for editable inputs and use custom `Frame` drawing for gutters and diff rows.

**Tech Stack:** Rust, FLTK, existing `diff_core`, `diff_view`, and app state modules.

---

### Task 1: Add Testable UI Helpers

**Files:**
- Modify: `src/ui_fltk.rs`

- [ ] Add helper tests for line counts, gutter labels, pin labels, diff canvas height, and overview rail slots.
- [ ] Run the targeted tests and verify the new tests fail before implementation.
- [ ] Implement the helpers with no widget side effects.
- [ ] Run the targeted tests and verify they pass.

### Task 2: Replace Diff Text Display With Drawn Canvas

**Files:**
- Modify: `src/ui_fltk.rs`

- [ ] Replace `TextDisplay` diff handles with a `Scroll` and canvas `Frame`.
- [ ] Store the current `RenderedDiffView` in shared UI state for the draw callback.
- [ ] Draw header, old/new/marker gutters, row backgrounds, inline token highlights, fold rows, and notice rows.
- [ ] Keep `Copy Diff` using `render_unified_diff(state.diff())`.
- [ ] Run `cargo test`.

### Task 3: Add Drawn Input Gutters

**Files:**
- Modify: `src/ui_fltk.rs`

- [ ] Wrap each `TextEditor` in a row flex containing a gutter `Frame` and the editor.
- [ ] Disable the built-in FLTK line number gutter.
- [ ] Maintain absolute top line tracking on the editors.
- [ ] Redraw gutters after buffer changes and editor events.
- [ ] Run `cargo test`.

### Task 4: Add Pin Toggle

**Files:**
- Modify: `src/ui_fltk.rs`

- [ ] Add a `Pin` button to the diff toolbar and store it in `UiHandles`.
- [ ] Add app-level pin state and labels.
- [ ] On enable, call `Window::set_on_top()` after `show()`.
- [ ] On disable, clear app-level state and show a status explaining native demotion may depend on the platform.
- [ ] Run `cargo test`.

### Task 5: Visual Verification And Docs

**Files:**
- Modify: `README.md`
- Modify: `DESIGN.md`

- [ ] Run the FLTK app.
- [ ] Load sample left/right text and compare.
- [ ] Capture a screenshot showing custom diff canvas, custom input gutters, overview rail, and Pin control.
- [ ] Update docs to describe the custom canvas, custom gutters, and topmost limitation.
- [ ] Run final `cargo test` and `git diff --check`.
