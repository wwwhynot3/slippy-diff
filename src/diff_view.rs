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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedDiffView {
    pub rows: Vec<DiffViewRow>,
    pub summary: ChangeSummary,
    pub marks: Vec<ChangeMark>,
    pub left_no_newline: bool,
    pub right_no_newline: bool,
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
}
