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
- Reusable React presentation belongs in `packages/ui`.

Reasoning: fixtures, patching, show data, MVR/GDTF handling and live values already cross several Light applications. A separate repository would encourage duplicate models, conversions and release coordination. Proposed crate names in the main plan are ownership suggestions, not permission to recreate behavior already present elsewhere.

## ADR-002 — Use Tauri and React for the application shell

Status: Accepted

The main application uses Tauri, React and TypeScript. React owns the UI composition, inspectors, lists, dialogs and workflows. It does not redraw the planning canvas through component reconciliation on every frame.

Reasoning: this preserves the existing ToskLight frontend stack, UI investment and product appearance while avoiding Electron. Tauri provides a comparatively lightweight desktop shell and a Rust integration boundary on Windows, macOS and Linux.

## ADR-003 — Reuse the shared UI library

Status: Accepted

Common controls come from `packages/ui`. Shared components remain presentational and accept typed values, view models and callbacks. Editor state, Tauri calls, document services and renderer coordination remain in app-owned adapters under `apps/viz-editor`.

Reasoning: visual consistency is a product requirement, and copying controls would create divergent behavior and styling. Storybook gives reusable components a deterministic environment before they are integrated into the live application.

Consequence: a broadly reusable missing component is added and tested in `packages/ui` first. A product-specific panel may remain in the planner app, but it still composes shared primitives and design tokens.

## ADR-004 — Use a retained WebGL editor for planning views

Status: Accepted

The synchronized top, side, front/back and isometric planning views use a retained WebGL2 renderer managed by TypeScript. React coordinates tools and document state, while camera movement, hover, drag previews and GPU resources remain viewport-local.

Reasoning: the planning view needs fast interaction with many objects, consistent cross-platform browser support and a path to a reduced browser-based edition. Ordinary React DOM rendering is unsuitable for dense geometry, while a native-only planning viewport would make reuse on the web substantially harder.

Consequence: the editor renderer favors clear geometry, selection, snapping, wireframe, hidden-line and flat-shaded views. It is not required to reproduce the high-quality lighting simulation.

## ADR-005 — Implement the high-quality visualizer in Rust

Status: Accepted

The native high-quality renderer is implemented in Rust. Phase 0 must select and
prove the Rust graphics stack against fixture counts, moving shadowed lights,
fog, volumetric beams, and cross-platform packaging.

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
