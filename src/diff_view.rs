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

fn ordered_char_selection(selection: DiffCharSelection) -> (DiffCharPosition, DiffCharPosition) {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedDiffView {
    pub rows: Vec<DiffViewRow>,
    pub summary: ChangeSummary,
    pub marks: Vec<ChangeMark>,
    pub left_no_newline: bool,
    pub right_no_newline: bool,
}

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

    /// The rendered text of a single row (its text column), or `None` for an
    /// out-of-range index.
    pub fn row_text(&self, index: usize) -> Option<String> {
        self.rows.get(index).map(|row| {
            row.segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect()
        })
    }

    /// The plain text of the selected row range (inclusive on both ends,
    /// order-independent), one line per row. Used when copying a user
    /// selection from the diff canvas.
    pub fn selection_text(&self, a: usize, b: usize) -> String {
        let lo = a.min(b);
        let hi = a.max(b);
        (lo..=hi)
            .filter_map(|index| self.row_text(index))
            .collect::<Vec<_>>()
            .join("\n")
    }

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
}

enum FoldItem {
    Op(DiffOp),
    Skipped(usize),
}

fn normal_segment(text: impl Into<String>) -> DiffViewSegment {
    DiffViewSegment {
        kind: DiffViewSegmentKind::Normal,
        text: text.into(),
    }
}

pub fn build_diff_view(diff: &DisplayDiff, options: &DiffOptions) -> RenderedDiffView {
    if diff.ops.is_empty() {
        return RenderedDiffView {
            rows: vec![DiffViewRow {
                kind: DiffViewRowKind::Notice,
                old_line: None,
                new_line: None,
                marker: "",
                segments: vec![normal_segment("No differences")],
                group_id: None,
            }],
            summary: ChangeSummary {
                removed: 0,
                added: 0,
                edited: 0,
            },
            marks: vec![],
            left_no_newline: diff.left_no_newline,
            right_no_newline: diff.right_no_newline,
        };
    }

    let mut rows = Vec::new();
    let mut marks = Vec::new();
    let mut summary = ChangeSummary {
        removed: 0,
        added: 0,
        edited: 0,
    };
    let mut old_line = 1usize;
    let mut new_line = 1usize;
    let mut next_group_id = 0usize;

    for item in fold_ops(&diff.ops, options) {
        match item {
            FoldItem::Op(DiffOp::Context { text }) => {
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::Context,
                    old_line: Some(old_line),
                    new_line: Some(new_line),
                    marker: "",
                    segments: vec![normal_segment(text)],
                    group_id: None,
                });
                old_line += 1;
                new_line += 1;
            }
            FoldItem::Op(DiffOp::Delete { text }) => {
                marks.push(ChangeMark {
                    row_index: rows.len(),
                    kind: ChangeMarkKind::Delete,
                });
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::Delete,
                    old_line: Some(old_line),
                    new_line: None,
                    marker: "-",
                    segments: vec![normal_segment(text)],
                    group_id: None,
                });
                old_line += 1;
                summary.removed += 1;
            }
            FoldItem::Op(DiffOp::Insert { text }) => {
                marks.push(ChangeMark {
                    row_index: rows.len(),
                    kind: ChangeMarkKind::Insert,
                });
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::Insert,
                    old_line: None,
                    new_line: Some(new_line),
                    marker: "+",
                    segments: vec![normal_segment(text)],
                    group_id: None,
                });
                new_line += 1;
                summary.added += 1;
            }
            FoldItem::Op(DiffOp::Inline { segments }) => {
                let group_id = next_group_id;
                next_group_id += 1;

                let mut old_segments = Vec::new();
                let mut new_segments = Vec::new();

                for segment in segments {
                    match segment.kind {
                        InlineDiffSegmentKind::Equal => {
                            old_segments.push(normal_segment(segment.text.clone()));
                            new_segments.push(normal_segment(segment.text));
                        }
                        InlineDiffSegmentKind::Delete => {
                            old_segments.push(DiffViewSegment {
                                kind: DiffViewSegmentKind::DeleteToken,
                                text: segment.text,
                            });
                        }
                        InlineDiffSegmentKind::Insert => {
                            new_segments.push(DiffViewSegment {
                                kind: DiffViewSegmentKind::InsertToken,
                                text: segment.text,
                            });
                        }
                    }
                }

                marks.push(ChangeMark {
                    row_index: rows.len(),
                    kind: ChangeMarkKind::Replace,
                });
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::ReplaceOld,
                    old_line: Some(old_line),
                    new_line: None,
                    marker: "~",
                    segments: old_segments,
                    group_id: Some(group_id),
                });
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::ReplaceNew,
                    old_line: None,
                    new_line: Some(new_line),
                    marker: "~",
                    segments: new_segments,
                    group_id: Some(group_id),
                });

                old_line += 1;
                new_line += 1;
                summary.edited += 1;
            }
            FoldItem::Skipped(count) => {
                rows.push(DiffViewRow {
                    kind: DiffViewRowKind::Fold,
                    old_line: None,
                    new_line: None,
                    marker: "",
                    segments: vec![normal_segment(format!("... {count} unchanged lines ..."))],
                    group_id: None,
                });
                old_line += count;
                new_line += count;
            }
        }
    }

    RenderedDiffView {
        rows,
        summary,
        marks,
        left_no_newline: diff.left_no_newline,
        right_no_newline: diff.right_no_newline,
    }
}

fn fold_ops(ops: &[DiffOp], options: &DiffOptions) -> Vec<FoldItem> {
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
            for slot in &mut keep[lo..hi] {
                *slot = true;
            }
        }
    }

    let mut out = Vec::new();
    let mut index = 0;
    while index < ops.len() {
        if keep[index] {
            out.push(FoldItem::Op(ops[index].clone()));
            index += 1;
            continue;
        }

        let start = index;
        while index < ops.len() && !keep[index] {
            index += 1;
        }
        out.push(FoldItem::Skipped(index - start));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_core::build_display_diff;

    fn text(row: &DiffViewRow) -> String {
        row.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    }

    #[test]
    fn equal_text_produces_no_differences_notice() {
        let diff = build_display_diff("same", "same", &DiffOptions::default());

        let rendered = build_diff_view(&diff, &DiffOptions::default());

        assert_eq!(
            rendered,
            RenderedDiffView {
                rows: vec![DiffViewRow {
                    kind: DiffViewRowKind::Notice,
                    old_line: None,
                    new_line: None,
                    marker: "",
                    segments: vec![DiffViewSegment {
                        kind: DiffViewSegmentKind::Normal,
                        text: "No differences".to_string(),
                    }],
                    group_id: None,
                }],
                summary: ChangeSummary {
                    removed: 0,
                    added: 0,
                    edited: 0,
                },
                marks: vec![],
                left_no_newline: true,
                right_no_newline: true,
            }
        );
    }

    #[test]
    fn delete_row_has_old_line_and_blank_new_line() {
        let diff = build_display_diff("a\nb\n", "a\n", &DiffOptions::default());

        let rendered = build_diff_view(&diff, &DiffOptions::default());

        assert_eq!(rendered.summary.removed, 1);
        assert_eq!(rendered.rows.len(), 2);
        assert_eq!(rendered.rows[1].kind, DiffViewRowKind::Delete);
        assert_eq!(rendered.rows[1].old_line, Some(2));
        assert_eq!(rendered.rows[1].new_line, None);
        assert_eq!(rendered.rows[1].marker, "-");
        assert_eq!(text(&rendered.rows[1]), "b");
    }

    #[test]
    fn insert_row_has_blank_old_line_and_new_line() {
        let diff = build_display_diff("a\n", "a\nb\n", &DiffOptions::default());

        let rendered = build_diff_view(&diff, &DiffOptions::default());

        assert_eq!(rendered.summary.added, 1);
        assert_eq!(rendered.rows.len(), 2);
        assert_eq!(rendered.rows[1].kind, DiffViewRowKind::Insert);
        assert_eq!(rendered.rows[1].old_line, None);
        assert_eq!(rendered.rows[1].new_line, Some(2));
        assert_eq!(rendered.rows[1].marker, "+");
        assert_eq!(text(&rendered.rows[1]), "b");
    }

    #[test]
    fn context_after_insert_shows_old_new_offset() {
        let diff = build_display_diff("a\nc\n", "a\nb\nc\n", &DiffOptions::default());

        let rendered = build_diff_view(&diff, &DiffOptions::default());

        assert_eq!(rendered.rows.len(), 3);
        assert_eq!(rendered.rows[0].kind, DiffViewRowKind::Context);
        assert_eq!(rendered.rows[0].old_line, Some(1));
        assert_eq!(rendered.rows[0].new_line, Some(1));
        assert_eq!(rendered.rows[1].kind, DiffViewRowKind::Insert);
        assert_eq!(rendered.rows[1].old_line, None);
        assert_eq!(rendered.rows[1].new_line, Some(2));
        assert_eq!(rendered.rows[2].kind, DiffViewRowKind::Context);
        assert_eq!(rendered.rows[2].old_line, Some(2));
        assert_eq!(rendered.rows[2].new_line, Some(3));
        assert_eq!(text(&rendered.rows[2]), "c");
    }

    #[test]
    fn inline_change_becomes_grouped_replace_rows() {
        let diff = build_display_diff(
            "let mode = \"old\";\n",
            "let mode = \"new\";\n",
            &DiffOptions::default(),
        );

        let rendered = build_diff_view(&diff, &DiffOptions::default());

        assert_eq!(
            rendered.summary,
            ChangeSummary {
                removed: 0,
                added: 0,
                edited: 1,
            }
        );
        assert_eq!(rendered.rows.len(), 2);

        let old_row = &rendered.rows[0];
        let new_row = &rendered.rows[1];

        assert_eq!(old_row.kind, DiffViewRowKind::ReplaceOld);
        assert_eq!(old_row.old_line, Some(1));
        assert_eq!(old_row.new_line, None);
        assert_eq!(old_row.marker, "~");
        assert_eq!(old_row.group_id, Some(0));
        assert_eq!(
            old_row.segments,
            vec![
                DiffViewSegment {
                    kind: DiffViewSegmentKind::Normal,
                    text: "let mode = \"".to_string(),
                },
                DiffViewSegment {
                    kind: DiffViewSegmentKind::DeleteToken,
                    text: "old".to_string(),
                },
                DiffViewSegment {
                    kind: DiffViewSegmentKind::Normal,
                    text: "\";".to_string(),
                },
            ]
        );

        assert_eq!(new_row.kind, DiffViewRowKind::ReplaceNew);
        assert_eq!(new_row.old_line, None);
        assert_eq!(new_row.new_line, Some(1));
        assert_eq!(new_row.marker, "~");
        assert_eq!(new_row.group_id, Some(0));
        assert_eq!(
            new_row.segments,
            vec![
                DiffViewSegment {
                    kind: DiffViewSegmentKind::Normal,
                    text: "let mode = \"".to_string(),
                },
                DiffViewSegment {
                    kind: DiffViewSegmentKind::InsertToken,
                    text: "new".to_string(),
                },
                DiffViewSegment {
                    kind: DiffViewSegmentKind::Normal,
                    text: "\";".to_string(),
                },
            ]
        );
    }

    #[test]
    fn rendered_view_preserves_no_newline_metadata_for_changed_text() {
        let diff = build_display_diff("left", "right", &DiffOptions::default());

        let rendered = build_diff_view(&diff, &DiffOptions::default());

        assert!(rendered.left_no_newline);
        assert!(rendered.right_no_newline);
    }

    #[test]
    fn change_marks_track_visible_change_rows() {
        let diff = build_display_diff(
            "i wanna eatt banana\ni wanna eatt banana",
            "i wanna eat bananas\ni wanna eatt banana\ni，",
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
        assert!(
            view.marks
                .iter()
                .all(|mark| mark.row_index < view.rows.len())
        );
    }

    #[test]
    fn folding_uses_context_limits_and_preserves_line_numbers_after_skips() {
        let options = DiffOptions {
            display_full_context_max_lines: 3,
            unified_context_radius: 1,
            ..DiffOptions::default()
        };
        let diff = build_display_diff("a\nb\nc\nd\ne\nf\ng\n", "a\nb\nX\nd\ne\nf\ng\n", &options);

        let rendered = build_diff_view(&diff, &options);

        let fold_index = rendered
            .rows
            .iter()
            .position(|row| row.kind == DiffViewRowKind::Fold)
            .expect("expected at least one fold row");

        assert_eq!(
            text(&rendered.rows[fold_index]),
            "... 1 unchanged lines ..."
        );

        let row_after_fold = &rendered.rows[fold_index + 1];
        assert_eq!(row_after_fold.kind, DiffViewRowKind::Context);
        assert_eq!(row_after_fold.old_line, Some(2));
        assert_eq!(row_after_fold.new_line, Some(2));
        assert_eq!(text(row_after_fold), "b");
    }

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
        let view = view_of(&[
            Context, Delete, Insert, Context, ReplaceOld, ReplaceNew, Context,
        ]);
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

    #[test]
    fn selection_text_joins_selected_rows_as_plain_text() {
        let diff = build_display_diff("a\nb\nc\n", "a\nx\nc\n", &DiffOptions::default());
        let view = build_diff_view(&diff, &DiffOptions::default());
        // rows: Context "a", Delete "b", Insert "x", Context "c"
        assert_eq!(view.selection_text(0, 0), "a");
        assert_eq!(view.selection_text(1, 2), "b\nx");
        // Order-independent: (2, 1) yields the same inclusive range.
        assert_eq!(view.selection_text(2, 1), "b\nx");
    }

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
}
