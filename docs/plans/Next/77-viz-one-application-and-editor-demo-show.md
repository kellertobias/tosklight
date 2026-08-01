# Viz: One Application, a Working Editor Window, and a Controllable Demo Show

## Status

**Specification only.** This plan records three requested changes to the ToskLight Viz products: one
application identity for the editor and the visualizer, a Viz editor window that draws its interface,
and a demo show the operator can open and drive from the editor when no DMX is arriving. It does not
implement window hosting, process lifecycle, Tauri asset loading, editor control surfaces, provider
value emission, help changes, or executable tests.

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

- The editor is the application that is opened and that owns the windows. The visualizer is a child
  surface or child window of the editor, or a child window of the ToskLight desk.
- Running the visualizer from the editor presents **one** application: one Dock tile, one App Switcher
  entry, one application menu, and one name.
- The preferred arrangement is the visualizer **inside the editor's main window**, beside the planning
  surface, so patching and the picture are visible at once. The split is operator-adjustable and the
  visualizer can be given the whole window.
- Started from the ToskLight desk, the visualizer may be a **separate window**, but it belongs to the
  desk's own application identity: still one Dock icon and one App Switcher entry, not a second
  product beside it.
- Quitting or hiding the owning application takes its visualizer surface with it. A visualizer window
  closed on its own does not end the editor session.

### Implementation direction

The architecture decisions already describe the shape this needs: the React application owns the
window, pane, toolbar and layout, while a native GPU child surface tracks a viewport rectangle. That
is the same boundary the built-in Stage is expected to use later, so the render core is hosted in the
editor's process rather than composited from a second process.

Candidate approaches to settle before implementation:

1. **Hosted render core.** The editor's Tauri process instantiates `crates/viz/render` on a child
   surface tracking a viewport rectangle in its window. One process, one identity, no IPC for scene or
   values. This matches the intended hierarchy most directly and is the recommended direction.
2. **Helper process under one identity.** The visualizer stays a separate executable inside the
   editor's bundle and runs with an accessory activation policy, so it contributes no Dock tile of its
   own while its window is owned by the editor. Cheaper to reach, but it keeps two processes, two
   lifecycles, and platform-specific activation behavior on each of macOS, Windows and Linux.

Whichever is chosen must keep the independent failure budget the MVP requires: a renderer fault must
not take down the desk, its server, or its output engine. If the hosted core cannot guarantee that
inside the editor process, say so explicitly and record which guarantee was traded.

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

**Load a Demo Show.** The editor offers the shipped demo show — the regular product demo,
`assets/demo.show`, 262 logical fixtures and 301 physical instances — as an explicit action, without
the operator locating a file. Opening it is a normal document open: the same patch sheet, the same
save behavior, and the visualizer draws it immediately.

Opening the demo must not modify the file in the repository or the packaged asset. Settle whether the
action opens a working copy in the operator's application-data folder or opens the shipped file
read-only with **Save As** required to keep changes; state which, and make the window say which.

**Control the lamps from the editor.** With the editor as the visualizer's source and no live DMX
arriving, the operator can drive the fixtures directly from the editor: intensity, colour, pan and
tilt at minimum, on the fixtures selected in the patch sheet, with a way to clear back to defaults.
This is what makes the demo show worth opening: a rig that can be looked at from every view and lit
without a desk, a network route, or a console.

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
- when that source is lost or terminated, the declared fallback applies — settle explicitly whether
  the fixture returns to the editor's value or to defaults, and document the choice; and
- editor values are never merged with received DMX for the same parameter.

The renderer's existing **Waiting for DMX** state must distinguish "nothing is arriving and nothing is
driving this" from "nothing is arriving and the editor is driving this".

### Ownership

- Editor-set values are session state of the planning window. They are not written into the show file
  and do not become a preset, a cue, or a stored look.
- The fixture library remains authoritative for channels, fine bytes, ranges, splits, defaults and
  logical heads; the editor sets semantic parameters and the existing projection layer decodes them.
- The visualizer stays read-only. It receives values; it does not originate or send them back.

### Acceptance

- **VIZ-EDITOR-DEMO-001.** From a fresh editor launch, open the demo show and see the full rig in the
  visualizer with no desk running and no output route configured.
- **VIZ-EDITOR-DEMO-002.** Select fixtures in the patch sheet, raise intensity, set a colour, and move
  pan and tilt; the visualizer shows exactly those fixtures change. Clear, and they return to defaults.
- **VIZ-EDITOR-DEMO-003.** With editor values active, start a real Art-Net or sACN source for one of
  the show's universes. Fixtures on that universe follow the network; the rest keep the editor's
  values; the status surface names both. Stop the source and verify the declared fallback.
- **VIZ-EDITOR-DEMO-004.** Editor values survive a view change, a quality change and a camera move,
  and are gone after the document is closed. The show file is byte-identical unless it was saved.
- **VIZ-EDITOR-DEMO-005.** Opening the demo show twice in a row gives the same rig; the shipped asset
  is unchanged either time.

## Open questions

1. Hosted render core inside the editor process, or a helper process under one activation identity —
   and what that does to the visualizer's independent failure budget.
2. Whether the desk-launched visualizer window is hosted by the desk process on the same terms, and
   what that means for the desk's own isolation rules.
3. Whether the demo show opens as a working copy or read-only.
4. Whether a lost network source returns a fixture to the editor's value or to defaults.
5. Whether the editor's control surface is part of the patch sheet or a separate panel beside it.

## Relationship to other work

- The desk-owned view, incremental scene deltas and anti-aliasing were closed separately; see the
  [MVP plan](../Later/viz/network-connected-renderer-mvp.md). Item 3 here builds on that delta and
  event plumbing rather than adding its own.
- Gobo artwork in packages and model geometry in shipped fixture packages remain open MVP gaps and
  are not solved here.
