# ToskLight branding assets

- `tosklight-app.icon` is the editable Apple Icon Composer source for the ToskLight application icon.
- `tosklight-app-icon.png` is the flattened 1024 px default Mac export used to generate the cross-platform application icon set.
- `tosklight-icon-print.svg` is the full print-ready application icon without blur or raster effects.
- `tosklight-mark.svg` is the clean standalone vector mark for dark backgrounds and print layouts.
- `tosklight-mark.png` is a high-resolution transparent raster export of the clean mark.
- `tosklight-mark-shadow.svg` is the standalone mark with a controlled presentation shadow.
- `tosklight-mark-shadow.png` is the high-resolution transparent raster export intended for the application and documentation.

The platform-specific macOS, Windows, PNG, iOS, and Android application assets are generated from `assets/branding/tosklight-app-icon.png` with the Tauri icon generator:

```sh
cd apps/control-ui
npm run tauri icon ../../assets/branding/tosklight-app-icon.png
```

The SVG assets remain the editable print and in-app mark variants; they are not the desktop application-icon source.

## Hardware Controls

- `tosklight-hardware-controls-icon.png` is the high-resolution transparent application tile.
- `tosklight-hardware-controls-icon-print.svg` is the print-ready application icon without raster effects.
- `tosklight-hardware-controls-mark.svg` is the standalone transparent vector mark.
- `tosklight-hardware-controls-mark.png` is the high-resolution transparent raster mark.

The platform-specific Hardware Controls assets are generated from
`assets/branding/tosklight-hardware-controls-icon.png` with the Tauri icon generator.
