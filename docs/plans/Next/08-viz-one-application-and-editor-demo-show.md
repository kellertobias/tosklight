# Viz: One Application, a Working Editor Window, and a Controllable Demo Show

## Status

**Specification only.** This plan records three requested changes to the ToskLight Viz products: one
application identity for the editor and the visualizer, a Viz editor window that draws its interface,
and a demo show the operator can open and drive from the editor when no DMX is arriving. It does not
implement window hosting, process lifecycle, Tauri asset loading, editor control surfaces, provider
value emission, help changes, or executable tests.

This is the eighth item in the current [Next plan order](README.md).

It continues the delivered
[Network-Connected Live Visualizer MVP](../Later/viz/network-connected-renderer-mvp.md) and stays
inside the ownership rules of the
[Rig Planner and Lighting Visualizer application plan](../Later/viz/rig-planner-visualizer-application-plan.md)
and its [architecture decisions](../Later/viz/architecture-decisions.md).

## 1. One application, one Dock and App Switcher entry

### Current behavior

`apps/viz-editor` is a Tauri application and `apps/viz-renderer` is a bare `wgpu` process wrapped on
macOS in `ToskLight Visualizer.app`. Launching the visualizer with nothing named starts the editor as
a second process, so the operator sees **two** Dock tiles and **two** App Switcher entries for what is
one product. The launch relationship is also inverted against the intended hierarchy: the visualizer
is the application an operator opens and the editor is what it spawns.

### Required behavior

- **ToskLight Visualizer** is one product and one application identity containing both visualization
  and show editing. A normal launch opens the visualizer when a show is already available.
- The application menu can open the editor. When no show is loaded, startup opens the editor first
  rather than presenting an empty renderer. The editor provides an explicit action to open the
  visualizer once a show exists.
- Editor and visualizer windows behave as one application: one Dock tile, one App Switcher entry, one
  application menu, and one name. Closing a visualizer window does not close the editor or its show.
- Started from the ToskLight desk, **Open Visualizer** creates an owned visualizer window under the
  desk's application identity: still one Dock icon and one App Switcher entry, not a second product.
- The desk-launched renderer runs in a supervised helper process. A renderer crash may close or
  replace the visualizer surface and report an error, but it must not crash the desk, headless
  server, Programmer, playback, or output engine.
- Investigate embedding that isolated output into a Tauri pane: the regular 3D View pane or a
  single-pane fullscreen screen. If ordinary HTML/web UI can overlay the rendered image correctly
  on macOS, Windows, and Linux, make this native renderer the only in-application renderer and remove
  the current web-based renderer. Browser-only access is allowed to omit visualization entirely.
  Otherwise keep the owned separate native window and retain the web renderer only where native
  embedding cannot meet the pane contract.

### Process and window architecture

Application identity, process isolation, and window placement are separate choices. One Dock/App
Switcher identity does not require one operating-system process.

For the standalone Visualizer application, the editor is an application-owned Tauri window and the
same process hosts the shared `crates/viz/render` core. A renderer fault can therefore end the
standalone planning/visualization product, which is an accepted tradeoff because no live lighting
desk is in that process. Editor and renderer remain different windows/modes of one product, not two
applications.

For desk launch, isolation is mandatory: the desk supervises a renderer helper packaged inside the
desk application. The helper receives scene/configuration and live values over a private bounded IPC
contract, uses an accessory/background application policy, and never owns a separate Dock or App
Switcher identity. The desk owns menu commands, window lifetime, restart/error state, and the visible
window relationship. This costs IPC, restart synchronization, and platform-specific child-window
handling, but a native renderer crash cannot unwind the desk process.

The first desk implementation should use an owned separate window because it is the least fragile
cross-platform isolation boundary. The embedded experiment may either attach a helper-owned
native child surface to a Tauri viewport or pass an offscreen/shared render target into a desk-owned
surface. It must prove clipping, resize, DPI, fullscreen, input routing, focus, accessibility, and
HTML overlay/z-order behavior on macOS, Windows, and Linux. A native surface merely floating above a
WebView is not acceptable: menus, selection overlays, status, dialogs, and other web UI must be able
to draw above it where the pane contract requires.

Removing the web renderer also removes browser rendering as a release requirement. The application
may still expose browser-based control and configuration pages, but those pages may show an explicit
**Visualizer available in the desktop application** state instead of a rendered scene.

### Demo-video and headless rendering

Demo-video generation must not depend on opening the retired web renderer. Reuse the native render
core in a deterministic headless/offscreen capture tool that loads the generated Demo Show, applies
the scripted camera and fixture-value states, renders frames or still snapshots, and writes them
under canonical `.artifacts/` paths. CI can then composite those captures into the demo video.

The headless path must use the same scene projection, materials, lighting, fixture models, quality
configuration, and render core as the interactive native visualizer. It may use an offscreen GPU
surface or a supported software adapter, but must fail visibly rather than silently substituting the
old web renderer. Pin resolution, device-pixel ratio, camera, time step, random seeds, and color
output so repeated CI captures are deterministic enough for the video workflow.

### Fixture-model consolidation decision

Consolidating the editor and renderer must also consolidate fixture-model ownership. An exact model
for a fixture belongs in that fixture's transferable fixture-library package and travels in the
embedded profile revision used by a show. The desk Stage, interactive native visualizer, editor
views where detailed geometry is appropriate, and headless capture must all consume that same
package-owned model rather than maintaining renderer-specific copies or choosing different
procedural bodies for the same fixture.

For fixtures without a product-specific model, retain one documented set of generic models. The
generic GLBs currently used by the native Viz renderer are the accepted generic-model baseline and
should become the shared generic fallback set unless a fixture package supplies something more
specific. Consolidation must audit the generic mapping against fixture type and capabilities so a
Sunstrip, blinder, strobe, conventional lamp, wash, scanner, laser, and other supported classes each
resolve to the intended body rather than merely to any visible proxy.

The profile moving light is the deliberate exception. Preserve both the native Viz renderer's
current generic profile-moving-head GLB and the current desk web Stage's procedural profile-moving
light. The desk version is preferred visually at present, but this plan does not select it as the
final shared model. Neither implementation may be deleted, overwritten, or made unrecoverable
during renderer/editor consolidation. Keep both available for direct comparison until the operator
makes a later explicit decision about which becomes authoritative.

Because patched shows embed immutable fixture-profile revisions, updating the desk-wide fixture
library alone is not proof that an existing or canonical show uses the corrected model. Regenerate
repository-owned Demo Show and Default Stage Show data, or provide the applicable migration or
explicit repatch path, so their embedded profiles carry the intended package models. Old operator
shows remain compatible and keep their embedded revisions unless a separately specified migration
changes that contract.

### Consequences to carry

- The Viz icon set is owned by the editor (`apps/viz-editor/src-tauri/icons`) and the visualizer takes
  its window mark and macOS bundle icon from there. One identity should make that ownership direct
  rather than a copy.
- macOS release staging currently nests `ToskLight Visualizer.app` inside the editor bundle's
  `Contents/Resources/`; Windows ships the visualizer as a separate zip beside the editor installer.
  Both need revisiting once the process arrangement is settled.
- `TOSKLIGHT_VIZ_EDITOR`, `TOSKLIGHT_VIZ_HEADLESS` and `TOSKLIGHT_VIZ_LAUNCHED_BY` exist because the
  two binaries are not installed together in a development tree. Whatever replaces them must still
  work from a plain `npm run open:viz-editor` in a source checkout.

### Application and isolation acceptance

- A normal standalone launch opens the visualizer for an available show; without a show it opens
  the editor first, and the menu can open the editor at any time.
- Editor and renderer expose one application name, Dock tile, App Switcher entry, and application
  menu on macOS, with equivalent single-application behavior on Windows and Linux.
- **Open Visualizer** from the desk opens an owned renderer window without another application
  identity. Killing or crashing its helper reports the failure and leaves desk control and output
  responsive; the helper can be restarted with a fresh authoritative scene/value snapshot.
- Embedded desk output is accepted only after the same isolated helper passes HTML overlay/z-order,
  clipping, resizing, DPI, fullscreen, input, focus, and accessibility checks in both the regular 3D
  View pane and a single-pane fullscreen screen on every supported desktop platform.
- Once those checks pass, the web renderer is removed and browser-only control surfaces clearly
  report that visualization requires the desktop application.
- Headless native capture renders the generated Demo Show without a WebView and supplies the stills
  or frame sequence used by CI demo-video generation.
- Every fixture with a package-owned model uses that exact model in the desk Stage, native
  visualizer, applicable editor views, and headless capture. Repository-owned Demo Show and Default
  Stage Show files embed the intended model-bearing profile revisions rather than relying on stale
  renderer-specific fallbacks.
- Fixtures without exact models resolve through the audited shared generic-model set based on their
  fixture type and capabilities. Golden scenes cover at least a Sunstrip, blinder, strobe,
  conventional lamp, wash, scanner, laser, and moving fixture across the interactive and headless
  paths.
- The profile moving light keeps both candidate representations: the native Viz GLB and the desk
  web Stage procedural model. Consolidation proves both still render and does not choose or delete
  either one without a later explicit operator decision.

## 2. The Viz editor window opens white — **fixed**

It was the second of the causes listed here: the dev-server URL used in a build that has no dev
server. Tauri decides between a development build and a real one by the `custom-protocol` feature —
without it the application embeds no frontend and opens `devUrl` instead. The Tauri CLI passes that
feature for `tauri build`, which is how the desk and the release workflow build their applications,
but `tools/build.sh` builds the editor with a plain `cargo build`, so every locally built editor was
a development build pointed at `http://127.0.0.1:4177` with nothing listening on it.

`tools/build.sh` now passes `--features custom-protocol`, and the crate's own `build.rs` tracks the
frontend directory so an edited interface is re-embedded rather than left stale in a binary cargo
reports as fresh.

The remaining piece is the acceptance coverage: a `--verify`-style check that opens the editor,
presents a frame and asserts its document surface actually mounted. Nothing in the build catches a
white window today — the evidence that it is fixed is that the binary now embeds the frontend
(`strings` finds the hashed asset names) and no longer resolves the dev URL.

## 3. Demo Show in the editor, and controlling it without DMX

### Required behavior

**Load a Demo Show.** CI generates the canonical product-demo show used for the demo video and
packages that exact generated show into every release. Future source changes to the demo generator
therefore produce and ship the updated demo show; a separately maintained stale copy is not allowed.
The editor offers it as an explicit action without requiring the operator to locate a file.

The packaged demo is an immutable template. Loading it always creates a normal writable copy in the
operator's application-data/documents workflow and opens that copy through the ordinary show path.
Saving can update the copy but can never overwrite the bundled template. Loading the demo again
creates a fresh copy from the currently shipped version. The UI must name the copy and make clear
that it originated from the Demo Show.

**Control the lamps from the editor.** Selecting fixtures in the patch sheet opens two preview modes:

- **Simple** exposes Pan, Tilt, Intensity, Color, and Gobo through fixture-aware semantic controls.
- **Full DMX** is available only when exactly one lamp/physical fixture instance is selected. It
  exposes every DMX slot/value for that fixture's complete mode, including all logical heads, using
  the fixture definition for channel labels, fine-byte grouping, ranges, and defaults. It is a
  testing tool, not a batch-programming surface; multi-selection disables it visibly.

Both are session-only preview controls and include a clear-to-defaults action. This is what makes the
demo show worth opening: a rig that can be looked at from every view and lit without a desk, a
network route, or a console.

### Where this sits in the existing contracts

This is the planning-software provider's already-specified capability, not a new hole in the MVP's
two-plane rule. The MVP forbids the **lighting-desk** provider from carrying live values over the API
and requires them to arrive as real Art-Net or sACN. The planning provider is explicitly allowed to
send "local preview values through the canonical engine-neutral renderer protocol, including
`UpdateFixtureValuesBatch`, rather than requiring the planner to emit Art-Net or sACN". `crates/viz/planning`
already serves the planning document and its configuration event stream; this adds a value plane to it.

The editor is therefore not a second desk. It has no programmer, no command line, no playbacks and no
cue stack — it has direct values for looking at a rig. Anything that starts to need cues, tracking or
LTP arbitration belongs on the desk, and the answer is to connect to one.

### Precedence with real DMX

The requirement is "if there is no DMX in". Make that deterministic and visible rather than implicit:

- editor-set values apply to a fixture while no healthy input mapping is delivering that fixture's
  logical universe;
- when a real source for that universe becomes healthy, it takes the fixture and the status surface
  says so;
- when that source is lost or terminated after delivering values, the visualizer holds the last
  received DMX values until a source resumes or the operator explicitly clears/resets them; and
- editor values are never merged with received DMX for the same parameter.

The renderer's existing **Waiting for DMX** state must distinguish "nothing is arriving and nothing is
driving this" from "nothing is arriving and the editor is driving this".

### Ownership

- Editor-set values are session state of the planning window. They are not written into the show file
  and do not become a preset, a cue, or a stored look.
- The fixture library remains authoritative for channels, fine bytes, ranges, splits, defaults and
  logical heads. Simple mode sets semantic parameters through the existing projection layer; Full
  DMX mode sets raw preview slots while still using fixture metadata to present them correctly.
- The visualizer stays read-only. It receives values; it does not originate or send them back.

### Acceptance

- **VIZ-EDITOR-DEMO-001.** From a fresh editor launch, open the demo show and see the full rig in the
  visualizer with no desk running and no output route configured.
- **VIZ-EDITOR-DEMO-002.** Select fixtures in the patch sheet, raise intensity, set a colour, and move
  pan and tilt; the visualizer shows exactly those fixtures change. Clear, and they return to defaults.
- **VIZ-EDITOR-DEMO-003.** With editor values active, start a real Art-Net or sACN source for one of
  the show's universes. Fixtures on that universe follow the network; the rest keep the editor's
  values; the status surface names both. Stop the source and verify that the last received values
  remain until explicitly cleared or replaced by resumed input.
- **VIZ-EDITOR-DEMO-004.** Editor values survive a view change, a quality change and a camera move,
  and are gone after the document is closed. The show file is byte-identical unless it was saved.
- **VIZ-EDITOR-DEMO-005.** CI proves that the packaged demo is generated from the canonical demo
  source. Opening it twice creates independent writable copies with the same rig and never changes
  the packaged template.
- **VIZ-EDITOR-DEMO-006.** Simple mode controls Pan, Tilt, Intensity, Color, and Gobo; with exactly
  one physical fixture selected, Full DMX independently controls every slot in its mode, including
  correct fine bytes and logical heads. Selecting zero or multiple fixtures disables Full DMX
  without changing preview values.

## Open questions

1. Whether the final cross-platform embedded-renderer experiment can satisfy HTML overlay/z-order,
   clipping, input, DPI, and crash-isolation acceptance. If not, the owned separate desk window is
   the final native-renderer integration.

## Relationship to other work

- The desk-owned view, incremental scene deltas and anti-aliasing were closed separately; see the
  [MVP plan](../Later/viz/network-connected-renderer-mvp.md). Item 3 here builds on that delta and
  event plumbing rather than adding its own.
- Gobo artwork in packages remains an open MVP gap. Model geometry in shipped fixture packages and
  shared generic-model ownership are resolved as part of this consolidation according to the
  fixture-model decision above.
