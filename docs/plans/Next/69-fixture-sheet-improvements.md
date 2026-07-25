# Fixture Sheet Filtering and Compact Mode

## Status

**Specification only.** This plan records future Fixture Sheet filtering and view-density rules. It does not implement filtering, selection behavior, pane settings, row rendering, value summaries, persistence, responsive layout, show migration, help changes, screenshots, testing scenarios, or executable tests.

## Goal

Keep venue scenery and other non-programmable Stage elements out of the regular Fixture Sheet. The Fixture Sheet is a live programming and output-inspection table, while the Stage and Show Patch remain the correct surfaces for visual-only venue objects.

Let each Fixture Sheet surface also trade visual richness for row density so more fixtures remain visible on smaller screens.

## Filtering rule

The full Fixture Sheet and every Fixture Sheet pane exclude a row when any of these is true:

- its fixture profile uses the `visual_only` patch policy;
- its manufacturer is **Venue**; or
- its fixture ID begins with the reserved `0.` prefix.

The checks are deliberately independent. Imported or legacy scenery must still be hidden when only one signal is present.

The `0.` rule applies only to the beginning of the complete fixture ID. It must not hide a normal multi-head master such as `100.0`, a subhead such as `100.1`, or another fixture merely because its ID contains `.0`.

This is an invariant of the regular Fixture Sheet, not a user-visible filter that can be disabled. Existing Fixture Sheet filters, ordering, Cuelist limits, included-head choices, columns, Group shortcuts, Highlight/step presentation, and Preload comparison operate only on the remaining programmable rows.

## Other surfaces

Hidden objects remain part of the show. They continue to appear and remain editable in:

- Show Patch;
- 2D and 3D Stage;
- Stage-position and scenery workflows;
- MVR and show persistence; and
- any future paperwork or renderer surface that includes scenery.

The filter must not delete them, unselect them globally, renumber them, or suppress their Stage geometry. If the authoritative selection contains only hidden Stage elements, the Fixture Sheet shows no selected row instead of substituting a programmable fixture.

## Compact mode

The setting is named **Compact mode** and has exactly three choices:

- **Off**;
- **Icon only**; and
- **Text only**.

Compact mode changes the presentation of live value cells and row density. It does not change fixture data, programmer values, selection, output, tracking, visible-column choices, or which fixtures are included. The programmable-fixture filter applies before density rendering.

## Settings placement and ownership

**Compact mode** lives in the **View** tab of Fixture Sheet settings everywhere the Fixture Sheet is configured:

- a normal configurable Fixture Sheet pane;
- the full Fixture Sheet built-in; and
- the planned fixed external-screen Fixture Sheet configuration.

It is not placed in a separate global appearance page or hidden in **Columns**. Pane Settings and the full built-in settings use the same labels, choices, renderer, and behavior.

The selected mode belongs to that Fixture Sheet surface's desk-local persisted configuration. Different panes may deliberately use different modes, such as Icon only on a small touch screen and Off on a large external monitor. Existing layouts migrate to **Off**.

## Off

**Off** preserves the normal detailed Fixture Sheet:

- the current standard row height and spacing;
- the intensity meter plus percentage;
- the color swatch plus text label;
- the position glyph plus Pan/Tilt text;
- Beam and Focus summaries with their normal visual and text presentation;
- Preload target visuals and text; and
- configured secondary details such as fixture type.

This remains the default and the reference presentation for existing layouts and screenshots.

## Icon only

**Icon only** reduces row height, cell padding, gaps, and value-column minimum widths. Live value cells retain their graphical summary and remove ordinary value text:

| Column | Icon-only presentation |
|---|---|
| Dimmer | The compact intensity/level bar without the percentage text. |
| Color | The resolved color swatch without the RGB/color text. |
| Position | The position glyph without Pan/Tilt degree text. |
| Beam | A compact semantic Beam glyph or state marker. |
| Focus | A compact semantic Focus/edge/frost/zoom glyph or state marker. |

The fixture **Icon** column remains a separately configurable identity column. Choosing Icon only does not automatically enable that column or hide ID, Name, Patch, or another column selected under **Columns**.

Beam and Focus icons must be derived from authoritative resolved attributes/functions. The implementation must not turn the current reserved placeholder summaries into invented live state merely to fill the compact cells.

Preload uses a clearly distinct secondary or overlaid icon/bar state with the same current-versus-pending meaning as the detailed view. Source ownership, Group-master limiting, selection, Highlight stepping, unavailable values, and errors retain non-text visual distinctions.

Every icon-only value has an accessible name containing its full text value. Pointer hover may show the same value as a tooltip, but hover is supplemental and cannot be the only way a required touch-screen operator obtains the value.

## Text only

**Text only** uses the same reduced row height, padding, gaps, and narrower value columns but removes decorative value graphics. It presents concise authoritative text such as:

- `100%` for Intensity;
- `RGB 0, 0, 0`, a semantic color name, or the normal neutral/unsupported label for Color;
- `50° / 50°` for Pan/Tilt;
- `Open`, `Gobo 2`, or another resolved Beam summary; and
- `Soft edge`, `Focus 45%`, `Frost open`, or another resolved Focus summary.

Text formatting uses the attribute's normal units and semantic names. It must not expose raw DMX merely because the visual icon is absent.

Preload targets remain visible as compact arrow/value text. Source ownership and current/pending state retain a non-color-only distinction without reintroducing the removed gauges and glyphs.

## Identity, status, and row behavior

Compact mode affects value presentation rather than silently changing the column model.

- ID, fixture Icon, Name, Patch, Dimmer, Color, Position, Beam, and Focus remain independently controlled by **Columns**.
- **Show fixture type**, Group-master status, unpatched state, source ownership, selection, remembered Highlight base, current Highlight step, and contained-head markers remain available.
- Secondary name/type or status text may use a compact inline layout, but it cannot disappear merely to achieve a one-line row.
- Rows remain selectable across their full rendered width.
- Multi-head indentation and `.0`, `.1`, and later IDs remain legible.

The two compact modes use one deterministic reduced row height under equivalent content. Preload, multi-head, status, or error state must not make rows jump unpredictably between normal and compact heights during live operation.

## Small-screen layout

Compact modes reduce row height and value-column minimum widths together. They must materially increase the number of visible fixture rows and reduce horizontal pressure at every supported small Fixture Sheet size.

The renderer must not automatically hide a configured column or change Icon only into Text only at a breakpoint. If every selected column still cannot fit after compact sizing, the Fixture Sheet uses its documented horizontal overflow behavior rather than silently dropping information.

Touch selection remains reliable at the deliberate compact target size. Compact rows must not overlap, clip status markers, truncate fixture IDs, or make current versus Preload values indistinguishable.

## Shared rendering

Pane, built-in, fixed external-screen, deterministic screenshots, and test renderers consume the same compact-mode view model. They must not maintain parallel rules for which visual or text fragments are shown.

Changing Compact mode is a view-only desk mutation. It must not:

- write to the portable show;
- modify programmer or Cue data;
- change Fixture Sheet filtering or ordering;
- reset scroll position or selection unnecessarily; or
- alter another Fixture Sheet surface's independently saved mode.

## Documentation and regression coverage

Implementation updates the existing Fixtures and Patch help text that currently says Venue `0.x` objects appear in the Fixture Sheet, plus the Fixture Sheet pane reference, settings screenshots, deterministic manual screenshots, human-readable Fixture Sheet testing scenario, coverage catalog, focused frontend tests, and root Playwright coverage.

Tests use populated rows with Intensity, Color, Position, Beam, Focus, Preload, Group-master limiting, source ownership, multi-head IDs, unavailable values, and Highlight step/base state. Geometry assertions compare Off with both compact modes at a supported small viewport and prove that compact mode displays more rows without overlap.

Visual/text assertions must prove:

- Off contains both graphics and text;
- Icon only contains the required graphics and omits ordinary value text;
- Text only contains authoritative text and omits the value graphics; and
- changing one surface does not mutate another surface or the show.

## Acceptance coverage

1. Venue fixtures are absent from both the full Fixture Sheet and compact Fixture Sheet panes.
2. Every `visual_only` fixture is absent even when its manufacturer is not Venue.
3. Every fixture whose complete ID starts with `0.` is absent even when imported legacy data lacks the other markers.
4. Ordinary fixtures `100`, masters `100.0`, and heads such as `100.1` remain visible when otherwise eligible.
5. Existing Fixture Sheet filters and head modes operate after the invariant exclusion and cannot reveal scenery.
6. Hidden objects remain intact in Show Patch, Stage, MVR, show files, and selection authority.
7. A selection containing hidden objects does not cause the Fixture Sheet to display or select an unrelated row.
8. Existing shows require no destructive migration.
9. Implementation updates the existing Fixtures and Patch help text that currently says Venue `0.x` objects appear in the Fixture Sheet.
10. Fixture Sheet **View** settings offer exactly Off, Icon only, and Text only.
11. The setting is available for normal panes, the full built-in, and planned fixed external-screen configuration using the same terminology.
12. Existing and new surfaces default to Off unless explicitly configured otherwise.
13. Off preserves the detailed current presentation.
14. Icon only shows compact intensity, color, position, Beam, and Focus graphics without their ordinary value text.
15. Text only shows concise authoritative values and semantic names without value bars, swatches, or position/Beam/Focus glyphs.
16. Fixture identity columns remain independently controlled and are not confused with the Icon-only value mode.
17. Preload, source ownership, Group-master status, unavailable state, selection, and Highlight step/base remain distinguishable in every mode.
18. Beam and Focus compact content comes from authoritative resolved data rather than placeholder strings.
19. Both compact modes use a stable reduced row height and narrower value columns, visibly fitting more rows on a supported small screen.
20. Compact rendering does not silently hide columns, clip fixture IDs, overlap rows, or depend on hover.
21. Each Fixture Sheet surface persists its own desk-local mode without changing portable show data or another surface.
22. Pane, built-in, external-screen, screenshots, and tests share one rendering contract.
23. Help, screenshot, human-readable scenario, focused tests, and Playwright coverage protect filtering, all three modes, and small-screen geometry.
