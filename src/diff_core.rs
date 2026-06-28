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

const BOTH_SIDES_NO_NEWLINE_MARKER: &str = "! Left and right text end without a trailing newline";
const LEFT_NO_NEWLINE_MARKER: &str = "! Left text ends without a trailing newline";
const RIGHT_NO_NEWLINE_MARKER: &str = "! Right text ends without a trailing newline";

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

    DisplayDiff {
        ops,
        left_no_newline,
        right_no_newline,
    }
}

pub fn render_unified_diff(diff: &DisplayDiff) -> String {
    if diff.ops.is_empty() {
        return "No differences\n".to_string();
    }

    let mut out = String::new();
    out.push_str("--- left\n+++ right\n");

    let left_count = diff
        .ops
        .iter()
        .filter(|op| !matches!(op, DiffOp::Insert { .. }))
        .count()
        .max(1);
    let right_count = diff
        .ops
        .iter()
        .filter(|op| !matches!(op, DiffOp::Delete { .. }))
        .count()
        .max(1);
    out.push_str(&format!("@@ -1,{left_count} +1,{right_count} @@\n"));

    for op in &diff.ops {
        match op {
            DiffOp::Context { text } => {
                out.push(' ');
                out.push_str(text);
                out.push('\n');
            }
            DiffOp::Delete { text } => {
                out.push('-');
                out.push_str(text);
                out.push('\n');
            }
            DiffOp::Insert { text } => {
                out.push('+');
                out.push_str(text);
                out.push('\n');
            }
            DiffOp::Inline { segments } => {
                let (mut old, mut new) = (String::new(), String::new());
                for s in segments {
                    match s.kind {
                        InlineDiffSegmentKind::Equal => {
                            old.push_str(&s.text);
                            new.push_str(&s.text);
                        }
                        InlineDiffSegmentKind::Delete => old.push_str(&s.text),
                        InlineDiffSegmentKind::Insert => new.push_str(&s.text),
                    }
                }
                out.push('-');
                out.push_str(&old);
                out.push('\n');
                out.push('+');
                out.push_str(&new);
                out.push('\n');
            }
        }
    }

    if let Some(notice) = no_newline_notice(diff.left_no_newline, diff.right_no_newline) {
        out.push_str(notice);
        out.push('\n');
    }

    ensure_single_trailing_newline(out)
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

fn no_newline_notice(
    left_missing_newline: bool,
    right_missing_newline: bool,
) -> Option<&'static str> {
    match (left_missing_newline, right_missing_newline) {
        (true, true) => Some(BOTH_SIDES_NO_NEWLINE_MARKER),
        (true, false) => Some(LEFT_NO_NEWLINE_MARKER),
        (false, true) => Some(RIGHT_NO_NEWLINE_MARKER),
        (false, false) => None,
    }
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
    fn render_unified_diff_emits_no_differences_for_empty_ops() {
        let d = DisplayDiff::no_changes("same\n", "same\n");
        assert_eq!(render_unified_diff(&d), "No differences\n");
    }

    #[test]
    fn render_unified_diff_formats_ops_as_standard_text() {
        let d = build_display_diff("a\nb", "a\nc", &DiffOptions::default());
        let text = render_unified_diff(&d);
        assert!(text.starts_with("--- left\n+++ right\n"));
        assert!(text.contains("@@ -1,2 +1,2 @@\n"));
        assert!(text.contains(" a\n"));
        assert!(text.contains("-b\n"));
        assert!(text.contains("+c\n"));
        assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
    }

    #[test]
    fn render_unified_diff_expands_inline_pair_to_minus_plus() {
        let d = build_display_diff(
            "i wanna eatt banana",
            "i wanna eat bananas",
            &DiffOptions::default(),
        );
        let text = render_unified_diff(&d);
        assert!(text.contains("-i wanna eatt banana\n"));
        assert!(text.contains("+i wanna eat bananas\n"));
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
