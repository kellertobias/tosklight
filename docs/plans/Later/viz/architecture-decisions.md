# Rig Planner and Visualizer — Architecture Decisions

Status: Living decision record  
Repository: `/Users/keller/repos/light`  
Canonical requirements: [`rig-planner-visualizer-application-plan.md`](./rig-planner-visualizer-application-plan.md)

This document explains the durable choices behind the application plan. It intentionally does not repeat the complete feature list, schemas, phase gates or acceptance criteria.

## Decision status

- **Accepted:** use unless new evidence justifies a recorded replacement.
- **Provisional:** validate during the Phase 0 spike before treating it as permanent.
- **Open:** no decision has been made.

## ADR-001 — Keep the product in the Light workspace

Status: Accepted

The planner and renderer integration live in `/Users/keller/repos/light`.

- The Tauri/React editor belongs under `apps/viz-editor`.
- The Rust renderer belongs under `apps/viz-renderer`.
- Reusable visualization behavior belongs under `crates/viz`.
- Reusable React presentation belongs in `apps/ui-library` and is consumed as `@tosklight/ui`.

Reasoning: fixtures, patching, show data, MVR/GDTF handling and live values already cross several Light applications. A separate repository would encourage duplicate models, conversions and release coordination. Proposed crate names in the main plan are ownership suggestions, not permission to recreate behavior already present elsewhere.

## ADR-002 — Use Tauri and React for the application shell

Status: Accepted

The main application uses Tauri, React and TypeScript. React owns the UI composition, inspectors, lists, dialogs and workflows. It does not redraw the planning canvas through component reconciliation on every frame.

Reasoning: this preserves the existing ToskLight frontend stack, UI investment and product appearance while avoiding Electron. Tauri provides a comparatively lightweight desktop shell and a Rust integration boundary on Windows, macOS and Linux.

## ADR-003 — Reuse the shared UI library

Status: Accepted

Common controls come from `@tosklight/ui` in `apps/ui-library`. Shared components remain presentational and accept typed values, view models and callbacks. Editor state, Tauri calls, document services and renderer coordination remain in app-owned adapters under `apps/viz-editor`.

Reasoning: visual consistency is a product requirement, and copying controls would create divergent behavior and styling. Storybook gives reusable components a deterministic environment before they are integrated into the live application.

Consequence: a broadly reusable missing component is added and tested in `apps/ui-library` first. A product-specific panel may remain in the planner app, but it still composes shared `@tosklight/ui` primitives and design tokens.

## ADR-004 — Use a retained WebGL editor for planning views

Status: Accepted

The synchronized top, side, front/back and isometric planning views use a retained WebGL2 renderer managed by TypeScript. React coordinates tools and document state, while camera movement, hover, drag previews and GPU resources remain viewport-local.

Reasoning: the planning view needs fast interaction with many objects, consistent cross-platform browser support and a path to a reduced browser-based edition. Ordinary React DOM rendering is unsuitable for dense geometry, while a native-only planning viewport would make reuse on the web substantially harder.

Consequence: the editor renderer favors clear geometry, selection, snapping, wireframe, hidden-line and flat-shaded views. It is not required to reproduce the high-quality lighting simulation.

## ADR-005 — Implement the high-quality visualizer in Rust and evaluate `wgpu` first

Status: Accepted

The native high-quality renderer is implemented in Rust. Phase 0 must select and
prove the Rust graphics stack against fixture counts, moving shadowed lights,
fog, volumetric beams, and cross-platform packaging.

`wgpu` is the preferred first candidate because it can target Metal, Direct3D
12 and Vulkan while keeping the renderer and its shaders behind one Rust-owned
surface abstraction. Selection of `wgpu` is provisional until the Phase 0
benchmark proves the required image quality, GPU timing, resource lifetime,
packaging and native-window integration on all three platforms.

Reasoning: Rust keeps renderer lifecycle, shared types, fixture behavior, and
packaging in one language while retaining a dedicated process boundary. The
selected rendering libraries must remain replaceable behind Viz-owned adapters.

Rejected baseline: making the Tauri WebView responsible for the highest-quality visualization. WebGL remains valuable for planning and a possible reduced web preview, but it is not the quality ceiling for the desktop product.

## ADR-006 — Run the renderer as a supervised separate process

Status: Accepted

The Rust visualizer runs as a separate process with its own native window. The
Tauri application owns the document, persistence, commands, and undo; the
renderer owns only its mirrored render scene and GPU resources.

Reasoning:

- A renderer crash must not lose document work.
- Heavy GPU work and render-loop stalls must not block the editor.
- The renderer and Tauri have independent window and event loops.
- The renderer can be restarted and resynchronized from a full snapshot.
- The boundary makes later renderer replacement possible.

Consequence: the renderer never writes the SQLite show file and never becomes the authoritative scene model.

The reusable render core must not assume that its only presentation target is a
standalone top-level window. It should accept a presentation-surface adapter so
the dedicated Viz process can own a native window while a future desk adapter
can present the same core in an embedded native viewport.

## ADR-007 — Keep one canonical semantic scene

Status: Accepted

The canonical scene and domain rules live in Rust. SQLite is the canonical persisted show document. TypeScript and the renderer consume generated types, view models or protocol projections rather than defining competing scenes.

Reasoning: attachment semantics, patch state, units, connector compatibility, cable calculations and load calculations must behave identically across the planner, reports and renderer. Mesh transforms alone are not a sufficient production model.

Consequence: frame-local UI and renderer state may be duplicated for performance, but identity, persistent properties and validated mutations return to the authoritative model.

## ADR-008 — Share the existing fixture system

Status: Accepted

Manufacturer → Fixture → Mode remains the fixture-library hierarchy. Fixtures may be placed without patching. `Venue` scenery uses the same hierarchy with addressless model variants. Existing icon/model assets, GDTF retention and patch behavior are reused.

Reasoning: the planning application and the lighting desk must agree on fixture identity, mode, physical data and channel behavior. A planner-specific fixture database would inevitably drift.

## ADR-009 — Use an engine-neutral snapshot and delta protocol

Status: Accepted

The editor sends a versioned full snapshot when the renderer starts or resynchronizes, followed by batched incremental changes and high-rate live values. The protocol describes semantic renderer inputs without exposing renderer-library object paths as the public contract.

Reasoning: this keeps process recovery deterministic, prevents a chatty per-property IPC design, and leaves room to replace the selected Rust rendering stack later.

Transport choice remains provisional until Phase 0 measures local IPC behavior on all three platforms. Correctness and observability take priority over prematurely adopting shared memory.

## ADR-010 — Target rasterized realism, not live path tracing

Status: Accepted

The quality baseline uses physically based materials, shadow maps, volumetric fog, bloom, exposure and selected temporal effects. Live path tracing is not required.

Reasoning: the scene may contain hundreds of moving fixtures and must remain responsive on a practical cross-platform GPU range. The most important perceptual requirements are beam shape, haze, occlusion, color, gobos, movement and stable shadows.

## ADR-011 — Keep a future browser edition possible, but out of the desktop critical path

Status: Provisional

Version 1 is desktop-first. TypeScript editor packages should remain browser-compatible where practical, and a future static-hosted edition may load a show locally and use a reduced WebGL preview without a server.

Reasoning: the browser edition is valuable for installation-free editing, but offline file handling, SQLite persistence, PDF generation and reduced graphics capabilities require a separately scoped product slice. It must not weaken the desktop renderer or force Rust domain logic to be reimplemented casually in TypeScript.

Open question: whether browser persistence uses an SQLite/WASM virtual filesystem, an archive import/export workflow, or another local-only storage adapter.

## ADR-012 — Prefer concise, shallow code over framework hierarchies

Status: Accepted

Methods and functions are short and single-purpose; names are concise but not cryptic; modules have clear ownership; class and trait hierarchies remain shallow; composition is preferred over inheritance.

Reasoning: this application spans Rust, TypeScript, SQL, WebGL, GPU APIs, and IPC. Deep abstraction layers would make cross-boundary behavior difficult to trace. A small public API and consistent domain vocabulary make calculations and renderer synchronization easier to test.

Consequence: code review and CI enforce dependency direction, shared-component boundaries, generated-type consistency and focused units. There is no arbitrary line-count rule; responsibility and comprehensibility are the standard.

## ADR-013 — Keep Viz separate from the built-in Stage visualizer

Status: Accepted

The high-quality Viz renderer and the Stage visualizer embedded in ToskLight
are separate runtime products. The embedded Stage currently remains a retained,
lightweight WebGL renderer inside `apps/light-desktop`; it consumes a dedicated,
bounded visualization feed and prioritizes Live/Preload latency and engine/DMX
isolation. A later implementation may replace only that viewport's pixels with
an in-process native `wgpu` surface while leaving the surrounding React pane and
window system intact. The Viz renderer remains a supervised Rust process with
its own native rendering surface and quality budget.

The built-in Stage does not stream video from `apps/viz-renderer`, and Viz is
not required for ToskLight to visualize a show. The two renderers may share
fixture definitions, model assets, semantic resolved-value contracts, and
benchmark scenes. They may also share a renderer-core crate if the desk owns
its own renderer instance and lifecycle; they do not share a running renderer
process or make one application depend on the other.

Reasoning: a rendered video path adds process startup, GPU readback,
encoding/decoding, and presentation latency to the desk's operational surface,
and would make basic Stage feedback depend on the much larger Viz delivery.
Conversely, forcing high-quality rendering into the Tauri WebView would weaken
the dedicated renderer's quality and isolation goals.

Consequence: high-quality materials, shadows, haze, volumetric occlusion,
recording, and advanced optics belong to Viz. The embedded Stage may use a
lower-cost quality profile of the shared render core, but it must still meet
the independent performance contract in
[`../../refactoring/doing/21-efficient-built-in-stage-visualizer.md`](../../refactoring/doing/21-efficient-built-in-stage-visualizer.md).

## ADR-014 — Evaluate an embedded native `wgpu` Stage viewport

Status: Provisional

Keep the existing ToskLight React component hierarchy and window-management
behavior. The Stage React component reserves and reports the rectangular
viewport. A platform adapter places a native GPU child surface at that physical
rectangle and follows moves, docking, resize, visibility and display-scale
changes. React continues to own the Stage toolbar, surrounding controls and
pane lifecycle; `wgpu` owns only the pixels and direct interaction inside the
viewport.

This is not a frameset, nested WebView or image/video stream. The native surface
is a sibling of the WebView inside the same operating-system window and only
appears to occupy the React placeholder. The renderer receives bounded,
latest-state typed snapshots in process and must never wait on or backpressure
the output engine.

Phase 0 must test:

- native child-surface creation, movement, clipping, DPI and resize on macOS,
  Windows and supported Linux window systems;
- mouse, touch, wheel, focus, selection and camera-input routing;
- z-order behavior for dialogs, menus, tooltips and panes crossing the viewport;
- hiding or detaching the surface while the pane is dragged or moved to another
  Tauri window;
- renderer failure containment and resource recreation; and
- Stage load with the maintained fixture-count benchmarks while DMX output and
  the rest of the desk remain within their independent latency budgets.

If native child-surface integration is not robust on a supported platform, keep
the current retained WebGL Stage there until a proven adapter exists. Do not
fall back to streaming rendered frames into the WebView as the primary desktop
architecture because GPU readback, copying, encoding and presentation would
reintroduce avoidable latency.

## Open decisions

The canonical plan maintains the complete decision list. The most consequential unresolved items are:

- final open-source license;
- show-package extension and internal packaging;
- supported Linux distributions and package formats;
- selected and pinned Rust graphics stack after the spike;
- representative visual benchmark scenes and minimum GPU;
- exact scope and safety wording for preliminary rigging calculations; and
- the initial browser-edition persistence strategy, if that edition is scheduled.

When one of these is settled, update both the canonical plan and the applicable ADR rather than leaving contradictory statements.
