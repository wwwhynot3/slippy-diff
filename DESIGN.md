# Slippy Design

## Direction

Slippy is a quiet, trustworthy native desktop utility for comparing two pasted text snippets. It should feel like a scratchpad: open it, paste left and right text, inspect the diff, copy the result if needed, close it, and leave no background process behind.

The design should stay utilitarian and compact. Avoid decorative chrome, marketing-style visuals, dashboard cards, icon-only controls, and custom visual complexity that does not improve the paste-diff-copy workflow.

## Layout

Primary layout:

```text
[ Left Input 50% ][ Right Input 50% ]
[ Paste Left | Paste Right | Compare | Swap | Clear | Copy Diff ]
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

Use Cmd on macOS where FLTK supports it and Ctrl elsewhere. If FLTK cannot map Cmd cleanly, keep Ctrl working and document the limitation.

## Visual System

Use the system UI font for controls and the system monospace font for editors and diff output where the toolkit exposes them. Allow config-only overrides with `ui_font` and `mono_font`, but do not build a settings UI in v1.

Theme support:

- `System` is the default.
- `Light` and `Dark` can be selected through config.
- No toolbar or menu theme switcher in v1.

Line-level diff coloring is required:

- The diff result uses a unified review layout with old/new line-number gutters, a marker column, rendered text, and a narrow change overview rail.
- `+` insertion rows use soft insert colors; `-` deletion rows use soft delete colors.
- Input panes show line numbers.
- Replacement blocks pair similar deleted and inserted lines before rendering; paired rows use a neutral block background with `~` markers and stronger red/green token highlights for the exact changed fragments.
- The old/new gutters are semantic references: inserted rows leave the old line blank, deleted rows leave the new line blank, and later context rows may show offset line numbers.
- The rendered review header shows `OLD  NEW  K | Text` above the diff rows, with a separator beneath it.
- `---` and `+++` header lines are part of the plain unified diff text emitted by Copy Diff.
- Preserve text prefixes so the diff remains understandable even if styling fails.
- An entirely uncolored diff is a defect unless FLTK styling is proven impossible during build verification.
- Adaptive folding: show all lines when the op count does not exceed `display_full_context_max_lines`; beyond that, display context lines within `unified_context_radius` with a `... N unchanged lines ...` marker.
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

## Accessibility Basics

- Tab order is left editor, right editor, action buttons left-to-right, diff display.
- Status bar is not focusable.
- Buttons use text labels, not icon-only controls.
- Visible keyboard focus is required.
- Keyboard shortcuts are documented in README.
- Diff meaning is not color-only because visible row markers remain present even without color.

## Not In Scope

- File or directory comparison.
- Clipboard watcher or automatic clipboard read.
- Background daemon, tray app, or global shortcut listener.
- Built-in always-on-top behavior.
- Side-by-side aligned diff view.
- Theme/font settings UI.
- Public-grade installers or app store packaging.
