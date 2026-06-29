# Character-Level Diff Copy Design

## Context

Slippy's Diff Text pane is a custom FLTK `Frame` canvas. It currently supports row-range selection only: mouse drag stores an inclusive row range, `Ctrl+C` copies `RenderedDiffView::selection_text(a, b)`, and drawing highlights selected rows as full-row bands.

Users need to copy exact substrings from the rendered diff text. The feature should preserve the existing row selection behavior and add precise character-range selection inside the text column.

## Goals

- Allow selecting and copying characters from the Diff Text text column.
- Support single-line and multi-line character selections.
- Preserve current row selection and row copy behavior for non-text-column drags.
- Keep the implementation inside the existing custom canvas path.
- Add tests for selection text extraction and hit-test math before changing production behavior.

## Non-Goals

- Replace the custom diff canvas with `TextDisplay` or another widget.
- Change the diff algorithm or inline diff segmentation.
- Add rich text clipboard formats.
- Implement editing inside the diff pane.

## Approach

Add a second selection model for character ranges, separate from the existing row selection:

- Row selection remains `Option<(usize, usize)>` and continues to copy whole rendered rows.
- Character selection stores anchor and focus positions as `(row_index, char_index)`.
- Copy chooses character selection first when present, then falls back to row selection.
- Starting a text-column drag clears row selection and creates character selection.
- Starting a gutter/row-background drag clears character selection and uses row selection.

The rendered row text remains the canonical copy source. Character indices are Unicode scalar indices over the row's rendered text, not byte offsets. This avoids invalid UTF-8 slicing while keeping behavior predictable for non-ASCII content. Hit testing maps mouse x positions to character indices using the existing monospace diff font metrics.

## UI Behavior

- Drag in the text column to select characters.
- Drag across rows selects from the anchor character to the focus character, including full middle rows.
- Copying a single-line selection copies only the selected substring.
- Copying a multi-line selection copies the suffix of the first row, all full middle rows, and the prefix of the final row, joined with `\n`.
- Empty selections do not replace the clipboard and should behave like no selection.
- Existing full-row selection remains available from the non-text portion of the diff canvas.

## Components

### `src/diff_view.rs`

Add reusable data types and pure text extraction:

- `DiffCharPosition { row: usize, char_index: usize }`
- `DiffCharSelection { anchor: DiffCharPosition, focus: DiffCharPosition }`
- `RenderedDiffView::char_selection_text(selection) -> String`

This keeps Unicode-safe slicing and multi-line behavior testable without FLTK.

### `src/ui_fltk.rs`

Add UI state and hit testing:

- Store `char_selection: Rc<Cell<Option<DiffCharSelection>>>` in `UiHandles`.
- Convert diff canvas mouse coordinates to row and character indices.
- Update the canvas event handler to create or extend character selection when dragging in the text column.
- Draw character selection rectangles over the selected text spans.
- Update copy handling to prefer `char_selection_text` before row `selection_text`.

## Error Handling

- Out-of-range rows are ignored during extraction.
- Character indices beyond a row's length clamp to the row length.
- If no valid selected text exists, copy returns without changing status or clipboard.
- Clipboard failure handling remains unchanged.

## Testing

Add unit tests before implementation:

- `char_selection_text` returns a substring for a single row.
- `char_selection_text` supports reverse drag order.
- `char_selection_text` returns suffix/full-middle/prefix text for a multi-line selection.
- `char_selection_text` clamps out-of-range character indices.
- UI hit-test helpers map text-column x positions to character indices.
- Copy helper prefers character selection over row selection.

Manual verification after automated checks:

- Run the app.
- Compare two texts.
- Drag-select characters in Diff Text and copy.
- Paste into another field to confirm the exact substring.
- Drag-select outside the text column and confirm whole-row copy still works.

## Risks

- FLTK font measurement may differ slightly from drawn glyph widths. The implementation should use the same font and size constants used by row drawing.
- Inline diff segments split styling but not copy text. Extraction should operate on concatenated row text to avoid segment-boundary bugs.
- Multi-byte characters need scalar-index slicing, not byte slicing.

## Alternatives Considered

1. Replace the custom canvas with a native text display widget. This would provide selection for free but would lose custom inline diff styling and likely require a larger UI rewrite.
2. Keep row selection only and add a context menu for copying tokens. This is lower effort but does not solve arbitrary character selection.
3. Add character selection to the existing canvas. This is the recommended path because it is scoped, preserves current behavior, and isolates most logic in pure testable helpers.

## Self-Review

- No placeholders remain.
- The design keeps the existing row selection behavior explicit.
- The feature is scoped to Diff Text copy selection only.
- The test plan covers pure extraction, hit testing, copy precedence, and manual UI verification.
