# Prev / Next Change Navigation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire up the existing-but-disabled `Prev` / `Next` toolbar buttons so the user can jump change-by-change (with wrap-around, a selection highlight, and keyboard shortcuts) through the current diff.

**Architecture:** A pure helper `RenderedDiffView::change_regions()` in `diff_view.rs` derives maximal runs of change rows (unit-tested). `ui_fltk.rs` owns a UI-local cursor (`Rc<Cell<Option<usize>>>`, the same pattern as `stale_diff_notice`), a `navigate_change()` function that steps the cursor with wrap-around, scrolls the `diff_scroll` to the region, sets a transient status, and (in a later task) renders a soft selection strip on the active region. The cursor resets whenever a new diff lands or inputs are cleared. `diff_core`, `app_state`, and `config` are untouched.

**Tech Stack:** Rust, `fltk-rs` 1.5.23 (`Scroll::scroll_to(&mut self, x: i32, y: i32)`, `fltk::enums::Key::Up` / `Key::Down`, `Shortcut::Command | Shortcut::Shift | <key>`), existing `similar`-based diff pipeline.

## Global Constraints

Copied verbatim from the spec / `CLAUDE.md`:

- **Layering invariant:** `diff_core` must not depend on FLTK/clipboard/config/threading. `app_state` must not depend on FLTK or `arboard`. `config` must never serialize pasted text or diff output. `ui_fltk` is the only module that touches FLTK and `arboard`. Pure logic lives in `diff_core` / `diff_view` and is unit-tested; `ui_fltk` is verified by the manual GUI smoke checklist.
- This feature touches **only** `diff_view.rs`, `ui_fltk.rs`, and the docs. `diff_core.rs`, `app_state.rs`, `config.rs` are NOT modified.
- **Palette tokens and status strings stay in sync with the tables in `DESIGN.md` / `IMPLEMENTATION_PLAN.md`** — no ad-hoc colors or status text. The new `Selection` palette token uses the exact hex values from the DESIGN.md color table; the new status string is `Change N of M.`.
- Commit style: lowercase conventional commits (`feat:` / `docs:` / `test:`), matching the existing history.
- `fltk` version is **1.5.23**. `Scroll::scroll_to(&mut self, x: i32, y: i32)` takes `&mut self`, so callers clone a `mut` scroll handle first.
- No pasted text or diff output is ever persisted.

## File Structure

- **`src/diff_view.rs`** (modify) — add the pure `RenderedDiffView::change_regions()` method + unit tests. This is the only automated-test surface for the feature.
- **`src/ui_fltk.rs`** (modify) — add the `Selection` palette token; store `prev_change` / `next_change` / `nav_cursor` on `UiHandles`; add `navigate_change()` + `scroll_to_change()`; wire callbacks, shortcuts, enable/disable, and cursor reset; thread the active region into `make_diff_canvas` / `draw_diff_canvas` and render the selection strip.
- **`DESIGN.md`** (modify) — add the `Selection` token's two hex values are already in the table; add Prev/Next shortcuts, a Diff-navigation note, and the `Change N of M.` status row.
- **`IMPLEMENTATION_PLAN.md`** (modify) — add Prev/Next button rules, the status row, the shortcuts, and a Diff-navigation subsection.
- **`README.md`** (modify) — add the two shortcuts to the keyboard-shortcuts table.

---

### Task 1: Pure `change_regions()` on `RenderedDiffView`

**Files:**
- Modify: `src/diff_view.rs` (add method on the `impl RenderedDiffView`/struct at line 57-64; add tests inside the existing `#[cfg(test)] mod tests` at line 274).

**Interfaces:**
- Produces: `pub fn change_regions(&self) -> Vec<std::ops::Range<usize>>` on `RenderedDiffView`. Later tasks consume this as `view.change_regions()`.

- [ ] **Step 1: Write the failing tests**

Add these tests to the end of the `mod tests` block in `src/diff_view.rs` (after the `folding_uses_context_limits_and_preserves_line_numbers_after_skips` test, before the closing `}` of `mod tests` at line 502):

```rust
    fn view_of(kinds: &[DiffViewRowKind]) -> RenderedDiffView {
        let row = |kind: DiffViewRowKind| DiffViewRow {
            kind,
            old_line: None,
            new_line: None,
            marker: "",
            segments: vec![],
            group_id: None,
        };
        RenderedDiffView {
            rows: kinds.iter().map(|&k| row(k)).collect(),
            summary: ChangeSummary {
                removed: 0,
                added: 0,
                edited: 0,
            },
            marks: vec![],
            left_no_newline: false,
            right_no_newline: false,
        }
    }

    #[test]
    fn change_regions_groups_contiguous_change_rows() {
        use DiffViewRowKind::*;
        // Context, Delete, Insert, Context, ReplaceOld, ReplaceNew, Context
        let view = view_of(&[Context, Delete, Insert, Context, ReplaceOld, ReplaceNew, Context]);
        assert_eq!(view.change_regions(), vec![1..3, 4..6]);
    }

    #[test]
    fn change_regions_collapses_adjacent_delete_insert() {
        use DiffViewRowKind::*;
        // Context, Delete, Delete, Insert, Context -> one region
        let view = view_of(&[Context, Delete, Delete, Insert, Context]);
        assert_eq!(view.change_regions(), vec![1..4]);
    }

    #[test]
    fn change_regions_breaks_on_fold_and_notice() {
        use DiffViewRowKind::*;
        // Delete, Fold, Insert -> two regions
        let view = view_of(&[Delete, Fold, Insert]);
        assert_eq!(view.change_regions(), vec![0..1, 2..3]);
    }

    #[test]
    fn change_regions_covers_trailing_change_run() {
        use DiffViewRowKind::*;
        // Context, Delete, Delete (change run at the end, no trailing context)
        let view = view_of(&[Context, Delete, Delete]);
        assert_eq!(view.change_regions(), vec![1..3]);
    }

    #[test]
    fn change_regions_empty_without_change_rows() {
        use DiffViewRowKind::*;
        let view = view_of(&[Context, Context, Notice]);
        assert!(view.change_regions().is_empty());
    }

    #[test]
    fn change_regions_empty_for_no_rows() {
        let view = view_of(&[]);
        assert!(view.change_regions().is_empty());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test diff_view::tests::change_regions`
Expected: **FAIL** — compile error `no method named change_regions found for struct RenderedDiffView`.

- [ ] **Step 3: Implement `change_regions()`**

Add this `impl` block immediately after the `RenderedDiffView` struct definition (after line 64, i.e. right after the struct's closing brace and before `enum FoldItem`):

```rust
impl RenderedDiffView {
    /// Maximal runs of consecutive change rows (Delete / Insert / ReplaceOld /
    /// ReplaceNew) as half-open row-index ranges. Context / Fold / Notice rows
    /// break runs. Empty for equal-text or empty diffs. Prev/Next navigation
    /// steps through these regions.
    pub fn change_regions(&self) -> Vec<std::ops::Range<usize>> {
        fn is_change(kind: DiffViewRowKind) -> bool {
            matches!(
                kind,
                DiffViewRowKind::Delete
                    | DiffViewRowKind::Insert
                    | DiffViewRowKind::ReplaceOld
                    | DiffViewRowKind::ReplaceNew
            )
        }
        let mut regions = Vec::new();
        let mut start: Option<usize> = None;
        for (idx, row) in self.rows.iter().enumerate() {
            if is_change(row.kind) {
                if start.is_none() {
                    start = Some(idx);
                }
            } else if let Some(begin) = start.take() {
                regions.push(begin..idx);
            }
        }
        if let Some(begin) = start {
            regions.push(begin..self.rows.len());
        }
        regions
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test diff_view::tests::change_regions`
Expected: **PASS** — 6 tests.

- [ ] **Step 5: Run the full suite to confirm no regressions**

Run: `cargo test`
Expected: **PASS** — all existing tests still green.

- [ ] **Step 6: Commit**

```bash
git add src/diff_view.rs
git commit -m "feat: add change-region computation to diff view"
```

---

### Task 2: Wire Prev/Next navigation (scroll + status + wrap), no highlight yet

This task makes Prev/Next actually move through changes: buttons scroll the canvas to each change region, status shows `Change N of M.`, wrap-around works, buttons enable/disable correctly, and the cursor resets on new diff / clear. The selection highlight is added in Task 3.

**Files:**
- Modify: `src/ui_fltk.rs`

**Interfaces:**
- Consumes: `RenderedDiffView::change_regions()` (Task 1), `AppState::has_current_diff()`, `AppState::set_status(impl Into<String>)`, `Scroll::scroll_to(&mut self, x: i32, y: i32)`, `Scroll::xposition()`.
- Produces: `nav_cursor` field on `UiHandles`; functions `navigate_change()` and `scroll_to_change()`.

- [ ] **Step 1: Add the `nav_cursor` field to `UiHandles`**

In `src/ui_fltk.rs`, the `UiHandles` struct (around line 108-126) currently ends:

```rust
    status: Frame,
    copy_diff: Button,
    pin: Button,
}
```

Change it to:

```rust
    status: Frame,
    copy_diff: Button,
    pin: Button,
    prev_change: Button,
    next_change: Button,
    nav_cursor: Rc<Cell<Option<usize>>>,
}
```

- [ ] **Step 2: Create the `nav_cursor` Rc near the other view state**

Around line 242 there is:

```rust
    let stale_diff_notice = Rc::new(Cell::new(false));
    let (diff_scroll, diff_canvas) = make_diff_canvas(
```

Change it to:

```rust
    let stale_diff_notice = Rc::new(Cell::new(false));
    let nav_cursor = Rc::new(Cell::new(Option::<usize>::None));
    let (diff_scroll, diff_canvas) = make_diff_canvas(
```

- [ ] **Step 3: Add Prev/Next shortcuts at button creation (keep `.deactivate()` as the startup default; `render_state` toggles enable later)**

Around line 218-221:

```rust
    let mut prev_change = make_button("Prev", false, palette);
    prev_change.deactivate();
    let mut next_change = make_button("Next", false, palette);
    next_change.deactivate();
```

Change it to:

```rust
    let mut prev_change = make_button("Prev", false, palette);
    prev_change.deactivate();
    prev_change.set_shortcut(Shortcut::Command | Shortcut::Shift | fltk::enums::Key::Up);
    let mut next_change = make_button("Next", false, palette);
    next_change.deactivate();
    next_change.set_shortcut(Shortcut::Command | Shortcut::Shift | fltk::enums::Key::Down);
```

- [ ] **Step 4: Store the buttons + cursor in `UiHandles`**

In the `UiHandles { ... }` construction (around line 373-391), which currently ends:

```rust
        copy_diff: copy_diff.clone(),
        pin: pin.clone(),
    }));
```

Change it to:

```rust
        copy_diff: copy_diff.clone(),
        pin: pin.clone(),
        prev_change: prev_change.clone(),
        next_change: next_change.clone(),
        nav_cursor: nav_cursor.clone(),
    }));
```

- [ ] **Step 5: Add the `navigate_change` and `scroll_to_change` functions**

Add these two functions in `src/ui_fltk.rs`, immediately before `fn render_state` (around line 800):

```rust
fn navigate_change(
    state: &Rc<RefCell<AppState>>,
    handles: &Rc<RefCell<UiHandles>>,
    forward: bool,
) {
    let regions = handles.borrow().diff_view.borrow().change_regions();
    let total = regions.len();
    if total == 0 {
        return;
    }
    let target = match handles.borrow().nav_cursor.get() {
        None => 0,
        Some(current) => {
            if forward {
                (current + 1) % total
            } else {
                (current + total - 1) % total
            }
        }
    };
    let start_row = regions[target].start;
    handles.borrow().nav_cursor.set(Some(target));
    state
        .borrow_mut()
        .set_status(format!("Change {} of {}.", target + 1, total));
    render_state(state, handles);
    let mut scroll = handles.borrow().diff_scroll.clone();
    let view = handles.borrow().diff_view.borrow().clone();
    scroll_to_change(&mut scroll, &view, start_row);
}

fn scroll_to_change(
    scroll: &mut Scroll,
    view: &crate::diff_view::RenderedDiffView,
    start_row: usize,
) {
    let canvas_height = diff_canvas_height(view.rows.len());
    let viewport = scroll.h();
    let max_y = (canvas_height - viewport).max(0);
    // Bring the region's first row to the top with one row of context above it.
    let mut target_y = DIFF_HEADER_HEIGHT + start_row as i32 * DIFF_ROW_HEIGHT - DIFF_ROW_HEIGHT;
    if target_y < 0 {
        target_y = 0;
    }
    if target_y > max_y {
        target_y = max_y;
    }
    scroll.scroll_to(scroll.xposition(), target_y);
}
```

- [ ] **Step 6: Enable / disable Prev/Next in `render_state`**

In `fn render_state` (around line 800-826), the enable block at the end currently is:

```rust
    if state.has_current_diff() {
        copy_diff.activate();
    } else {
        copy_diff.deactivate();
    }
}
```

Change it to:

```rust
    if state.has_current_diff() {
        copy_diff.activate();
    } else {
        copy_diff.deactivate();
    }

    let can_navigate = state.has_current_diff() && !view.change_regions().is_empty();
    let mut prev_change = handles.prev_change.clone();
    let mut next_change = handles.next_change.clone();
    if can_navigate {
        prev_change.activate();
        next_change.activate();
    } else {
        prev_change.deactivate();
        next_change.deactivate();
    }
}
```

(`view` is the locally-built `RenderedDiffView` from the top of `render_state`; it is still in scope here.)

- [ ] **Step 7: Reset the cursor when a new diff lands**

In the `UiMessage::DiffReady` arm of the event loop (around line 519-522):

```rust
                UiMessage::DiffReady(result) => {
                    state.borrow_mut().apply_result(result);
                    render_state(&state, &handles);
                }
```

Change it to:

```rust
                UiMessage::DiffReady(result) => {
                    state.borrow_mut().apply_result(result);
                    handles.borrow().nav_cursor.set(None);
                    render_state(&state, &handles);
                }
```

- [ ] **Step 8: Reset the cursor on Clear**

In `fn clear_all` (around line 733-744), which currently begins:

```rust
fn clear_all(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) {
    state.borrow_mut().clear();
    {
```

Change it to:

```rust
fn clear_all(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) {
    state.borrow_mut().clear();
    handles.borrow().nav_cursor.set(None);
    {
```

- [ ] **Step 9: Wire the Prev/Next button callbacks**

Add these two callback blocks alongside the other button callbacks (insert them right after the `pin.set_callback(...)` block, which ends around line 493 — before `clear.set_callback`):

```rust
    {
        let state = state.clone();
        let handles = handles.clone();
        prev_change.set_callback(move |_| {
            navigate_change(&state, &handles, false);
        });
    }

    {
        let state = state.clone();
        let handles = handles.clone();
        next_change.set_callback(move |_| {
            navigate_change(&state, &handles, true);
        });
    }
```

- [ ] **Step 10: Build and run the full test suite**

Run: `cargo build`
Expected: compiles with no errors and no new warnings.

Run: `cargo test`
Expected: **PASS** — all tests green (no automated tests for this task; this confirms no regressions).

- [ ] **Step 11: Manual GUI smoke — navigation works**

Run: `cargo run`

Paste into **Left**:
```
line one
change me A
keep
modify this B
end
```
Paste into **Right**:
```
line one
changed A
keep
modified B
end
```
(Wait for the auto-diff, or press Compare.)

Verify:
- Both Prev and Next are **enabled**.
- Clicking **Next** scrolls to the first change (`changed A`) and the status bar reads `Change 1 of 2.`.
- Clicking **Next** again scrolls to the second change (`modified B`) and status reads `Change 2 of 2.`.
- Clicking **Next** once more **wraps** back to the first change (`Change 1 of 2.`).
- Clicking **Prev** from the first change **wraps** to the last (`Change 2 of 2.`).
- Make both inputs identical (paste the same text on both sides). After the auto-diff, Prev/Next are **disabled**.
- Click **Clear**. Prev/Next are **disabled**, status returns to ready.

(The selection highlight is NOT visible yet — that is Task 3.)

- [ ] **Step 12: Commit**

```bash
git add src/ui_fltk.rs
git commit -m "feat: navigate diff changes with prev/next buttons"
```

---

### Task 3: Render the soft selection strip on the active region

This task makes the active change region visually marked with a left-edge selection strip (the `Selection` palette token), so the user can see where Prev/Next landed.

**Files:**
- Modify: `src/ui_fltk.rs`

**Interfaces:**
- Consumes: `nav_cursor` (Task 2), `RenderedDiffView::change_regions()` (Task 1).
- Produces: `selection` field on `Palette`; `highlight: Option<std::ops::Range<usize>>` parameter on `draw_diff_canvas`; `nav_cursor` parameter on `make_diff_canvas`.

- [ ] **Step 1: Add the `Selection` palette token**

In the `Palette` struct (around line 29-50), the last field is:

```rust
    header_bg: Color,
}
```

Change it to:

```rust
    header_bg: Color,
    selection: Color,
}
```

- [ ] **Step 2: Populate `selection` in both themes**

In `fn palette_for` (around line 1296), the `Theme::System | Theme::Light` arm ends:

```rust
            header_bg: Color::from_rgb(240, 238, 232), // #F0EEE8
        },
        Theme::Dark => Palette {
```

Change it to:

```rust
            header_bg: Color::from_rgb(240, 238, 232), // #F0EEE8
            selection: Color::from_rgb(200, 216, 217), // #C8D8D9
        },
        Theme::Dark => Palette {
```

And the `Theme::Dark` arm ends:

```rust
            header_bg: Color::from_rgb(46, 49, 42), // #2E312A
        },
    }
}
```

Change it to:

```rust
            header_bg: Color::from_rgb(46, 49, 42), // #2E312A
            selection: Color::from_rgb(54, 86, 90), // #36565A
        },
    }
}
```

- [ ] **Step 3: Add the strip-width constant**

In the constants block (around line 77, after `const DIFF_TEXT_LEFT_PAD: i32 = 10;`), add:

```rust
const SELECTION_STRIP_WIDTH: i32 = 5;
```

- [ ] **Step 4: Thread the active region into `make_diff_canvas`**

Replace `fn make_diff_canvas` (around line 584-608) entirely with:

```rust
fn make_diff_canvas(
    palette: Palette,
    view: Rc<RefCell<crate::diff_view::RenderedDiffView>>,
    stale_notice: Rc<Cell<bool>>,
    nav_cursor: Rc<Cell<Option<usize>>>,
) -> (Scroll, Frame) {
    let mut scroll = Scroll::default();
    scroll.set_type(ScrollType::Both);
    scroll.set_frame(FrameType::FlatBox);
    scroll.set_color(palette.pane);
    scroll.set_scrollbar_size(14);

    let mut canvas = Frame::default().with_size(
        DIFF_CANVAS_MIN_WIDTH,
        diff_canvas_height(view.borrow().rows.len()),
    );
    canvas.set_frame(FrameType::FlatBox);
    canvas.set_color(palette.pane);
    canvas.draw({
        let view = view.clone();
        let stale_notice = stale_notice.clone();
        let nav_cursor = nav_cursor.clone();
        move |frame| {
            let view = view.borrow();
            let highlight = nav_cursor
                .get()
                .and_then(|idx| view.change_regions().get(idx).cloned());
            draw_diff_canvas(frame, &view, stale_notice.get(), highlight, palette)
        }
    });
    scroll.end();
    (scroll, canvas)
}
```

- [ ] **Step 5: Pass `nav_cursor` at the `make_diff_canvas` call site**

Around line 243-247:

```rust
    let (diff_scroll, diff_canvas) = make_diff_canvas(
        palette,
        initial_diff_view.clone(),
        stale_diff_notice.clone(),
    );
```

Change it to:

```rust
    let (diff_scroll, diff_canvas) = make_diff_canvas(
        palette,
        initial_diff_view.clone(),
        stale_diff_notice.clone(),
        nav_cursor.clone(),
    );
```

- [ ] **Step 6: Render the selection strip in `draw_diff_canvas`**

Replace the signature and add the strip at the end of `fn draw_diff_canvas` (around line 952-973). The current function:

```rust
fn draw_diff_canvas(
    frame: &Frame,
    view: &crate::diff_view::RenderedDiffView,
    stale_notice: bool,
    palette: Palette,
) {
    draw::set_draw_color(palette.pane);
    draw::draw_rectf(frame.x(), frame.y(), frame.w(), frame.h());

    draw_diff_header(frame, stale_notice, palette);

    let mut y = frame.y() + DIFF_HEADER_HEIGHT;
    if view.rows.is_empty() {
        draw_empty_diff_row(frame, y, palette);
        return;
    }

    for row in &view.rows {
        draw_diff_row(frame, y, row, palette);
        y += DIFF_ROW_HEIGHT;
    }
}
```

Change it to:

```rust
fn draw_diff_canvas(
    frame: &Frame,
    view: &crate::diff_view::RenderedDiffView,
    stale_notice: bool,
    highlight: Option<std::ops::Range<usize>>,
    palette: Palette,
) {
    draw::set_draw_color(palette.pane);
    draw::draw_rectf(frame.x(), frame.y(), frame.w(), frame.h());

    draw_diff_header(frame, stale_notice, palette);

    let mut y = frame.y() + DIFF_HEADER_HEIGHT;
    if view.rows.is_empty() {
        draw_empty_diff_row(frame, y, palette);
        return;
    }

    for row in &view.rows {
        draw_diff_row(frame, y, row, palette);
        y += DIFF_ROW_HEIGHT;
    }

    if let Some(region) = highlight {
        let top = frame.y() + DIFF_HEADER_HEIGHT + region.start as i32 * DIFF_ROW_HEIGHT;
        let bottom = frame.y() + DIFF_HEADER_HEIGHT + region.end as i32 * DIFF_ROW_HEIGHT;
        draw::set_draw_color(palette.selection);
        draw::draw_rectf(frame.x(), top, SELECTION_STRIP_WIDTH, bottom - top);
    }
}
```

(The strip is drawn after the rows so it overlays the left edge of the active region. It sits in the old-line-number gutter (the gutter text is right-aligned), so it never covers line numbers or diff text. Hairlines were considered and dropped to keep the diff readable; the left strip alone is the "soft selection band.")

- [ ] **Step 7: Build and run the full test suite**

Run: `cargo build`
Expected: compiles with no errors and no new warnings.

Run: `cargo test`
Expected: **PASS** — all tests green.

- [ ] **Step 8: Manual GUI smoke — highlight tracks the active region**

Run: `cargo run`

Reproduce the Task 2 multi-change diff (Left: `line one / change me A / keep / modify this B / end`; Right: `line one / changed A / keep / modified B / end`).

Verify:
- Before pressing Prev/Next, **no** selection strip is visible (cursor is `None`).
- Press **Next**: a selection-colored vertical strip appears on the left edge of the first change region, and it follows the region when it scrolls into view.
- Press **Next** again: the strip moves to the second change region.
- Press **Next** once more (wrap): the strip returns to the first region.
- Toggle theme to **Dark** (via config `theme = "Dark"` if testing manually) and confirm the strip uses the dark selection color and remains visible.
- Run the smoke from Task 2 step 11 again to confirm equal-text disables nav and Clear resets (no stray strip after clear).

- [ ] **Step 9: Commit**

```bash
git add src/ui_fltk.rs
git commit -m "feat: highlight current diff change on the canvas"
```

---

### Task 4: Document Prev/Next navigation

**Files:**
- Modify: `DESIGN.md`, `IMPLEMENTATION_PLAN.md`, `README.md`

**Interfaces:** None (docs only).

- [ ] **Step 1: README — add the two shortcuts**

In `README.md` the keyboard-shortcuts table (lines 94-101) ends:

```markdown
| `Ctrl/Cmd+Shift+P` | Toggle Pin |
```

Change it to:

```markdown
| `Ctrl/Cmd+Shift+P` | Toggle Pin |
| `Ctrl/Cmd+Shift+↑` | Previous change |
| `Ctrl/Cmd+Shift+↓` | Next change |
```

- [ ] **Step 2: DESIGN.md — add shortcuts**

In `DESIGN.md` the Shortcuts list (around line 56-65) ends:

```markdown
- `Ctrl/Cmd+Shift+P`: Toggle Pin
```

Change it to:

```markdown
- `Ctrl/Cmd+Shift+P`: Toggle Pin
- `Ctrl/Cmd+Shift+Up`: Previous change
- `Ctrl/Cmd+Shift+Down`: Next change
```

- [ ] **Step 3: DESIGN.md — add a Diff-navigation note**

In `DESIGN.md`, under the **Visual System** section, after the "Adaptive folding" bullet (around line 88) and before "All diff thresholds/ratios are configurable with defaults.", add:

```markdown
- Change navigation: Prev/Next step through maximal runs of adjacent change rows (delete/insert/replace), wrapping around at both ends. The active region is marked with a soft `Selection` strip on the canvas's left edge, and the status bar reads `Change N of M.` while navigating. The position resets when a new diff is produced or the inputs are cleared.
```

- [ ] **Step 4: DESIGN.md — add the status row**

In `DESIGN.md` the States table (around line 121-134), after the Pin-disabled row, add:

```markdown
| Prev/Next navigation | Preserve current diff | `Change N of M.` |
```

- [ ] **Step 5: IMPLEMENTATION_PLAN.md — add button rules**

In `IMPLEMENTATION_PLAN.md` the Button rules table (around line 157-165), after the Copy Diff row, add:

```markdown
| Prev | Enabled when a current diff has at least one change; steps to the previous change region (wraps) |
| Next | Enabled when a current diff has at least one change; steps to the next change region (wraps) |
```

- [ ] **Step 6: IMPLEMENTATION_PLAN.md — add the status row**

In `IMPLEMENTATION_PLAN.md` the Status behavior table (around line 169-180), after the Config save failure row, add:

```markdown
| Prev/Next navigation | Preserve current diff | `Change N of M.` |
```

- [ ] **Step 7: IMPLEMENTATION_PLAN.md — add shortcuts**

In `IMPLEMENTATION_PLAN.md` the Keyboard shortcuts table (around line 184-190), after the Copy Diff row, add:

```markdown
| Ctrl/Cmd+Shift+Up | Previous change |
| Ctrl/Cmd+Shift+Down | Next change |
```

- [ ] **Step 8: IMPLEMENTATION_PLAN.md — add a Diff-navigation subsection**

In `IMPLEMENTATION_PLAN.md`, under the **UI Contract** section, after the "Button rules" table, add:

```markdown
Diff navigation:

- Prev/Next step through maximal runs of adjacent change rows (Delete/Insert/ReplaceOld/ReplaceNew); Context/Fold/Notice rows break runs (see `diff_view::RenderedDiffView::change_regions`).
- Navigation wraps around at both ends. From a fresh diff the first Prev or Next lands on the first change.
- The active region is marked with a soft `Selection` strip on the canvas's left edge; status reads `Change N of M.` during navigation.
- The navigation cursor is UI-local ephemeral state (`Rc<Cell<Option<usize>>>`); it is not persisted and is not part of `app_state`. It resets to `None` whenever a new diff result is applied or the inputs are cleared.
```

- [ ] **Step 9: Commit**

```bash
git add DESIGN.md IMPLEMENTATION_PLAN.md README.md
git commit -m "docs: document prev/next change navigation"
```

---

## Self-Review

**1. Spec coverage** — each spec section maps to a task:
- Pure region logic + unit tests → Task 1.
- Cursor (UI-local ephemeral, not in `app_state`/`config`) → Task 2 (field + reset) ; layering invariant restated in Global Constraints.
- Stepping + wrap + from-`None`→0 → Task 2 step 5 (`navigate_change`).
- Scroll-to-region with one row of context, clamped → Task 2 step 5 (`scroll_to_change`).
- Status `Change N of M.` → Task 2 step 5 + docs Task 4.
- Button enable/disable (current diff + non-empty regions) → Task 2 step 6.
- Reset on new diff and Clear → Task 2 steps 7-8.
- Shortcuts `Cmd/Ctrl+Shift+↑/↓` → Task 2 step 3 + docs Task 4.
- Soft selection strip using `palette.selection` → Task 3.
- Docs (button rules, status table, shortcuts, DESIGN/PLAN/README) → Task 4.
- Layering: only `diff_view.rs` + `ui_fltk.rs` + docs touched; `diff_core`/`app_state`/`config` untouched → enforced by file lists.

**2. Placeholder scan** — no TBD/TODO/"add error handling"/"similar to Task N". Every code step contains the full code. The "hairlines dropped" note in Task 3 step 6 is a deliberate, documented simplification, not a placeholder.

**3. Type/signature consistency** —
- `change_regions() -> Vec<std::ops::Range<usize>>` (Task 1) is consumed identically in Task 2 (`navigate_change`), Task 2 step 6 (`render_state`), and Task 3 (`make_diff_canvas` closure).
- `nav_cursor: Rc<Cell<Option<usize>>>` defined Task 2 step 2, stored Task 2 step 4, read in `navigate_change` (Task 2 step 5), threaded into `make_diff_canvas` (Task 3 step 4-5), reset in DiffReady/Clear (Task 2 step 7-8). Type is consistent throughout.
- `draw_diff_canvas(..., highlight: Option<std::ops::Range<usize>>, ...)` (Task 3 step 6) matches the `highlight` computed in Task 3 step 4.
- `selection: Color` added to `Palette` (Task 3 step 1) and populated in both theme arms (Task 3 step 2) and consumed in Task 3 step 6.
- `SELECTION_STRIP_WIDTH` (Task 3 step 3) used in Task 3 step 6.
- `Scroll::scroll_to(&mut self, x, y)` and `Key::Up`/`Key::Down` confirmed against fltk 1.5.23.

No issues found.
