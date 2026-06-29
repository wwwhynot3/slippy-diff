# Character-Level Diff Copy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add character-level selection and copy support to the Diff Text canvas while preserving the existing whole-row copy behavior.

**Architecture:** Keep whole-row selection as the existing `Option<(usize, usize)>` row range and add a separate `DiffCharSelection` model for text-column selection. Put copy extraction in `src/diff_view.rs` so it is unit-testable, and keep FLTK-specific mouse handling thin by delegating coordinate math and selected-copy resolution to pure helpers in `src/ui_fltk.rs`.

**Tech Stack:** Rust, FLTK custom `Frame` drawing, `arboard` clipboard, existing `cargo test` unit tests.

---

## File Structure

- Modify `src/diff_view.rs`: define `DiffCharPosition` and `DiffCharSelection`; add `RenderedDiffView::char_selection_text`; test Unicode-safe substring, reverse selection, multi-line selection, and clamping.
- Modify `src/ui_fltk.rs`: add `char_selection` state to `UiHandles`; add pure hit-test/copy helpers; update canvas mouse/key handling; update drawing to render selected character ranges behind text.
- Create no new production module: the feature is small and tightly coupled to the existing diff view/canvas.

## Task 1: Diff View Character Extraction

**Files:**
- Modify: `src/diff_view.rs`

- [ ] **Step 1: Write failing tests for character selection extraction**

Add these tests inside the existing `#[cfg(test)] mod tests` in `src/diff_view.rs`, after `selection_text_joins_selected_rows_as_plain_text`.

```rust
    fn view_with_plain_rows(lines: &[&str]) -> RenderedDiffView {
        RenderedDiffView {
            rows: lines
                .iter()
                .map(|line| DiffViewRow {
                    kind: DiffViewRowKind::Context,
                    old_line: None,
                    new_line: None,
                    marker: "",
                    segments: vec![DiffViewSegment {
                        kind: DiffViewSegmentKind::Normal,
                        text: (*line).to_string(),
                    }],
                    group_id: None,
                })
                .collect(),
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
    fn char_selection_text_copies_single_row_character_range() {
        let view = view_with_plain_rows(&["abcdef"]);
        let selection = DiffCharSelection {
            anchor: DiffCharPosition {
                row_index: 0,
                char_index: 1,
            },
            focus: DiffCharPosition {
                row_index: 0,
                char_index: 4,
            },
        };

        assert_eq!(view.char_selection_text(selection), "bcd");
    }

    #[test]
    fn char_selection_text_normalizes_reverse_drag_order() {
        let view = view_with_plain_rows(&["abcdef"]);
        let selection = DiffCharSelection {
            anchor: DiffCharPosition {
                row_index: 0,
                char_index: 5,
            },
            focus: DiffCharPosition {
                row_index: 0,
                char_index: 2,
            },
        };

        assert_eq!(view.char_selection_text(selection), "cde");
    }

    #[test]
    fn char_selection_text_copies_multiline_suffix_middle_and_prefix() {
        let view = view_with_plain_rows(&["abcdef", "second", "uvwxyz"]);
        let selection = DiffCharSelection {
            anchor: DiffCharPosition {
                row_index: 0,
                char_index: 3,
            },
            focus: DiffCharPosition {
                row_index: 2,
                char_index: 2,
            },
        };

        assert_eq!(view.char_selection_text(selection), "def\nsecond\nuv");
    }

    #[test]
    fn char_selection_text_clamps_character_indices_and_handles_unicode() {
        let view = view_with_plain_rows(&["a界b"]);
        let selection = DiffCharSelection {
            anchor: DiffCharPosition {
                row_index: 0,
                char_index: 1,
            },
            focus: DiffCharPosition {
                row_index: 0,
                char_index: 99,
            },
        };

        assert_eq!(view.char_selection_text(selection), "界b");
    }
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test diff_view::tests::char_selection_text -- --nocapture
```

Expected: compilation fails because `DiffCharSelection`, `DiffCharPosition`, and `RenderedDiffView::char_selection_text` do not exist.

- [ ] **Step 3: Add character selection types and extraction implementation**

Add the two public structs after `ChangeMark` in `src/diff_view.rs`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffCharPosition {
    pub row_index: usize,
    pub char_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffCharSelection {
    pub anchor: DiffCharPosition,
    pub focus: DiffCharPosition,
}
```

Add these helpers near the top-level helper functions in `src/diff_view.rs`, before `impl RenderedDiffView`.

```rust
fn ordered_char_selection(
    selection: DiffCharSelection,
) -> (DiffCharPosition, DiffCharPosition) {
    let anchor_key = (selection.anchor.row_index, selection.anchor.char_index);
    let focus_key = (selection.focus.row_index, selection.focus.char_index);
    if anchor_key <= focus_key {
        (selection.anchor, selection.focus)
    } else {
        (selection.focus, selection.anchor)
    }
}

fn byte_index_for_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .map(|(byte_index, _)| byte_index)
        .nth(char_index)
        .unwrap_or(text.len())
}

fn slice_chars(text: &str, start: usize, end: usize) -> String {
    let lo = start.min(end);
    let hi = start.max(end);
    let start_byte = byte_index_for_char(text, lo);
    let end_byte = byte_index_for_char(text, hi);
    text[start_byte..end_byte].to_string()
}
```

Add this method inside `impl RenderedDiffView`, after `selection_text`.

```rust
    /// The plain text inside a character selection. Selection endpoints use
    /// character indices over each rendered row's text column and are half-open:
    /// the anchor character is included and the focus character is excluded
    /// after normalizing drag direction.
    pub fn char_selection_text(&self, selection: DiffCharSelection) -> String {
        let (start, end) = ordered_char_selection(selection);
        if start.row_index == end.row_index {
            return self
                .row_text(start.row_index)
                .map(|text| slice_chars(&text, start.char_index, end.char_index))
                .unwrap_or_default();
        }

        (start.row_index..=end.row_index)
            .filter_map(|row_index| {
                let text = self.row_text(row_index)?;
                let selected = if row_index == start.row_index {
                    slice_chars(&text, start.char_index, text.chars().count())
                } else if row_index == end.row_index {
                    slice_chars(&text, 0, end.char_index)
                } else {
                    text
                };
                Some(selected)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
rtk cargo test diff_view::tests::char_selection_text -- --nocapture
```

Expected: the four new `char_selection_text_*` tests pass.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
rtk git add src/diff_view.rs
rtk git commit -m "feat: add diff character selection extraction"
```

Expected: a commit containing only `src/diff_view.rs`.

## Task 2: UI Pure Helpers For Hit Testing And Copy Resolution

**Files:**
- Modify: `src/ui_fltk.rs`

- [ ] **Step 1: Write failing tests for canvas coordinate mapping and copy priority**

Add these tests inside the existing `#[cfg(test)] mod tests` in `src/ui_fltk.rs`, after `diff_canvas_width_reserves_scrollbar_and_keeps_minimum_width`.

```rust
    fn test_diff_view(lines: &[&str]) -> crate::diff_view::RenderedDiffView {
        crate::diff_view::RenderedDiffView {
            rows: lines
                .iter()
                .map(|line| crate::diff_view::DiffViewRow {
                    kind: crate::diff_view::DiffViewRowKind::Context,
                    old_line: None,
                    new_line: None,
                    marker: "",
                    segments: vec![crate::diff_view::DiffViewSegment {
                        kind: crate::diff_view::DiffViewSegmentKind::Normal,
                        text: (*line).to_string(),
                    }],
                    group_id: None,
                })
                .collect(),
            summary: crate::diff_view::ChangeSummary {
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
    fn diff_text_origin_matches_header_and_gutters() {
        assert_eq!(
            diff_text_x(10),
            10 + DIFF_OLD_GUTTER_WIDTH
                + DIFF_NEW_GUTTER_WIDTH
                + DIFF_MARKER_WIDTH
                + DIFF_TEXT_LEFT_PAD
        );
    }

    #[test]
    fn diff_row_at_clamps_vertical_drag_to_existing_rows() {
        let frame_y = 100;
        assert_eq!(
            diff_row_at(frame_y + DIFF_HEADER_HEIGHT, frame_y, 3),
            Some(0)
        );
        assert_eq!(
            diff_row_at(frame_y + DIFF_HEADER_HEIGHT + DIFF_ROW_HEIGHT * 99, frame_y, 3),
            Some(2)
        );
        assert_eq!(diff_row_at(frame_y, frame_y, 0), None);
    }

    #[test]
    fn diff_char_position_at_maps_text_column_x_to_character_index() {
        let view = test_diff_view(&["abcdef"]);
        let frame_x = 20;
        let frame_y = 50;
        let char_width = 8;
        let x = diff_text_x(frame_x) + (char_width * 3) + 2;
        let y = frame_y + DIFF_HEADER_HEIGHT + 4;

        assert_eq!(
            diff_char_position_at(&view, x, y, frame_x, frame_y, char_width),
            Some(crate::diff_view::DiffCharPosition {
                row_index: 0,
                char_index: 3,
            })
        );
    }

    #[test]
    fn diff_char_position_at_returns_none_before_text_column() {
        let view = test_diff_view(&["abcdef"]);
        let frame_x = 20;
        let frame_y = 50;
        let y = frame_y + DIFF_HEADER_HEIGHT + 4;

        assert_eq!(
            diff_char_position_at(&view, diff_text_x(frame_x) - 1, y, frame_x, frame_y, 8),
            None
        );
    }

    #[test]
    fn selected_diff_copy_text_prefers_character_selection_over_row_selection() {
        let view = test_diff_view(&["abcdef", "second"]);
        let char_selection = crate::diff_view::DiffCharSelection {
            anchor: crate::diff_view::DiffCharPosition {
                row_index: 0,
                char_index: 1,
            },
            focus: crate::diff_view::DiffCharPosition {
                row_index: 0,
                char_index: 4,
            },
        };

        assert_eq!(
            selected_diff_copy_text(&view, Some(char_selection), Some((0, 1))),
            Some((String::from("bcd"), DiffCopyStatus::Selection))
        );
    }

    #[test]
    fn selected_diff_copy_text_keeps_existing_row_selection_behavior() {
        let view = test_diff_view(&["first", "second"]);

        assert_eq!(
            selected_diff_copy_text(&view, None, Some((1, 0))),
            Some((String::from("first\nsecond"), DiffCopyStatus::Lines(2)))
        );
    }
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test ui_fltk::tests:: -- --nocapture
```

Expected: compilation fails because the helper functions and `DiffCopyStatus` do not exist.

- [ ] **Step 3: Add pure helper implementations**

In `src/ui_fltk.rs`, add this enum and helper functions after `diff_canvas_width`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffCopyStatus {
    Lines(usize),
    Selection,
}

fn diff_text_x(frame_x: i32) -> i32 {
    frame_x
        + DIFF_OLD_GUTTER_WIDTH
        + DIFF_NEW_GUTTER_WIDTH
        + DIFF_MARKER_WIDTH
        + DIFF_TEXT_LEFT_PAD
}

fn diff_row_at(event_y: i32, frame_y: i32, row_count: usize) -> Option<usize> {
    if row_count == 0 {
        return None;
    }
    let max_row = (row_count - 1) as i32;
    Some(((event_y - frame_y - DIFF_HEADER_HEIGHT) / DIFF_ROW_HEIGHT).clamp(0, max_row) as usize)
}

fn diff_char_position_at(
    view: &crate::diff_view::RenderedDiffView,
    event_x: i32,
    event_y: i32,
    frame_x: i32,
    frame_y: i32,
    char_width: i32,
) -> Option<crate::diff_view::DiffCharPosition> {
    let row_index = diff_row_at(event_y, frame_y, view.rows.len())?;
    let text_x = diff_text_x(frame_x);
    if event_x < text_x {
        return None;
    }
    let char_width = char_width.max(1);
    let row_len = view
        .row_text(row_index)
        .map(|text| text.chars().count())
        .unwrap_or_default();
    let char_index = ((event_x - text_x) / char_width).clamp(0, row_len as i32) as usize;
    Some(crate::diff_view::DiffCharPosition {
        row_index,
        char_index,
    })
}

fn selected_diff_copy_text(
    view: &crate::diff_view::RenderedDiffView,
    char_selection: Option<crate::diff_view::DiffCharSelection>,
    row_selection: Option<(usize, usize)>,
) -> Option<(String, DiffCopyStatus)> {
    if let Some(selection) = char_selection {
        let text = view.char_selection_text(selection);
        if !text.is_empty() {
            return Some((text, DiffCopyStatus::Selection));
        }
    }

    row_selection.map(|(a, b)| {
        let lo = a.min(b);
        let hi = a.max(b);
        (view.selection_text(a, b), DiffCopyStatus::Lines(hi - lo + 1))
    })
}
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
rtk cargo test ui_fltk::tests:: -- --nocapture
```

Expected: the new helper tests pass.

- [ ] **Step 5: Commit Task 2**

Run:

```bash
rtk git add src/ui_fltk.rs
rtk git commit -m "feat: add diff selection UI helpers"
```

Expected: a commit containing only helper/test changes in `src/ui_fltk.rs`.

## Task 3: Canvas State, Mouse Handling, And Copy Integration

**Files:**
- Modify: `src/ui_fltk.rs`

- [ ] **Step 1: Write failing source-level regression tests for integration wiring**

Add these tests inside `#[cfg(test)] mod tests` in `src/ui_fltk.rs`, after `selected_diff_copy_text_keeps_existing_row_selection_behavior`.

```rust
    #[test]
    fn ui_handles_tracks_character_selection_separately_from_row_selection() {
        let source = include_str!("ui_fltk.rs");
        assert!(
            source.contains("char_selection: Rc<Cell<Option<crate::diff_view::DiffCharSelection>>>"),
            "UiHandles should store character selection separately from row selection"
        );
    }

    #[test]
    fn copy_canvas_selection_uses_shared_selected_text_helper() {
        let source = include_str!("ui_fltk.rs");
        let start = source
            .find("fn copy_canvas_selection(")
            .expect("copy_canvas_selection should exist");
        let end = start
            + source[start..]
                .find("\nfn copy_current_diff")
                .expect("copy_canvas_selection should end before copy_current_diff");
        let body = &source[start..end];

        assert!(
            body.contains("selected_diff_copy_text("),
            "copy_canvas_selection should use the helper that prefers character selection"
        );
        assert!(
            body.contains("char_selection.get()"),
            "copy_canvas_selection should read character selection state"
        );
    }

    #[test]
    fn diff_canvas_event_handler_distinguishes_text_and_row_selection() {
        let source = include_str!("ui_fltk.rs");
        let start = source
            .find("canvas.handle(move |frame, event| match event {")
            .expect("diff canvas handler should exist");
        let end = start
            + source[start..]
                .find("\n        });")
                .expect("diff canvas handler should close");
        let handler = &source[start..end];

        assert!(
            handler.contains("diff_char_position_at("),
            "text-column mouse events should use character hit testing"
        );
        assert!(
            handler.contains("char_selection.set(None);"),
            "row selection path should clear character selection"
        );
        assert!(
            handler.contains("selection.set(None);"),
            "character selection path should clear row selection"
        );
    }
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test ui_fltk::tests:: -- --nocapture
```

Expected: tests fail because `char_selection` has not been added and the handler still only uses row selection.

- [ ] **Step 3: Add `char_selection` state to handles and canvas construction**

Update `UiHandles` in `src/ui_fltk.rs`.

```rust
    nav_cursor: Rc<Cell<Option<usize>>>,
    selection: Rc<Cell<Option<(usize, usize)>>>,
    char_selection: Rc<Cell<Option<crate::diff_view::DiffCharSelection>>>,
    clipboard: Option<Clipboard>,
```

In `run`, after creating `selection`, create `char_selection`.

```rust
    let selection = Rc::new(Cell::new(None));
    let char_selection = Rc::new(Cell::new(None));
```

Pass it to `make_diff_canvas`.

```rust
    let (mut diff_scroll, mut diff_canvas) = make_diff_canvas(
        palette_cell.clone(),
        initial_diff_view.clone(),
        stale_diff_notice.clone(),
        nav_cursor.clone(),
        selection.clone(),
        char_selection.clone(),
    );
```

Store it in `UiHandles`.

```rust
        nav_cursor: nav_cursor.clone(),
        selection: selection.clone(),
        char_selection: char_selection.clone(),
        clipboard,
```

Update `make_diff_canvas` signature and draw closure.

```rust
fn make_diff_canvas(
    palette_cell: Rc<Cell<Palette>>,
    view: Rc<RefCell<crate::diff_view::RenderedDiffView>>,
    stale_notice: Rc<Cell<bool>>,
    nav_cursor: Rc<Cell<Option<usize>>>,
    selection: Rc<Cell<Option<(usize, usize)>>>,
    char_selection: Rc<Cell<Option<crate::diff_view::DiffCharSelection>>>,
) -> (Scroll, Frame) {
```

Inside the draw closure, clone and pass `char_selection`.

```rust
        let char_selection = char_selection.clone();
```

```rust
                selection.get(),
                char_selection.get(),
                palette_cell.get(),
```

- [ ] **Step 4: Integrate character selection into copy**

Replace `copy_canvas_selection` with this implementation.

```rust
fn copy_canvas_selection(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) {
    let (text, status) = {
        let handles = handles.borrow();
        let view = handles.diff_view.borrow();
        match selected_diff_copy_text(&view, handles.char_selection.get(), handles.selection.get()) {
            Some(selection) => selection,
            None => return,
        }
    };

    let copied = match handles.borrow_mut().clipboard.as_mut() {
        Some(clipboard) => clipboard.set_text(text).is_ok(),
        None => false,
    };
    if copied {
        match status {
            DiffCopyStatus::Lines(line_count) => state
                .borrow_mut()
                .set_status(format!("Copied {line_count} lines.")),
            DiffCopyStatus::Selection => state.borrow_mut().set_status("Copied selection."),
        }
    } else {
        state
            .borrow_mut()
            .set_status("Copy failed: clipboard unavailable.");
    }
    render_state(state, handles);
}
```

- [ ] **Step 5: Integrate character hit testing into the diff canvas handler**

In the `if surface_mode == SurfaceMode::Full` block, clone `char_selection` next to `selection`.

```rust
        let selection = selection.clone();
        let char_selection = char_selection.clone();
```

In the `Event::Push` branch, replace the row-only body with this body.

```rust
                let count = handles.borrow().diff_view.borrow().rows.len();
                if count > 0 {
                    draw::set_font(Font::Courier, 14);
                    let (char_width, _) = draw::measure("M", false);
                    let char_position = {
                        let handles = handles.borrow();
                        let view = handles.diff_view.borrow();
                        diff_char_position_at(
                            &view,
                            app::event_x(),
                            app::event_y(),
                            frame.x(),
                            frame.y(),
                            char_width,
                        )
                    };
                    if let Some(position) = char_position {
                        selection.set(None);
                        char_selection.set(Some(crate::diff_view::DiffCharSelection {
                            anchor: position,
                            focus: position,
                        }));
                    } else if let Some(row) = diff_row_at(app::event_y(), frame.y(), count) {
                        char_selection.set(None);
                        selection.set(Some((row, row)));
                    }
                    let _ = frame.take_focus();
                    frame.redraw();
                }
                true
```

In the `Event::Drag` branch, replace the row-only body with this body.

```rust
                if let Some(active) = char_selection.get() {
                    draw::set_font(Font::Courier, 14);
                    let (char_width, _) = draw::measure("M", false);
                    let next_focus = {
                        let handles = handles.borrow();
                        let view = handles.diff_view.borrow();
                        diff_char_position_at(
                            &view,
                            app::event_x(),
                            app::event_y(),
                            frame.x(),
                            frame.y(),
                            char_width,
                        )
                    };
                    if let Some(focus) = next_focus {
                        char_selection.set(Some(crate::diff_view::DiffCharSelection {
                            anchor: active.anchor,
                            focus,
                        }));
                        frame.redraw();
                    }
                } else if let Some((anchor, _)) = selection.get() {
                    let count = handles.borrow().diff_view.borrow().rows.len();
                    if let Some(row) = diff_row_at(app::event_y(), frame.y(), count) {
                        selection.set(Some((anchor, row)));
                        frame.redraw();
                    }
                }
                true
```

In the `Event::KeyDown` branch, replace the copy and Escape handling with this logic.

```rust
                if is_copy && (selection.get().is_some() || char_selection.get().is_some()) {
                    copy_canvas_selection(&state, &handles);
                    return true;
                }
                if app::event_key() == Key::Escape {
                    selection.set(None);
                    char_selection.set(None);
                    frame.redraw();
                    return true;
                }
```

- [ ] **Step 6: Run integration wiring tests to verify GREEN**

Run:

```bash
rtk cargo test ui_fltk::tests:: -- --nocapture
```

Expected: the three source-level wiring tests pass.

- [ ] **Step 7: Commit Task 3**

Run:

```bash
rtk git add src/ui_fltk.rs
rtk git commit -m "feat: wire character selection copy into diff canvas"
```

Expected: a commit containing state/copy/handler integration in `src/ui_fltk.rs`.

## Task 4: Draw Character Selection Highlight

**Files:**
- Modify: `src/ui_fltk.rs`

- [ ] **Step 1: Write failing tests for selection span calculation**

Add these tests inside `#[cfg(test)] mod tests` in `src/ui_fltk.rs`, after `diff_char_position_at_returns_none_before_text_column`.

```rust
    #[test]
    fn diff_char_selection_span_for_row_returns_single_row_range() {
        let selection = crate::diff_view::DiffCharSelection {
            anchor: crate::diff_view::DiffCharPosition {
                row_index: 2,
                char_index: 5,
            },
            focus: crate::diff_view::DiffCharPosition {
                row_index: 2,
                char_index: 1,
            },
        };

        assert_eq!(diff_char_selection_span_for_row(selection, 2, 10), Some(1..5));
        assert_eq!(diff_char_selection_span_for_row(selection, 1, 10), None);
    }

    #[test]
    fn diff_char_selection_span_for_row_returns_multiline_ranges() {
        let selection = crate::diff_view::DiffCharSelection {
            anchor: crate::diff_view::DiffCharPosition {
                row_index: 0,
                char_index: 3,
            },
            focus: crate::diff_view::DiffCharPosition {
                row_index: 2,
                char_index: 2,
            },
        };

        assert_eq!(diff_char_selection_span_for_row(selection, 0, 6), Some(3..6));
        assert_eq!(diff_char_selection_span_for_row(selection, 1, 6), Some(0..6));
        assert_eq!(diff_char_selection_span_for_row(selection, 2, 6), Some(0..2));
        assert_eq!(diff_char_selection_span_for_row(selection, 3, 6), None);
    }
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
rtk cargo test ui_fltk::tests::diff_char_selection_span_for_row -- --nocapture
```

Expected: compilation fails because `diff_char_selection_span_for_row` does not exist.

- [ ] **Step 3: Add selection span helper**

Add this helper after `diff_char_position_at`.

```rust
fn diff_char_selection_span_for_row(
    selection: crate::diff_view::DiffCharSelection,
    row_index: usize,
    row_char_count: usize,
) -> Option<std::ops::Range<usize>> {
    let start_key = (selection.anchor.row_index, selection.anchor.char_index);
    let end_key = (selection.focus.row_index, selection.focus.char_index);
    let (start, end) = if start_key <= end_key {
        (selection.anchor, selection.focus)
    } else {
        (selection.focus, selection.anchor)
    };

    if row_index < start.row_index || row_index > end.row_index {
        return None;
    }
    let range = if start.row_index == end.row_index {
        start.char_index.min(row_char_count)..end.char_index.min(row_char_count)
    } else if row_index == start.row_index {
        start.char_index.min(row_char_count)..row_char_count
    } else if row_index == end.row_index {
        0..end.char_index.min(row_char_count)
    } else {
        0..row_char_count
    };
    if range.start == range.end {
        None
    } else {
        Some(range)
    }
}
```

- [ ] **Step 4: Thread character selection into drawing functions**

Update the `draw_diff_canvas` signature.

```rust
fn draw_diff_canvas(
    frame: &Frame,
    view: &crate::diff_view::RenderedDiffView,
    stale_notice: bool,
    highlight: Option<std::ops::Range<usize>>,
    selection: Option<(usize, usize)>,
    char_selection: Option<crate::diff_view::DiffCharSelection>,
    palette: Palette,
) {
```

Inside the row loop, calculate and pass the character span.

```rust
        let selected = idx >= sel_lo && idx <= sel_hi;
        let char_span = char_selection.and_then(|selection| {
            let row_char_count = row
                .segments
                .iter()
                .map(|segment| segment.text.chars().count())
                .sum();
            diff_char_selection_span_for_row(selection, idx, row_char_count)
        });
        draw_diff_row(frame, y, row, selected, char_span, palette);
```

Update the `draw_diff_row` signature.

```rust
fn draw_diff_row(
    frame: &Frame,
    y: i32,
    row: &crate::diff_view::DiffViewRow,
    selected: bool,
    char_span: Option<std::ops::Range<usize>>,
    palette: Palette,
) {
```

Before the text segment loop in `draw_diff_row`, add this highlight drawing block after `let mut x = marker_right + DIFF_TEXT_LEFT_PAD;`.

```rust
    if let Some(span) = char_span.clone() {
        let (char_width, _) = draw::measure("M", false);
        let char_width = char_width.max(1);
        let selection_x = x + span.start as i32 * char_width;
        let selection_w = (span.end - span.start) as i32 * char_width;
        if selection_w > 0 {
            draw::set_draw_color(palette.selection);
            draw::draw_rectf(selection_x - 1, y + 3, selection_w + 2, DIFF_ROW_HEIGHT - 6);
        }
    }
```

- [ ] **Step 5: Run highlight tests to verify GREEN**

Run:

```bash
rtk cargo test ui_fltk::tests::diff_char_selection_span_for_row -- --nocapture
```

Expected: the two span tests pass.

- [ ] **Step 6: Commit Task 4**

Run:

```bash
rtk git add src/ui_fltk.rs
rtk git commit -m "feat: draw character selection in diff canvas"
```

Expected: a commit containing drawing integration in `src/ui_fltk.rs`.

## Task 5: Full Verification And Manual Smoke

**Files:**
- Modify: no source files unless verification finds a failure.

- [ ] **Step 1: Run focused test suites**

Run:

```bash
rtk cargo test char_selection -- --nocapture
rtk cargo test ui_fltk::tests:: -- --nocapture
```

Expected: all character-selection tests pass.

- [ ] **Step 2: Run full test suite**

Run:

```bash
rtk cargo test
```

Expected: all tests pass.

- [ ] **Step 3: Manual smoke test native FLTK path**

Run:

```bash
rtk cargo run
```

Manual actions:

1. Paste or type two multi-line inputs that produce a visible diff.
2. Drag inside the Diff Text text column from the middle of one rendered row to the middle of another rendered row.
3. Press `Ctrl+C`.
4. Paste into the left editor or an external text field.
5. Confirm the pasted content is only the selected substring/multiline text, not full diff rows.
6. Drag from the gutter/background outside the text column.
7. Press `Ctrl+C`.
8. Confirm whole-row copy still works and the status says `Copied N lines.`.

Expected: character selection copies selected characters; row selection still copies whole rendered rows.

- [ ] **Step 4: Manual smoke test Wayland feature build**

Run:

```bash
rtk cargo run --features wayland
```

Manual actions: repeat the same character selection and row selection copy checks from Step 3.

Expected: behavior matches the native FLTK path.

- [ ] **Step 5: Final status check**

Run:

```bash
rtk git status --short
```

Expected: only intentionally uncommitted local files remain. Do not stage unrelated `.cargo/` or `scripts/` files.
