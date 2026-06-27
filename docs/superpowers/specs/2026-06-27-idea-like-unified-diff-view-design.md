# IDEA-like Unified Diff View Design

## Goal

Redesign Slippy's diff output area so it feels closer to IntelliJ IDEA's diff viewer without turning the app into a full code review workspace.

The chosen direction is a unified diff viewer with IDE-style reading aids. It keeps one result pane and standard unified diff copy semantics, but improves the rendered view with semantic old/new gutters, stronger replacement grouping, inline token highlights, and a right-side change overview rail.

## Context

The current UI renders the diff into a single FLTK `TextDisplay` with one text buffer and one style buffer. That keeps implementation simple, but the result still reads like a colored text box rather than an IDE diff viewer.

The redesign should not be constrained by that current rendering shape. The final implementation may replace the single output buffer with a richer structured renderer if needed.

Existing constraints to keep:

- Slippy remains paste-first: left input, right input, compare, inspect, copy diff.
- `Copy Diff` continues to emit standard unified diff text with `---`, `+++`, and `@@` hunks.
- The app stays native Rust/FLTK and compact.
- No fake file tree, review workspace, or directory comparison.

## Chosen Direction

Use a single unified review pane, not a true side-by-side split pane.

The visual design combines:

- B-style unified viewer as the base: one result area, compact IDE toolbar, old/new line references, standard patch semantics.
- A-style strengths: change overview rail, paired replacement grouping, stronger visual chunking, and navigation affordances.
- B+C color semantics: soft row color for pure insert/delete rows, neutral replacement blocks for paired edits, and strong inline token highlights.

This is intentionally not a full IntelliJ clone. It borrows the parts that improve readability inside Slippy's paste workflow.

## Layout

The main app layout remains:

```text
[ Left Input ][ Right Input ]
[ Paste Left | Paste Right | Compare | Swap | Clear | Copy Diff ]
[ Unified Review Diff View | Change Overview Rail ]
[ Status Bar ]
```

The diff output area is replaced by an IDE-like unified review view:

```text
[ Unified Review | Prev | Next | Fit | 2 removed | 3 added | 2 edited ]
[ old line ][ new line ][ kind ][ rendered diff row                  ][ rail ]
[    18    ][    18    ][      ][ context                            ][  |   ]
[    19    ][          ][  -   ][ old-only row                       ][ red  ]
[          ][    19    ][  +   ][ new-only row                       ][ green]
[    20    ][          ][  ~   ][ OLD replacement token row          ][ mark ]
[          ][    20    ][  ~   ][ NEW replacement token row          ][ mark ]
```

The toolbar is part of the diff view, not the global action bar. It should be compact and quiet.

Toolbar contents:

- Mode label: `Unified Review`.
- Previous change and next change controls.
- Optional fit/center control if useful in FLTK.
- Change summary: removed, added, edited counts.

## Semantic Gutters

The two line-number gutters are semantic old/new references, not duplicated display counters.

Rules:

- Context rows show both old and new line numbers.
- Deleted rows show an old line number and leave the new line number blank.
- Inserted rows leave the old line number blank and show a new line number.
- Replacement pairs render as two adjacent rows:
  - old replacement row: old line number present, new line number blank.
  - new replacement row: old line number blank, new line number present.
- After insertions or deletions, later context rows may show different old/new numbers. That offset is the point of keeping both gutters.
- Fold rows leave both gutters blank unless a concise range label is feasible.

This must be treated as a correctness requirement for the rendered view.

## Row Kinds

The renderer should produce display rows with explicit kinds:

- `Context`: unchanged line.
- `Delete`: old-only line.
- `Insert`: new-only line.
- `ReplaceOld`: old side of a paired replacement.
- `ReplaceNew`: new side of a paired replacement.
- `Fold`: collapsed unchanged range.
- `Notice`: stale diff or empty state message.
- `Hunk`: optional hunk separator/header row.

Each row carries:

- old line number: optional.
- new line number: optional.
- kind marker: blank, `-`, `+`, `~`, or notice/fold marker.
- rendered text segments.
- row group id for paired replacement rows when available.

## Red/Green Rendering

Use color in layers. Do not make red/green the only source of meaning.

### Pure Insert/Delete Rows

Pure inserted and deleted rows use B-style rendering:

- row background is lightly tinted red or green.
- changed text may use a stronger token background when token spans are available.
- `-` and `+` markers remain visible.
- old/new gutters still encode the side.

This keeps classic diff scanability.

### Replacement Pairs

Paired inline changes use C-style rendering:

- render the pair as a neutral replacement block instead of two loud red/green rows.
- use a shared left accent line or group marker so the old and new rows read as one replacement group.
- mark both replacement rows with `~` in the marker column.
- rely on the semantic old/new gutters to show which replacement row came from which side.
- use red token highlights only for removed fragments.
- use green token highlights only for inserted fragments.

The intent is to make replacement blocks read as "this line changed" before they read as two unrelated delete/insert operations.

### Accessibility

Meaning must remain available without color:

- kind marker column: `-`, `+`, `~`.
- semantic old/new line-number gutters.
- visible group marker for replacement pairs.
- unified diff prefixes remain available in `Copy Diff`.

## Change Overview Rail

Add a narrow right-side overview rail inspired by IDE scrollbars.

The rail shows approximate positions of changes:

- red marks for delete-heavy regions.
- green marks for insert-heavy regions.
- neutral/blue/amber marks for paired replacements if supported by the palette.

Initial behavior may be passive only. It does not need to support clicking in the first implementation unless FLTK makes that cheap.

The rail should remain narrow, about 10-16 px, and must not dominate the paste-diff workflow.

## Folding

Keep adaptive folding.

Fold rows should be rendered as calm separators:

```text
... 42 unchanged lines ...
```

Use ASCII in source strings unless the surrounding file already justifies Unicode. The copied unified diff may keep standard textual hunk output from `diff_core`.

Fold rows leave old/new gutters blank in the first pass. If later useful, a folded range can be shown in muted text.

## Empty, Stale, and Status States

Empty/equal state:

- Render `No differences` in the diff view as a centered or first-row notice.
- Keep status text `No differences.`

Stale state:

- Keep the previous diff visible.
- Add a muted notice row above the rendered diff:
  - `Previous diff is stale. Press Compare to update.`
- The notice row should not break old/new line-number accounting for the diff rows below.

Large input auto-skip:

- Preserve current behavior: status tells the user to press Compare.
- If there is an old result, stale notice appears above it.

## Implementation Boundaries

The likely implementation should introduce a small rendering model between `DisplayDiff` and FLTK widgets.

Suggested internal types:

- `DiffViewRow`
- `DiffViewRowKind`
- `DiffTextSegment`
- `RenderedDiffView`

This model should be pure and unit-tested separately from FLTK.

`diff_core` should continue owning diff computation and standard unified diff serialization. UI rendering should not mutate diff semantics.

`ui_fltk` should own:

- converting `RenderedDiffView` to FLTK display buffers or custom drawing data.
- applying palette styles.
- wiring toolbar buttons and overview rail.

## Testing

Unit tests should cover the pure display-row model:

- equal text produces a no-differences notice.
- delete row has old line number and blank new line number.
- insert row has blank old line number and new line number.
- context after insertion/deletion shows offset old/new numbers.
- paired inline change produces adjacent `ReplaceOld` and `ReplaceNew` rows with the same group id.
- pure insert/delete rows are classified separately from replacement pairs.
- fold rows do not consume line numbers incorrectly.
- stale notice does not alter row line numbering.

Manual GUI verification should cover:

- old/new gutters are visibly different on insertions and deletions.
- replacement blocks read as grouped edits, not unrelated red/green noise.
- pure add/delete lines remain easy to scan.
- overview rail appears and does not crowd the diff.
- Copy Diff still outputs standard unified diff text.
- small window width remains usable.
- light and dark themes keep enough contrast for row backgrounds and token highlights.

## Non-goals

- Side-by-side aligned diff as the primary display.
- File tree, file tabs, directory compare, or review workspace.
- Editing text inside the diff result.
- Applying/reverting hunks.
- Clickable overview rail in the first implementation.
- Changing the `Copy Diff` format.
