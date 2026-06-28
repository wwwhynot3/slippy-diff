# Prev / Next Change Navigation — Design

**Date:** 2026-06-28
**Status:** Approved (pending written-spec review)
**Scope:** Wire up the existing-but-disabled `Prev` / `Next` toolbar buttons so they move the user through the changes in the current diff.

## Problem

The diff toolbar already contains `Prev` and `Next` buttons
(`src/ui_fltk.rs`, `prev_change` / `next_change`), but they are created with
`.deactivate()` and **no callbacks** — they were disabled as placeholders in
commit `89dcd24` ("polish: disable placeholder diff navigation"). Neither
`DESIGN.md` nor `IMPLEMENTATION_PLAN.md` defines their behavior; the button-rules
and status tables omit them entirely. This spec defines that behavior.

## Goals

- Let the user jump change-by-change through a rendered diff with the mouse or
  keyboard.
- Keep the implementation inside the project's layering invariants: pure logic
  in `diff_view`, all FLTK/scroll concerns in `ui_fltk`, nothing diff-derived
  persisted in `config`.

## Non-goals (out of scope)

- Clicking the overview rail to jump to a mark (the rail is a non-interactive
  `Frame`).
- Persisting the navigation cursor across app launches.
- A toggle between region-level and per-line (mark-level) navigation.
- Side-by-side diff navigation.

## Decisions (confirmed with the user)

| Decision | Choice |
| --- | --- |
| Navigation unit | **Contiguous change region** — a maximal run of adjacent non-context rows |
| Behavior at the ends | **Wrap around** (last → first, first → last) |
| Visual feedback | **Soft selection band** highlighting the active region |
| Keyboard shortcuts | **Cmd/Ctrl+Shift+↑** = Prev, **Cmd/Ctrl+Shift+↓** = Next |

## Architecture: where the state lives

The navigation cursor is **UI-local ephemeral state**, held in `ui_fltk` as
`Rc<Cell<Option<usize>>>`, mirroring the existing `stale_diff_notice:
Rc<Cell<bool>>` precedent. It is **not** added to `app_state` or `config`.

Rationale:

- Navigation is purely a view/scroll concern. It never affects the diff output,
  the copy result, or any persisted state.
- The only piece that benefits from unit testing — *deriving the list of change
  regions from a view* — is pure and belongs in `diff_view.rs`. Keeping it there
  preserves the CLAUDE.md invariant that pure logic stays out of FLTK.
- Leaving `config` untouched keeps with the privacy invariant ("config never
  serializes diff-derived state") — the cursor is derived from the current diff,
  so it must not be persisted.

Alternative considered: storing the cursor in `app_state`. Rejected — it would
give the state layer knowledge of the rendered view for no consumer besides the
UI.

## Navigation model

### What counts as one "change"

A **change region** is a maximal run of consecutive rows whose kind is one of
`Delete`, `Insert`, `ReplaceOld`, `ReplaceNew`. Rows of kind `Context`, `Fold`,
or `Notice` break a run. Consequences:

- A paired replacement (`ReplaceOld` immediately followed by `ReplaceNew`) is a
  single region.
- An unpaired delete block immediately followed by an insert block collapses into
  one region — the "next hunk" feel.
- The `Notice` row produced for equal text (`No differences`) yields **zero**
  regions.

### Pure helper (in `diff_view.rs`)

```rust
impl RenderedDiffView {
    /// Maximal runs of consecutive change rows (Delete/Insert/ReplaceOld/
    /// ReplaceNew), as half-open row-index ranges. Context/Fold/Notice rows
    /// break runs. Empty for equal-text / empty diffs.
    pub fn change_regions(&self) -> Vec<std::ops::Range<usize>> { /* ... */ }
}
```

This is fully unit-tested (see Testing).

### Cursor and stepping

- `nav_cursor: Rc<Cell<Option<usize>>>` — index into `change_regions()`;
  `None` means "nothing focused yet."
- **From `None`:** first `Next` → region `0`; first `Prev` → region `0` (both
  "start here"). *(Open to change: if `Prev` from `None` should wrap to the last
  region instead, this is a one-line flip.)*
- **Once positioned:** `Next` → `(cur + 1) % n`; `Prev` → `(cur + n - 1) % n`
  (wrap-around).

## UI behavior on each Prev / Next

1. Read `change_regions()` from the current `diff_view`. If empty, no-op (the
   buttons are disabled in that state anyway).
2. Compute the target index per the stepping rules; store it in `nav_cursor`.
3. **Scroll** `diff_scroll` so the region's first row sits at the top of the
   viewport with ~1 row of context above it, clamped to the valid scroll range,
   via `scroll.scroll_to(x, y)`. Target canvas-y of a row `r` is
   `DIFF_HEADER_HEIGHT + r * DIFF_ROW_HEIGHT`.
4. **Highlight** the active region (see below).
5. Set status to `Change N of M.` (new status string; `N = target + 1`).
6. Trigger a canvas redraw so the highlight appears. `render_state` is **not**
   invoked for navigation (it rebuilds the view); navigate sets the cursor, the
   status label, redraws the canvas, then scrolls.

## Highlight rendering (soft selection band)

The active region's row span is threaded into the draw path as
`Option<Range<usize>>`:

- `make_diff_canvas` captures `nav_cursor` (alongside `view` and
  `stale_notice`) and, inside the draw closure, computes the highlighted range
  from `view.change_regions()` + the cursor.
- `draw_diff_canvas(frame, view, stale_notice, highlight, palette)` passes
  per-row membership down to `draw_diff_row`.
- For rows inside the highlighted region, draw:
  - a vertical **selection strip** on the canvas's left edge spanning the
    region's rows, colored `palette.selection` (~5px wide), and
  - thin **selection hairlines** at the region's top and bottom row boundaries.
- Existing row coloring (delete/insert/replace tints, `-`/`+`/`~` markers, inline
  token highlights, line numbers) is **unchanged**, so the strip reads as a focus
  marker rather than a recoloring. Exact widths tuned during visual verification.

## Reset semantics (when the cursor clears)

- **New diff applied:** reset `nav_cursor` to `None` in the `DiffReady` handler
  (`ui_fltk.rs`, the `UiMessage::DiffReady` arm that calls `apply_result` then
  `render_state`).
- **Clear:** reset in `clear_all`.
- **All other `render_state` calls** (ordinary edits, paste, swap, pin, status
  changes) **preserve** the cursor. This is safe because while a diff is pending
  or stale the *displayed* diff — and therefore the region list — is unchanged,
  so the cursor stays valid. The cursor is defensively clamped into the current
  region range on every navigate, so a stale index can never panic.

## Button enable / disable

In `render_state`, after the view is built: activate both `Prev` and `Next` iff
a current diff exists **and** `change_regions()` is non-empty. Disabled for
no-diff, empty, and `No differences` states. Buttons remain enabled when the
stale notice is shown (navigating a still-visible stale diff is fine).

## Shortcuts

- `Prev` → `Shortcut::Command | Shortcut::Shift | Key::Up`
- `Next` → `Shortcut::Command | Shortcut::Shift | Key::Down`

Wired as button shortcuts — the same mechanism `Cmd+Enter` (Compare) uses, so
they fire even when an editor has focus. Added to the DESIGN.md and
`IMPLEMENTATION_PLAN.md` shortcuts tables and to the README.

**Risk:** `Shift+↑/↓` may collide with an editor command when a `TextEditor`
holds focus. If it does, fall back to a window-level `handle` that intercepts
the chord. To be confirmed during the manual GUI smoke check.

## Status string

New status row added to the DESIGN.md and `IMPLEMENTATION_PLAN.md` status tables:

| Trigger | Status |
| --- | --- |
| Prev / Next navigation | `Change N of M.` |

This is the only new status text; it follows the existing "keep status strings
in sync with the tables" convention.

## Code changes by file

- **`src/diff_view.rs`**
  - Add `RenderedDiffView::change_regions() -> Vec<Range<usize>>`.
  - Unit tests: regions split by context; a replacement pair sits in one region;
    an adjacent delete-block + insert-block collapses to one region; equal-text
    and empty diffs yield zero regions.

- **`src/ui_fltk.rs`**
  - Store `prev_change: Button`, `next_change: Button`, and
    `nav_cursor: Rc<Cell<Option<usize>>>` in `UiHandles`.
  - Thread `Option<Range<usize>>` (current region) through `make_diff_canvas` →
    `draw_diff_canvas` → `draw_diff_row`.
  - Add `navigate_change(state, handles, forward: bool)` implementing the
    stepping, scroll, status, and redraw.
  - Wire `prev_change` / `next_change` callbacks and shortcuts; remove the
    `.deactivate()` placeholders (enable state is now driven by `render_state`).
  - Reset `nav_cursor` in the `DiffReady` arm and in `clear_all`.
  - Enable/disable both buttons in `render_state` based on region count.

- **`DESIGN.md` + `IMPLEMENTATION_PLAN.md`**
  - Add `Prev` / `Next` to the button rules.
  - Add `Change N of M.` to the status tables.
  - Add the two shortcuts to the shortcuts tables.
  - Note the contiguous-region navigation semantics.

- **`README.md`**
  - Add `Cmd/Ctrl+Shift+↑/↓` to the shortcuts list.

## Testing

- **`diff_view` unit tests** (pure): `change_regions()` correctness across the
  cases above. These are the new automated coverage.
- **`app_state`:** unchanged — no new state is added there. Existing tests
  remain green.
- **`config`:** unchanged — nothing new is persisted; the privacy test still
  passes.
- **Manual GUI smoke (added to the checklist):**
  - Prev/Next scroll the canvas to each change and the selection band tracks the
    active region.
  - Wrap-around works at both ends.
  - Buttons are disabled for no-diff, empty, and `No differences`; enabled once a
    diff with changes exists.
  - Cursor resets to the start after Compare produces a new diff, and after
    Clear.
  - `Cmd/Ctrl+Shift+↑/↓` fire Prev/Next even while an editor has focus (and do
    not collide with editor commands; if they do, the window-level fallback is
    applied).

## Layering invariant check

- `diff_core`: untouched.
- `app_state`: untouched (no new field, no new dependency).
- `config`: untouched (nothing persisted).
- `ui_fltk`: owns all new FLTK/scroll/cursor/highlight/shortcut logic.
- Pure region logic lives in `diff_view` and is unit-tested.

All four dependency rules from CLAUDE.md are preserved.
