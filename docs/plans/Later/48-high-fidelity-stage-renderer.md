# High-Fidelity Stage Renderer

## Status

**Specification only.** This feature is planned for later. It defines the Stage rendering and acceptance contract but does not implement the renderer or executable tests.

## Goal

Add two independently selectable render profiles to every full Stage window and Stage pane:

- **Efficient** remains the default and prioritizes live-operation performance.
- **High fidelity** adds realistic volumetric beams, illumination, beam occlusion, soft shadows, improved materials, haze, bloom, ambient occlusion, and cinematic tone mapping.

Quality is selected independently for each Stage surface but resets to **Efficient** whenever ToskLight launches. Portable shows and persisted desk layouts remain unchanged. Stage remains a visualization and programming aid rather than calibrated photometric proof.

## Renderer architecture

Refactor the current Three.js Stage renderer into a persistent scene runtime:

- Build fixture hierarchies, Venue scenery, GLB models, shared geometry, and materials only when structural data changes.
- Retain handles for emitters, lights, beams, moving parts, selections, and transforms.
- Apply each visualization snapshot by updating existing objects and shader uniforms in place instead of rebuilding the scene.
- Cache decoded GLBs and shared resources, and dispose quality-specific resources when switching modes or removing fixtures.
- Preserve authoritative resolved output, Grand Master, blackout, Follow Preload, Highlight, multi-head ownership, multipatch positioning, selection, and Stage setup behavior in both profiles.

### Efficient profile

Efficient mode must remain suitable for live operation:

- Preserve the existing additive-beam appearance and resolved-output behavior.
- Create no surface lights, shadow maps, atmosphere, or post-processing passes.
- Render only when the scene becomes dirty, a live snapshot arrives, the surface resizes, or camera interaction and damping remain active. Do not run an idle animation loop.
- Cap device pixel ratio at 1.25.
- Retain deterministic video-capture behavior until a test-backed replacement for `preserveDrawingBuffer` exists.

### High-fidelity profile

High fidelity is one curated visual preset rather than a collection of detailed operator controls. It should provide:

- ACES filmic tone mapping;
- a neutral procedural image-based environment;
- PBR procedural materials while preserving authored GLB PBR materials;
- GTAO, selective bloom, and fixed subtle atmospheric haze;
- directional surface illumination aligned to each directional emitter; and
- soft-edged volumetric scattering derived from beam angle, field angle, feather, focus, zoom, intensity, and resolved color.

Broad and non-directional sources render as emissive surfaces with broad area illumination. They must not gain invented directional cones, aim guides, or shadows.

Use a half-resolution, 32-step volumetric pass. Every beam is depth-clipped against opaque scene geometry. Shadow-map sampling adds internal occluder silhouettes to the budgeted beams. Appropriate fixture meshes and Venue GLBs cast shadows; the floor, scenery, and fixture bodies receive them. Metre-authored Venue scale and fixture-geometry motion hierarchies remain authoritative.

## Shadow budget

Render every active light and beam, but allocate 1024-pixel PCF-soft shadow maps to at most eight active directional emitters.

Rank candidates by resolved intensity, use stable fixture, head, emitter, and source IDs to break ties, and require a 10% score advantage before replacing an allocated light. This prevents shadow-map churn when two sources have nearly equal levels.

Non-budgeted beams remain depth-clipped and continue to illuminate surfaces, but they do not produce complete internal geometry silhouettes or cast surface shadows.

## Fixture compatibility

Use authored geometry emitters when a fixture profile provides them. When an embedded DMX fixture profile has no emitter metadata, synthesize the existing conservative point-emitter behavior from its available head, movement, beam, and fixture metadata. Never synthesize an emitter for a visual-only Venue object.

This work does not add IES photometry, calibrated lumen distributions, aperture assets, or projected gobo textures. Shipped and embedded fixture profiles do not yet contain dependable source data for those features, so the Stage documentation must continue to state that the result is not photometric proof.

## Settings and state lifetime

Add the internal type `StageRenderQuality = "efficient" | "high_fidelity"` and pass the selected value through the Stage surface to the renderer.

The full Stage settings and every Stage pane's Stage settings expose **Render quality** with **Efficient** and **High fidelity** choices. Changing one surface must not affect another.

Render quality is transient runtime state keyed by the full Stage or pane ID. Do not store it in the portable show, Stage layout, fixture profile, user desk layout, REST API, or another persisted schema. A new ToskLight process always begins with every Stage surface in Efficient mode.

Capability-check the required WebGL features. When they are unavailable, keep Efficient active and disable High fidelity with a visible explanation instead of silently degrading or showing a blank scene.

Patch preview, Cue thumbnails, help screenshots, and the product-demo baseline remain explicitly Efficient unless a test intentionally requests High fidelity.

## Required implementation contract and tests

Future implementation must cover at least:

1. Persistent scene objects, in-place live-value updates, model and resource caching, and complete disposal when fixtures or quality profiles change.
2. Efficient mode creating no high-fidelity lights, shadow maps, atmosphere, or post-processing resources.
3. Efficient mode performing no full-scene rebuild on a visualization refresh, no continuous idle rendering, and at most one render for each settled snapshot.
4. High-fidelity beams following hierarchical motion, multi-source emitter layouts, resolved color and intensity, zoom, focus, blackout, Grand Master, Preload, and Highlight.
5. Floor, fixture, and Venue geometry receiving illumination and shadows, with volumetric beams visibly blocked by an intervening occluder.
6. The eight-emitter shadow budget, stable tie ordering, 10% replacement hysteresis, and safe behavior with more than eight active directional lamps.
7. Broad sources remaining non-directional and the existing Sunstrip selection and empty-geometry regressions remaining fixed.
8. Renderer-side emitter synthesis for legacy DMX fixtures and no synthesis for visual-only Venue objects.
9. Independent full-Stage and pane choices, Efficient launch defaults, reset after process restart, and capability-gated settings behavior.
10. Repeated five-hertz visualization updates and repeated quality switches without duplicated lights, retained scenes, unbounded GPU-resource growth, or stale GLB models.

Add a focused `STAGE-001` visual acceptance scenario using a deterministic Default Stage Show arrangement, fixed camera, and Venue occluder. It must prove the Efficient baseline, High-fidelity surface illumination, a cast shadow, beam occlusion, bounded behavior with more than eight active fixtures, legacy-emitter fallback, and broad-source handling.

Update the Stage help and its deterministic settings screenshots when implementing the feature. Final verification uses the focused frontend checks followed by the packaged `./build open` path, readiness, the app-owned server log, fixed-camera visual inspection, switching both directions, and reopening ToskLight to prove the Efficient reset.

## Deferred work

This plan does not define calibrated photometry, IES import, projected gobo media, fixture-specific atmospheric response, ray tracing, offline rendering, or detailed operator controls for haze, bloom, ambient occlusion, tone mapping, shadows, or materials. Those capabilities require richer fixture data and separate acceptance contracts.
