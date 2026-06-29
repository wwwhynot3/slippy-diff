# Slippy Logo Design

## Decision

Use the selected Split Slip direction as Slippy's application logo.

## Visual Requirements

- Represent Slippy as a native desktop diff scratchpad, not a generic code editor.
- Keep the mark readable at small icon sizes.
- Use the existing product palette: warm surface, teal primary, red deletion, green insertion, and quiet borders.
- Preserve a simple rounded-square app icon silhouette.
- Encode the app's core workflow with a two-pane divider, a slipping comparison path, and small delete/insert marks.

## Implementation Requirements

- Store one editable SVG source at `assets/icons/slippy.svg`.
- Generate PNG exports at common app icon sizes under `assets/icons/png/`.
- Set the FLTK window icon from the embedded SVG source so runtime launch does not depend on external asset paths.
- Do not add new Rust dependencies for icon loading.
- Keep packaging assets separate from UI behavior.
