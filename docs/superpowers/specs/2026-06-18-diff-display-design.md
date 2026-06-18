# Slippy Diff Display Redesign

- Date: 2026-06-18
- Status: Draft, pending user review
- Owner: product decision captured in brainstorming session

## Goal

Replace the current "Myers line diff + adjacent-pair inline highlighting" display
with a **similarity-first** diff that:

1. Pairs lines the way a human expects (like IntelliJ), even when duplicate lines
   would otherwise make a standard LCS match the "wrong" copy.
2. Renders changed fragments with **colored backgrounds** (no `[- ][+ ]` text
   brackets), inline on a single line.
3. Makes **every ratio and threshold config-driven**, falling back to defaults
   when missing or invalid.

## Background / Problem

Today `diff_core::build_unified_diff` runs `similar`'s Myers line diff, and the
`~ ...[-x][+y]` inline highlighting only applies to a delete line and an insert
line that the line diff placed **adjacent** in the same change block.

When duplicate lines exist, line-level LCS matches an exact copy as **context**,
which splits a near-pair across a context line so it is never inline-diffed.

Concrete failure (the trigger for this redesign):

```
left:
  i wanna eatt banana
  i wanna eatt banana
right:
  i wanna eat bananas
  i wanna eatt banana
  i，
```

Current output (Myers matched `left[0] ↔ right[1]` as context):

```
+i wanna eat bananas
 i wanna eatt banana
-i wanna eatt banana
+i
```

Desired output:

```
 i wanna eat[t] banana[s]      # left[0] ↔ right[0] inline (t removed, s added)
 i wanna eatt banana            # left[1] = right[1] context
+i，                            # right[2] inserted
```

Root cause: line-level diff only sees exact equality; inline diff is a post-pass
on adjacent pairs, so it cannot promote a near-pair that the line diff split apart.

## Non-Goals

- File or directory diff.
- Syntax highlighting, word-level diff (we do **char-level**).
- Patch apply / merge / conflict resolution.
- Side-by-side aligned view.
- Settings UI (config is file-only in v1).

## Design

### 1. Data model (`diff_core`)

Replace the "produce a unified-diff string, then parse it by prefix" pipeline with
**structured output**:

```rust
pub enum DiffOp {
    Context { text: String },                    // identical on both sides
    Delete  { text: String },                    // left only
    Insert  { text: String },                    // right only
    Inline  { segments: Vec<InlineDiffSegment> },// similar pair, char-level segments
}

pub fn build_display_diff(left: &str, right: &str, options: &DiffOptions) -> Vec<DiffOp>;
```

- `InlineDiffSegment` (existing) is reused: `{ kind: Equal|Delete|Insert, text }`.
- `ui_fltk` consumes `Vec<DiffOp>` directly. The prefix-parsing render path
  (`render_diff_display`, the `~ `/`[-]` markers) is removed from the display path.
- `build_unified_diff` is replaced by a unified-diff **renderer over `Vec<DiffOp>`**
  used only for Copy (see §6). `classify_diff_line` is removed — the structured
  model makes prefix classification unnecessary.
- `app_state` stores the diff result as `Vec<DiffOp>` instead of a `String`.
  Copy Diff renders unified text from those ops on demand; the stale-worker guard
  is unchanged (see §7).

### 2. Pairing algorithm — similarity-weighted banded alignment ("fuzzy LCS")

Instead of Myers (exact equality only), treat the two line sequences as a
similarity-weighted alignment:

- For left line `i` and right line `j` within a position band `|i − j| ≤ alignment_band`,
  compute similarity `= 1 − edit_distance / max(len_i, len_j)` using a char-level
  diff (`similar::TextDiff::from_chars`). Identical lines score 1.0.
- A match is allowed only when `changed_ratio ≤ inline_max_changed_ratio`
  (equivalently similarity `≥ 1 − inline_max_changed_ratio`).
- A banded Needleman–Wunsch-style DP maximizes **total** similarity and is
  **monotone (non-crossing)**. Reconstruct ops from the optimal alignment:
  - matched identical → `Context`
  - matched similar  → `Inline { segments }` (the char-level segments)
  - unmatched left   → `Delete`
  - unmatched right  → `Insert`

Why it fixes the example: the alignment matches **more lines**
(`left[0]↔right[0]` at 0.95 **plus** `left[1]↔right[1]` at 1.0 = 1.95) versus
Myers (only `left[0]↔right[1]` at 1.0). Maximizing total similarity prefers the
human-intuitive pairing.

Performance guards:

- Position band (`alignment_band`) bounds the DP to `O(n · band)`.
- A cheap similarity pre-filter (length-ratio + character trigram overlap) skips
  obviously-dissimilar pairs before running the full char diff.
- `similarity_pairing_max_lines` is a hard cap: when total lines exceed it, the
  display falls back to exact-match LCS (no similarity pairing) to stay fast.

### 3. Config-driven tunables (hard requirement)

**Every ratio and number is configurable; missing or invalid → default.**

- Canonical defaults live in `diff_core::DiffOptions::default()` — the **single
  source of truth** for default values.
- `config::AppConfig` gains an optional `diff` overrides block. Every field is
  `Option<T>` defaulting to `None` (i.e. the JSON may omit any/all of them).
- `ui_fltk` applies the overrides onto `DiffOptions::default()` and passes the
  result into `build_display_diff`.

Layering is preserved: `diff_core` takes `&DiffOptions` as a parameter and still
**does not depend on `config`**; `config` does not depend on `diff_core` (it stores
raw `Option<T>` overrides); `ui_fltk` is the bridge.

| Field | Type | Default | Range | Meaning |
|---|---|---|---|---|
| `debounce_ms` | u64 | 300 | ≥ 0 | auto-diff debounce delay |
| `auto_diff_max_bytes` | usize | 262144 | ≥ 0 | auto-diff byte gate |
| `auto_diff_max_lines` | usize | 8000 | ≥ 0 | auto-diff line gate |
| `unified_context_radius` | usize | 3 | ≥ 0 | context radius when folded |
| `inline_max_changed_ratio` | f32 | 0.50 | 0.0–1.0 | pairing threshold (max changed fraction) |
| `display_full_context_max_lines` | usize | 200 | ≥ 0 | show all lines when the (pre-fold) op count ≤ this, else fold |
| `similarity_pairing_max_lines` | usize | 4000 | ≥ 0 | input lines (left + right) above this skip similarity pairing |
| `alignment_band` | usize | 25 | ≥ 1 | position window for fuzzy LCS |

Validation: a field that is present but out of range or the wrong type is treated
as **absent** → that field uses the default. Existing config behavior applies
(invalid → defaults + status reported). The privacy invariant is unchanged:
config stores only metadata and tunables, **never** pasted text or diff output.

### 4. Rendering (`ui_fltk`)

- The diff pane switches to `StyleTableEntryExt` (`color`, `font`, `size`, `attr`,
  `bgcolor`) via the ext highlight API.
- `Inline` op: equal segments render normal; delete segments on the delete-bg
  color; insert segments on the insert-bg color. **No `[- ]` brackets, no `~ `
  prefix.**
- `Context` / `Delete` / `Insert` lines use the existing DESIGN.md line-level
  bg/text tokens.
- Keep the `--- left` / `+++ right` header lines as side labels.

### 5. Display rules

- **No `@@` hunk headers** in the display.
- **Adaptive folding**: if the pre-fold op count ≤ `display_full_context_max_lines`,
  show every line (all matched lines as context plus the changes). Otherwise fold to
  `unified_context_radius` and replace each hidden run with a
  `⋯ N unchanged ⋯` marker line.
- `--- left` / `+++ right` headers shown.

### 6. Copy Diff

Copy produces standard unified-diff **text** over the **same** `Vec<DiffOp>` and
the **same** folding as the display:

- `Inline` → `-old\n+new` (reconstruct old/new text from the segments).
- `Context` → ` line`; `Delete` → `-line`; `Insert` → `+line`.
- Include `@@` hunk headers; single trailing newline.

Result: what you see (same pairing, same visible lines) is what you copy, in a
portable `+`/`-` text form that `patch` accepts and that reads cleanly in
chats/PRs.

### 7. Preserved contracts

- Equal text → exactly `No differences\n`.
- Output ends with exactly one trailing newline.
- No-trailing-newline friendly notice retained (`! Left/right text ends without a
  trailing newline`).
- `app_state` stale-worker guard (monotonic request id + `dirty_since_latest_request`)
  is unchanged.

### 8. Edge cases

- Both empty; one side empty; single line.
- Unicode and CJK (char-level pairing highlights individual differing characters —
  ideal for CJK, which has no word boundaries).
- No trailing newline on either or both sides.
- Whitespace-only differences.
- Config: missing file, malformed JSON, out-of-range values → defaults + status.

### 9. Testing

- **Golden**: the trigger example → expected `Vec<DiffOp>`
  (`Inline left[0]↔right[0]`, `Context left[1]=right[1]`, `Insert right[2]`).
- `diff_core`: pairing is similarity-first (regression vs the Myers split);
  `inline_max_changed_ratio` boundary; band behavior; fallback above
  `similarity_pairing_max_lines`; empty / unicode / no-newline inputs;
  `No differences\n`.
- `config`: each tunable override round-trips; missing → default; out-of-range →
  default + status; no text/diff persisted; path injection for tests.
- `ui_fltk` rendering: bg colors applied per segment; adaptive folding at the
  threshold; copy text mirrors the display ops.
- Separately investigate the dropped `，` in `+i` (real bug vs paste/terminal
  artifact) and fix it if real.

### 10. Documentation updates

- `DESIGN.md`: drop the `@@` hunk-coloring requirement; change inline rendering to
  bg-color (no brackets); change folding to adaptive; note the structured op model.
- `IMPLEMENTATION_PLAN.md`: update the diff output contract, the state/UI contract
  references, and the test plan to match.
- `CLAUDE.md`: update the "Diff output contract" and inline-rendering notes.

## Open items to verify during implementation

- The exact `alignment_band` and `similarity_pairing_max_lines` defaults may need
  tuning after performance testing.
- Confirm `StyleTableEntryExt` per-character background renders cleanly in the
  bundled FLTK (alignment, line-height fill). Fallback to foreground-color +
  strikethrough if it does not.
- The `，` truncation.
