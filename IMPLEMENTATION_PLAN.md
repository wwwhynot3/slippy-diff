# Slippy v1 Implementation Plan

## Summary

Build Slippy as a personal, paste-first, cross-platform Rust desktop text diff tool using `fltk-rs`. v1 optimizes for daily copy-paste comparison, not file or directory diff, not daemon behavior, and not public packaging.

The reviewed v1 contract is:

- Two editable input panes.
- Action bar between input and output.
- Read-only colored unified diff pane.
- Bottom status bar.
- Debounced auto-diff for normal input.
- Manual Compare for large input.
- No persistence of pasted text or diff output.
- Source-level developer documentation.

## Architecture

```text
main
  |
  v
ui_fltk
  |-- FLTK widgets, styling, debounce timer, worker channel, clipboard
  v
app_state
  |-- text state, dirty/stale state, request ids, status transitions
  v
diff_core
  |-- similar-based unified diff, line classification, auto-diff guard

config
  |-- layout/theme/font metadata only
```

Rules:

- `diff_core` must not depend on FLTK, clipboard, config, or threading.
- `app_state` must not depend on FLTK or `arboard`.
- `ui_fltk` owns FLTK widgets, styling, clipboard integration, worker spawning, timers, shortcuts, and widget refresh.
- `config` must never persist pasted text or diff output.
- `main` only starts the application.

## Dependencies

Target Cargo shape:

```toml
[dependencies]
fltk = { version = "1", features = ["fltk-bundled"] }
similar = "2"
arboard = "3"
directories = "5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"

[dev-dependencies]
tempfile = "3"

[features]
default = []
wayland = ["fltk/use-wayland"]
```

Implementation note: if the current `fltk-rs` release rejects the exact Wayland feature spelling, adjust only the Cargo feature mapping to the official current equivalent. The product intent remains default bundled FLTK plus optional Wayland build support.

## Core Interfaces

`diff_core`:

```rust
pub struct DiffOptions { /* debounce_ms, auto_diff_max_bytes, auto_diff_max_lines,
                            unified_context_radius, inline_max_changed_ratio,
                            display_full_context_max_lines, similarity_pairing_max_lines,
                            alignment_band — all with Default impl */ }
pub enum DiffOp { Context { text: String }, Delete { text: String },
                  Insert { text: String }, Inline { segments: Vec<InlineDiffSegment> } }
pub struct DisplayDiff { pub ops: Vec<DiffOp>, pub left_no_newline: bool,
                         pub right_no_newline: bool }

pub fn should_auto_diff(left: &str, right: &str, options: &DiffOptions) -> bool;
pub fn build_display_diff(left: &str, right: &str, options: &DiffOptions) -> DisplayDiff;
pub fn render_unified_diff(diff: &DisplayDiff) -> String;
```

Diff output contract:

| Case | Output |
| --- | --- |
| Equal text | Exactly `No differences\n` |
| Changed text (display) | Structured `Vec<DiffOp>` rendered with background colors via FLTK `StyleTableEntryExt`; no `@@` lines in display |
| Changed text (copy) | Standard unified text via `render_unified_diff` from the same ops; includes `@@` hunk headers |
| Newline policy | Display and copy output always end with exactly one trailing newline |

`app_state`:

```rust
pub struct DiffRequest {
    pub id: u64,
    pub left: String,
    pub right: String,
}

pub enum ApplyOutcome {
    Applied,
    IgnoredStaleRequest,
    IgnoredBecauseDirty,
}
```

State rules:

- Editing or pasting updates text and marks the diff dirty.
- Compare creates a monotonic request id and snapshots left/right text.
- Editing after a request starts sets `dirty_since_latest_request = true`.
- A worker result applies only if its id is still latest and no edit happened after it started.
- Clear invalidates in-flight results and resets text, diff, dirty state, and status.
- Swap swaps left/right text, marks dirty, and schedules auto-diff only if input is within threshold.
- Manual Compare bypasses auto-diff thresholds.

`config`:

- Store `version`, `width`, `height`, `vertical_split`, `pinned`, `theme`, `ui_font`, and `mono_font`.
- Use the OS config directory via `directories` with app identity `dev.wwwhynot3.slippy` / `Slippy`.
- Default window is 1120x760.
- Minimum window is 720x520.
- `vertical_split` defaults to 0.45 and clamps to 0.30-0.70; adjusted live by dragging the sash between the input and diff areas and persisted on close.
- `pinned` defaults to `false`; when `true` the window opens always-on-top and the Pin button reflects that state on launch.
- Theme enum is `System`, `Light`, or `Dark`; default is `System`.
- `AppConfig.diff: DiffOverrides` carries optional overrides for every `DiffOptions` field; `sanitized()` clamps values before the bridge applies them.
- Invalid or missing config returns defaults and reports status.
- Save errors report status and never crash the app.

## UI Contract

Layout:

```text
[ Left Input 50% ][ Right Input 50% ]
[ Paste Left | Paste Right | Compare | Swap | Clear | Copy Diff ]
[ Read-only Unified Diff Result ]
[ Bottom Status Bar ]
```

Layout rules:

- Use FLTK `TextEditor` for both input panes.
- Use FLTK `TextDisplay` or an equivalent read-only styled text widget for the diff pane.
- The action bar is fixed-height and does not participate in the split ratio.
- The status bar is fixed-height.
- Persist only the input-vs-diff split, not pasted text.
- Below 760px width, stack input panes vertically while keeping the action bar between input area and diff.
- The action bar remains a single row; the 720px minimum width prevents normal overflow.

Button rules:

| Button | Rule |
| --- | --- |
| Compare | Primary visual button, always available, starts the newest request |
| Paste Left/Right | Reads clipboard text into target pane, then returns focus to that pane |
| Swap | Swaps panes, marks diff dirty, schedules auto-diff if allowed |
| Clear | Clears inputs and diff, invalidates workers |
| Copy Diff | Enabled when a current diff result exists, including `No differences\n` |
| Prev | Enabled when a current diff has at least one change; steps to the previous change region (wraps) |
| Next | Enabled when a current diff has at least one change; steps to the next change region (wraps) |

Diff navigation:

- Prev/Next step through maximal runs of adjacent change rows (Delete/Insert/ReplaceOld/ReplaceNew); Context/Fold/Notice rows break runs (see `diff_view::RenderedDiffView::change_regions`).
- Navigation wraps around at both ends. From a fresh diff the first Prev or Next lands on the first change.
- The active region is marked with a soft `Selection` strip on the canvas's left edge; status reads `Change N of M.` during navigation.
- The navigation cursor is UI-local ephemeral state (`Rc<Cell<Option<usize>>>`); it is not persisted and is not part of `app_state`. It resets to `None` whenever a new diff result is applied or the inputs are cleared.

Status behavior:

| Trigger | Diff pane | Status |
| --- | --- | --- |
| Startup | Empty helper text | `Ready. Paste left and right text.` |
| Normal edit | Keep previous diff if any | `Diff pending...` |
| Debounced worker running | Keep previous diff if any | `Diff running...` |
| Latest diff applied | New diff | `Diff updated.` |
| Equal text | `No differences\n` | `No differences.` |
| Large input auto skipped | Prefix stale notice before previous diff if any | `Large input - press Compare to update.` |
| Paste failure | Preserve pane text | `Paste failed: clipboard text unavailable.` |
| Copy failure | Preserve diff | `Copy Diff failed: clipboard unavailable.` |
| Config load invalid | Use defaults | `Config invalid; using defaults.` |
| Config save failure | Keep app running | `Could not save layout config.` |
| Prev/Next navigation | Preserve current diff | `Change N of M.` |

Keyboard shortcuts:

| Shortcut | Action |
| --- | --- |
| Ctrl/Cmd+Enter | Compare |
| Ctrl/Cmd+L | Paste Left |
| Ctrl/Cmd+R | Paste Right |
| Ctrl/Cmd+Shift+S | Swap |
| Ctrl/Cmd+Shift+C | Copy Diff |
| Ctrl/Cmd+Shift+Up | Previous change |
| Ctrl/Cmd+Shift+Down | Next change |

Use Cmd on macOS where FLTK supports it and Ctrl elsewhere. If FLTK cannot map Cmd cleanly, keep Ctrl working and document the limitation.

Focus and accessibility basics:

- Tab order is left editor, right editor, action buttons left-to-right, diff display.
- Status bar is not focusable.
- Paste buttons return focus to their target pane.
- Compare keeps focus in the originating editor when possible.
- Copy Diff leaves focus unchanged.
- Buttons use text labels, not icon-only controls.
- Visible keyboard focus is required.

## Visual Contract

Line-level diff coloring is required for v1. Plain prefixes are retained, but an entirely uncolored diff is a defect unless FLTK styling is proven impossible during build verification.

Use FLTK text styling, such as style buffers and style table entries, to apply per-line colors based on `DiffLineKind`. Show line numbers on both input editors and the diff display. For replacement blocks, pair similar deleted and inserted lines before rendering; reliable pairs use inline `[-removed][+added]` display, while unmatched lines stay line-level.

Color tokens:

| Token | Light | Dark |
| --- | --- | --- |
| Surface | `#F7F5F0` | `#1F211E` |
| Pane | `#FFFEFA` | `#252822` |
| Text | `#25231F` | `#ECE8DD` |
| Muted | `#6E675E` | `#A69F91` |
| Border | `#D8D2C7` | `#3A3E35` |
| Primary | `#2F6F73` | `#6FA8AD` |
| Primary text | `#FFFFFF` | `#102022` |
| Insert bg/text | `#E8F4EA` / `#1F6B3A` | `#1F3A29` / `#A8D8B2` |
| Delete bg/text | `#F8E7E1` / `#9A3A25` | `#44251F` / `#F0A08A` |
| Hunk bg/text | `#E9EDF5` / `#42526B` | `#263245` / `#B7C6E6` |
| Header bg | `#F0EEE8` | `#2E312A` |
| Selection | `#C8D8D9` | `#36565A` |
| Error | `#A33A2A` | `#E18B78` |
| Status bg | `#EFECE4` | `#252820` |

Density:

- Compact desktop spacing with a 4px base scale.
- Buttons around 28-32px tall.
- Pane gap around 8px.
- Editor and diff text around 13-14px by default.
- Use system UI font for controls and system monospace font for editors/diff where FLTK exposes them.

## Worker Flow

```text
edit/paste
  -> UI updates AppState
  -> AppState marks dirty
  -> if should_auto_diff: debounce 300ms
  -> state creates DiffRequest with request_id and cloned text
  -> worker computes diff_core::build_unified_diff(left, right)
  -> UI receives worker result via FLTK-safe channel
  -> state applies only if request_id is latest and no edit happened after request start
  -> UI refreshes diff pane and status
```

Worker rules:

- One worker thread per request is acceptable for v1.
- No cancellation is required.
- Stale results must be ignored.
- Compare while computing starts a newer request.
- Large input skips debounce auto-diff but still allows manual Compare.
- Worker errors preserve current text and report status.

## Clipboard Rules

- Clipboard access belongs at the UI boundary.
- `app_state` must not depend on `arboard`.
- Clipboard paste failure must not clear or modify pane text.
- Copy Diff failure must not clear or modify current diff.
- Clipboard behavior should be testable through a small adapter seam or equivalent boundary.

## Source DX

Add `README.md` before considering v1 complete.

README must include:

- What Slippy is and what v1 is not.
- Prerequisites for source builds.
- `cargo test`.
- `cargo run`.
- `cargo build --release`.
- `cargo run --features wayland`.
- Native FLTK build dependency notes, including C/C++ toolchain, CMake, pkg-config, X11/OpenGL packages, and Wayland packages when using the Wayland feature.
- Common `fltk-sys` troubleshooting notes.
- Config and privacy policy: only layout/theme/font is stored; pasted text and diff output are never stored.
- Shortcuts.
- Manual GUI smoke checklist.
- Note that Windows/macOS detailed build docs and installers are not v1 scope.

## Implementation Order

1. Add Cargo dependencies, features, and `src/lib.rs` module skeleton.
2. Implement `diff_core` test-first.
3. Implement `config` test-first with path injection.
4. Implement `app_state` test-first, including stale worker semantics.
5. Implement `ui_fltk` layout, styling, actions, debounce, workers, and clipboard.
6. Add README source-level DX documentation.
7. Run `cargo test`, default build, and Wayland feature build where dependencies are present.
8. Run manual GUI smoke checks.

## Test Plan

Run `cargo test`.

Required `diff_core` tests:

- Equal text returns exactly `No differences\n`.
- Insert, delete, and replace cases include expected unified diff lines.
- Replacement blocks pair similar deleted and inserted lines, render matched pairs inline, and leave unmatched lines as plain delete/insert lines.
- Headers include `--- left` and `+++ right`.
- Hunk output includes `@@`.
- Context radius is 3.
- Output ends with exactly one trailing newline.
- Empty left, empty right, and both empty inputs.
- Unicode/CJK text is preserved.
- Whitespace-only changes are deterministic.
- Auto-diff byte threshold boundary.
- Auto-diff line threshold boundary.
- `classify_diff_line` classifies header, hunk, insert, delete, and context lines correctly.

Required `app_state` tests:

- Edit marks dirty.
- Compare creates increasing request ids.
- Latest worker result applies.
- Stale request id is ignored.
- Edit during in-flight request prevents old result from applying.
- Clear invalidates in-flight results.
- Swap swaps text and marks dirty.
- Large input skips auto-diff but manual Compare is allowed.
- Clipboard-style failures become user-visible status without text loss.
- Config-style failures become user-visible status without text loss.

Required `config` tests:

- Missing config returns defaults.
- Malformed config returns defaults with status.
- Out-of-range width, height, and split values are clamped.
- Save/load round trip preserves layout/theme/font metadata.
- Config APIs accept injected paths for tests.
- Invalid theme falls back to default.
- No text content or diff output is serialized.

Manual GUI smoke:

- App opens and closes with no daemon, tray, or background process.
- Paste Left and Paste Right target the correct pane.
- Keyboard paste still works in editors.
- Compare, Swap, Clear, and Copy Diff work.
- Debounced auto-diff updates after normal edits.
- Rapid edits do not allow stale worker output to overwrite newer text.
- Large input shows manual Compare status and manual Compare updates diff.
- Insert, delete, and hunk line colors are visible in light and dark themes.
- The action bar is visually between input and diff.
- Small window layout remains usable.
- Idle CPU returns near zero after debounce.

Build verification:

- `cargo test` passes.
- `cargo build` passes.
- `cargo build --features wayland` is attempted on Linux when dependencies are present.
- If Wayland feature build cannot be verified locally, document the reason in README or TODO.

## Acceptance Criteria

- Open, paste left, paste right, inspect diff, copy diff, and close works without background behavior.
- Normal-sized edits auto-diff after 300ms.
- Combined input above 256 KiB or 8,000 lines does not auto-diff and asks for manual Compare.
- UI does not freeze for normal input.
- Stale worker results never overwrite newer edits.
- Pasted text and diff output are never persisted.
- Line-level diff colors are visible for insert, delete, and hunk lines.
- Source checkout has clear build/run/test instructions.

## Not In Scope

- File or directory diff.
- Clipboard watcher or automatic clipboard read.
- Background daemon, tray app, or global shortcut listener.
- Custom/compositor-specific always-on-top (use the OS window-manager hint instead).
- Side-by-side aligned diff.
- Syntax highlighting.
- Inline word diff.
- Patch apply.
- Merge/conflict support.
- Settings UI.
- Clipboard-into-both-panes workflow.
- Public packaging, CI release matrix, package manager distribution, or installers.
