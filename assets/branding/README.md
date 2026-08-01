# ToskLight branding assets

- `tosklight-app.icon` is the editable Apple Icon Composer source for the ToskLight application icon.
- `tosklight-app-icon.svg` is the full application icon artwork.
- `tosklight-app-icon.png` is its flattened 1024 px export used to generate the cross-platform application icon set.
- `tosklight-icon-print.svg` is the full application icon without the outer shadow and illuminated-edge overlays.
- `tosklight-mark.svg` is the standalone transparent mark without those shadow effects.
- `tosklight-mark.png` is a high-resolution transparent raster export of the clean mark.
- `tosklight-mark-shadow.svg` is the standalone transparent mark with the approved shadow effects.
- `tosklight-mark-shadow.png` is the high-resolution transparent raster export intended for the application and documentation.

To import approved source artwork and derive the four SVG variants:

```sh
python3 tools/import_tosklight_brand.py /path/to/approved-artwork.svg
```

The platform-specific macOS, Windows, PNG, iOS, and Android application assets are generated from `assets/branding/tosklight-app-icon.png` with the Tauri icon generator:

```sh
cd apps/light-desktop
npm run tauri icon ../../assets/branding/tosklight-app-icon.png
```

The shadowed standalone mark is used in the application UI. The full square icon remains the desktop bundle icon.

## Hardware Controls

- `tosklight-hardware-controls-icon.png` is the high-resolution transparent application tile.
- `tosklight-hardware-controls-icon-print.svg` is the print-ready application icon without raster effects.
- `tosklight-hardware-controls-mark.svg` is the standalone transparent vector mark.
- `tosklight-hardware-controls-mark.png` is the high-resolution transparent raster mark.

The platform-specific Hardware Controls assets are generated from
`assets/branding/tosklight-hardware-controls-icon.png` with the Tauri icon generator.

## Viz

The Viz products show the same desk in three dimensions, so they do not get separate artwork. Their
icon is the ToskLight application icon with a glowing "3D" in the bottom-right corner, drawn in the
same neon-tube language as the mark.

- `tosklight-viz-badge.svg` is the editable "3D" overlay on its own 1024 px canvas. Its letters are
  round-capped strokes rather than type, so the badge needs no font to reproduce.
- `tosklight-viz-icon.svg` is the composed application icon. It is generated; edit the badge or the
  ToskLight icon instead.
- `tosklight-viz-icon.png` is its flattened 1024 px export.

Recompose the icon after changing either source, then regenerate the platform assets:

```sh
python3 tools/generate_viz_icon.py
npm run --prefix apps/viz-editor tauri icon ../../assets/branding/tosklight-viz-icon.png
```

The generator needs `rsvg-convert` (`brew install librsvg`), which is what produced the committed
ToskLight export as well.

The Viz Editor is the Tauri application that owns this icon set. The visualizer is not a Tauri
application, so it has no generator of its own: it embeds the editor's `128x128.png` for its window
and corner mark, and `tools/bundle-visualizer-macos.sh` puts the editor's `icon.icns` into the
macOS bundle it assembles. `npm run test:app-icons` holds those three uses together.
