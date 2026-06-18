# Slippy Similarity-First Diff Display — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Myers-line-diff + adjacent-pair inline highlighting with a similarity-first, IntelliJ-style colored-background diff, where every ratio and threshold is config-driven with defaults.

**Architecture:** `diff_core` produces a structured `Vec<DiffOp>` via a similarity-weighted line alignment (fuzzy LCS), parameterized by a `DiffOptions` struct whose defaults are the single source of truth. `config` stores optional per-field overrides (sanitized on load); `ui_fltk` bridges config→`DiffOptions`, renders `DiffOp`s with FLTK `StyleTableEntryExt` background colors, and renders Copy text as standard unified diff from the same ops. `app_state` stores a `DisplayDiff` instead of a string and threads `DiffOptions` through.

**Tech Stack:** Rust 2024, `similar` 2 (char + line diff), `fltk` 1.5 (`StyleTableEntryExt` / `set_highlight_data_ext`), `serde`/`serde_json`, `arboard`, `directories`.

**Spec:** `docs/superpowers/specs/2026-06-18-diff-display-design.md`

## Global Constraints

- `diff_core` must NOT depend on `fltk`, `arboard`, `config`, or threading. It takes `&DiffOptions` as a parameter.
- `app_state` must NOT depend on `fltk` or `arboard`.
- `config` must NOT depend on `diff_core` (stores raw `Option<T>` overrides only). `ui_fltk` is the bridge that maps overrides → `DiffOptions`.
- Config stores only metadata and tunables — NEVER pasted text or diff output.
- Every tunable number/ratio has a default in `DiffOptions::default()` (single source of truth). Missing or out-of-range config values fall back to the default.
- Preserved contracts: equal text ⇒ `No differences\n`; output ends with exactly one trailing newline; stale-worker guard unchanged.
- Default `similarity_pairing_max_lines = 1000` (note: spec draft said 4000; lowered to 1000 because the alignment matrix is O(n·m) ≈ `(value)²·9 bytes`; 1000 ≈ 9 MB worst case). It remains user-configurable.
- Validate with `cargo fmt && cargo test` after every task. Single command to build: `cargo build`.

---

## File Structure

- `src/diff_core.rs` — ADD `DiffOptions`, `DiffOp`, `DisplayDiff`, `build_display_diff`, `render_unified_diff`, char helpers; REMOVE `build_unified_diff`, `classify_diff_line`, `make_no_newline_markers_friendly`, `inline_diff_match`, `inline_changed_byte_ranges`, `analyze_inline_diff`, old const-based `should_auto_diff`.
- `src/config.rs` — ADD `DiffOverrides` (all `Option<T>`), `diff` field on `AppConfig`, `sanitized()` validation.
- `src/app_state.rs` — `DiffResult.diff` becomes `DisplayDiff`; `DiffRequest` carries `DiffOptions`; `AppState` holds `options` + `has_result`; `should_auto_diff` takes `&DiffOptions`.
- `src/ui_fltk.rs` — ADD `diff_options_from_config`, op-driven bg renderer (`StyleTableEntryExt`), adaptive folding; REMOVE `render_diff_display`, `classify_diff_line` usage, `best_inline_pairs`, prefix parsing.
- `DESIGN.md`, `IMPLEMENTATION_PLAN.md`, `CLAUDE.md` — updated in the final task.

---

## Task 1: Add `DiffOptions` to `diff_core`

**Files:**
- Modify: `src/diff_core.rs` (add struct near top, after the existing `pub const` lines)
- Test: `src/diff_core.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `pub struct DiffOptions { debounce_ms, auto_diff_max_bytes, auto_diff_max_lines, unified_context_radius, inline_max_changed_ratio, display_full_context_max_lines, similarity_pairing_max_lines, alignment_band }` with `impl Default`.

- [ ] **Step 1: Write the failing test**

Add to the test module in `src/diff_core.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test diff_core::tests::diff_options_default_matches_contract`
Expected: FAIL — `DiffOptions` not defined.

- [ ] **Step 3: Add the struct**

Insert after the existing `pub const UNIFIED_CONTEXT_RADIUS: usize = 3;` line in `src/diff_core.rs`:

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test diff_core::tests::diff_options_default_matches_contract`
Expected: PASS.

- [ ] **Step 5: Run full suite + format + commit**

Run: `cargo fmt && cargo test`
Expected: all existing tests still PASS (nothing else changed).
```bash
git add src/diff_core.rs
git commit -m "feat(diff_core): add DiffOptions tunables struct with defaults"
```

---

## Task 2: Add `DiffOp` and extract char-level helpers

**Files:**
- Modify: `src/diff_core.rs`
- Test: `src/diff_core.rs`

**Interfaces:**
- Produces: `pub enum DiffOp`, private `char_level_segments(old, new) -> Vec<InlineDiffSegment>`, private `changed_ratio(old, new) -> f32`.
- Refactors `analyze_inline_diff` to use them (existing `inline_diff_match` tests must still pass).

- [ ] **Step 1: Write failing tests**

Add to the test module:

```rust
#[test]
fn char_level_segments_isolate_changed_characters() {
    let segs = char_level_segments("i wanna eatt banana", "i wanna eat bananas");
    assert_eq!(
        segs,
        vec![
            InlineDiffSegment { kind: InlineDiffSegmentKind::Equal, text: "i wanna eat".to_string() },
            InlineDiffSegment { kind: InlineDiffSegmentKind::Delete, text: "t".to_string() },
            InlineDiffSegment { kind: InlineDiffSegmentKind::Equal, text: " banana".to_string() },
            InlineDiffSegment { kind: InlineDiffSegmentKind::Insert, text: "s".to_string() },
        ]
    );
}

#[test]
fn changed_ratio_measures_changed_fraction() {
    assert!(changed_ratio("i wanna eatt banana", "i wanna eat bananas") < 0.2);
    assert!((changed_ratio("abcdef", "uvwxyz") - 1.0).abs() < f32::EPSILON);
    assert!((changed_ratio("same", "same") - 0.0).abs() < f32::EPSILON);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test char_level_segments_isolate_changed_characters changed_ratio_measures_changed_fraction`
Expected: FAIL — functions not defined.

- [ ] **Step 3: Add `DiffOp` enum and the two helpers**

Add near the other public types (after `InlineDiffMatch`):

```rust
/// One rendered line of the structured diff. The UI consumes this directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    Context { text: String },
    Delete { text: String },
    Insert { text: String },
    Inline { segments: Vec<InlineDiffSegment> },
}
```

Add the two private helpers (anywhere among the private fns):

```rust
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
```

- [ ] **Step 4: Refactor `analyze_inline_diff` to reuse the helpers**

In `analyze_inline_diff`, replace the inline segment-building loop body so Equal/Delete/Insert arms call `push_inline_segment(&mut segments, …, &value)` exactly as the existing code does, but compute `delete_ranges`/`insert_ranges` as before. (The function already builds segments from the char diff after the earlier fix; this step only ensures both helpers share the same logic.) Concretely, replace the line `let segments = char_level_segments(old_text, new_text);` is NOT used yet — instead keep the existing in-loop `push_inline_segment` calls. No behavioral change. Verify `inline_diff_match` tests still pass.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test`
Expected: PASS (new tests + all existing `inline_diff_match` tests).

- [ ] **Step 6: Format + commit**

```bash
cargo fmt
git add src/diff_core.rs
git commit -m "feat(diff_core): add DiffOp enum and char-level segment/ratio helpers"
```

---

## Task 3: Similarity-weighted alignment (`build_display_diff`)

**Files:**
- Modify: `src/diff_core.rs`
- Test: `src/diff_core.rs`

**Interfaces:**
- Consumes: `DiffOptions`, `DiffOp`, `char_level_segments`, `changed_ratio` (Task 1, 2).
- Produces: `pub struct DisplayDiff { ops: Vec<DiffOp>, left_no_newline: bool, right_no_newline: bool }`, `DisplayDiff::no_changes(left, right)`, `pub fn build_display_diff(left: &str, right: &str, options: &DiffOptions) -> DisplayDiff`.

- [ ] **Step 1: Write failing tests**

```rust
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
    assert!(matches!(d.ops[0], DiffOp::Inline { .. }), "first op should be an inline pair");
    assert_eq!(d.ops[1], DiffOp::Context { text: "i wanna eatt banana".to_string() });
    assert_eq!(d.ops[2], DiffOp::Insert { text: "i，".to_string() });
    if let DiffOp::Inline { segments } = &d.ops[0] {
        assert_eq!(*segments, char_level_segments("i wanna eatt banana", "i wanna eat bananas"));
    } else {
        panic!("expected inline op");
    }
}

#[test]
fn build_display_diff_emits_pure_insert_and_delete() {
    let ins = build_display_diff("a\n", "a\nb\n", &DiffOptions::default());
    assert!(ins.ops.iter().any(|o| matches!(o, DiffOp::Insert { text } if text == "b")));

    let del = build_display_diff("a\nb\n", "a\n", &DiffOptions::default());
    assert!(del.ops.iter().any(|o| matches!(o, DiffOp::Delete { text } if text == "b")));
}

#[test]
fn build_display_diff_falls_back_to_exact_when_over_cap() {
    let mut o = DiffOptions::default();
    o.similarity_pairing_max_lines = 0; // force fallback
    let d = build_display_diff("a\nb\nc", "a\nx\nc", &o);
    assert!(d.ops.iter().all(|op| !matches!(op, DiffOp::Inline { .. })), "fallback must not inline-pair");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test build_display_diff`
Expected: FAIL — `DisplayDiff`/`build_display_diff` not defined.

- [ ] **Step 3: Add `DisplayDiff` and `build_display_diff`**

```rust
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
        return DisplayDiff { ops: Vec::new(), left_no_newline, right_no_newline };
    }

    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();

    let ops = if left_lines
        .len()
        .max(right_lines.len())
        > options.similarity_pairing_max_lines
    {
        exact_ops(left, right)
    } else {
        similarity_ops(&left_lines, &right_lines, options)
    };

    DisplayDiff { ops, left_no_newline, right_no_newline }
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
                    ops_rev.push(DiffOp::Context { text: li.to_string() });
                } else {
                    ops_rev.push(DiffOp::Inline {
                        segments: char_level_segments(li, rj),
                    });
                }
                i -= 1;
                j -= 1;
            }
            1 if i > 0 => {
                ops_rev.push(DiffOp::Delete { text: left[i - 1].to_string() });
                i -= 1;
            }
            _ => {
                if j > 0 {
                    ops_rev.push(DiffOp::Insert { text: right[j - 1].to_string() });
                    j -= 1;
                } else {
                    ops_rev.push(DiffOp::Delete { text: left[i - 1].to_string() });
                    i -= 1;
                }
            }
        }
    }
    ops_rev.reverse();
    ops_rev
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test build_display_diff`
Expected: PASS (all four tests).

- [ ] **Step 5: Run full suite + format + commit**

Run: `cargo fmt && cargo test`
```bash
git add src/diff_core.rs
git commit -m "feat(diff_core): similarity-weighted line alignment (build_display_diff)"
```

---

## Task 4: Unified-diff renderer for Copy (`render_unified_diff`)

**Files:**
- Modify: `src/diff_core.rs`
- Test: `src/diff_core.rs`

**Interfaces:**
- Consumes: `DisplayDiff`, `DiffOp`, `InlineDiffSegmentKind`, existing `no_newline_notice`, `ensure_single_trailing_newline`.
- Produces: `pub fn render_unified_diff(diff: &DisplayDiff) -> String`.

- [ ] **Step 1: Write failing tests**

```rust
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
    let d = build_display_diff("i wanna eatt banana", "i wanna eat bananas", &DiffOptions::default());
    let text = render_unified_diff(&d);
    assert!(text.contains("-i wanna eatt banana\n"));
    assert!(text.contains("+i wanna eat bananas\n"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test render_unified_diff`
Expected: FAIL — not defined.

- [ ] **Step 3: Implement the renderer**

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test render_unified_diff`
Expected: PASS.

- [ ] **Step 5: Full suite + format + commit**

Run: `cargo fmt && cargo test`
```bash
git add src/diff_core.rs
git commit -m "feat(diff_core): render_unified_diff over DisplayDiff for copy"
```

---

## Task 5: Config diff overrides

**Files:**
- Modify: `src/config.rs`
- Test: `src/config.rs`

**Interfaces:**
- Produces: `pub struct DiffOverrides` (all `Option<T>`, `#[derive(Default)]`), field `pub diff: DiffOverrides` on `AppConfig`, `DiffOverrides::sanitized(self) -> Self`. `AppConfig::normalized` sanitizes `diff`.

- [ ] **Step 1: Write failing tests**

Add to `src/config.rs` test module:

```rust
#[test]
fn missing_diff_overrides_defaults_to_all_none() {
    let config = AppConfig::default();
    assert_eq!(config.diff, DiffOverrides::default());
}

#[test]
fn diff_overrides_round_trip_through_save_load() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("config.json");
    let config = AppConfig {
        diff: DiffOverrides {
            debounce_ms: Some(500),
            inline_max_changed_ratio: Some(0.25),
            alignment_band: Some(40),
            ..DiffOverrides::default()
        },
        ..AppConfig::default()
    };
    save_config_to_path(&path, &config).expect("save");
    let loaded = load_config_from_path(path).config;
    assert_eq!(loaded.diff.debounce_ms, Some(500));
    assert_eq!(loaded.diff.inline_max_changed_ratio, Some(0.25));
    assert_eq!(loaded.diff.alignment_band, Some(40));
}

#[test]
fn out_of_range_overrides_are_dropped_on_load() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("config.json");
    fs::write(
        &path,
        r#"{
            "version": 1, "width": 1120, "height": 760, "vertical_split": 0.45,
            "theme": "System", "ui_font": "", "mono_font": "",
            "diff": { "inline_max_changed_ratio": 1.5, "alignment_band": 0 }
        }"#,
    )
    .expect("write");
    let loaded = load_config_from_path(path).config;
    assert_eq!(loaded.diff.inline_max_changed_ratio, None);
    assert_eq!(loaded.diff.alignment_band, None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test config::tests`
Expected: FAIL — `DiffOverrides` not defined.

- [ ] **Step 3: Add `DiffOverrides`, the field, and sanitization**

Add after the `Theme` enum in `src/config.rs`:

```rust
/// Optional per-field overrides for the diff engine tunables. Every field
/// defaults to `None`, meaning "use the `DiffOptions` default". `ui_fltk`
/// applies these on top of `DiffOptions::default()`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DiffOverrides {
    pub debounce_ms: Option<u64>,
    pub auto_diff_max_bytes: Option<usize>,
    pub auto_diff_max_lines: Option<usize>,
    pub unified_context_radius: Option<usize>,
    pub inline_max_changed_ratio: Option<f32>,
    pub display_full_context_max_lines: Option<usize>,
    pub similarity_pairing_max_lines: Option<usize>,
    pub alignment_band: Option<usize>,
}

impl DiffOverrides {
    /// Drop values that are out of range so the bridge falls back to defaults.
    pub fn sanitized(self) -> Self {
        let keep_ge = |v: Option<usize>, min: usize| v.filter(|x| *x >= min);
        Self {
            debounce_ms: self.debounce_ms,
            auto_diff_max_bytes: keep_ge(self.auto_diff_max_bytes, 0),
            auto_diff_max_lines: keep_ge(self.auto_diff_max_lines, 0),
            unified_context_radius: keep_ge(self.unified_context_radius, 0),
            inline_max_changed_ratio: self
                .inline_max_changed_ratio
                .filter(|x| (0.0..=1.0).contains(x)),
            display_full_context_max_lines: keep_ge(self.display_full_context_max_lines, 0),
            similarity_pairing_max_lines: keep_ge(self.similarity_pairing_max_lines, 0),
            alignment_band: keep_ge(self.alignment_band, 1),
        }
    }
}
```

Add the field to `AppConfig` (after `mono_font`):

```rust
    pub mono_font: String,
    #[serde(default)]
    pub diff: DiffOverrides,
```

In `impl Default for AppConfig`, add `diff: DiffOverrides::default(),`.

In `AppConfig::normalized`, append before `self`:

```rust
        self.diff = self.diff.sanitized();
        self
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test config::tests`
Expected: PASS.

- [ ] **Step 5: Full suite + format + commit**

Run: `cargo fmt && cargo test`
```bash
git add src/config.rs
git commit -m "feat(config): add sanitized DiffOverrides block to AppConfig"
```

---

## Task 6: Wire `app_state` + `ui_fltk` to the new diff API

**Goal of this task:** switch the data flow end-to-end so the app builds and uses `DisplayDiff`, while the **display** still renders via the existing prefix parser fed by `render_unified_diff` (the bg-color rewrite is Task 7). Build must be green and all tests pass.

**Files:**
- Modify: `src/app_state.rs`, `src/ui_fltk.rs`
- Test: `src/app_state.rs`, `src/ui_fltk.rs`

**Interfaces:**
- `DiffResult { id: u64, diff: DisplayDiff }` (was `diff: String`).
- `DiffRequest { id, left, right, options: DiffOptions }`.
- `AppState::new(options: DiffOptions)`, `AppState::options() -> &DiffOptions`, `AppState::should_auto_diff() -> bool`, `AppState::diff() -> &DisplayDiff`.
- `diff_core::should_auto_diff(left, right, &DiffOptions) -> bool` (replaces the const version).

- [ ] **Step 1: Update `should_auto_diff` signature in `diff_core`**

Replace the existing `should_auto_diff` in `src/diff_core.rs`:

```rust
pub fn should_auto_diff(left: &str, right: &str, options: &DiffOptions) -> bool {
    let total_bytes = left.len() + right.len();
    let total_lines = left.lines().count() + right.lines().count();
    total_bytes <= options.auto_diff_max_bytes && total_lines <= options.auto_diff_max_lines
}
```

Remove the now-unused `pub const DEBOUNCE_MS`, `AUTO_DIFF_MAX_BYTES`, `AUTO_DIFF_MAX_LINES`, `UNIFIED_CONTEXT_RADIUS`, and `INLINE_DIFF_MAX_CHANGED_RATIO` constants (their values live in `DiffOptions::default()`). Update any `diff_core` test that referenced them to use `DiffOptions::default()` fields instead (e.g. `DiffOptions::default().auto_diff_max_bytes`).

- [ ] **Step 2: Update `app_state.rs`**

Replace the import line with:

```rust
use crate::diff_core::{build_display_diff, should_auto_diff, DiffOptions, DisplayDiff};
```

Replace `DiffResult` and `DiffRequest`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRequest {
    pub id: u64,
    pub left: String,
    pub right: String,
    pub options: DiffOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffResult {
    pub id: u64,
    pub diff: DisplayDiff,
}
```

Change `AppState` fields: replace `diff: String` with `diff: DisplayDiff` and add `options: DiffOptions`, `has_result: bool`:

```rust
#[derive(Debug, Clone)]
pub struct AppState {
    left: String,
    right: String,
    diff: DisplayDiff,
    has_result: bool,
    options: DiffOptions,
    status: String,
    latest_request_id: u64,
    dirty_since_latest_request: bool,
    dirty: bool,
}
```

`impl Default`:

```rust
impl Default for AppState {
    fn default() -> Self {
        Self::new(DiffOptions::default())
    }
}

impl AppState {
    pub fn new(options: DiffOptions) -> Self {
        Self {
            left: String::new(),
            right: String::new(),
            diff: DisplayDiff::no_changes("", ""),
            has_result: false,
            options,
            status: STARTUP_STATUS.to_string(),
            latest_request_id: 0,
            dirty_since_latest_request: false,
            dirty: false,
        }
    }
```

Update the accessors and methods inside `impl AppState`:

```rust
    pub fn diff(&self) -> &DisplayDiff {
        &self.diff
    }

    pub fn options(&self) -> &DiffOptions {
        &self.options
    }

    pub fn should_auto_diff(&self) -> bool {
        should_auto_diff(&self.left, &self.right, &self.options)
    }

    pub fn has_current_diff(&self) -> bool {
        self.has_result && !self.dirty
    }

    pub fn has_stale_diff(&self) -> bool {
        self.has_result && self.dirty
    }
```

In `set_left`, `set_right`, and `mark_dirty_after_edit`, replace `should_auto_diff(&self.left, &self.right)` with `self.should_auto_diff()`. In `create_auto_request`, likewise. In `clear`:

```rust
    pub fn clear(&mut self) {
        self.left.clear();
        self.right.clear();
        self.diff = DisplayDiff::no_changes("", "");
        self.has_result = false;
        self.latest_request_id = self.latest_request_id.saturating_add(1);
        self.dirty_since_latest_request = false;
        self.dirty = false;
        self.status = STATUS_CLEARED.to_string();
    }
```

In `create_request`, snapshot options into the request:

```rust
    fn create_request(&mut self) -> DiffRequest {
        self.latest_request_id = self.latest_request_id.saturating_add(1);
        self.dirty_since_latest_request = false;
        self.status = STATUS_DIFF_RUNNING.to_string();
        DiffRequest {
            id: self.latest_request_id,
            left: self.left.clone(),
            right: self.right.clone(),
            options: self.options,
        }
    }
```

In `apply_result`:

```rust
    pub fn apply_result(&mut self, result: DiffResult) -> ApplyOutcome {
        if result.id != self.latest_request_id {
            return ApplyOutcome::IgnoredStaleRequest;
        }
        if self.dirty_since_latest_request {
            return ApplyOutcome::IgnoredBecauseDirty;
        }
        self.diff = result.diff;
        self.has_result = true;
        self.dirty = false;
        self.status = if self.diff.ops.is_empty() {
            STATUS_NO_DIFFERENCES.to_string()
        } else {
            STATUS_DIFF_UPDATED.to_string()
        };
        ApplyOutcome::Applied
    }
```

`DiffRequest::compute`:

```rust
impl DiffRequest {
    pub fn compute(self) -> DiffResult {
        DiffResult {
            id: self.id,
            diff: build_display_diff(&self.left, &self.right, &self.options),
        }
    }
}
```

- [ ] **Step 3: Rewrite `app_state` tests**

The old tests assert on `state.diff()` as a `String`. Replace those assertions. Concretely, change:
- `state.diff()` String checks → use `crate::diff_core::render_unified_diff(state.diff())` for substring checks, and `state.diff().ops.is_empty()` for the no-differences / cleared cases.
- `AUTO_DIFF_MAX_BYTES + 1` → `crate::diff_core::DiffOptions::default().auto_diff_max_bytes + 1`.

For example, `applying_latest_clean_result_updates_diff_and_status`:

```rust
    assert!(render_unified_diff(state.diff()).contains("-left"));
    assert!(render_unified_diff(state.diff()).contains("+right"));
    assert_eq!(state.status(), STATUS_DIFF_UPDATED);
    assert!(state.has_current_diff());
```

`equal_result_uses_no_differences_status`:

```rust
    assert!(state.diff().ops.is_empty());
    assert_eq!(state.status(), STATUS_NO_DIFFERENCES);
```

`clear_invalidates_in_flight_results`:

```rust
    assert_eq!(state.diff().ops.len(), 0);
```

Add `use crate::diff_core::render_unified_diff;` to the test module imports.

- [ ] **Step 4: Update `ui_fltk.rs` wiring**

Add imports (and drop the now-unused `DEBOUNCE_MS`/`should_auto_diff` direct imports — keep `classify_diff_line`, `DiffLineKind`, `inline_diff_match` because the prefix render path still uses them this task):

```rust
use crate::diff_core::{render_unified_diff, DiffOptions};
```

Add the bridge function near the top of `ui_fltk.rs` (after the constants):

```rust
fn diff_options_from_config(overrides: &crate::config::DiffOverrides) -> DiffOptions {
    let mut o = DiffOptions::default();
    if let Some(v) = overrides.debounce_ms { o.debounce_ms = v; }
    if let Some(v) = overrides.auto_diff_max_bytes { o.auto_diff_max_bytes = v; }
    if let Some(v) = overrides.auto_diff_max_lines { o.auto_diff_max_lines = v; }
    if let Some(v) = overrides.unified_context_radius { o.unified_context_radius = v; }
    if let Some(v) = overrides.inline_max_changed_ratio { o.inline_max_changed_ratio = v; }
    if let Some(v) = overrides.display_full_context_max_lines { o.display_full_context_max_lines = v; }
    if let Some(v) = overrides.similarity_pairing_max_lines { o.similarity_pairing_max_lines = v; }
    if let Some(v) = overrides.alignment_band { o.alignment_band = v; }
    o
}
```

In `run()`, replace the state construction:

```rust
    let options = diff_options_from_config(&config.config.diff);
    let state = Rc::new(RefCell::new(AppState::new(options)));
```

In `schedule_auto_compare`, replace the guard and the timeout delay:

```rust
fn schedule_auto_compare(
    state: &Rc<RefCell<AppState>>,
    handles: &Rc<RefCell<UiHandles>>,
    sender: app::Sender<UiMessage>,
    debounce_generation: &Rc<Cell<u64>>,
) {
    let (should, debounce_ms) = {
        let s = state.borrow();
        (s.should_auto_diff(), s.options().debounce_ms)
    };
    if !should {
        render_state(state, handles);
        return;
    }

    let generation = debounce_generation.get().saturating_add(1);
    debounce_generation.set(generation);
    let state = state.clone();
    let handles = handles.clone();
    let debounce_generation = debounce_generation.clone();
    app::add_timeout3(debounce_ms as f64 / 1000.0, move |_| {
        if debounce_generation.get() == generation {
            sync_state_from_buffers(&state, &handles);
            let Some(request) = state.borrow_mut().create_auto_request() else {
                render_state(&state, &handles);
                return;
            };
            render_state(&state, &handles);
            spawn_diff_worker(request, sender);
        }
    });
}
```

In `render_state`, feed the renderer from `render_unified_diff`:

```rust
fn render_state(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) {
    let state = state.borrow();
    let handles = handles.borrow();
    let mut diff_buffer = handles.diff_buffer.clone();
    let mut diff_style_buffer = handles.diff_style_buffer.clone();
    let mut status = handles.status.clone();
    let mut copy_diff = handles.copy_diff.clone();
    let source_diff = if state.has_stale_diff() {
        format!(
            "Previous diff is stale. Press Compare to update.\n\n{}",
            render_unified_diff(state.diff())
        )
    } else {
        render_unified_diff(state.diff())
    };
    let rendered = render_diff_display(&source_diff);
    diff_buffer.set_text(&rendered_diff.text);
    diff_style_buffer.set_text(&rendered_diff.styles);
    status.set_label(state.status());
    if state.has_current_diff() {
        copy_diff.activate();
    } else {
        copy_diff.deactivate();
    }
}
```

In `copy_current_diff`, replace `let diff = state_snapshot.diff().to_string();` with:

```rust
    let diff = render_unified_diff(state_snapshot.diff());
```

Remove the `rendered_diff_text` helper (now inlined above). Remove the now-unused imports (`DEBOUNCE_MS`, `should_auto_diff` from the `diff_core` use list — leave `classify_diff_line`, `DiffLineKind`, `InlineDiffSegmentKind`, `inline_diff_match`).

- [ ] **Step 5: Update the `ui_fltk` render tests**

The existing tests assert Myers-style unified output. They now flow through `render_unified_diff` (similarity pairing). Update assertions to match similarity-paired output. Concretely:
- `render_diff_display_compacts_reliable_single_line_replacements`: input `"i wanna eat bananas" → "i wanna eat banana"`. Still pairs inline (trailing `-s`). Keep the existing assertion (`~ i wanna eat banana[-s]\n`) — verify it still holds, adjust if the prefix path changed.
- `render_diff_display_pairs_replacements_inside_multi_line_change_blocks`: update expected text to the similarity-paired output produced by `render_unified_diff` then `render_diff_display`. Run the test, read the actual `left`/`right` from the failure, and set the expected string to it (the pairing is now similarity-first, e.g. `~ i wanna ea[+a]t[+e] banana[-s]` style).
- Any test asserting on Myers split output must be updated to the new pairing.

- [ ] **Step 6: Build + test**

Run: `cargo fmt && cargo build && cargo test`
Expected: build OK, all tests PASS.

- [ ] **Step 7: Commit**

```bash
git add src/app_state.rs src/ui_fltk.rs src/diff_core.rs
git commit -m "refactor: wire app_state and ui_fltk to DisplayDiff + DiffOptions"
```

---

## Task 7: IntelliJ-style bg-color display renderer

**Goal:** replace the prefix-parsing display path with an op-driven `StyleTableEntryExt` renderer: colored backgrounds, no `@@`, adaptive folding, no `[- ]`/`~ ` markers.

**Files:**
- Modify: `src/ui_fltk.rs`
- Test: `src/ui_fltk.rs`

**Interfaces:**
- Produces: `fn render_display_ops(diff: &DisplayDiff, options: &DiffOptions, palette: Palette) -> RenderedDiff`, `fn style_table_ext(palette: Palette) -> Vec<StyleTableEntryExt>`, plus a folding helper.
- Removes: `render_diff_display`, `is_change_block_start`, `change_block_end`, `push_change_block`, `best_inline_pairs`, `InlinePair`, `push_inline_replacement_line`, `push_diff_line`, `plain_style_line`, and the `classify_diff_line`/`inline_diff_match`/`DiffLineKind`/`InlineDiffSegmentKind` imports from `ui_fltk`.

- [ ] **Step 1: Add background colors to `Palette`**

In the `Palette` struct add fields and populate them in `palette_for` (values from DESIGN.md tokens):

```rust
struct Palette {
    // ...existing fields...
    insert_bg: Color,
    delete_bg: Color,
    header_bg: Color,
}
```

Light (`Theme::System | Theme::Light`):

```rust
            insert_bg: Color::from_rgb(232, 244, 234),   // #E8F4EA
            delete_bg: Color::from_rgb(248, 231, 225),   // #F8E7E1
            header_bg: Color::from_rgb(240, 238, 232),   // #F0EEE8
```

Dark (`Theme::Dark`):

```rust
            insert_bg: Color::from_rgb(31, 58, 41),      // #1F3A29
            delete_bg: Color::from_rgb(68, 37, 31),      // #44251F
            header_bg: Color::from_rgb(46, 49, 42),      // #2E312A
```

- [ ] **Step 2: Write a failing render test**

The renderer returns `RenderedDiff { text, styles }` where `styles` is a byte-per-char style string. Add a test that exercises the structured path directly:

```rust
#[test]
fn render_display_ops_colors_inline_fragments() {
    use crate::diff_core::{build_display_diff, DiffOptions};
    let palette = palette_for(Theme::Light);
    let diff = build_display_diff("i wanna eatt banana", "i wanna eat bananas", &DiffOptions::default());
    let rendered = render_display_ops(&diff, &DiffOptions::default(), palette);

    assert!(rendered.text.contains("i wanna eat"));
    assert_eq!(rendered.text.len(), rendered.styles.len());
    // 'E' = inline-delete-fragment bg style, 'F' = inline-insert-fragment bg style (see table below)
    assert!(rendered.styles.contains('E'), "delete fragment must be styled");
    assert!(rendered.styles.contains('F'), "insert fragment must be styled");
    assert!(!rendered.text.contains("[-"), "no brackets in display");
    assert!(!rendered.text.contains("@@"), "no hunk header in display");
}

#[test]
fn render_display_ops_shows_no_differences_marker() {
    use crate::diff_core::{build_display_diff, DiffOptions};
    let palette = palette_for(Theme::Light);
    let diff = build_display_diff("same\n", "same\n", &DiffOptions::default());
    let rendered = render_display_ops(&diff, &DiffOptions::default(), palette);
    assert!(rendered.text.contains("No differences"));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test render_display_ops`
Expected: FAIL — not defined.

- [ ] **Step 4: Implement the op-driven renderer**

Replace `make_diff_display`'s style table call to use the ext variant. Update imports:

```rust
use fltk::text::{StyleTableEntryExt, TextAttr};
```

The ext style table (index = style char − 'A'):

```rust
fn style_table_ext(palette: Palette) -> Vec<StyleTableEntryExt> {
    vec![
        // 'A' normal / context
        StyleTableEntryExt { color: palette.text, font: Font::Courier, size: 14, attr: TextAttr::None, bgcolor: palette.pane },
        // 'B' header (--- left / +++ right)
        StyleTableEntryExt { color: palette.muted, font: Font::Courier, size: 14, attr: TextAttr::None, bgcolor: palette.header_bg },
        // 'C' insert line
        StyleTableEntryExt { color: palette.insert_text, font: Font::Courier, size: 14, attr: TextAttr::None, bgcolor: palette.insert_bg },
        // 'D' delete line
        StyleTableEntryExt { color: palette.delete_text, font: Font::Courier, size: 14, attr: TextAttr::None, bgcolor: palette.delete_bg },
        // 'E' inline delete fragment
        StyleTableEntryExt { color: palette.delete_text, font: Font::CourierBold, size: 14, attr: TextAttr::None, bgcolor: palette.delete_bg },
        // 'F' inline insert fragment
        StyleTableEntryExt { color: palette.insert_text, font: Font::CourierBold, size: 14, attr: TextAttr::None, bgcolor: palette.insert_bg },
        // 'G' skipped-context marker
        StyleTableEntryExt { color: palette.muted, font: Font::Courier, size: 14, attr: TextAttr::None, bgcolor: palette.pane },
    ]
}
```

In `make_diff_display`, switch the highlight call:

```rust
    display.set_highlight_data_ext(style_buffer.clone(), style_table_ext(palette));
```

The renderer and folding:

```rust
fn render_display_ops(
    diff: &crate::diff_core::DisplayDiff,
    options: &DiffOptions,
    palette: Palette,
) -> RenderedDiff {
    let mut text = String::new();
    let mut styles = String::new();

    if diff.ops.is_empty() {
        push_styled(&mut text, &mut styles, "No differences\n", 'A');
        return RenderedDiff { text, styles };
    }

    push_styled(&mut text, &mut styles, "--- left\n", 'B');
    push_styled(&mut text, &mut styles, "+++ right\n", 'B');

    let ops = fold_ops(&diff.ops, options);
    for item in ops {
        match item {
            FoldItem::Op(crate::diff_core::DiffOp::Context { text: body }) => {
                push_styled(&mut text, &mut styles, body, 'A');
                push_styled(&mut text, &mut styles, "\n", 'A');
            }
            FoldItem::Op(crate::diff_core::DiffOp::Delete { text: body }) => {
                push_styled(&mut text, &mut styles, body, 'D');
                push_styled(&mut text, &mut styles, "\n", 'D');
            }
            FoldItem::Op(crate::diff_core::DiffOp::Insert { text: body }) => {
                push_styled(&mut text, &mut styles, body, 'C');
                push_styled(&mut text, &mut styles, "\n", 'C');
            }
            FoldItem::Op(crate::diff_core::DiffOp::Inline { segments }) => {
                use crate::diff_core::InlineDiffSegmentKind::*;
                for s in segments {
                    match s.kind {
                        Equal => push_styled(&mut text, &mut styles, &s.text, 'A'),
                        Delete => push_styled(&mut text, &mut styles, &s.text, 'E'),
                        Insert => push_styled(&mut text, &mut styles, &s.text, 'F'),
                    }
                }
                push_styled(&mut text, &mut styles, "\n", 'A');
            }
            FoldItem::Skipped(count) => {
                push_styled(&mut text, &mut styles, &format!("⋯ {count} unchanged ⋯\n"), 'G');
            }
        }
    }

    RenderedDiff { text, styles }
}

enum FoldItem {
    Op(crate::diff_core::DiffOp),
    Skipped(usize),
}

/// Adaptive folding: if the op count is within the threshold, show all ops;
/// otherwise keep changes plus `radius` context and collapse the rest.
fn fold_ops(ops: &[crate::diff_core::DiffOp], options: &DiffOptions) -> Vec<FoldItem> {
    fn is_change(op: &crate::diff_core::DiffOp) -> bool {
        !matches!(op, crate::diff_core::DiffOp::Context { .. })
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
            for k in lo..hi {
                keep[k] = true;
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

fn push_styled(text: &mut String, styles: &mut String, value: &str, style: char) {
    text.push_str(value);
    styles.extend(std::iter::repeat_n(style, value.len()));
}
```

- [ ] **Step 5: Switch `render_state` to use the new renderer**

```rust
fn render_state(state: &Rc<RefCell<AppState>>, handles: &Rc<RefCell<UiHandles>>) {
    let state = state.borrow();
    let handles = handles.borrow();
    let mut diff_buffer = handles.diff_buffer.clone();
    let mut diff_style_buffer = handles.diff_style_buffer.clone();
    let mut status = handles.status.clone();
    let mut copy_diff = handles.copy_diff.clone();

    let rendered = if state.has_stale_diff() {
        let mut r = render_display_ops(state.diff(), state.options(), handles.palette);
        r.text = format!("Previous diff is stale. Press Compare to update.\n\n{}", r.text);
        r
    } else {
        render_display_ops(state.diff(), state.options(), handles.palette)
    };
    diff_buffer.set_text(&rendered.text);
    diff_style_buffer.set_text(&rendered.styles);
    status.set_label(state.status());
    if state.has_current_diff() {
        copy_diff.activate();
    } else {
        copy_diff.deactivate();
    }
}
```

This requires `UiHandles` (or a sibling) to hold the `Palette`. Add `palette: Palette` to `UiHandles`, set it when building handles:

```rust
struct UiHandles {
    // ...existing fields...
    palette: Palette,
}
```

and in the `UiHandles { ... }` construction in `run()`, add `palette,` (the local `palette` variable is in scope).

- [ ] **Step 6: Delete the now-dead prefix-render code**

Remove from `src/ui_fltk.rs`: `render_diff_display`, `RenderedDiff` (keep the struct definition — the new renderer returns it; ensure it's defined once), `is_change_block_start`, `change_block_end`, `push_change_block`, `InlinePair`, `best_inline_pairs`, `push_inline_replacement_line`, `push_diff_line`, `plain_style_line`, and the old `push_styled_text` (renamed `push_styled`). Remove `classify_diff_line`, `DiffLineKind`, `InlineDiffSegmentKind`, `inline_diff_match` from the `use crate::diff_core` import (keep `render_unified_diff`, `DiffOptions`). Keep `RenderedDiff { text, styles }` defined once.

- [ ] **Step 7: Build + test**

Run: `cargo fmt && cargo build && cargo test`
Expected: build OK; the two new render tests PASS; any leftover prefix-render tests removed/updated.

- [ ] **Step 8: Manual smoke check**

Run: `cargo run`
Paste the trigger example (left `i wanna eatt banana\ni wanna eatt banana`, right `i wanna eat bananas\ni wanna eatt banana\ni，`) and confirm: first line shows `t` on red bg and `s` on green bg inline; second line is plain context; third is a green-bg insert; no `@@`, no `[-]`. Hit Copy Diff and paste elsewhere — confirm standard unified text.

- [ ] **Step 9: Commit**

```bash
git add src/ui_fltk.rs
git commit -m "feat(ui): IntelliJ-style bg-color op-driven diff renderer with adaptive folding"
```

---

## Task 8: Investigate and fix the `，` (fullwidth comma) truncation

**Files:**
- Possibly `src/diff_core.rs` or `src/ui_fltk.rs`
- Test: wherever the bug is reproduced

- [ ] **Step 1: Write a reproducing test**

Add to `src/diff_core.rs` tests:

```rust
#[test]
fn fullwidth_comma_is_preserved_in_insert_op() {
    let d = build_display_diff("a\n", "a\ni，\n", &DiffOptions::default());
    let inserted = d.ops.iter().find_map(|op| match op {
        DiffOp::Insert { text } if text.contains('i') => Some(text.clone()),
        _ => None,
    });
    assert_eq!(inserted.as_deref(), Some("i，"));
}
```

- [ ] **Step 2: Run test**

Run: `cargo test fullwidth_comma_is_preserved_in_insert_op`
- If PASS: the truncation was a paste/terminal artifact, not a code bug. Note that in the commit message and finish this task.
- If FAIL: the bug is real; continue.

- [ ] **Step 3: Fix the root cause**

If the test fails, the likely cause is byte-index math (a multibyte char counted as one byte) somewhere a length is used to slice a `String`. Audit `changed_ratio`/`char_level_segments`/`render_unified_diff` for any `.len()`-based slicing of line text (char-based APIs like `.chars()`/`.lines()` are safe; `.split(n)` or `[..n]` on byte indices are not). Fix to use char-aware operations, then re-run until the test passes.

- [ ] **Step 4: Commit**

```bash
cargo fmt && cargo test
git add src/diff_core.rs
git commit -m "fix(diff_core): preserve multibyte (e.g. fullwidth comma) chars in ops"
```
(If no fix was needed, skip the commit and leave a note in the task log.)

---

## Task 9: Update documentation

**Files:**
- Modify: `DESIGN.md`, `IMPLEMENTATION_PLAN.md`, `CLAUDE.md`

- [ ] **Step 1: Update `DESIGN.md`**

- In the Visual System / diff-coloring section: replace the `[-removed][+added]` inline description with "inline changed fragments use background colors (insert bg / delete bg), no text brackets; the structured model is `DiffOp`."
- Remove the requirement that `@@` hunk lines are colored/shown in the display (note Copy still emits them).
- Change "context radius 3 folding" to "adaptive folding: show all lines when the op count ≤ `display_full_context_max_lines`, else fold to `unified_context_radius` with a `⋯ N unchanged ⋯` marker."
- Add a sentence: all diff thresholds/ratios are configurable with defaults.

- [ ] **Step 2: Update `IMPLEMENTATION_PLAN.md`**

- Replace the `diff_core` interface block: document `DiffOptions`, `DiffOp`, `DisplayDiff`, `build_display_diff(left, right, &DiffOptions) -> DisplayDiff`, `render_unified_diff(&DisplayDiff) -> String`. Remove `build_unified_diff`, `classify_diff_line`, `inline_changed_byte_ranges` from the documented interface.
- Update the "Diff output contract" table: display is structured + bg-colored; copy is standard unified text from the same ops; equal ⇒ `No differences\n`; no `@@` in display.
- Add `DiffOverrides` to the `config` section.

- [ ] **Step 3: Update `CLAUDE.md`**

- In the "Diff output contract" paragraph: replace the prefix-classification description with "display consumes `Vec<DiffOp>` directly; copy renders standard unified text via `render_unified_diff`." Replace the inline-rendering note with "similarity-weighted banded alignment; inline fragments get background colors via `StyleTableEntryExt`."
- Add a line to the conventions: "All diff thresholds/ratios live in `diff_core::DiffOptions::default()`; `config::DiffOverrides` carries optional overrides; `ui_fltk::diff_options_from_config` bridges them."

- [ ] **Step 4: Commit**

```bash
git add DESIGN.md IMPLEMENTATION_PLAN.md CLAUDE.md
git commit -m "docs: update design/plan/CLAUDE for similarity-first bg-color diff"
```

---

## Self-Review (completed during planning)

**Spec coverage:** §1 data model → T1–T4; §2 pairing algorithm → T3; §3 config tunables → T1 (defaults) + T5 (overrides) + T6 (bridge); §4 rendering → T7; §5 display rules (no `@@`, adaptive folding) → T7; §6 copy → T4 + T6; §7 preserved contracts → T3/T4/T6; §8 edge cases → T3/T8 + existing unicode tests; §9 testing → every task; §10 docs → T9. Open item "FLTK bg renders cleanly" → T7 Step 8 manual check. Open item "defaults tuning" → noted in Global Constraints. Open item "`，`" → T8.

**Placeholder scan:** Task 6 Step 5 and Task 7 Step 6 instruct the implementer to run tests and reconcile actual output — this is intentional (exact strings depend on the similarity pairing the implementer just wired in), not a placeholder. Every code step includes real code.

**Type consistency:** `DisplayDiff`, `DiffOp`, `DiffOptions`, `DiffOverrides`, `AppState::new/options/should_auto_diff/diff`, `build_display_diff`, `render_unified_diff`, `render_display_ops`, `diff_options_from_config` are used consistently across tasks. `DiffResult.diff` is `DisplayDiff` everywhere after T6.

**Note on `similarity_pairing_max_lines` default:** spec draft said 4000; plan uses 1000 (Global Constraints explains why: O(n·m) matrix). Still configurable.
