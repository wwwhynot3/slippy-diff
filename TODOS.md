# Slippy TODOs

Explicitly out-of-scope / deferred items. v1 functionality is complete; nothing here blocks normal use.

## Later UI

- Add a settings UI for theme and font selection after config-only overrides prove useful.
- Consider a side-by-side aligned diff view after unified diff is stable in daily use.
- Consider inline word diff only if line-level unified diff is not enough in real use.
- Consider optional always-on-top documentation per platform, but avoid built-in compositor-specific behavior in v1.
- Consider compact button labels if real small-window use shows action bar crowding.

## Later Platform

- Add Linux `.desktop` documentation for KDE shortcut setup.
- Add Windows build notes after a local Windows source build works.
- Add macOS build notes after a local macOS source build works.
- Add CI release artifacts only after the tool becomes part of daily use.
- Add public installers or package-manager distribution only if the tool grows beyond personal use.

## Later Diff Features

- Explore loading clipboard into both panes with explicit buttons only, not a watcher.
- Explore patch apply only if Slippy grows beyond inspection into editing.
- Explore merge/conflict support only if there is a concrete workflow that needs it.
- Explore syntax highlighting only if code diff readability becomes a real pain.

## Later DX

- Add screenshots to README (light + dark themes) — the UI exists, just needs capturing.
- Add platform-specific troubleshooting only after failures are observed on that platform.
- Add release checklist only after public packaging becomes scope.
- Add a LICENSE once the project's distribution story is decided.
