# Pixel manual implementation references

The Pixel operator chapters are under `docs/help/30-ToskLight-Pixel`. This file records source anchors and developer details needed when maintaining those chapters.

## Development launch

`npm run open:media` opens the latest development build and seeds its configuration once. `npm run build:media:open` rebuilds before opening. Both use the repository runtime artifact directory; packaged applications use platform application data.

## Source owners

| Operator behavior | Source |
| --- | --- |
| Monitor moves live; other picture settings require restart | `apps/media/src/features/settings/OutputSettings.tsx` |
| Network, picture, sound and library Settings tabs | `apps/media/src/features/settings/SettingsPage.tsx` |
| Audio device, live gains and microphone permission actions | `apps/media/src/features/audio/AudioPage.tsx` |
| Takeover, layer tabs and visualizer tuning | `apps/media/src/features/layers/MediaPanePage.tsx` |
| Four effect-bank parameter controls | `apps/media/src/features/layers/layerDmxSections.ts` |
| Beat Slice Grid typography limits and fields | `apps/media/src/features/effects/SliceGridTypographyEditor.tsx` |
| Effect presets and seeded slot assignments | `crates/media/domain/src/effect_library.rs` |
| Pixel discovery and suggested patches in Control | `apps/light-desktop/src/components/setup/MediaServerSetup.tsx`, `DiscoveredMediaOutputCard.tsx` |

## Protocol detail

Each Layer block has 59 slots, including two effect banks at slots 29-32 and 40-47 and Visualizer Parameter channels at 52-55. A Master block has 40 slots. The two-layer personality occupies 158 slots; eight layers plus Master occupy 512.

MagicQ downloads include native `.hed` personalities and generated channel/range `.csv` sources. The CSV files support maintaining the attribute mapping and indexed Media Folder/File encoders; operators install the HED files.

The model library stores imported models in `.models` under the media-library root. Slot assignments live in Pixel's configuration. Master scale X/Y use raw value 40960 for 1×. A selected data-folder override is stored in `media-server-location.json` beside the original configuration.

## Source discrepancy to resolve

`MediaPanePage.tsx` still exposes a legacy Playback **Blur** value, while `DmxPage.tsx` instructs operators to use a Blur preset in an effect bank. The operator manual documents bank-based Blur and its current DMX layout. Removing the redundant browser control requires a separate implementation change.

Text Tracking credits ff-tracking (`https://github.com/mrsarac/ff-tracking`) as visual inspiration. Black Hole is an artistic approximation inspired by Eric Bruneton's imagery, with original procedural graphics and no upstream code or assets.
