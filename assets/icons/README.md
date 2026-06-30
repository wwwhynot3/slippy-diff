# Slippy Icons

`slippy.svg` is the editable source icon. PNG files in `png/` are generated from that SVG for launchers, packages, and stores that require raster icon sizes. `slippy.ico` is the Windows packaging derivative used for embedding the app icon into release `.exe` files.

Regenerate the PNG exports with:

```bash
for size in 16 32 48 64 128 256 512; do
  rsvg-convert -w "$size" -h "$size" assets/icons/slippy.svg \
    -o "assets/icons/png/slippy-${size}.png"
done
```

The FLTK window icon is embedded directly from `slippy.svg` at compile time.
