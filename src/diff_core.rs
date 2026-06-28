use similar::{ChangeTag, TextDiff};

/// All tunable numbers and ratios for the diff engine. The canonical defaults
/// live in `Default`; config overrides are applied on top of these.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiffOptions {
    pub debounce_ms: u64,
    pub auto_diff_max_bytes: usize,
    pub auto_diff_max_lines: usize,
    pub unified_context_radius: usize,
    pub inline_max_changed_ratio: f32,
    pub display_full_context_max_lines: usize,
    pub similarity_pairing_max_lines: usize,
    pub alignment_band: usize,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            debounce_ms: 300,
            auto_diff_max_bytes: 256 * 1024,
            auto_diff_max_lines: 8_000,
            unified_context_radius: 3,
            inline_max_changed_ratio: 0.50,
            display_full_context_max_lines: 200,
            similarity_pairing_max_lines: 1_000,
            alignment_band: 25,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineDiffSegmentKind {
    Equal,
    Delete,
    Insert,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineDiffSegment {
    pub kind: InlineDiffSegmentKind,
    pub text: String,
}

/// One rendered line of the structured diff. The UI consumes this directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    Context { text: String },
    Delete { text: String },
    Insert { text: String },
    Inline { segments: Vec<InlineDiffSegment> },
}

pub fn should_auto_diff(left: &str, right: &str, options: &DiffOptions) -> bool {
    let total_bytes = left.len() + right.len();
    let total_lines = left.lines().count() + right.lines().count();

    total_bytes <= options.auto_diff_max_bytes && total_lines <= options.auto_diff_max_lines
}

fn ensure_single_trailing_newline(mut value: String) -> String {
    while value.ends_with('\n') {
        value.pop();
    }

    value.push('\n');
    value
}

fn push_inline_segment(
    segments: &mut Vec<InlineDiffSegment>,
    kind: InlineDiffSegmentKind,
    value: &str,
) {
    if value.is_empty() {
        return;
    }

    if let Some(last) = segments.last_mut()
        && last.kind == kind
    {
        last.text.push_str(value);
        return;
    }

    segments.push(InlineDiffSegment {
        kind,
        text: value.to_string(),
    });
}

fn char_level_segments(old: &str, new: &str) -> Vec<InlineDiffSegment> {
    let diff = TextDiff::from_chars(old, new);
    let mut segments = Vec::new();
    for change in diff.iter_all_changes() {
        let value = change.to_string_lossy();
        let kind = match change.tag() {
            ChangeTag::Equal => InlineDiffSegmentKind::Equal,
            ChangeTag::Delete => InlineDiffSegmentKind::Delete,
            ChangeTag::Insert => InlineDiffSegmentKind::Insert,
        };
        push_inline_segment(&mut segments, kind, &value);
    }
    segments
}

fn changed_ratio(old: &str, new: &str) -> f32 {
    if old.is_empty() && new.is_empty() {
        return 0.0;
    }
    let diff = TextDiff::from_chars(old, new);
    let mut changed_old = 0usize;
    let mut changed_new = 0usize;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => changed_old += change.to_string_lossy().chars().count(),
            ChangeTag::Insert => changed_new += change.to_string_lossy().chars().count(),
            ChangeTag::Equal => {}
        }
    }
    let total = old.chars().count().max(new.chars().count()).max(1);
    (changed_old.max(changed_new) as f32) / (total as f32)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayDiff {
    pub ops: Vec<DiffOp>,
    pub left_no_newline: bool,
    pub right_no_newline: bool,
}

impl DisplayDiff {
    /// Sentinel for "no differences" (left == right, including both empty).
    pub fn no_changes(left: &str, right: &str) -> Self {
        Self {
            ops: Vec::new(),
            left_no_newline: !left.is_empty() && !left.ends_with('\n'),
            right_no_newline: !right.is_empty() && !right.ends_with('\n'),
        }
    }
}

pub fn build_display_diff(left: &str, right: &str, options: &DiffOptions) -> DisplayDiff {
    let left_no_newline = !left.is_empty() && !left.ends_with('\n');
    let right_no_newline = !right.is_empty() && !right.ends_with('\n');

    if left == right {
        return DisplayDiff {
            ops: Vec::new(),
            left_no_newline,
            right_no_newline,
        };
    }

    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();

    let ops = if left_lines.len().max(right_lines.len()) > options.similarity_pairing_max_lines {
        exact_ops(left, right)
    } else {
        similarity_ops(&left_lines, &right_lines, options)
    };
    let ops = normalize_op_order(ops);

    DisplayDiff {
        ops,
        left_no_newline,
        right_no_newline,
    }
}

/// Reorder so that within each maximal run of standalone Delete/Insert ops, all
/// Deletes precede all Inserts — the order `diff -u` uses, and what the Copy
/// Diff output (via `similar`) already produces. Context, Inline (already a
/// paired old/new replace), and other ops are left in place. Line counters in
/// `build_diff_view` are independent per side, so this does not change any
/// row's line numbers.
fn normalize_op_order(ops: Vec<DiffOp>) -> Vec<DiffOp> {
    let mut out: Vec<DiffOp> = Vec::with_capacity(ops.len());
    let mut deletes: Vec<DiffOp> = Vec::new();
    let mut inserts: Vec<DiffOp> = Vec::new();
    for op in ops {
        match &op {
            DiffOp::Delete { .. } => deletes.push(op),
            DiffOp::Insert { .. } => inserts.push(op),
            _ => {
                out.append(&mut deletes);
                out.append(&mut inserts);
                out.push(op);
            }
        }
    }
    out.append(&mut deletes);
    out.append(&mut inserts);
    out
}

/// Render a standard unified diff — the text Copy Diff produces. Uses
/// `similar`'s unified-diff printer so hunk headers with correct line
/// numbers, delete-before-insert ordering, the configured context radius,
/// and the `\ No newline at end of file` marker all match `diff -u`.
pub fn render_unified_diff(left: &str, right: &str, options: &DiffOptions) -> String {
    if left == right {
        return "No differences\n".to_string();
    }
    let diff = TextDiff::from_lines(left, right);
    let rendered = diff
        .unified_diff()
        .context_radius(options.unified_context_radius)
        .header("left", "right")
        .to_string();
    if rendered.is_empty() {
        "No differences\n".to_string()
    } else {
        ensure_single_trailing_newline(rendered)
    }
}

fn exact_ops(left: &str, right: &str) -> Vec<DiffOp> {
    let diff = TextDiff::from_lines(left, right);
    let mut ops = Vec::new();
    for change in diff.iter_all_changes() {
        let text = change.to_string_lossy().trim_end_matches('\n').to_string();
        match change.tag() {
            ChangeTag::Equal => ops.push(DiffOp::Context { text }),
            ChangeTag::Delete => ops.push(DiffOp::Delete { text }),
            ChangeTag::Insert => ops.push(DiffOp::Insert { text }),
        }
    }
    ops
}

fn similarity_ops(left: &[&str], right: &[&str], options: &DiffOptions) -> Vec<DiffOp> {
    let n = left.len();
    let m = right.len();
    let w = m + 1;
    let band = options.alignment_band;
    let min_sim = 1.0 - options.inline_max_changed_ratio.clamp(0.0, 1.0) as f64;
    let neg = f64::NEG_INFINITY;

    let mut score = vec![neg; (n + 1) * w];
    let mut from = vec![0u8; (n + 1) * w];
    score[0] = 0.0;

    let in_band = |i: usize, j: usize| (i as isize - j as isize).abs() <= band as isize;

    for i in 0..=n {
        for j in 0..=m {
            if i == 0 && j == 0 {
                continue;
            }
            let cur = i * w + j;
            let mut best = neg;
            let mut best_from = 1u8;

            if i > 0 && j > 0 && in_band(i, j) {
                let li = left[i - 1];
                let rj = right[j - 1];
                let sim = if li == rj {
                    1.0
                } else {
                    1.0 - changed_ratio(li, rj) as f64
                };
                if sim >= min_sim {
                    let s = score[(i - 1) * w + (j - 1)] + sim;
                    if s > best {
                        best = s;
                        best_from = 0;
                    }
                }
            }
            if i > 0 {
                let s = score[(i - 1) * w + j];
                if s > best {
                    best = s;
                    best_from = 1;
                }
            }
            if j > 0 {
                let s = score[i * w + (j - 1)];
                if s > best {
                    best = s;
                    best_from = 2;
                }
            }
            score[cur] = best;
            from[cur] = best_from;
        }
    }

    let mut ops_rev: Vec<DiffOp> = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        match from[i * w + j] {
            0 if i > 0 && j > 0 => {
                let li = left[i - 1];
                let rj = right[j - 1];
                if li == rj {
                    ops_rev.push(DiffOp::Context {
                        text: li.to_string(),
                    });
                } else {
                    ops_rev.push(DiffOp::Inline {
                        segments: char_level_segments(li, rj),
                    });
                }
                i -= 1;
                j -= 1;
            }
            1 if i > 0 => {
                ops_rev.push(DiffOp::Delete {
                    text: left[i - 1].to_string(),
                });
                i -= 1;
            }
            _ => {
                if j > 0 {
                    ops_rev.push(DiffOp::Insert {
                        text: right[j - 1].to_string(),
                    });
                    j -= 1;
                } else {
                    ops_rev.push(DiffOp::Delete {
                        text: left[i - 1].to_string(),
                    });
                    i -= 1;
                }
            }
        }
    }
    ops_rev.reverse();
    ops_rev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_diff_allows_small_inputs() {
        assert!(should_auto_diff(
            "left\n",
            "right\n",
            &DiffOptions::default()
        ));
    }

    #[test]
    fn auto_diff_rejects_large_byte_inputs() {
        let left = "x".repeat(DiffOptions::default().auto_diff_max_bytes + 1);

        assert!(!should_auto_diff(&left, "", &DiffOptions::default()));
    }

    #[test]
    fn auto_diff_rejects_large_line_inputs() {
        let left = "x\n".repeat(DiffOptions::default().auto_diff_max_lines + 1);

        assert!(!should_auto_diff(&left, "", &DiffOptions::default()));
    }

    #[test]
    fn diff_options_default_matches_contract() {
        let o = DiffOptions::default();
        assert_eq!(o.debounce_ms, 300);
        assert_eq!(o.auto_diff_max_bytes, 256 * 1024);
        assert_eq!(o.auto_diff_max_lines, 8_000);
        assert_eq!(o.unified_context_radius, 3);
        assert!((o.inline_max_changed_ratio - 0.50).abs() < f32::EPSILON);
        assert_eq!(o.display_full_context_max_lines, 200);
        assert_eq!(o.similarity_pairing_max_lines, 1_000);
        assert_eq!(o.alignment_band, 25);
    }

    #[test]
    fn char_level_segments_isolate_changed_characters() {
        let segs = char_level_segments("i wanna eatt banana", "i wanna eat bananas");
        assert_eq!(
            segs,
            vec![
                InlineDiffSegment {
                    kind: InlineDiffSegmentKind::Equal,
                    text: "i wanna eat".to_string()
                },
                InlineDiffSegment {
                    kind: InlineDiffSegmentKind::Delete,
                    text: "t".to_string()
                },
                InlineDiffSegment {
                    kind: InlineDiffSegmentKind::Equal,
                    text: " banana".to_string()
                },
                InlineDiffSegment {
                    kind: InlineDiffSegmentKind::Insert,
                    text: "s".to_string()
                },
            ]
        );
    }

    #[test]
    fn changed_ratio_measures_changed_fraction() {
        assert!(changed_ratio("i wanna eatt banana", "i wanna eat bananas") < 0.2);
        assert!((changed_ratio("abcdef", "uvwxyz") - 1.0).abs() < f32::EPSILON);
        assert!((changed_ratio("same", "same") - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn build_display_diff_returns_empty_for_equal_text() {
        let d = build_display_diff("same\n", "same\n", &DiffOptions::default());
        assert!(d.ops.is_empty());
    }

    #[test]
    fn build_display_diff_pairs_similar_lines_over_duplicates() {
        let left = "i wanna eatt banana\ni wanna eatt banana";
        let right = "i wanna eat bananas\ni wanna eatt banana\ni，";
        let d = build_display_diff(left, right, &DiffOptions::default());

        assert_eq!(d.ops.len(), 3);
        assert!(
            matches!(d.ops[0], DiffOp::Inline { .. }),
            "first op should be an inline pair"
        );
        assert_eq!(
            d.ops[1],
            DiffOp::Context {
                text: "i wanna eatt banana".to_string()
            }
        );
        assert_eq!(
            d.ops[2],
            DiffOp::Insert {
                text: "i，".to_string()
            }
        );
        if let DiffOp::Inline { segments } = &d.ops[0] {
            assert_eq!(
                *segments,
                char_level_segments("i wanna eatt banana", "i wanna eat bananas")
            );
        } else {
            panic!("expected inline op");
        }
    }

    #[test]
    fn build_display_diff_emits_pure_insert_and_delete() {
        let ins = build_display_diff("a\n", "a\nb\n", &DiffOptions::default());
        assert!(
            ins.ops
                .iter()
                .any(|o| matches!(o, DiffOp::Insert { text } if text == "b"))
        );

        let del = build_display_diff("a\nb\n", "a\n", &DiffOptions::default());
        assert!(
            del.ops
                .iter()
                .any(|o| matches!(o, DiffOp::Delete { text } if text == "b"))
        );
    }

    #[test]
    fn build_display_diff_falls_back_to_exact_when_over_cap() {
        let o = DiffOptions {
            similarity_pairing_max_lines: 0, // force fallback
            ..DiffOptions::default()
        };
        let d = build_display_diff("a\nb\nc", "a\nx\nc", &o);
        assert!(
            d.ops.iter().all(|op| !matches!(op, DiffOp::Inline { .. })),
            "fallback must not inline-pair"
        );
    }

    #[test]
    fn render_unified_diff_emits_no_differences_for_equal_text() {
        let text = render_unified_diff("same\n", "same\n", &DiffOptions::default());
        assert_eq!(text, "No differences\n");
    }

    #[test]
    fn render_unified_diff_emits_standard_unified_diff() {
        // Trailing newlines on both sides -> no "\ No newline" marker, fully
        // predictable output that matches `diff -u`.
        let text = render_unified_diff("a\nb\n", "a\nc\n", &DiffOptions::default());
        assert_eq!(text, "--- left\n+++ right\n@@ -1,2 +1,2 @@\n a\n-b\n+c\n");
    }

    #[test]
    fn render_unified_diff_puts_deletes_before_inserts() {
        // Single-line replace: the deletion (-) must precede the insertion (+),
        // which the previous hand-rolled renderer got backwards.
        let text = render_unified_diff("1\n", "2\n", &DiffOptions::default());
        let minus = text.find("-1\n").expect("deletion line present");
        let plus = text.find("+2\n").expect("insertion line present");
        assert!(minus < plus, "deletion must precede insertion");
    }

    #[test]
    fn render_unified_diff_marks_missing_trailing_newline() {
        // No trailing newline -> the standard "\ No newline at end of file" marker.
        let text = render_unified_diff("a\nb", "a\nc", &DiffOptions::default());
        assert!(text.contains("\\ No newline at end of file"));
    }

    #[test]
    fn mixed_delete_insert_ops_emit_deletes_before_inserts() {
        let d = build_display_diff("a\nb\nc\n", "a\nx\nc\n", &DiffOptions::default());
        // b is deleted, x is inserted -> the first change op must be a Delete.
        let first_change_kind = d.ops.iter().find_map(|op| match op {
            DiffOp::Delete { .. } => Some('D'),
            DiffOp::Insert { .. } => Some('I'),
            _ => None,
        });
        assert_eq!(first_change_kind, Some('D'));
    }

    #[test]
    fn fullwidth_comma_is_preserved_in_insert_op() {
        let d = build_display_diff("a\n", "a\ni，\n", &DiffOptions::default());
        let inserted = d.ops.iter().find_map(|op| match op {
            DiffOp::Insert { text } if text.contains('i') => Some(text.clone()),
            _ => None,
        });
        assert_eq!(inserted.as_deref(), Some("i，"));
    }
}
