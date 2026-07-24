# 01 — Browser UI and Desktop panes

## Outcome

Deliver the minimum browser-backed scenario world and the first complete operator helper family:
opening built-ins, creating/using Desktops, adding typed panes, acting on pane handles, and
capturing application or pane screenshots.

This step comes first because every later visible helper needs a stable way to locate and operate
the current application, Desktop, modal, and pane without embedding selectors.

## Public helpers

```ts
export enum PaneType {
  Stage = "stage",
  Groups = "groups",
  Fixtures = "fixtures",
  Presets = "presets",
  Cuelists = "cuelists",
  CuelistPool = "cuelist_pool",
  Cues = "cues",
  Playback = "playback",
  PlaybackPool = "playback_pool",
  CueList = "cue_list",
  Dynamics = "dynamics",
  Channels = "channels",
  Dmx = "dmx",
  Patch = "patch",
  Setup = "setup",
  Help = "help",
  Development = "development",
  VirtualPlaybacks = "virtual_playbacks",
  FileManager = "file_manager",
  TextEditor = "text_editor",
}
```

The minimum API is:

- `scenario(id, title, callback)`;
- `app.open()` and `app.expect.ready()`;
- `builtIn.open(type)` and `builtIn.expect.active(type)`;
- `desktop.create/open/use/configure/rename/clone/delete(...)`;
- `DesktopConfiguration.addPane(type, placement)`;
- `DesktopConfiguration.apply()`;
- `desktop.getPane(type, slug)`;
- typed pane configuration, geometry, focus, maximize, restore, remove, screenshot, and
  assertions;
- `screenshot.application`, `screenshot.builtIn`, and `screenshot.dialog`.

## Typed pane handles

`addPane` accepts the pane enum and the complete grid placement. It returns a handle typed from
the enum member:

```ts
const layout = t.desktop.configure("Programming experiment");
const stage = layout.addPane(PaneType.Stage, {
  slug: "main-stage",
  column: 1,
  row: 1,
  width: 16,
  height: 18,
});

stage.configure({
  view: StageView.ThreeDimensional,
  followPreload: false,
  beamGuides: true,
});

await layout.apply();
await stage.focus();
await stage.maximize();
await stage.screenshot("main-stage-maximized");
await stage.restore();

const sameStage = t.desktop.getPane(PaneType.Stage, "main-stage");
await sameStage.resize({ width: 12, height: 9 });
await sameStage.expect.geometry({
  column: 1,
  row: 1,
  width: 12,
  height: 9,
});
```

The required slug is unique within the Desktop, kebab-case, and stable across recreation and
reopen. It is test-domain identity, not the application's persisted pane ID.

`PaneHandle<PaneType.Stage>` accepts `StagePaneConfiguration`;
`PaneHandle<PaneType.Fixtures>` accepts `FixtureSheetPaneConfiguration`, and so on. Passing a
Stage-only option to a Fixture Sheet is a type error. Common actions live on the base handle;
real pane-specific operator actions may be added to the corresponding subtype.

Before `apply`, `pane.configure` contributes initial typed configuration. After binding,
`pane.configure` performs a real reconfiguration action. Runtime actions fail clearly when used
on an unbound handle.

## Registered layouts

Central fixed layouts use the same builder:

```ts
defineDesktop(Desktop.Programming, (desktop) => {
  const presets = desktop.addPane(PaneType.Presets, {
    slug: "presets",
    column: 1,
    row: 1,
    width: 9,
    height: 18,
  });
  presets.configure({
    family: PresetFamily.Mixed,
    poolColors: true,
  });

  const fixtures = desktop.addPane(PaneType.Fixtures, {
    slug: "fixtures",
    column: 10,
    row: 1,
    width: 15,
    height: 9,
  });

  const stage = desktop.addPane(PaneType.Stage, {
    slug: "main-stage",
    column: 10,
    row: 10,
    width: 15,
    height: 9,
  });
  stage.configure({
    view: StageView.TwoDimensional,
    followPreload: false,
    beamGuides: true,
  });
});

await t.desktop.use(Desktop.Programming);
```

`desktop.use` invokes the registered recipe through `desktop.configure`; it does not inject a
separate raw persisted layout.

## Stable UI seams

Add accessibility and semantic identity only at composition boundaries:

- application and built-in root;
- active Desktop;
- runtime pane ID, `PaneType`, slug binding, and accessible pane title;
- dialog role and accessible name;
- maximize/restore state and grid geometry.

Scenario authors never import or query these attributes directly. The adapter owns locators.

## Browser secondary-screen intents

This step also provides typed helpers for the secondary-screen configuration visible from the
browser application:

- create/configure a named screen;
- assign its Desktop, Dock, Playbacks, Page Controls, display metadata, bounds, fullscreen, and
  desired-open state;
- configure Playback rows, first slots, faders, buttons, and Follow Main versus Dedicated Page;
- open/close/remove through the controllable browser bridge;
- assert the requested bridge action and persisted screen state.

These helpers prove the frontend intent and controllable browser-bridge request. They do not
claim that Playwright drove or captured a native OS window.

## Helper-contract scenarios

Add focused browser scenarios for:

1. open each supported built-in by enum and assert the active semantic root;
2. build a three-pane Desktop and assert type, slug, title, and exact grid geometry;
3. reject duplicate slugs, out-of-grid rectangles, and collisions before UI mutation;
4. prove Stage and Fixture Sheet configurations are type-distinct;
5. resolve a pane again after Desktop reopen using type plus slug;
6. move, resize, focus, maximize, restore, and remove a returned handle;
7. capture the complete application, one built-in, one modal, and one pane;
8. prove a modal portal is included in application capture;
9. produce actionable diagnostics for a missing slug and type mismatch.
10. configure a secondary screen and assert its browser-bridge action without claiming native
    window coverage.

Use the actual visible Desktop and pane workflows. API coverage is added only for persisted layout
intents that have an independent typed API.

## Done gate

- No public Desktop helper accepts a CSS selector, pixel coordinate, raw pane kind string, or
  persisted pane ID.
- `addPane` returns an actionable, pane-specific typed handle.
- Registered and inline layouts share the same implementation.
- The focused helper-contract scenarios pass in the browser.
- Later steps can request a built-in or pane handle without creating new locators.

## Result

Completed on 2026-07-24.

- Added the browser-backed `scenario(...)` world with typed application, Built-in,
  Desktop, pane, screenshot, and secondary-screen intents. Public helpers accept pane
  enums, semantic names, kebab-case slugs, and 24 × 18 grid rectangles; selector,
  pixel, runtime-ID, and browser-bridge details remain adapter-private.
- Added typed pane configurations and a compile-only contract gate. Stage-only settings
  are rejected for Fixture Sheet handles, inline and registered Desktop layouts share
  the same builder, validation rejects bad slugs/bounds/collisions before UI mutation,
  and bound handles survive Desktop reopen through the test-domain slug registry.
- Added stable composition seams for the application, Built-ins, Desktops, panes,
  dialogs, and optional screens. Pane Settings now exposes exact column/row/size plus
  visible maximize/restore controls; pane focus is a real DOM focus interaction.
- Extended optional-screen setup with Desktop assignment and explicit window bounds.
  The browser helper configures the real setup controls, observes persisted screen
  state, and asserts the controllable desktop-bridge action without claiming native
  window control.
- Added `BENCH-UI-001` through `BENCH-UI-003`, covering the ten helper-contract
  requirements as three cohesive isolated browser scenarios.

Verification:

- `npm run test:bench-types`: passed.
- `npm run test:architecture`: passed.
- focused UI unit coverage: 36 passed.
- complete frontend unit suite: 282 files / 2,000 tests passed.
- Rust workspace unit run was green except the sandbox-blocked CITP socket tests;
  `cargo test -p light-media --lib` passed 5/5 with localhost permission.
- focused browser contract: 3 passed.
- full `npm run test:e2e`: 289 passed / 9 skipped with one unrelated
  `HIGHLIGHT-003 @ui` timing failure; the exact isolated rerun passed.

The original `Pane.tsx` file already contained unrelated user work for DELETE-mode pane
removal. Only the semantic pane-boundary and always-available Settings changes from this
chunk are included in the chunk commit.
