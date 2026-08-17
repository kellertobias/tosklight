# ToskLight branding assets

- `ToskLight Control.svg` and `ToskLight Control.png` are the approved application-icon sources
  for ToskLight Control.
- `ToskLight Architect.svg` and `ToskLight Architect.png` are the approved application-icon
  sources for ToskLight Architect.
- `ToskLight Pixel.svg` and `ToskLight Pixel.png` are the approved application-icon sources for
  ToskLight Pixel.
- `tosklight-app.icon` is the historical editable Apple Icon Composer source for the shared
  ToskLight artwork.
- `tosklight-app-icon.svg` and `.png` are the legacy shared application tile. They are retained as
  source history, not used as a product application icon.
- `tosklight-icon-print.svg` is the full application icon without the outer shadow and illuminated-edge overlays.
- `tosklight-mark.svg` is the standalone transparent mark without those shadow effects.
- `tosklight-mark.png` is a high-resolution transparent raster export of the clean mark.
- `tosklight-mark-shadow.svg` is the standalone transparent mark with the approved shadow effects.
- `tosklight-mark-shadow.png` is the high-resolution transparent raster export intended for the application and documentation.

To import approved source artwork and derive the four SVG variants:

```sh
python3 tools/import_tosklight_brand.py /path/to/approved-artwork.svg
```

The platform-specific macOS, Windows, PNG, iOS, and Android Control assets are generated with the
Tauri icon generator:

```sh
cd apps/light-desktop
npm run tauri -- icon "../../assets/branding/ToskLight Control.png"
cp "../../assets/branding/ToskLight Control.svg" src-tauri/icons/icon.svg
```

The shadowed standalone mark is used in the application UI. The full square icon remains the desktop bundle icon.

## Hardware Controls

- `tosklight-hardware-controls-icon.png` is the high-resolution transparent application tile.
- `tosklight-hardware-controls-icon-print.svg` is the print-ready application icon without raster effects.
- `tosklight-hardware-controls-mark.svg` is the standalone transparent vector mark.
- `tosklight-hardware-controls-mark.png` is the high-resolution transparent raster mark.

The platform-specific Hardware Controls assets are generated from
`assets/branding/tosklight-hardware-controls-icon.png` with the Tauri icon generator.

## Architect

Architect owns the CAD/rig editor and the standalone visualizer. Both surfaces use the same
approved Architect icon. The editor owns the generated Tauri platform set; the renderer embeds its
`128x128.png` for the window and corner mark, while the macOS renderer bundle uses its `icon.icns`.

Regenerate the platform assets from the approved source:

```sh
npm run --prefix apps/viz-editor tauri -- icon "../../assets/branding/ToskLight Architect.png"
cp "assets/branding/ToskLight Architect.svg" apps/viz-editor/src-tauri/icons/icon.svg
```

The old `tosklight-viz-badge.svg` and `tosklight-viz-icon.*` files are retained only as superseded
artwork and must not be used for product branding.

## Pixel

Pixel is a browser-administered media server rather than a Tauri application. Its approved SVG is
used in the operator web surface and as its favicon. Its PNG is embedded in output windows and the
standby picture, and is the source for the generated macOS bundle icon.

The old `tosklight-media-badge.svg` and `tosklight-media-icon.*` files are retained only as
superseded artwork and must not be used for product branding. `npm run test:app-icons` holds the
Control, Architect, Pixel, renderer, manual, and public-site uses together.
