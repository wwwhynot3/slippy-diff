# Custom Diff Canvas And Pin Design

## Goal

Make the diff result area feel closer to an IDE diff viewer by replacing the styled text output with a drawn row canvas, adding a topmost-window pin toggle, and replacing the input editors' built-in line numbers with matching drawn gutters.

## Scope

This is a UI-layer refactor only. The existing diff algorithm and `diff_view.rs` semantic model remain the source of truth. The system title bar remains native; minimize, maximize, close, dragging, resizing, and platform window integration stay with the operating system.

## Diff Canvas

The diff result area will render `RenderedDiffView` rows in a custom FLTK drawing surface inside a scroll container. Each visible row draws its own old line gutter, new line gutter, marker gutter, content area, row background, and inline token highlights.

Rows keep the semantics from `DiffViewRowKind`:

- context rows use the neutral pane background
- delete rows use the delete background and `-` marker
- insert rows use the insert background and `+` marker
- replacement rows use a neutral block background with inline red/green token highlights and `~` marker
- fold and notice rows use muted text

The overview rail continues to show compact change markers derived from `RenderedDiffView::marks`.

## Input Gutters

The left and right input editors remain FLTK `TextEditor` widgets for IME, selection, clipboard, undo, cursor movement, and editing behavior. Their built-in line number columns are disabled. Each editor is wrapped with a narrow drawn gutter using the same gutter palette and typography as the diff canvas.

The gutter tracks the editor's maintained absolute top line number and visible height. It redraws after buffer changes and editor input/scroll events.

## Pin Toggle

The diff toolbar gets a `Pin` toggle. When enabled, it calls FLTK's `Window::set_on_top()` after the window is shown and changes the button label to `Pinned`. FLTK exposes a one-way topmost call but not a symmetric unset call across all platforms, so disabling the toggle clears the app-level button state and status text but may not demote the native window on all window managers.

This limitation is explicit in code comments and user-facing status text. We do not use modal windows as a workaround because modal state would change focus behavior and block other windows.

## Verification

Automated tests cover helper behavior for line labels, layout sizing, pin labels, overview rail slots, and the retained copy-diff path. Visual verification runs the FLTK app, loads sample text, compares it, and captures a screenshot showing custom gutters, custom diff rows, overview rail, and the pin control.
