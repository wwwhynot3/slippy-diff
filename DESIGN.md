# Slippy Design

## Direction

Slippy is a quiet, trustworthy native desktop utility for comparing two pasted text snippets. It should feel like a scratchpad: open it, paste left and right text, inspect the diff, copy the result if needed, close it, and leave no background process behind.

The design should stay utilitarian and compact. Avoid decorative chrome, marketing-style visuals, dashboard cards, icon-only controls, and custom visual complexity that does not improve the paste-diff-copy workflow.

## Layout

Primary layout:

```text
[ Left Input 50% ][ Right Input 50% ]
[ Paste Left | Paste Right | Compare | Swap | Clear | Copy Diff ]
[ Unified Review | Prev | Next | Pin | Summary ]
[ Read-only Unified Diff Result ]
[ Bottom Status Bar ]
```

The action bar belongs between the input area and the diff result. This keeps the mouse path short after editing or pasting, and it acts as the semantic bridge between source text and generated output.

Layout rules:

- The left and right input panes share the upper area equally.
- The diff result pane occupies the lower area and is read-only.
- The action bar has fixed height and does not participate in the split ratio.
- The bottom status bar has fixed compact height.
- Persist only the vertical split between the input area and diff area.
- Default window is 1120x760.
- Minimum window is 720x520.
- Under 760px window width, stack the two input panes vertically while keeping the action bar between inputs and diff.
- A draggable sash between the input area and the diff area adjusts their relative heights; the position is persisted as `vertical_split`.
- On short windows the action bar, diff toolbar, and status bar switch to compact heights with a smaller action-button font, giving the inputs and diff more vertical room.
- The action bar remains a single row; the minimum width is chosen to prevent ordinary label overflow.

## Action Bar

The action bar is a compact command strip, not a top navigation bar.

Button order:

```text
Paste Left | Paste Right | Compare | Swap | Clear | Copy Diff
```

Interaction priorities:

- `Compare` is the primary action and should be visually distinct.
- `Paste Left` and `Paste Right` sit near the left side because they feed the input panes.
- `Swap`, `Clear`, and `Copy Diff` are secondary actions.
- Keyboard paste inside the editors must continue to work normally.
- Paste buttons return focus to their target pane.
- Copy Diff leaves focus unchanged.

Shortcuts:

- `Ctrl/Cmd+Enter`: Compare
- `Ctrl/Cmd+L`: Paste Left
- `Ctrl/Cmd+R`: Paste Right
- `Ctrl/Cmd+Shift+S`: Swap
- `Ctrl/Cmd+Shift+C`: Copy Diff
- `Ctrl/Cmd+Shift+P`: Toggle Pin
- `Ctrl/Cmd+Shift+Up`: Previous change
- `Ctrl/Cmd+Shift+Down`: Next change
- `Ctrl/Cmd+Shift+T`: Cycle theme (System / Light / Dark)

Use Cmd on macOS where FLTK supports it and Ctrl elsewhere. If FLTK cannot map Cmd cleanly, keep Ctrl working and document the limitation.

## Visual System

Use the system UI font for controls and the system monospace font for editors and diff output where the toolkit exposes them. Allow config-only overrides with `ui_font` and `mono_font`, but do not build a settings UI in v1.

Theme support:

- `System` is the default.
- `Light` and `Dark` can be set through config or cycled live with the **Theme** toolbar button (`Ctrl/Cmd+Shift+T`); the choice persists as `theme`.
- The Theme button re-colors the entire window immediately (live switch), including the custom-drawn diff canvas and gutters.

Line-level diff coloring is required:

- The diff result uses a custom-drawn unified review canvas with old/new line-number gutters, a marker column, rendered text, and a narrow change overview rail.
- `+` insertion rows use soft insert colors; `-` deletion rows use soft delete colors.
- Input panes keep FLTK `TextEditor` behavior for editing, selection, clipboard, undo, and IME, but use custom-drawn line-number gutters beside the editors.
- Replacement blocks pair similar deleted and inserted lines before rendering; paired rows use a neutral block background with `~` markers and stronger red/green token highlights for the exact changed fragments.
- The old/new gutters are semantic references: inserted rows leave the old line blank, deleted rows leave the new line blank, and later context rows may show offset line numbers.
- The rendered review header shows `LEFT  RIGHT  K | Text` above the diff rows, with a separator beneath it.
- `---` and `+++` header lines are part of the plain unified diff text emitted by Copy Diff.
- Preserve visible marker and line-number gutters so the diff remains understandable even if colors are hard to distinguish.
- An entirely uncolored or blank rendered diff is a defect unless FLTK drawing is proven impossible during build verification.
- Adaptive folding: show all lines when the op count does not exceed `display_full_context_max_lines`; beyond that, display context lines within `unified_context_radius` with a `... N unchanged lines ...` marker.
- Change navigation: Prev/Next step through maximal runs of adjacent change rows (delete/insert/replace), wrapping around at both ends. The active region is marked with a soft `Selection` strip on the canvas's left edge, and the status bar reads `Change N of M.` while navigating. The position resets when a new diff is produced or the inputs are cleared.
- All diff thresholds/ratios are configurable with defaults.

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
| Replace bg/text | `#F5F1E0` / `#443E30` | `#373426` / `#E6DDC5` |
| Inline insert/delete bg | `#C4E7CF` / `#EFC7BB` | `#31573B` / `#5B3026` |
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

## States

| State | Diff pane | Status |
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
| Pin enabled | Preserve current diff | `Pinned above other windows.` |
| Pin disabled | Preserve current diff | `Pin cleared. Some window managers keep native topmost until refocus.` |
| Prev/Next navigation | Preserve current diff | `Change N of M.` |
| Theme cycled | Re-color whole window | `Theme: System/Light/Dark.` |

## Accessibility Basics

- Tab order is left editor, right editor, action buttons left-to-right, diff display.
- Status bar is not focusable.
- Buttons use text labels, not icon-only controls.
- Visible keyboard focus is required.
- Keyboard shortcuts are documented in README.
- Diff meaning is not color-only because visible row markers remain present even without color.
- Pin is a text button rather than a custom titlebar control; native minimize, maximize, close, drag, and resize remain owned by the OS window manager.

## Not In Scope

- File or directory comparison.
- Clipboard watcher or automatic clipboard read.
- Background daemon, tray app, or global shortcut listener.
- Side-by-side aligned diff view.
- Theme/font settings UI.
- Public-grade installers or app store packaging.
