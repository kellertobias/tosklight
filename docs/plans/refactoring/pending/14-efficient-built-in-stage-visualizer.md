# Efficient Built-in Stage Visualizer

This plan follows the currently claimed
[`Dynamics`](../doing/16-dynamics/README.md) plan.

## Status

**IMPLEMENTABLE.** This plan defines the refactor, settings, and acceptance
contract for the Stage visualizer embedded in ToskLight.

This remains a planning contract. Updating it does not implement the renderer,
transport, benchmarks, executable tests, or UI changes. Implementation starts
only after Dynamics is complete and this file moves from `pending/` to
`doing/` under the refactoring queue workflow.

This plan supersedes the earlier proposal to add an opt-in high-fidelity profile
to the embedded Stage renderer. High-quality rendering belongs to the separate
[`viz` application](../../Later/viz/README.md), not to the live desk surface.

## Product boundary

ToskLight has two deliberately different visualization products:

- The **built-in Stage visualizer** remains inside `apps/light-desktop`. It is an
  always-available operational view of authoritative Live or Preload output and
  a practical way to build and inspect a show without connected fixtures. Its
  priorities are correctness, low latency, low overhead, and predictable
  availability.
- The future **Viz application** owns realistic materials, shadows, haze,
  volumetric occlusion, high-quality optics, recording, and other sophisticated
  rendering. It lives in `apps/viz-editor` and `apps/viz-renderer`, with a
  supervised Rust renderer process.

The built-in visualizer must not wait for Viz to be implemented and must not
embed, supervise, or stream video from the Viz renderer. The two products may
share fixture definitions, geometry contracts, asset caches, resolved-value
types, and engine-neutral test scenes, but neither renderer is a runtime
dependency of the other.

## Operator outcome

The built-in Stage visualizer must:

1. Show what the engine is producing on **Live** or what the selected desk will
   produce on **Preload**.
2. Show unpatched fixtures, multi-patch instances, logical heads, Venue scenery,
   fixture motion, intensity, color, zoom, focus, beam direction, Grand Master,
   blackout, Highlight, and current selection according to their existing
   operator semantics.
3. Remain useful for offline show preparation with no physical fixtures or DMX
   output connected.
4. Stay within 200 ms of the corresponding authoritative engine frame. The
   normal target is a 10 Hz authoritative value feed, with smooth local display
   between samples while values are changing.
5. Consume so little CPU, GPU, memory, network, and server time that the
   operator can leave one or more Stage surfaces open without thinking about
   their cost.
6. Never delay engine evaluation, playback, Programmer transitions, command
   handling, or DMX/network output.

The embedded view remains a visualization and programming aid. It is not
photometric proof and does not attempt the dedicated Viz application's quality
ceiling.

## Stage settings contract

The refactor preserves the existing settings and adds one **Render quality**
choice. Do not rename, remove, merge, or change the meaning of the existing
settings:

- **Group shortcuts** controls whether the Group shortcut strip is shown.
- **Show selection** controls whether selected fixtures receive selection
  decoration inside the 3D view.
- **Floor grid** controls the neutral floor plane and its grid. It does not
  suppress a line-mode beam footprint at the ground reference.
- **Beam direction guidelines** controls whether directional emitters that are
  off show their existing dotted aim guide. Broad and non-directional sources
  never gain an invented guideline.

Add **Render quality** with these choices in this exact order and wording:

1. **Lines only**
2. **Lines + beams**
3. **Beams**
4. **Improved beams**

Use the internal type:

```ts
type StageRenderQuality =
	| "lines_only"
	| "lines_and_beams"
	| "beams"
	| "improved_beams";
```

### Settings ownership and persistence

Render quality is a display preference, not show content:

- the full Stage window stores its own choice with the existing desk-layout
  Stage settings;
- every Stage pane stores its own choice in that pane's settings;
- changing one Stage surface does not change another;
- new and legacy layouts default to **Lines + beams**, preserving the current
  active-beam-plus-outline appearance;
- the value is not written to the portable show, fixture profile, Stage layout,
  patch, Cue, or engine state; and
- unknown future values migrate safely to **Lines + beams** rather than
  blanking the Stage.

Patch Preview Stage, deterministic help screenshots, Cue thumbnails, stories,
and the product-demo baseline use an explicit render quality rather than
depending on a user's persisted setting. Their migration default is
**Lines + beams** because it most closely preserves the current checked-in
appearance.

### Interaction with Beam direction guidelines

Render quality controls how an **active** directional emitter is drawn. **Beam
direction guidelines** remains the independent control for an **off**
directional emitter:

| Render quality | Active directional emitter | Off directional emitter when guidelines are on | Off directional emitter when guidelines are off |
|---|---|---|---|
| **Lines only** | Center line and ground-footprint outline; no beam volume | Existing dotted direction guideline only | Hidden |
| **Lines + beams** | Existing beam volume plus center line and ground-footprint outline | Existing dotted direction guideline only | Hidden |
| **Beams** | Existing beam volume without the active center line or footprint outline | Existing dotted direction guideline only | Hidden |
| **Improved beams** | Feathered beam volume and, when capability/performance gates pass, bounded surface lighting, occlusion, and shadows | Existing dotted direction guideline only | Hidden |

Broad and non-directional sources show their emissive source surface in every
quality. They do not receive center lines, ground footprints, directional beam
cones, or shadow-casting spotlights merely because a quality is selected.

Every quality that draws an active beam must use the authoritative resolved
color and intensity and the fixture's authored beam angle, field angle,
feather, focus, and resolved zoom/focus values. A quality switch changes only
the representation; it does not change which values or fixture geometry drive
the result.

### Line and footprint geometry

The line modes are functional aiming views, not decorative overlays:

- the center line starts at the exact emitter origin and follows its complete
  fixture geometry hierarchy, mounting transform, pan, tilt, and emitter
  orientation;
- the ground-footprint outline is the intersection of the emitter's field
  angle with the Stage ground reference;
- it appears circular when the beam is perpendicular to the ground and
  elliptical when the beam strikes at an angle;
- its size respects the resolved beam/field angle and zoom;
- active lines and footprints use the resolved color and a bounded,
  intensity-aware opacity that remains readable at low output;
- Grand Master, blackout, Highlight, Live, and Preload affect the same
  authoritative active/off state as the corresponding beam;
- focus may affect the distinction between the inner beam indication and outer
  field outline where the fixture data supports both;
- a beam parallel to the ground or aimed away from it shows the center line but
  does not invent a ground footprint; and
- hiding **Floor grid** does not hide the footprint outline.

The line and footprint geometry is retained and updated in place. It must not
rebuild the fixture or allocate new geometry for every visualization sample.

## Decision summary

Keep Three.js and WebGL in the embedded Stage for the first implementation.
The current technology is adequate once the renderer stops rebuilding the
scene and stops drawing unnecessary frames. Replacing the graphics library
before measuring the retained-mode implementation would add migration risk
without addressing the known allocation, polling, and transparent-overdraw
costs.

Do not build the Rust Viz renderer first and feed its output back as video.
That design would add process startup, scene synchronization, GPU readback,
video encoding, transport, decoding, and presentation latency to a surface
whose primary requirement is low-cost operational feedback. It would also make
the desk depend on the substantially larger Viz delivery schedule.

OffscreenCanvas or a render worker is a permitted later optimization only if
the retained Three.js renderer still causes measurable main-thread contention
on supported Tauri WebViews. It is not part of the initial architecture and
must be capability-tested on Windows, macOS, and Linux before adoption.

**Improved beams** remains deliberately smaller than the dedicated Viz
renderer. Soft edge falloff is required. Surface illumination, beam termination
at opaque scenery, and cast shadows are implemented in the built-in renderer
only when the Phase 0 capability and performance spike proves a simple bounded
implementation can satisfy the same 200 ms latency and engine/DMX-isolation
gates as the other qualities.

## Current path to replace

The current implementation:

- polls a complete visualization snapshot every 200 ms for each claimed lane;
- rebuilds the complete Three.js scene when a visualization snapshot, selection,
  environment option, or fixture collection changes;
- reconstructs geometry and materials for fixture bodies, emitter surfaces,
  beam volumes, beam cores, outlines, and guides;
- disposes the previous scene after each rebuild;
- fetches and parses GLB assets without a renderer-wide decoded-model cache;
- renders continuously through `requestAnimationFrame`, including while the
  scene and camera are idle;
- enables antialiasing, `preserveDrawingBuffer`, and a device-pixel-ratio cap of
  2 for every built-in Stage canvas; and
- allows separate visualization consumers and surfaces to multiply polling and
  projection work.

The first implementation slice must instrument this path in the packaged app.
Source inspection and unit timing are not substitutes for measuring the
complete engine-frame-to-visible-canvas path.

## Target architecture

```text
Engine and playback evaluation
        │
        ├── DMX/network output path
        │     never awaits or calls visualization work
        │
        └── latest resolved-frame publication
              bounded, overwrite-old, non-blocking
                        │
              visualization projection task
              samples the latest frame at 10 Hz
              builds shared Live/Preload deltas
                        │
              dedicated visualization WebSocket
              bounded latest-value delivery
                        │
              desktop visualization runtime store
              one connection and one lane claim per
              desktop process, never per Stage pane
                        │
              retained Three.js Stage scenes
              apply values in place and render only
              when dirty or interpolating
```

The engine, transport, React state, and Three.js renderer remain separate
owners. No renderer object, browser callback, JSON serialization, socket write,
or client-specific delta calculation may execute on the engine or DMX-output
critical path.

## Server publication and output isolation

### Non-blocking frame publication

After producing an authoritative resolved frame, the engine publishes an
immutable reference or compact value projection to a capacity-one,
latest-value channel. Publication must be non-blocking:

- the engine never awaits capacity;
- a new frame replaces an unread older visualization frame;
- a missing, slow, disconnected, or crashing consumer has no effect on engine
  or output timing;
- no network or client lock is held while the engine publishes;
- no JSON or binary serialization happens on the engine thread; and
- Live and Preload remain separate authoritative lanes.

Dropping an intermediate visualization frame is correct. Delaying DMX output
to preserve every visualization frame is not.

### Projection task

A visualization-owned Rust task reads the newest available engine state on a
fixed 10 Hz cadence while at least one client has claimed a lane. It:

- performs the resolved-output-to-visualization projection away from the
  engine/output thread;
- computes one shared ordered frame or delta per active lane, not one projection
  per client or pane;
- reuses serialized immutable payloads for clients at the same revision where
  practical;
- does no work for an unclaimed lane;
- records projection duration, payload size, source-frame age, skipped source
  frames, and subscriber count; and
- shuts down its cadence when no visualization client is subscribed.

The target rate is 10 Hz. Five hertz is a degraded lower bound, not the normal
acceptance target, because a 200 ms sampling interval leaves no latency budget
for transport and presentation.

## Dedicated visualization WebSocket

Add one internal WebSocket endpoint dedicated to visualization telemetry. Its
provisional route is:

```text
GET /api/v2/visualization/stream
```

It follows `docs/engineering/api-rules.md`:

- the route has no show or desk path segment;
- the optional `X-Tosk-Show` guard protects against a show-switch race;
- desk context uses the authenticated socket session or `X-Tosk-Desk`;
- volatile state is pushed rather than polled;
- wire messages are typed under `crates/light/contracts/wire`;
- unknown fields are tolerated and logged without values; and
- the existing authentication and trusted-lighting-network rules apply.

The visualization socket is separate from the desk's live-control WebSocket so
large visualization frames and slow browser consumption cannot create
head-of-line blocking for operator actions. A separate socket does not by
itself provide thread isolation; the server must also run projection,
serialization, and socket writes outside the engine/output task as described
above.

### Subscription ownership

One desktop visualization runtime owns the connection and multiplexes every
visible Stage consumer in that desktop process:

- a Live Stage claims the Live lane;
- a Follow Preload Stage claims the Preload lane;
- duplicate panes share the same lane feed;
- hidden, closed, or inactive surfaces release their claims;
- the socket closes when no visualization surface needs either lane; and
- additional Tauri screen processes may own their own connection, but never one
  connection per pane.

The wire may retain `"normal"` as the internal lane identifier for compatibility,
while operator-facing text remains **Live**. `"preload"` remains the Preload
lane identifier.

### Message contract

The versioned protocol must provide:

- `hello` and server capabilities;
- `subscribe` and `unsubscribe` with requested lanes and a maximum rate of
  10 Hz;
- a full lane snapshot after subscription, reconnection, show change, or
  explicit resynchronization;
- ordered incremental batches for fixture/head values, Grand Master, blackout,
  Highlight, and other render-relevant volatile state;
- structural invalidation when patch, fixture profile, geometry, model asset,
  multipatch, Stage position, or Venue data changes;
- monotonic sequence and source-frame numbers;
- a source timestamp and server publication timestamp;
- heartbeat and observable connection state;
- client detection of a sequence gap and an explicit full-snapshot request; and
- protocol/version mismatch diagnostics rather than a blank Stage.

Structural show data remains in the appropriate authoritative patch, profile,
and Stage-layout projections. The high-rate socket must not repeatedly send
complete fixture definitions, GLBs, the show database, or unrelated bootstrap
state.

Each client has a bounded latest-value outgoing queue. A client that cannot
keep up loses intermediate visualization batches and receives the newest
coherent batch or a fresh snapshot. It must never accumulate an unbounded
backlog. Repeated failure to consume within the budget closes only that
visualization connection.

When the visualization socket is healthy, Stage must perform no periodic HTTP
visualization reads. Reconnection obtains one authoritative full snapshot; it
does not replay stale client-side frames or operator actions.

## Retained embedded renderer

### Structural scene

Create one persistent scene runtime per visible 3D Stage surface. Build or
replace scene objects only for structural changes:

- fixture or multipatch instance added or removed;
- fixture mode, profile geometry, or retained model asset changed;
- Stage position or mounting transform changed;
- Venue scenery changed;
- floor-grid or other structural display option changed; or
- a WebGL context is created or recovered.

Retain stable handles by fixture instance, logical head, geometry node, emitter,
and source identity. Selection and Highlight update existing decorations and
materials rather than rebuilding fixture hierarchies.

### Live value application

Apply visualization batches directly to retained objects:

- update pan/tilt and other geometry-node transforms;
- update intensity, color, zoom, focus, beam visibility, and material uniforms;
- update Grand Master, blackout, Live/Preload, and virtual Highlight state;
- mark only affected fixtures or shared environment state dirty; and
- ignore an older sequence or a batch for a stale show/desk scope.

React installs connection state and coarse structural state. Frame-by-frame
fixture values do not pass through React component reconciliation.

### Resource ownership

Add renderer-scoped caches for:

- fetched and decoded GLBs keyed by stable asset identity and revision;
- cloneable model templates;
- shared fixture-body and marker geometry;
- beam cone/core geometry, preferably transformed or scaled instead of rebuilt
  for every zoom value;
- shared materials with per-object color/intensity data where safe; and
- selection/guide geometry.

Reference-count or otherwise explicitly own cached resources. Removing a
fixture, closing the last Stage surface, changing an asset revision, or losing
the WebGL context must dispose the correct resources without disposing
resources still used by another instance.

### Beam cost

The three basic qualities keep the current schematic additive-beam language.
They create no shadow maps, surface lights, atmosphere, ambient occlusion,
bloom, or post-processing.

- Inactive emitters do not submit zero-opacity beam-volume or beam-core draw
  calls.
- Off-state direction guides remain available only for directional emitters.
- Broad/non-directional sources remain emissive surfaces without invented
  directional cones or guides.
- Repeated matrix, ring, and strip sources use shared geometry or instancing
  where profiling proves a benefit.
- Selection outlines are created only for selected objects and must retain the
  empty-geometry safety behavior.
- Distance and frustum culling may omit detail, but never omit an on-screen
  active fixture's essential intensity, color, or direction.

### Improved beams

The minimum **Improved beams** implementation is a retained beam shader with
visible but subtle falloff from its brighter core toward its field edge. It
must respect:

- resolved color and intensity;
- authored beam angle, field angle, feather, and focus;
- resolved zoom and focus attributes;
- emitter origin, orientation, multi-source layout, logical head, and complete
  fixture movement hierarchy;
- Grand Master, blackout, Highlight, Live, and Preload; and
- broad/non-directional source behavior.

The falloff must not be simulated by stacking many transparent cone meshes.
Use one bounded shader/material path or another measured approach that reduces,
rather than multiplies, transparent overdraw.

During Phase 0, build a contained technical spike for surface illumination,
beam occlusion, and shadows using the retained Three.js renderer. The preferred
bounded approach is:

- opaque Stage, Venue, and fixture-body geometry writes usable depth;
- improved beam volume is clipped at visible opaque intersections;
- eligible active directional emitters may illuminate intersected surfaces;
- a fixed budget of at most eight highest-contributing directional emitters may
  allocate shadow resources;
- stable fixture/head/emitter/source identity breaks equal scores;
- hysteresis prevents rapid shadow-budget churn; and
- sources outside the budget retain the feathered beam but create no shadow
  map.

The spike must include a fixture aimed at a Stage element, a separate occluder,
and a receiving surface. It succeeds only if the Stage element visibly receives
the resolved color, the beam does not visibly continue through the first opaque
occluder from the accepted camera views, the occluder casts a stable shadow,
and the complete large-scene benchmark remains within budget.

If that spike passes, surface illumination, bounded occlusion, and shadows are
required parts of **Improved beams**. If it fails on a supported graphics
capability or performance gate:

- keep the required feathered falloff;
- do not ship fake or visibly incorrect beam termination;
- record the failed technique and measurements in the plan result;
- disable only the unavailable illumination/occlusion extension with a visible
  capability explanation; and
- leave advanced physically richer behavior to Viz.

The basic three qualities must never allocate improved-beam lights, shadow
maps, depth targets, or improved materials.

### Render scheduling

Do not run an unconditional idle animation loop.

Request rendering when:

- a live-value batch changes visible state;
- an interpolation remains active;
- camera controls move or damping has not settled;
- the canvas resizes;
- selection, Highlight, Stage options, or structural scene data changes; or
- a context-recovery redraw is required.

Stop requesting frames once the scene is settled. While camera interaction or
live interpolation is active, render at the display cadence subject to the
device budget. This allows a 10 Hz authoritative feed to show smooth curves
without pretending the server produced unauthoritative intermediate values.

Continuous numeric attributes such as intensity, pan, tilt, color, zoom, and
focus may move from the currently displayed value to the newest authoritative
sample over a bounded interval no longer than the next 10 Hz publication
window. Discrete changes, blackout, selection, and connection state apply
immediately. The client must converge to each newest authoritative sample,
must not extrapolate beyond it, and must discard interpolation toward a
superseded sequence.

### Canvas budget

The initial efficient settings are:

- device pixel ratio capped at 1.25;
- antialiasing retained only if the packaged benchmark stays within budget;
- no shadow map or post-processing allocation in **Lines only**,
  **Lines + beams**, or **Beams**;
- a fixed measured GPU-resource budget for **Improved beams**; and
- `preserveDrawingBuffer` removed from the live Stage if focused capture,
  Playwright screenshot, help screenshot, and operator screen-recording tests
  prove that it is unnecessary.

Cue thumbnails continue to use their separate deterministic, one-shot renderer.
Dynamic resolution while moving the camera is allowed only if it is visually
stable and the full resolution returns immediately when movement settles.

## Failure and recovery

- Before the first snapshot, Stage shows a lightweight connecting state without
  blocking the rest of the pane.
- A broken visualization connection freezes the last coherent scene, marks it
  visibly stale, and reconnects with bounded backoff.
- Reconnection or a sequence gap replaces volatile state from a full snapshot.
- A missing or invalid model uses the existing placeholder and reports the
  asset problem without blanking the scene.
- WebGL context loss is isolated to that Stage surface and rebuilds from
  structural state plus the newest authoritative visualization snapshot.
- A renderer exception or slow Stage never closes the live-control WebSocket,
  changes engine state, or stops DMX output.
- If the server must shed load, visualization telemetry is dropped before
  live-control or output work.

## Performance and safety acceptance

Create deterministic packaged-app benchmarks for:

- the Default Stage Show;
- a large show with 500 fixture instances, representative moving heads,
  multi-source washes/blinders, multi-patch instances, and Venue scenery;
- every render quality, including rapid switching without resource retention;
- the Improved-beams occluder scene with a lit Stage element and cast shadow
  when that capability passes its spike;
- Live and Preload changing simultaneously in separate Stage surfaces;
- a stalled visualization client;
- repeated Stage open/close, 2D/3D switching, show switching, and WebGL context
  recovery; and
- one main window plus representative additional-screen consumers.

### Visual latency and cadence

Instrument the same resolved engine frame through publication, projection,
WebSocket send/receive, value application, and canvas render submission.

- Normal publication is 10 Hz while values change and a lane is claimed.
- The packaged local path has a target p95 engine-frame-to-canvas latency of
  120 ms.
- No measured changing frame may exceed 200 ms from authoritative engine frame
  to the corresponding canvas submission during the five-minute canonical
  transition run.
- There is no changing-state presentation gap longer than 200 ms.
- Continuous values move smoothly between authoritative samples without
  overshoot or prediction.
- Live and Follow Preload surfaces retain independent correct lane state.
- Every render quality uses current resolved color, intensity, movement, and
  beam/field angle within the same latency budget.

The benchmark reports each stage separately. A fast WebSocket receive time
does not prove a fast visible canvas, and a high canvas frame rate does not
prove current authoritative values.

### Engine and DMX isolation

Run the same engine/output stress sequence with no Stage, one Live Stage, Live
plus Preload, the large scene, and a deliberately stalled client.

- The engine/output path performs zero awaited visualization operations and
  zero client-specific work.
- No visualization condition causes a missed DMX/network-output deadline.
- Engine/output p99 work time may regress by no more than 1 ms or 5 percent,
  whichever is larger, compared with the same run without visualization.
- A stalled client retains at most one pending latest-value batch and does not
  increase engine, output, or other client latency.
- Opening duplicate panes does not multiply engine projection work.
- Projection, serialization, and socket metrics remain separately observable.
- Selecting **Improved beams** may consume a separately recorded GPU budget,
  but it may not weaken any engine, control, network-output, or DMX gate.

These are release gates, not development-only aspirations. If the target
hardware cannot meet them, reduce built-in rendering work or update the
measured hardware support contract; do not weaken engine/output isolation.

### Renderer resource stability

- A settled scene performs no idle renders.
- A visualization batch causes no full-scene rebuild.
- Repeated ten-hertz updates do not create new fixture hierarchies, decoded
  models, beam geometry, or materials.
- Switching render quality replaces or toggles only quality-owned resources and
  does not rebuild fixture/model hierarchies.
- Basic qualities allocate zero improved-beam lights, depth targets, or shadow
  maps.
- The large scene maintains responsive camera interaction while live values
  update.
- A 30-minute changing-show run has bounded CPU, GPU, and memory use.
- Repeated show switches and Stage open/close cycles leave no retained WebGL
  contexts, scenes, models, socket claims, timers, or animation loops.

Record draw calls, triangles, transparent draw calls, CPU frame time, GPU frame
time where available, JavaScript heap, GPU-resource counts, payload bytes,
projection time, and dropped/coalesced visualization frames.

## Correctness acceptance

Focused unit, contract, integration, and packaged visual checks must cover:

1. Live versus Preload lane subscription, switching, simultaneous surfaces,
   reconnect, show/desk scope changes, and sequence-gap recovery.
2. Programmer, Cue, Playback, Highlight, Grand Master, blackout, and transition
   values matching the authoritative engine projection.
3. Unpatched fixtures remaining visible and programmable while producing no
   DMX output.
4. Ordered logical heads, profile geometry, multipatch transforms, fixture
   mounting, Stage positions, Venue scenery, model assets, and conservative
   fallback geometry.
5. Pan/tilt hierarchy, intensity, color systems, zoom, focus, directional
   emitters, broad sources, and inactive guides.
6. Exact render-quality ordering, persistence, legacy default, per-surface
   independence, and the active/off guideline interaction table.
7. Lines-only center and ground-footprint geometry respecting movement,
   mounting, color, intensity, beam/field angle, zoom, floor-grid independence,
   parallel rays, and rays aimed away from the ground.
8. Lines + beams and Beams differing only in their active line/footprint
   overlay while preserving the same authoritative beam state.
9. Improved-beam edge falloff, color, beam angle, focus, zoom, and bounded
   resource behavior; if the capability spike passes, also surface lighting,
   first-occluder termination, cast shadows, the eight-emitter budget, stable
   ordering, and hysteresis.
10. Selection gestures and outlines, including empty geometry and the Sunstrip
   regression.
11. One shared desktop lane feed for duplicate panes and release when surfaces
   become inactive.
12. A slow, malformed, unauthorized, or incompatible visualization client
   remaining isolated from control and output behavior.
13. Cue-thumbnail generation remaining deterministic and independent of the live
   Stage render loop.
14. The built-in visualizer starting and working without Viz installed,
    running, or implemented.

Add a focused `STAGE-001` operator scenario using the Default Stage Show with
fixed camera positions. It must compare a Live surface with a Follow Preload
surface, execute a fade and moving-light transition, prove the latency/cadence
marks, select fixtures through the real Stage interaction path, disconnect and
recover the visualization socket, switch through all four render qualities,
prove line/footprint geometry and the accepted Improved-beam capability, and
confirm output continues throughout.

## Delivery sequence

### Phase 0 — packaged baseline

1. Add timing marks and resource counters without changing behavior.
2. Measure the existing HTTP polling, scene rebuild, render loop, draw-call,
   memory, and engine/output costs in `./build open`.
3. Capture Default Stage and large-scene baselines.
4. Spike the bounded Improved-beam falloff, surface-lighting, first-occluder,
   and shadow path without integrating it into production settings.
5. Record supported reference hardware, WebGL capabilities, accepted/rejected
   Improved-beam techniques, and the exact measurement method.

Gate: the complete operator-visible and DMX-output paths are measured
separately. Do not select a new renderer technology from source inspection
alone.

### Phase 1 — isolated visualization transport

1. Add the latest-value engine publication boundary.
2. Add the claimed-lane projection task and typed dedicated WebSocket.
3. Add full snapshot, delta, sequencing, scope, reconnection, and metrics.
4. Add stress coverage proving that slow and disconnected clients cannot block
   engine or output.
5. Keep the existing Stage consumer available until the new path passes
   correctness and isolation gates.

Gate: the dedicated stream meets the 10 Hz and 200 ms contracts while the
engine/DMX isolation benchmark passes.

### Phase 2 — retained Stage scene

1. Separate structural scene construction from live-value application.
2. Add stable object handles and in-place attribute updates.
3. Add GLB, geometry, and material caches with explicit disposal.
4. Replace unconditional rendering with dirty/interpolation scheduling.
5. Reduce inactive-beam and transparent-overdraw cost.
6. Add retained center lines and ground-footprint outlines.
7. Add the four render qualities and exact existing-settings interaction.
8. Add the required feathered Improved-beam path and the illumination,
   occlusion, and shadow extension if its Phase 0 gate passed.
9. Apply the per-quality canvas and GPU-resource budgets.

Gate: ten-hertz updates perform no scene rebuild or resource growth, and the
Default Stage passes exact visual-regression review.

### Phase 3 — integrate and retire polling

1. Move Stage 2D and 3D Live/Preload consumers to the shared visualization
   runtime.
2. Ensure duplicate panes and additional screens claim only required lanes.
3. Add bounded continuous-value interpolation outside React reconciliation.
4. Remove Stage's periodic visualization HTTP reads after the WebSocket path is
   authoritative.
5. Preserve one-shot authoritative reads for consumers that genuinely need a
   single snapshot rather than a subscription.

Gate: the full correctness, visual latency, resource, and output-isolation
acceptance suite passes in the packaged desktop app.

### Phase 4 — scale and cross-platform hardening

1. Profile large fixtures, multi-source emitters, Venue GLBs, and several Stage
   surfaces.
2. Add only evidence-backed instancing, culling, dynamic resolution, or worker
   isolation.
3. Validate Windows, Apple Silicon macOS, and representative Linux GPUs.
4. Validate WebGL context loss, app suspend/resume, show switching, and renderer
   teardown.
5. Publish minimum and recommended hardware based on measurements.

Gate: the built-in visualizer can remain open during normal operation on
supported hardware without noticeable lag or output regression.

### Phase 5 — documentation and completion

1. Update the Stage help with its Live/Preload purpose and performance boundary.
2. Document the four exact render qualities, existing settings, line/footprint
   meaning, and any visible Improved-beam capability limit.
3. Update deterministic Stage settings and view screenshots only through the
   documented workflow.
4. Record the dedicated Viz application's separate quality responsibility.
5. Run focused frontend/server checks, `STAGE-001`, the performance suite,
   desktop smoke, and the authoritative `./build open` workflow.
6. Inspect `.artifacts/runtime/light-data/light-headless.log`, readiness, and
   visualization metrics during packaged verification.

Gate: add implementation evidence and a result section before moving this plan
to Done. Documentation or static checks alone are not runtime proof.

## Explicit non-goals

The built-in visualizer does not add:

- a general high-fidelity profile or detailed quality controls beyond the four
  specified render qualities;
- unbounded surface lights or shadow maps, haze, fog, bloom, ambient occlusion,
  cinematic tone mapping, path tracing, or photometric calibration;
- IES import, projected gobo textures, rich media playback, particles, or
  offline rendering;
- video encoding, a rendered video stream, or dependency on
  `apps/viz-renderer`;
- a second fixture/profile/show schema;
- browser-side engine, playback, Programmer, or Preload evaluation; or
- any code path that writes live values or DMX from the visualizer.

Those rendering capabilities belong to the separate Viz plan. The built-in
Stage consumes authoritative ToskLight state and remains intentionally simple.
