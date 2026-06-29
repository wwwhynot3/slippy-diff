# Slippy

Slippy is a lightweight, native desktop text-diff tool for comparing two pasted snippets. It is built around a paste-first workflow: paste left, paste right, compare, copy the unified diff, close — and leave no background process behind.

It aims for a quiet, trustworthy scratchpad feel: no daemons, no tray icon, no clipboard watcher, no file picker. Just two text panes and a diff.

Built with Rust and [`fltk-rs`](https://github.com/fltk-rs/fltk-rs).

## Features

- **Two editable input panes** (left / right) with custom line-number gutters.
- **Compact action bar**: Paste Left · Paste Right · Compare · Swap · Clear · Copy Diff.
- **Read-only unified review diff pane** with a custom-drawn IntelliJ-inspired canvas:
  - Semantic old/new line-number gutters, so inserted rows have no old line number and deleted rows have no new line number.
  - Soft row coloring for pure insertions/deletions.
  - Neutral replacement blocks for paired edits, with red/green token highlights for the exact changed fragments.
  - A compact change overview rail showing where edits occur in the rendered diff.
  - Adaptive folding: large diffs collapse runs of unchanged context into a `... N unchanged lines ...` marker instead of scrolling forever.
- **Copy Diff** copies standard unified diff text (with `@@` hunks and `---`/`+++` headers) from the same computed diff the display derives from.
- **Pin toggle** keeps the window above other windows where the native window manager supports FLTK's topmost request.
- **Debounced auto-diff** (300 ms) for normal-sized edits.
- **Manual Compare** for large input — combined size above 256 KiB or 8,000 lines skips auto-diff and asks you to compare explicitly.
- **Themes**: System / Light / Dark, switchable live via the Theme toolbar button (`Ctrl/Cmd+Shift+T`) or config.
- **Status bar** reflecting the current state (ready, pending, running, updated, large-input notice, paste/copy failures).
- **Privacy**: only UI metadata (window size, split, theme, fonts) is persisted. Pasted text and diff output are **never** stored to disk.

## What v1 is not

- No file or directory comparison — pasted text only.
- No clipboard watcher or automatic clipboard read.
- No background daemon, tray app, or global shortcut listener.
- No side-by-side aligned diff view.
- No theme/font settings UI (themes and fonts are config-only).
- No public installers or app-store packaging.

## Prerequisites

Slippy bundles FLTK, but `fltk-rs` still needs a native C/C++ toolchain plus the usual X11 / OpenGL / Pango development libraries to build it.

**Arch / CachyOS / Manjaro (pacman):**
```bash
sudo pacman -S --needed base-devel cmake git pkgconf \
  libx11 libxext libxft libxinerama libxcursor libxfixes libxrender \
  mesa glu pango fontconfig glib2 alsa-lib
```

**Debian / Ubuntu (apt):**
```bash
sudo apt install build-essential cmake git pkg-config \
  libx11-dev libxext-dev libxft-dev libxinerama-dev libxcursor-dev libxfixes-dev libxrender-dev \
  libgl1-mesa-dev libglu1-mesa-dev libpango1.0-dev libfontconfig1-dev libglib2.0-dev libasound2-dev
```

**Fedora (dnf):**
```bash
sudo dnf install gcc gcc-c++ cmake git pkgconfig \
  libX11-devel libXext-devel libXft-devel libXinerama-devel libXcursor-devel libXfixes-devel libXrender-devel \
  mesa-libGL-devel mesa-libGLU-devel pango-devel fontconfig-devel glib2-devel alsa-lib-devel
```

**macOS:** `xcode-select --install` for the C/C++ toolchain, then `brew install cmake` (FLTK is built from source via CMake).
**Windows:** MSVC build tools (Visual Studio Build Tools) plus Git.

Detailed Windows/macOS build docs and installers are **not** part of v1 scope. For the complete list, see the [`fltk-rs` dependency docs](https://github.com/fltk-rs/fltk-rs#dependencies).

## Build & run

```bash
cargo run                 # run the app (default, bundled FLTK)
cargo test                # run all tests
cargo test diff_core      # run one module's tests
cargo build --release     # release build
```

**Optional Wayland build:**
```bash
cargo run --features wayland
```
Wayland additionally needs `wayland`, `wayland-protocols`, and `libxkbcommon` (or the matching `-devel` packages). If an `fltk-rs` release changes the Wayland feature spelling, update only the feature mapping in `Cargo.toml` (`wayland = ["fltk/use-wayland"]`) — no code changes needed.

## Troubleshooting (`fltk-sys` build errors)

`fltk-sys` compiles FLTK from source, so most first-time failures are missing native libraries:

- `cmake: command not found` → install **cmake**.
- `error: could not find X11` / missing `X11/Xlib.h` → install the **libx11** dev package.
- Linker errors mentioning `GL` / `GLU` → install **mesa** / OpenGL dev packages.
- `pkg-config` not found → install **pkg-config** (or **pkgconf**).
- Pango / fontconfig errors → install the **pango** and **fontconfig** dev packages.
- Wayland build fails on a feature-name error → adjust only the Cargo feature mapping (see above).

## Known Issues

- **Black flicker during continuous manual resize on KDE Plasma Wayland:** on at least one KDE Plasma Wayland setup, the window can briefly flash black while being dragged-resized, especially when shrinking. This reproduces with both `cargo run` and `cargo run --features wayland`, and also with the minimal `examples/fltk_resize_probe.rs` empty-window probe across `DoubleWindow`, `SingleWindow`, explicit opaque redraw, `Mode::Rgb8`, and `FLTK_BACKEND=x11/wayland`. That points to the FLTK top-level window / compositor live-resize path rather than Slippy's diff canvas, layout, or resize callback. A real Plasma X11 login session may behave differently; if it does, treat this as a Wayland/XWayland environment limitation.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl/Cmd+Enter` | Compare |
| `Ctrl/Cmd+L` | Paste Left |
| `Ctrl/Cmd+R` | Paste Right |
| `Ctrl/Cmd+Shift+S` | Swap |
| `Ctrl/Cmd+Shift+C` | Copy Diff |
| `Ctrl/Cmd+Shift+P` | Toggle Pin |
| `Ctrl/Cmd+Shift+↑` | Previous change |
| `Ctrl/Cmd+Shift+↓` | Next change |
| `Ctrl/Cmd+Shift+T` | Cycle theme |

FLTK maps `Cmd` on macOS and `Ctrl` on Linux/Windows through `Shortcut::Command`.

## Config & privacy

Slippy stores only UI metadata through the OS config directory (app identity `dev.wwwhynot3.slippy`): window size, the input/diff split (default 0.45, clamped 0.30–0.70), theme, and font choices.

It **never** persists pasted text or diff output. Invalid or missing config falls back to defaults and reports a status message; save errors never crash the app.

## Manual GUI smoke checklist

- App opens and closes with no daemon, tray, or background process.
- Paste Left / Paste Right target the correct pane; keyboard paste still works inside the editors.
- Compare, Swap, Clear, and Copy Diff all work.
- Debounced auto-diff updates shortly after normal edits.
- Rapid edits never let a stale diff overwrite newer text.
- Large input shows the "press Compare" status, and manual Compare updates the diff.
- Insertion, deletion, and inline fragment colors are visible in both light and dark themes.
- The custom input gutters track visible line numbers while editing and scrolling.
- The Pin button changes to `Pinned` and reports topmost status when toggled.
- The action bar sits visually between the inputs and the diff.
- The window stays usable when resized small (input panes stack below 760 px width).
- Idle CPU returns near zero after debounce settles.

## Architecture

Slippy is deliberately layered, and the dependency rules between layers are the most important invariant to preserve:

- **`diff_core`** — pure diff logic (`similar`-based, line classification, inline pairing, auto-diff thresholds). No FLTK, clipboard, config, or threading.
- **`app_state`** — UI-independent state machine (text, dirty/stale flags, monotonic request ids, status transitions). No FLTK or clipboard.
- **`ui_fltk`** — all FLTK widgets, styling/coloring, clipboard, worker threads, debounce timers, and shortcuts.
- **`config`** — persists only layout/theme/font metadata (a privacy invariant, asserted in tests).

Diffing runs on a fresh worker thread per request with **no cancellation** — correctness depends on ignoring stale results via monotonic request ids and a dirty-since-request flag, not on killing workers. `diff_core` and `app_state` are pure and fully unit-tested; `ui_fltk` is the integration boundary.

See [`CLAUDE.md`](CLAUDE.md), [`DESIGN.md`](DESIGN.md), and [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) for the full product direction, behavioral contract, status tables, and test plan.
