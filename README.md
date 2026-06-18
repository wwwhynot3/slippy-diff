# Slippy

Slippy is a lightweight desktop text diff tool for comparing two pasted text snippets.
It is built for a paste-first workflow: paste left, paste right, compare, copy the unified diff, close the app.

## Current Scope

- Native desktop GUI using Rust and `fltk-rs`.
- Two editable input panes.
- Action bar between inputs and diff output.
- Input and diff panes with line numbers.
- Read-only unified diff output with line-level coloring and inline display for reliably matched replacements.
- Clipboard paste/copy buttons.
- Debounced auto-diff for normal-sized edits.
- Manual Compare for large input.
- No background daemon, tray app, clipboard watcher, or global shortcut listener.
- No file or directory comparison in v1.

## Run

```bash
cargo run
```

## Test

```bash
cargo test
```

## Wayland Build

The default build uses bundled FLTK. Optional Wayland support is exposed as:

```bash
cargo run --features wayland
```

If the active `fltk-rs` release changes the Wayland feature name, update only the feature mapping in `Cargo.toml`.

## Shortcuts

- `Ctrl/Cmd+Enter`: Compare
- `Ctrl/Cmd+L`: Paste Left
- `Ctrl/Cmd+R`: Paste Right
- `Ctrl/Cmd+Shift+S`: Swap
- `Ctrl/Cmd+Shift+C`: Copy Diff

FLTK maps `Cmd` on macOS and `Ctrl` on Linux/Windows through `Shortcut::Command`.

## Config

Slippy persists only UI metadata such as window size, split/theme/font fields, and never persists pasted input or diff output.

Config is stored through the OS config directory using the app identity `dev.wwwhynot3.slippy` / `Slippy`.
