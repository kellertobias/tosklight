# Fixture Sheet Filtering and Compact Mode

## Status

**Doing.** Claimed on 2026-08-02 after completing Plan 01. This is the sole active implementation
contract for Fixture Sheet invariant filtering, attribute-group base-value presentation, Dynamic
identity feedback, per-surface Compact mode, and shared dense rendering.

This follows Group and Dynamics spatial mapping in the current [Next plan order](../Next/README.md).
The Group-master and Dynamics indicators implemented here must consume the authoritative contracts
delivered by that completed plan rather than recreating them in the Fixture Sheet.

## Progress

- [x] Claimed from `docs/plans/Next` in numeric order.
- [x] Audit existing Fixture Sheet projection, filters, columns, settings ownership, persistence,
  compact rendering, Dynamic/base-value telemetry, help, screenshots, and acceptance seams.
- [x] Implement invariant programmable-row filtering without changing show or selection authority.
- [ ] Implement authoritative attribute-group base summaries and stable Dynamic identity/state
  feedback without sampled-value repainting.
- [x] Implement per-surface Off/Icon only/Text only configuration, migration, and shared rendering.
- [ ] Update help, deterministic screenshot contracts, human scenarios, and focused root Playwright
  coverage.
- [ ] Run focused checks, required major suites, migration/recovery proof, small-screen geometry, and
  the real desktop path.

## Implementation decisions

- Preserve one Fixture Sheet projection and renderer contract for pane, built-in, external-screen,
  screenshot, and test consumers; do not create compact-only parallel business rules.
- Treat the three independent scenery exclusions, base-versus-sampled Dynamic distinction, exact
  attribute-group inventory, literal Compact mode labels, and desk-local per-surface ownership as
  acceptance requirements.
- Reuse Plan 01 Group-master and Dynamic runtime identities and the authoritative attribute registry;
  the Fixture Sheet must not infer or recreate those authorities.
- Keep the existing scoped `dynamic_stack_only` visualization read as the backend boundary for this
  implementation slice: it is already Fixture-Sheet-specific. Its values now mean stable ordinary
  bases, and its Dynamic entries retain exact attributes and identities without sampled values or
  lossy fixture/group aggregation. The frontend must publish only semantically changed projections
  while this existing eventually-consistent read remains in use.

## Implementation verification

- Plan 01 is complete in commit `9216b28e`; its assigned-only Group Master runtime and authoritative
  Dynamic identity/projection contracts are available dependencies for this plan.
- The first implementation slice filters parent fixtures before logical-head expansion, ordering,
  active/Cuelist filters, or rendering. `visual_only`, manufacturer `Venue`, a projected reserved
  virtual identity, and a legacy complete ID beginning `0.` are independent exclusions; physical
  fixture 100 and identities `100.0`/`100.1` remain eligible. Thirteen focused filter/target tests,
  desktop typecheck, focused Biome, and the diff check passed.
- The completed read-only audit found one shared renderer but nine legacy columns, fabricated
  Beam/Focus row data, no density mode, a hard-coded 43 px virtualized row metric, and independent
  built-in/pane/fixed-screen persistence seams. It also found that the Fixture-Sheet-specific
  backend read discarded ordinary bases, restricted attributes, stripped Dynamic IDs/pool numbers,
  and merged identities. Existing help explicitly contradicts the new scenery invariant.
- The backend projection now returns the authoritative ordinary pre-Dynamic/FAT base in `values`,
  overlays current-session Preload ordinary values (including canonical ordered Group spreads),
  preserves individual Dynamic IDs, pool numbers, member attributes, runtime/controller/lane IDs,
  and paused/pending/hidden/winning state, and removes sampled/resolved values. Fixture scoping still
  applies to both bases and identities. The two focused projection tests and the focused route test
  pass in `light-headless-runtime`.
- The shared frontend projection now consumes those bases through the authoritative attribute
  registry for all eight groups, resolves profile semantic labels and units, keeps Media and Mask
  folder/file members distinct, and removes production Beam/Focus demo placeholders. Dynamic
  indicators stay attached to exact member attributes, retain each pool or stable snapshot identity
  and state separately, and omit sampled values. Poll responses publish React state only when the
  semantic bases or identities change; timestamps, revisions, and sampled-only fields do not cause
  repainting. Desktop typecheck, focused Biome, and 25 focused Fixture Sheet tests pass.
- Built-in, pane, and fixed-screen configuration now share `Off`, `Icon only`, and `Text only`
  persistence with independent desk-local ownership and `Off` migration defaults. The saved
  `dimmer` column alias migrates to `intensity`; Shapers, Control, and Media are selectable without
  becoming visible in existing layouts. Fixed-screen wire/domain contracts support all 12 identity
  and value columns, migrate old JSON, and generated TypeScript is current. Fifty-nine focused
  frontend persistence/surface tests, the focused show-store tests, fixed-screen route test,
  generated-contract test, desktop typecheck, Rust package checks, and focused Biome pass.
- The shared table now uses an explicit 43 px normal metric and deterministic 32 px compact metric
  for row layout, virtualization, fill rows, and spacers. Icon only retains graphical ordinary and
  Preload summaries while hiding ordinary value text; Text only retains concise ordinary and
  Preload text while hiding decorative glyphs. Both keep exact per-member Dynamic identities,
  source/status styling, fixture identity details, and all configured columns. Compact value-column
  minimums are narrower, while the table exposes real horizontal overflow when the selected set
  still cannot fit. A 430 px deterministic story proves both compact modes show more equal-height
  rows than Off, retain `.0`/`.1` IDs and both Dynamic identities without clipping, preserve distinct
  Media/Mask markers or text, and do not silently drop columns. The Storybook build, two focused
  browser cases, 70 focused unit cases, and both frontend typechecks pass.

## Remaining work

- Add accurate base-source ownership, Group-master flash/Highlight-bypass presentation, and the
  deterministic base-versus-changing-DMX acceptance proof in a later coherent slice.
- Add the human scenario, root E2E coverage, screenshot contract, and operator help updates.
- Keep every acceptance item below unclaimed until current source and focused verification prove it.

## Goal

Keep venue scenery and other non-programmable Stage elements out of the regular Fixture Sheet. The Fixture Sheet is a programming-state inspection table: it shows the base value held on each attribute group and whether Dynamic content is running there. It is not a continuously changing DMX-output monitor. The DMX window remains the surface for inspecting the actual, continuously resolved DMX output, while the Stage and Show Patch remain the correct surfaces for visual-only venue objects.

Let each Fixture Sheet surface also trade visual richness for row density so more fixtures remain visible on smaller screens.

## Base values and Dynamic feedback

Each Fixture Sheet attribute-group cell shows the winning ordinary static base value before Dynamic/FAT modulation. A running Dynamic does not replace that base with its sampled value and does not make the cell's percentage, text, swatch, position, or other value summary update on every Dynamic tick.

When Dynamic content applies to an attribute in the group, the cell also shows a compact Dynamics indicator:

- use the canonical Dynamics icon followed by the Dynamic pool number;
- keep the indicator visible in **Off**, **Icon only**, and **Text only** compact modes;
- associate the indicator with the particular member attribute/base value it affects when a group cell contains multiple values;
- show each applicable Dynamic when more than one Dynamic contributes to attributes represented by the cell, without merging their identities;
- retain a stable embedded-snapshot label only where recorded Dynamic content has no pool number; and
- distinguish running, paused, pending, hidden, or non-winning content when those states are relevant, without substituting the sampled output value.

The indicator answers “is a Dynamic running on this attribute, and which one?” The base summary answers “what value is the Dynamic operating around or alongside?” Neither is intended to answer “what is the output at this instant?”

This Fixture Sheet-specific presentation narrows the more verbose sampled/resolved Fixture Sheet telemetry described by the completed Dynamics plan. Authoritative Dynamic identity and state still come from the shared runtime projection, but the Fixture Sheet projection must not subscribe or repaint merely to display every sampled Dynamic value. The DMX window must continue to show the actual changing DMX output, including the result of Dynamics.

Preload follows the same separation: show the pending base value and pending Dynamic identity/state, not a continuously sampled pending Dynamic result. Source ownership, Group-master limiting, Highlight, unavailable values, and errors remain visible status information rather than reasons to replace the base with resolved DMX.

## Attribute-group columns

The Fixture Sheet provides one independently configurable value column for every authoritative attribute group:

- **Intensity**;
- **Color**;
- **Position**;
- **Beam**;
- **Shapers**;
- **Focus**;
- **Control**; and
- **Media**.

The columns are driven by the authoritative attribute registry rather than a separate Fixture Sheet-only classification. Adding an authoritative attribute group therefore requires a corresponding Fixture Sheet column contract. Fixed, indexed, continuous, and control functions inside an attribute do not create extra columns.

An attribute-group cell may summarize multiple member attributes. It shows their base values and any applicable Dynamic indicators without exposing raw DMX. Unsupported groups use the normal unavailable/empty presentation and must not invent placeholder live values.

The **Media** column is a multi-value summary. Once Media attributes are fully implemented, it shows:

- Media Folder plus Media File; and
- Mask Folder plus Mask File.

Folder/file pairs remain visibly associated and use their semantic names or identifiers. The column must not collapse the four fields into an ambiguous single number. A fixture without mask capability shows only Media Folder and Media File; missing or unavailable values remain distinguishable from valid zero-valued selections.

The existing saved **Dimmer** column choice migrates to the **Intensity** attribute-group column. Existing layouts gain no newly visible columns merely because Shapers, Control, or Media becomes available; every group column remains independently selectable in **Columns**.

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

Compact mode changes the presentation of attribute-group base-value cells and row density. It does not change fixture data, programmer values, selection, output, tracking, visible-column choices, or which fixtures are included. The programmable-fixture filter applies before density rendering.

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
- the Intensity base-value meter plus percentage;
- the Color base-value swatch plus text label;
- the Position base-value glyph plus Pan/Tilt text;
- Beam, Shapers, Focus, Control, and Media base-value summaries with their normal visual and text presentation;
- the Dynamics icon and number beside the affected base-value summary;
- Preload target visuals and text; and
- configured secondary details such as fixture type.

This remains the default and the reference presentation for existing layouts and screenshots.

## Icon only

**Icon only** reduces row height, cell padding, gaps, and value-column minimum widths. Live value cells retain their graphical summary and remove ordinary value text:

| Column | Icon-only presentation |
|---|---|
| Intensity | The compact base intensity/level bar without the percentage text. |
| Color | The base color swatch without the RGB/color text. |
| Position | The base position glyph without Pan/Tilt degree text. |
| Beam | A compact semantic base Beam glyph or state marker. |
| Shapers | A compact semantic base Shapers glyph or state marker. |
| Focus | A compact semantic base Focus/edge/frost/zoom glyph or state marker. |
| Control | A compact semantic base Control state marker. |
| Media | Compact, distinct Media and Mask source markers. |

The fixture **Icon** column remains a separately configurable identity column. Choosing Icon only does not automatically enable that column or hide ID, Name, Patch, or another column selected under **Columns**.

Beam, Shapers, Focus, Control, and Media icons must be derived from authoritative base attributes/functions. The implementation must not turn current reserved placeholder summaries into invented live state merely to fill the compact cells.

Preload uses a clearly distinct secondary or overlaid icon/bar state with the same current-versus-pending meaning as the detailed view. Dynamics indicators remain the Dynamics icon plus number rather than disappearing with ordinary value text. Source ownership, Group-master limiting, selection, Highlight stepping, unavailable values, and errors retain non-text visual distinctions.

Every icon-only value has an accessible name containing its full base-value text and applicable Dynamic identity. Pointer hover may show the same information as a tooltip, but hover is supplemental and cannot be the only way a required touch-screen operator obtains it.

## Text only

**Text only** uses the same reduced row height, padding, gaps, and narrower value columns but removes decorative value graphics. It presents concise authoritative text such as:

- `100%` for the Intensity base;
- `RGB 0, 0, 0`, a semantic color name, or the normal neutral/unsupported label for the Color base;
- `50° / 50°` for the Pan/Tilt base;
- `Open`, `Gobo 2`, or another Beam base summary;
- concise Shapers, Focus, and Control base summaries; and
- `Folder 2 / File 7` plus `Mask Folder 1 / Mask File 4`, using semantic media names when available.

Text formatting uses the attribute's normal units and semantic names. It must not expose raw DMX merely because the visual icon is absent.

Preload targets remain visible as compact arrow/value text. The Dynamics icon and number remain beside affected text-only values because they communicate running content rather than decorative value graphics. Source ownership and current/pending state retain a non-color-only distinction without reintroducing the removed gauges and ordinary value glyphs.

## Identity, status, and row behavior

Compact mode affects value presentation rather than silently changing the column model.

- ID, fixture Icon, Name, Patch, Intensity, Color, Position, Beam, Shapers, Focus, Control, and Media remain independently controlled by **Columns**.
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

Tests use populated rows with every authoritative attribute group, base values, single and multiple Dynamics indicators, Media and Mask folder/file pairs, Preload, Group-master limiting, source ownership, multi-head IDs, unavailable values, and Highlight step/base state. A deterministic Dynamic clock proves that the actual DMX output changes while the Fixture Sheet base summary remains stable and continues to identify the running Dynamic. Geometry assertions compare Off with both compact modes at a supported small viewport and prove that compact mode displays more rows without overlap.

Visual/text assertions must prove:

- Off contains both graphics and text;
- Icon only contains the required graphics and omits ordinary value text;
- Text only contains authoritative text and omits the value graphics; and
- every mode keeps the Dynamics icon and number without displaying sampled Dynamic values;
- the Media column keeps Media Folder/File and Mask Folder/File distinguishable; and
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
13. Off preserves the detailed presentation while showing base values rather than sampled Dynamic output.
14. Icon only shows compact attribute-group base graphics without their ordinary value text.
15. Text only shows concise authoritative base values and semantic names without ordinary value bars, swatches, or attribute-group glyphs.
16. Fixture identity columns remain independently controlled and are not confused with the Icon-only value mode.
17. Preload, source ownership, Group-master status, unavailable state, selection, and Highlight step/base remain distinguishable in every mode.
18. Beam, Shapers, Focus, Control, and Media compact content comes from authoritative base data rather than placeholder strings.
19. Both compact modes use a stable reduced row height and narrower value columns, visibly fitting more rows on a supported small screen.
20. Compact rendering does not silently hide columns, clip fixture IDs, overlap rows, or depend on hover.
21. Each Fixture Sheet surface persists its own desk-local mode without changing portable show data or another surface.
22. Pane, built-in, external-screen, screenshots, and tests share one rendering contract.
23. Help, screenshot, human-readable scenario, focused tests, and Playwright coverage protect filtering, all three modes, and small-screen geometry.
24. The Fixture Sheet offers independently configurable Intensity, Color, Position, Beam, Shapers, Focus, Control, and Media columns, one for every authoritative attribute group.
25. Each attribute-group cell shows the ordinary static base and identifies applicable Dynamic content with the Dynamics icon and number beside the particular member attribute it affects; it does not repaint with every sampled Dynamic value.
26. Multiple applicable Dynamics remain individually identifiable, and embedded snapshots without a pool number receive an unambiguous stable label.
27. The DMX window shows the actual continuously changing output produced by Dynamics while the Fixture Sheet base value remains stable.
28. The Media column shows Media Folder plus Media File and, when supported, Mask Folder plus Mask File as distinct associated pairs.
29. Existing saved Dimmer visibility migrates to Intensity, while newly available attribute-group columns do not become visible automatically.
