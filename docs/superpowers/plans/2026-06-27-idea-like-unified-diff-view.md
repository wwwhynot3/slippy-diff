# IDEA-like Unified Diff View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an IDEA-like unified diff result view with semantic old/new gutters, B+C red/green rendering, grouped replacement rows, and a compact change overview rail.

**Architecture:** Add a pure `diff_view` model that converts `DisplayDiff` into line-numbered view rows. Keep `diff_core` responsible for diff computation and `render_unified_diff`; make `ui_fltk` responsible only for converting view rows into FLTK buffers/widgets. Implement the visual redesign in small vertical slices so semantic row tests land before FLTK work.

**Tech Stack:** Rust 2024, `similar`, `fltk-rs`, existing `cargo test` suite, native FLTK manual verification.

---

## File Structure

- Create `src/diff_view.rs`
  - Owns `DiffViewRow`, `DiffViewRowKind`, `DiffViewSegment`, `DiffViewSegmentKind`, `RenderedDiffView`, `ChangeSummary`, `build_diff_view`, and `fold_ops_for_view`.
  - Pure Rust, no FLTK imports.
- Modify `src/lib.rs`
  - Exports the new `diff_view` module.
- Modify `src/ui_fltk.rs`
  - Replaces the old `render_display_ops` flow with `build_diff_view` plus a FLTK renderer.
  - Adds old/new/kind textual gutters inside the diff output buffer.
  - Adds an overview rail widget beside the diff display.
  - Adds a compact diff toolbar above the diff output area.
- Keep `src/diff_core.rs` unchanged unless tests expose a bug in existing diff semantics.
- Modify `docs/superpowers/specs/2026-06-27-idea-like-unified-diff-view-design.md` only if implementation discovers the approved design is technically impossible in FLTK.

## Task 1: Add Pure Diff View Row Model

**Files:**
- Create: `src/diff_view.rs`
- Modify: `src/lib.rs`
- Test: `src/diff_view.rs`

- [ ] **Step 1: Export the new module**

Modify `src/lib.rs`:

```rust
pub mod app_state;
pub mod config;
pub mod diff_core;
pub mod diff_view;
pub mod ui_fltk;
```

- [ ] **Step 2: Create the initial pure model and tests**

Create `src/diff_view.rs` with this initial content:

```rust
use crate::diff_core::{DiffOp, DiffOptions, DisplayDiff, InlineDiffSegmentKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffViewRowKind {
    Context,
    Delete,
    Insert,
    ReplaceOld,
    ReplaceNew,
    Fold,
    Notice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffViewSegmentKind {
    Normal,
    DeleteToken,
    InsertToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffViewSegment {
    pub kind: DiffViewSegmentKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffViewRow {
    pub kind: DiffViewRowKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub marker: &'static str,
    pub segments: Vec<DiffViewSegment>,
    pub group_id: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSummary {
    pub removed: usize,
    pub added: usize,
    pub edited: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedDiffView {
    pub rows: Vec<DiffViewRow>,
    pub summary: ChangeSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FoldItem {
    Op(DiffOp),
    Skipped(usize),
}

pub fn build_diff_view(diff: &DisplayDiff, options: &DiffOptions) -> RenderedDiffView {
    if diff.ops.is_empty() {
        return RenderedDiffView {
            rows: vec![DiffViewRow {
                kind: DiffViewRowKind::Notice,
                old_line: None,
                new_line: None,
                marker: "",
                segments: vec![normal("No differences")],
                group_id: None,
            }],
            summary: ChangeSummary {
                removed: 0,
                added: 0,
                edited: 0,
            },
        };
    }

    let mut rows = Vec::new();
    let mut old_line = 1usize;
    let mut new_line = 1usize;
    let mut group_id = 1usize;
    let mut summary = ChangeSummary {
        removed: 0,
        added: 0,
        edited: 0,
    };

    for item in fold_ops_for_view(&diff.ops, options) {
        match item {
            FoldItem::Op(DiffOp::Context { text }) => {
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::Context,
                    old_line: Some(old_line),
                    new_line: Some(new_line),
                    marker: "",
                    segments: vec![normal(text)],
                    group_id: None,
                });
                old_line += 1;
                new_line += 1;
            }
            FoldItem::Op(DiffOp::Delete { text }) => {
                summary.removed += 1;
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::Delete,
                    old_line: Some(old_line),
                    new_line: None,
                    marker: "-",
                    segments: vec![normal(text)],
                    group_id: None,
                });
                old_line += 1;
            }
            FoldItem::Op(DiffOp::Insert { text }) => {
                summary.added += 1;
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::Insert,
                    old_line: None,
                    new_line: Some(new_line),
                    marker: "+",
                    segments: vec![normal(text)],
                    group_id: None,
                });
                new_line += 1;
            }
            FoldItem::Op(DiffOp::Inline { segments }) => {
                summary.edited += 1;
                let current_group = group_id;
                group_id += 1;
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::ReplaceOld,
                    old_line: Some(old_line),
                    new_line: None,
                    marker: "~",
                    segments: old_segments(&segments),
                    group_id: Some(current_group),
                });
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::ReplaceNew,
                    old_line: None,
                    new_line: Some(new_line),
                    marker: "~",
                    segments: new_segments(&segments),
                    group_id: Some(current_group),
                });
                old_line += 1;
                new_line += 1;
            }
            FoldItem::Skipped(count) => rows.push(DiffViewRow {
                kind: DiffViewRowKind::Fold,
                old_line: None,
                new_line: None,
                marker: "",
                segments: vec![normal(format!("... {count} unchanged lines ..."))],
                group_id: None,
            }),
        }
    }

    RenderedDiffView { rows, summary }
}

fn normal(text: impl Into<String>) -> DiffViewSegment {
    DiffViewSegment {
        kind: DiffViewSegmentKind::Normal,
        text: text.into(),
    }
}

fn token(kind: DiffViewSegmentKind, text: impl Into<String>) -> DiffViewSegment {
    DiffViewSegment {
        kind,
        text: text.into(),
    }
}

fn old_segments(segments: &[crate::diff_core::InlineDiffSegment]) -> Vec<DiffViewSegment> {
    let mut out = Vec::new();
    for segment in segments {
        match segment.kind {
            InlineDiffSegmentKind::Equal => out.push(normal(segment.text.clone())),
            InlineDiffSegmentKind::Delete => {
                out.push(token(DiffViewSegmentKind::DeleteToken, segment.text.clone()));
            }
            InlineDiffSegmentKind::Insert => {}
        }
    }
    out
}

fn new_segments(segments: &[crate::diff_core::InlineDiffSegment]) -> Vec<DiffViewSegment> {
    let mut out = Vec::new();
    for segment in segments {
        match segment.kind {
            InlineDiffSegmentKind::Equal => out.push(normal(segment.text.clone())),
            InlineDiffSegmentKind::Delete => {}
            InlineDiffSegmentKind::Insert => {
                out.push(token(DiffViewSegmentKind::InsertToken, segment.text.clone()));
            }
        }
    }
    out
}

fn fold_ops_for_view(ops: &[DiffOp], options: &DiffOptions) -> Vec<FoldItem> {
    fn is_change(op: &DiffOp) -> bool {
        !matches!(op, DiffOp::Context { .. })
    }

    if ops.len() <= options.display_full_context_max_lines {
        return ops.iter().cloned().map(FoldItem::Op).collect();
    }

    let radius = options.unified_context_radius;
    let mut keep = vec![false; ops.len()];
    for (idx, op) in ops.iter().enumerate() {
        if is_change(op) {
            let lo = idx.saturating_sub(radius);
            let hi = (idx + radius + 1).min(ops.len());
            for slot in keep.iter_mut().take(hi).skip(lo) {
                *slot = true;
            }
        }
    }

    let mut out = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        if keep[i] {
            out.push(FoldItem::Op(ops[i].clone()));
            i += 1;
        } else {
            let start = i;
            while i < ops.len() && !keep[i] {
                i += 1;
            }
            out.push(FoldItem::Skipped(i - start));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_core::{DiffOptions, build_display_diff};

    #[test]
    fn equal_text_produces_no_differences_notice() {
        let diff = build_display_diff("same\n", "same\n", &DiffOptions::default());
        let view = build_diff_view(&diff, &DiffOptions::default());

        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.rows[0].kind, DiffViewRowKind::Notice);
        assert_eq!(view.rows[0].segments[0].text, "No differences");
        assert_eq!(
            view.summary,
            ChangeSummary {
                removed: 0,
                added: 0,
                edited: 0,
            }
        );
    }

    #[test]
    fn delete_row_has_old_line_and_blank_new_line() {
        let diff = build_display_diff("a\nb\n", "a\n", &DiffOptions::default());
        let view = build_diff_view(&diff, &DiffOptions::default());
        let delete = view
            .rows
            .iter()
            .find(|row| row.kind == DiffViewRowKind::Delete)
            .expect("delete row");

        assert_eq!(delete.old_line, Some(2));
        assert_eq!(delete.new_line, None);
        assert_eq!(delete.marker, "-");
    }

    #[test]
    fn insert_row_has_blank_old_line_and_new_line() {
        let diff = build_display_diff("a\n", "a\nb\n", &DiffOptions::default());
        let view = build_diff_view(&diff, &DiffOptions::default());
        let insert = view
            .rows
            .iter()
            .find(|row| row.kind == DiffViewRowKind::Insert)
            .expect("insert row");

        assert_eq!(insert.old_line, None);
        assert_eq!(insert.new_line, Some(2));
        assert_eq!(insert.marker, "+");
    }

    #[test]
    fn context_after_insert_shows_old_new_offset() {
        let diff = build_display_diff("a\nc\n", "a\nb\nc\n", &DiffOptions::default());
        let view = build_diff_view(&diff, &DiffOptions::default());
        let c_row = view
            .rows
            .iter()
            .find(|row| row.segments.iter().any(|segment| segment.text == "c"))
            .expect("context row after insert");

        assert_eq!(c_row.kind, DiffViewRowKind::Context);
        assert_eq!(c_row.old_line, Some(2));
        assert_eq!(c_row.new_line, Some(3));
    }

    #[test]
    fn inline_change_becomes_grouped_replace_rows() {
        let diff = build_display_diff(
            "let mode = \"old\";\n",
            "let mode = \"new\";\n",
            &DiffOptions::default(),
        );
        let view = build_diff_view(&diff, &DiffOptions::default());

        assert_eq!(view.rows.len(), 2);
        assert_eq!(view.rows[0].kind, DiffViewRowKind::ReplaceOld);
        assert_eq!(view.rows[1].kind, DiffViewRowKind::ReplaceNew);
        assert_eq!(view.rows[0].old_line, Some(1));
        assert_eq!(view.rows[0].new_line, None);
        assert_eq!(view.rows[1].old_line, None);
        assert_eq!(view.rows[1].new_line, Some(1));
        assert_eq!(view.rows[0].group_id, Some(1));
        assert_eq!(view.rows[1].group_id, Some(1));
        assert_eq!(view.rows[0].marker, "~");
        assert_eq!(view.rows[1].marker, "~");
        assert!(
            view.rows[0]
                .segments
                .iter()
                .any(|segment| segment.kind == DiffViewSegmentKind::DeleteToken)
        );
        assert!(
            view.rows[1]
                .segments
                .iter()
                .any(|segment| segment.kind == DiffViewSegmentKind::InsertToken)
        );
    }
}
```

- [ ] **Step 3: Run the focused tests**

Run:

```bash
cargo test diff_view
```

Expected: PASS for all new `diff_view` tests.

- [ ] **Step 4: Commit**

Run:

```bash
git add src/lib.rs src/diff_view.rs
git commit -m "feat: add semantic diff view model"
```

## Task 2: Render the New View into a Styled Text Buffer

**Files:**
- Modify: `src/ui_fltk.rs`
- Test: `src/ui_fltk.rs`

- [ ] **Step 1: Replace old output renderer tests with view-renderer tests**

In `src/ui_fltk.rs`, update the tests module so it tests a new function named `render_diff_view_text`.

Add these tests inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn render_diff_view_text_shows_semantic_old_new_gutters() {
    use crate::{
        diff_core::{DiffOptions, build_display_diff},
        diff_view::build_diff_view,
    };

    let diff = build_display_diff("a\nc\n", "a\nb\nc\n", &DiffOptions::default());
    let view = build_diff_view(&diff, &DiffOptions::default());
    let rendered = render_diff_view_text(&view);

    assert!(rendered.text.contains("OLD  NEW  K"));
    assert!(rendered.text.contains("   | 2   | + | b"));
    assert!(rendered.text.contains("2   | 3   |   | c"));
    assert_eq!(rendered.text.len(), rendered.styles.len());
}

#[test]
fn render_diff_view_text_marks_replacement_rows_neutral_with_token_styles() {
    use crate::{
        diff_core::{DiffOptions, build_display_diff},
        diff_view::build_diff_view,
    };

    let diff = build_display_diff(
        "let mode = \"old\";\n",
        "let mode = \"new\";\n",
        &DiffOptions::default(),
    );
    let view = build_diff_view(&diff, &DiffOptions::default());
    let rendered = render_diff_view_text(&view);

    assert!(rendered.text.contains("~ | let mode"));
    assert!(rendered.styles.contains('H'), "replacement block style required");
    assert!(rendered.styles.contains('E'), "delete token style required");
    assert!(rendered.styles.contains('F'), "insert token style required");
    assert_eq!(rendered.text.len(), rendered.styles.len());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test ui_fltk::tests::render_diff_view_text -- --nocapture
```

Expected: FAIL because `render_diff_view_text` does not exist.

- [ ] **Step 3: Extend the style table for neutral replacement rows**

In `style_table_ext`, append style entries after existing `G`:

```rust
        // 'H' neutral replacement row
        StyleTableEntryExt {
            color: palette.text,
            font: Font::Courier,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.header_bg,
        },
        // 'I' gutter / marker
        StyleTableEntryExt {
            color: palette.muted,
            font: Font::Courier,
            size: 14,
            attr: TextAttr::None,
            bgcolor: palette.header_bg,
        },
```

- [ ] **Step 4: Disable the built-in diff display line-number gutter**

The diff result will render semantic old/new line numbers inside the text buffer. The FLTK display counter would become a third, misleading gutter.

Modify `make_diff_display` so it does not call `configure_line_numbers`. Replace:

```rust
    configure_line_numbers(&mut display, palette);
```

with:

```rust
    display.set_linenumber_width(0);
```

- [ ] **Step 5: Add text rendering helpers**

In `src/ui_fltk.rs`, replace `render_display_ops` usage with a new rendering helper. Add these functions near the old `RenderedDiff` helpers:

```rust
fn render_diff_view_text(view: &crate::diff_view::RenderedDiffView) -> RenderedDiff {
    use crate::diff_view::{DiffViewRowKind, DiffViewSegmentKind};

    let mut text = String::new();
    let mut styles = String::new();

    push_styled(&mut text, &mut styles, "OLD  NEW  K | Text\n", 'B');
    push_styled(&mut text, &mut styles, "---------------\n", 'B');

    for row in &view.rows {
        let row_style = match row.kind {
            DiffViewRowKind::Context => 'A',
            DiffViewRowKind::Delete => 'D',
            DiffViewRowKind::Insert => 'C',
            DiffViewRowKind::ReplaceOld | DiffViewRowKind::ReplaceNew => 'H',
            DiffViewRowKind::Fold | DiffViewRowKind::Notice => 'G',
        };

        push_styled(
            &mut text,
            &mut styles,
            &format_line_no(row.old_line),
            'I',
        );
        push_styled(&mut text, &mut styles, " | ", 'I');
        push_styled(
            &mut text,
            &mut styles,
            &format_line_no(row.new_line),
            'I',
        );
        push_styled(&mut text, &mut styles, " | ", 'I');
        push_styled(&mut text, &mut styles, &format!("{:<1}", row.marker), 'I');
        push_styled(&mut text, &mut styles, " | ", 'I');

        for segment in &row.segments {
            let segment_style = match segment.kind {
                DiffViewSegmentKind::Normal => row_style,
                DiffViewSegmentKind::DeleteToken => 'E',
                DiffViewSegmentKind::InsertToken => 'F',
            };
            push_styled(&mut text, &mut styles, &segment.text, segment_style);
        }
        push_styled(&mut text, &mut styles, "\n", row_style);
    }

    RenderedDiff { text, styles }
}

fn format_line_no(line: Option<usize>) -> String {
    match line {
        Some(value) => format!("{value:<3}"),
        None => "   ".to_string(),
    }
}
```

- [ ] **Step 6: Update `render_state` to use `build_diff_view`**

Replace the current `let rendered = if state.has_stale_diff() { ... } else { ... };` block in `render_state` with:

```rust
    let view = crate::diff_view::build_diff_view(state.diff(), state.options());
    let rendered = if state.has_stale_diff() {
        with_leading_notice(
            render_diff_view_text(&view),
            "Previous diff is stale. Press Compare to update.\n\n",
            'G',
        )
    } else {
        render_diff_view_text(&view)
    };
```

Leave the buffer assignment below it unchanged.

- [ ] **Step 7: Remove old render/fold helpers after tests pass**

Delete these old items from `src/ui_fltk.rs` after `render_state` compiles with `render_diff_view_text`:

```rust
fn render_display_ops(...)
enum FoldItem { ... }
fn fold_ops(...)
```

Keep `RenderedDiff`, `with_leading_notice`, and `push_styled`.

- [ ] **Step 8: Run focused tests**

Run:

```bash
cargo test ui_fltk::tests
```

Expected: PASS.

- [ ] **Step 9: Run full tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 10: Commit**

Run:

```bash
git add src/ui_fltk.rs
git commit -m "feat: render semantic unified diff text"
```

## Task 3: Add Diff Toolbar and Overview Rail Widgets

**Files:**
- Modify: `src/ui_fltk.rs`
- Test: `src/ui_fltk.rs`

- [ ] **Step 1: Add UI constants**

Near existing layout constants in `src/ui_fltk.rs`, add:

```rust
const DIFF_TOOLBAR_HEIGHT: i32 = 32;
const OVERVIEW_RAIL_WIDTH: i32 = 14;
```

- [ ] **Step 2: Extend `UiHandles`**

Modify `UiHandles`:

```rust
struct UiHandles {
    left_editor: TextEditor,
    right_editor: TextEditor,
    left_buffer: TextBuffer,
    right_buffer: TextBuffer,
    diff_buffer: TextBuffer,
    diff_style_buffer: TextBuffer,
    diff_summary: Frame,
    overview_rail: Frame,
    status: Frame,
    copy_diff: Button,
}
```

- [ ] **Step 3: Build a diff result container**

Replace the single call:

```rust
let (mut diff_display, diff_buffer, diff_style_buffer) = make_diff_display(palette);
```

with:

```rust
let mut diff_container = Flex::default().column();
let mut diff_toolbar = Flex::default().row();
diff_toolbar.set_pad(6);
let mut diff_mode = Frame::default().with_label("Unified Review");
diff_mode.set_frame(FrameType::FlatBox);
diff_mode.set_color(palette.header_bg);
diff_mode.set_label_color(palette.text);
diff_mode.set_label_size(13);
let mut prev_change = make_button("Prev", false, palette);
let mut next_change = make_button("Next", false, palette);
let mut diff_summary = Frame::default().with_label("0 removed  0 added  0 edited");
diff_summary.set_frame(FrameType::FlatBox);
diff_summary.set_color(palette.header_bg);
diff_summary.set_label_color(palette.muted);
diff_summary.set_label_size(13);
diff_summary.set_align(fltk::enums::Align::Right | fltk::enums::Align::Inside);
diff_toolbar.fixed(&diff_mode, 120);
diff_toolbar.fixed(&prev_change, 58);
diff_toolbar.fixed(&next_change, 58);
diff_toolbar.end();

let mut diff_body = Flex::default().row();
let (mut diff_display, diff_buffer, diff_style_buffer) = make_diff_display(palette);
let mut overview_rail = Frame::default();
overview_rail.set_frame(FrameType::FlatBox);
overview_rail.set_color(palette.header_bg);
diff_body.fixed(&overview_rail, OVERVIEW_RAIL_WIDTH);
diff_body.end();

diff_container.fixed(&diff_toolbar, DIFF_TOOLBAR_HEIGHT);
diff_container.end();
```

The `diff_container` is created between the action bar and status bar, so it remains the flexible middle child. Keep the root sizing block in this shape:

```rust
root.fixed(
    &input_row,
    input_height_for(config.config.height, config.config.vertical_split),
);
root.fixed(&actions, ACTION_BAR_HEIGHT);
root.fixed(&status, STATUS_BAR_HEIGHT);
root.end();
```

No explicit fixed height is needed for `diff_container`; it occupies remaining space because it is added before `status`.

- [ ] **Step 4: Store new handles**

Update `UiHandles` initialization:

```rust
let handles = Rc::new(RefCell::new(UiHandles {
    left_editor,
    right_editor,
    left_buffer,
    right_buffer,
    diff_buffer,
    diff_style_buffer,
    diff_summary,
    overview_rail,
    status,
    copy_diff: copy_diff.clone(),
}));
```

- [ ] **Step 5: Update render state summary and rail color**

In `render_state`, after `let handles = handles.borrow();`, clone the new handles:

```rust
let mut diff_summary = handles.diff_summary.clone();
let mut overview_rail = handles.overview_rail.clone();
```

After building `view`, set:

```rust
diff_summary.set_label(&format!(
    "{} removed  {} added  {} edited",
    view.summary.removed, view.summary.added, view.summary.edited
));

overview_rail.redraw();
```

This first pass keeps the rail passive and visually present. Task 4 makes it draw change marks.

- [ ] **Step 6: Add a layout smoke test for summary formatting**

In `ui_fltk` tests, add:

```rust
#[test]
fn diff_summary_label_formats_counts() {
    let summary = crate::diff_view::ChangeSummary {
        removed: 2,
        added: 3,
        edited: 1,
    };

    assert_eq!(
        diff_summary_label(&summary),
        "2 removed  3 added  1 edited"
    );
}
```

Add helper near rendering functions:

```rust
fn diff_summary_label(summary: &crate::diff_view::ChangeSummary) -> String {
    format!(
        "{} removed  {} added  {} edited",
        summary.removed, summary.added, summary.edited
    )
}
```

Then use `diff_summary_label(&view.summary)` in `render_state` instead of duplicating the `format!`.

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test ui_fltk::tests
cargo test
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```bash
git add src/ui_fltk.rs
git commit -m "feat: add unified diff toolbar"
```

## Task 4: Draw Passive Change Overview Marks

**Files:**
- Modify: `src/diff_view.rs`
- Modify: `src/ui_fltk.rs`
- Test: `src/diff_view.rs`

- [ ] **Step 1: Add change mark model and tests**

In `src/diff_view.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeMarkKind {
    Delete,
    Insert,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeMark {
    pub row_index: usize,
    pub kind: ChangeMarkKind,
}
```

Add marks to `RenderedDiffView`:

```rust
pub struct RenderedDiffView {
    pub rows: Vec<DiffViewRow>,
    pub summary: ChangeSummary,
    pub marks: Vec<ChangeMark>,
}
```

Update all constructors to include `marks`.

Add this test:

```rust
#[test]
fn change_marks_track_visible_change_rows() {
    let diff = build_display_diff(
        "a\nold\nsame\n",
        "a\nnew\nsame\nextra\n",
        &DiffOptions::default(),
    );
    let view = build_diff_view(&diff, &DiffOptions::default());

    assert!(
        view.marks
            .iter()
            .any(|mark| mark.kind == ChangeMarkKind::Replace)
    );
    assert!(
        view.marks
            .iter()
            .any(|mark| mark.kind == ChangeMarkKind::Insert)
    );
}
```

- [ ] **Step 2: Populate marks in `build_diff_view`**

When pushing rows, record marks:

```rust
let mut marks = Vec::new();
```

For delete:

```rust
marks.push(ChangeMark {
    row_index: rows.len(),
    kind: ChangeMarkKind::Delete,
});
```

For insert:

```rust
marks.push(ChangeMark {
    row_index: rows.len(),
    kind: ChangeMarkKind::Insert,
});
```

For inline replacement, push one mark at the old row index:

```rust
marks.push(ChangeMark {
    row_index: rows.len(),
    kind: ChangeMarkKind::Replace,
});
```

Return `RenderedDiffView { rows, summary, marks }`.

- [ ] **Step 3: Run model tests**

Run:

```bash
cargo test diff_view
```

Expected: PASS.

- [ ] **Step 4: Implement rail label fallback**

FLTK custom drawing can be finicky. First implementation should be simple and reliable: render the rail as a narrow text label with vertical marks.

Add helper in `src/ui_fltk.rs`:

```rust
fn overview_rail_label(view: &crate::diff_view::RenderedDiffView) -> String {
    use crate::diff_view::ChangeMarkKind;

    if view.rows.is_empty() || view.marks.is_empty() {
        return String::new();
    }

    let height = 12usize;
    let mut slots = vec![' '; height];
    let last_row = view.rows.len().saturating_sub(1).max(1);
    for mark in &view.marks {
        let slot = (mark.row_index * (height - 1)) / last_row;
        slots[slot] = match mark.kind {
            ChangeMarkKind::Delete => '-',
            ChangeMarkKind::Insert => '+',
            ChangeMarkKind::Replace => '~',
        };
    }

    slots
        .into_iter()
        .map(|slot| slot.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}
```

In `render_state`, replace the temporary rail color update with:

```rust
overview_rail.set_label(&overview_rail_label(&view));
overview_rail.set_label_color(handles.status.label_color());
overview_rail.redraw();
```

- [ ] **Step 5: Add rail label test**

Add to `ui_fltk` tests:

```rust
#[test]
fn overview_rail_label_places_change_markers() {
    use crate::{
        diff_core::{DiffOptions, build_display_diff},
        diff_view::build_diff_view,
    };

    let diff = build_display_diff("a\nold\n", "a\nnew\nextra\n", &DiffOptions::default());
    let view = build_diff_view(&diff, &DiffOptions::default());
    let label = overview_rail_label(&view);

    assert!(label.contains('~'));
    assert!(label.contains('+'));
}
```

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test diff_view
cargo test ui_fltk::tests
cargo test
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add src/diff_view.rs src/ui_fltk.rs
git commit -m "feat: add diff overview rail marks"
```

## Task 5: Preserve Copy Diff and Stale Behavior

**Files:**
- Modify: `src/ui_fltk.rs`
- Test: `src/ui_fltk.rs`, existing `src/diff_core.rs`

- [ ] **Step 1: Add stale notice renderer test**

Update the existing stale notice test in `ui_fltk` to use `build_diff_view` and `render_diff_view_text`:

```rust
#[test]
fn with_leading_notice_keeps_text_and_styles_aligned() {
    use crate::{
        diff_core::{DiffOptions, build_display_diff},
        diff_view::build_diff_view,
    };

    let diff = build_display_diff(
        "i wanna eatt banana",
        "i wanna eat bananas",
        &DiffOptions::default(),
    );
    let view = build_diff_view(&diff, &DiffOptions::default());
    let rendered = render_diff_view_text(&view);
    assert_eq!(rendered.text.len(), rendered.styles.len());

    let noticed = with_leading_notice(
        rendered,
        "Previous diff is stale. Press Compare to update.\n\n",
        'G',
    );
    assert_eq!(noticed.text.len(), noticed.styles.len());
    assert!(
        noticed
            .text
            .starts_with("Previous diff is stale. Press Compare to update.\n\n")
    );
    let notice_len = "Previous diff is stale. Press Compare to update.\n\n".len();
    let (head, tail) = noticed.styles.split_at(notice_len);
    assert!(head.chars().all(|c| c == 'G'));
    assert!(!tail.is_empty());
    assert!(tail.contains('F'));
}
```

- [ ] **Step 2: Confirm Copy Diff path still uses `render_unified_diff`**

Verify `copy_current_diff` still contains:

```rust
let diff = render_unified_diff(state_snapshot.diff());
```

Do not change this line.

- [ ] **Step 3: Run copy/unified tests**

Run:

```bash
cargo test render_unified_diff
cargo test ui_fltk::tests::with_leading_notice_keeps_text_and_styles_aligned
cargo test
```

Expected: PASS.

- [ ] **Step 4: Commit**

Run:

```bash
git add src/ui_fltk.rs
git commit -m "test: preserve stale and copy diff behavior"
```

## Task 6: Manual Native UI Verification and Polish

**Files:**
- Modify: `src/ui_fltk.rs` if visual polish is needed.
- Test: manual native app run plus automated tests.

- [ ] **Step 1: Run all tests before visual verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 2: Run the native app**

Run:

```bash
cargo run
```

Expected: Slippy opens a native FLTK window.

- [ ] **Step 3: Manual smoke input**

Paste this into the left input:

```text
fn render_panel(state: &AppState) {
    let mode = "unified";
    show_prefixes(buffer);
    draw_header(title);
    draw_rows(state.rows());
    render_status(state.status());
}
copy_diff.set_enabled(has_current_diff);
save_layout();
```

Paste this into the right input:

```text
fn render_panel(state: &AppState) {
    let mode = "unified-plus";
    align_paired_rows(buffer);
    draw_header(title);
    draw_rows(state.rows());
    draw_change_overview();
    render_status(state.status());
}
copy_diff.set_enabled(has_current_diff);
copy_diff.set_tooltip("Copy standard unified diff");
save_layout();
```

Click `Compare`.

Expected visual result:

- Diff toolbar shows non-zero removed/added/edited counts.
- Replacement pair rows use `~`.
- Deleted-only rows have old line number and blank new line number.
- Inserted-only rows have blank old line number and new line number.
- Later context rows show old/new line offsets after the insertion.
- Pure insert rows are green-tinted.
- Replacement rows are neutral-tinted with red/green token highlights.
- Right rail shows `~` and `+` marks.

- [ ] **Step 4: Verify Copy Diff output**

Click `Copy Diff`, paste into a scratch editor, and verify it contains standard unified text:

```text
--- left
+++ right
@@
-...
+...
```

Expected: copied text is not the visual table with `OLD  NEW  K`.

- [ ] **Step 5: Verify small window behavior**

Resize the app close to minimum width.

Expected:

- Inputs still stack below 760 px as before.
- Diff toolbar text does not overlap action buttons.
- Old/new/kind gutters remain visible.
- Overview rail does not crowd out the diff text.

- [ ] **Step 6: Apply minimal polish if needed**

Only make polish changes that directly address the smoke results. Examples:

```rust
const OVERVIEW_RAIL_WIDTH: i32 = 18;
```

or:

```rust
diff_summary.set_label_size(12);
```

Do not add a file tree, tabs, hunk apply buttons, or custom settings UI.

- [ ] **Step 7: Run final tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 8: Commit final polish**

If no polish changes were needed, skip this commit. If changes were made:

```bash
git add src/ui_fltk.rs
git commit -m "polish: refine unified diff view"
```

## Task 7: Documentation Update

**Files:**
- Modify: `README.md`
- Modify: `DESIGN.md`

- [ ] **Step 1: Update README feature wording**

In `README.md`, replace the current `Read-only diff pane with IntelliJ-style rendering` bullet sublist with:

```markdown
- **Read-only unified review diff pane** with IntelliJ-inspired rendering:
  - Semantic old/new line-number gutters, so inserted rows have no old line number and deleted rows have no new line number.
  - Soft row coloring for pure insertions/deletions.
  - Neutral replacement blocks for paired edits, with red/green token highlights for the exact changed fragments.
  - A compact change overview rail showing where edits occur in the rendered diff.
  - Adaptive folding: large diffs collapse runs of unchanged context into a `... N unchanged lines ...` marker instead of scrolling forever.
```

- [ ] **Step 2: Update DESIGN visual system**

In `DESIGN.md`, update the diff rendering bullets under "Visual System" to say:

```markdown
- The diff result uses a unified review layout with old/new line-number gutters, a marker column, rendered text, and a narrow change overview rail.
- `+` insertion rows use soft insert colors; `-` deletion rows use soft delete colors.
- Replacement blocks pair similar deleted and inserted lines before rendering; paired rows use a neutral block background with `~` markers and stronger red/green token highlights for the exact changed fragments.
- The old/new gutters are semantic references: inserted rows leave the old line blank, deleted rows leave the new line blank, and later context rows may show offset line numbers.
```

Also replace the old adaptive folding marker example `⋯ N unchanged ⋯` with:

```markdown
`... N unchanged lines ...`
```

- [ ] **Step 3: Run docs grep check**

Run:

```bash
rg "side-by-side aligned diff view|⋯ N unchanged|diff output where" README.md DESIGN.md docs/superpowers/specs/2026-06-27-idea-like-unified-diff-view-design.md
```

Expected:

- `README.md` and `DESIGN.md` should no longer describe the old single colored output box.
- The non-goal about side-by-side may remain if phrased as "not primary display".

- [ ] **Step 4: Commit docs**

Run:

```bash
git add README.md DESIGN.md
git commit -m "docs: update unified diff view description"
```

## Final Verification

- [ ] **Step 1: Run full tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 2: Check worktree**

Run:

```bash
git status --short
```

Expected: no modified tracked files. Pre-existing unrelated untracked files may remain, such as `scripts/`.

- [ ] **Step 3: Summarize completed commits**

Run:

```bash
git log --oneline -8
```

Expected: recent commits include the design commit plus implementation commits from this plan.
