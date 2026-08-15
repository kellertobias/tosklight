# Visualizer GPU cost: where it goes and what to do about it

Two complaints, and they have almost nothing in common:

- **Draft, and the outline view, cost far more than what they draw.** An outline view is a few
  thousand lines. It should be nearly free and is not.
- **The former Ultra maximum pins the GPU at 100%.** It looks right, and it costs everything.

The first is waste. The second is a budget that is not bounded by anything. They need different
answers, so they are listed separately, cheapest and most certain first.

## What the renderer does per frame now

The pipeline remains readable in `Renderer::render`, but its expensive work is now conditional.
Plans and 3D Lines issue no light cull, shadow or bloom pass. The standalone window and embedded
Stage pane share one redraw gate and submit nothing when the scene revision, value frame, camera,
view, size, selection and overlay are unchanged.

The gate deliberately keeps drawing while display time can change the picture: physical velocity
or unsettled position motion, persistence decay, laser scans, and animated Ultra/Extreme haze.

Two remaining constraints matter:

- The unlit views still use the common HDR/composite target. Removing that copy is a separate
  target-layout change, not required to stop their light-simulation work.
- A benchmark intentionally forces redraws, so it measures the cost of a requested frame rather
  than the static gate's near-zero steady-state submission rate.

## Cheap and certain: stop drawing frames nobody asked for

**1. Redraw only when something changed.** The biggest single win, and it costs no picture quality
at all. The renderer already has the two facts it needs: `SceneValues.frame` is a monotonic counter
bumped only when an emitter value actually changed, and the camera is known. A frame where the
value counter, the camera, the view and the size are all unchanged is a frame identical to the one
already on screen.

Careful with what is genuinely animated: gobo rotation, prism rotation, persistence of vision and
the Ultra/Extreme fog all move on a clock rather than on a value change, so "nothing changed" has to mean
"and nothing in this frame is time-driven", which the instance builder knows. A still rig in the
outline view — the common case while patching — would drop to zero frames.

**2. Idle at a lower rate rather than at zero.** If a full stop is thought too aggressive, cap the
unchanged case at 5–10 fps instead. Most of the win, none of the "is it frozen?" question.

**3. Do not draw a pane nobody can see.** A Stage pane behind another pane, on a hidden desk, or in
a window the compositor has stopped presenting, is still drawing. The desk already knows the pane's
geometry; it does not currently tell the renderer to stop.

## Cheap and certain: stop running passes the view does not use

For any view where `simulates_light()` is false — the outline view and all five plans — these are
pure waste and can be skipped outright:

- **The light cull compute pass.** No surface reads the tile lists.
- **The shadow atlas.** Already budgeted to zero, but the pass and its clear still run.
- **Bloom.** Already skipped for these views since the exposure work, but the bloom targets are
  still allocated and resized.
- **The composite.** An unlit view applies exposure 1.0 and no tonemap, so the full-screen pass is
  a copy. It could render straight to the swapchain and skip the HDR target entirely.
- **MSAA on an outline view.** Lines want anti-aliasing, but 4× MSAA on a full HDR target is an
  expensive way to get it for a few thousand lines.

That is most of a frame's fixed cost removed from the cheap views, which is what makes the "high
GPU for what it draws" complaint. My expectation is that this plus redraw-on-change takes the
outline view to near zero.

## Ultra and Extreme: one bounded tier and one adaptive maximum

Extreme preserves the former Ultra maximum: 64 ray-march steps per beam fragment, a 3D noise
lookup at every step, up to ten shadow-mapped lights, 1,024 crowd members and 8,192 effect
particles. Ultra is now a deliberately lighter fixed tier with 48 volumetric steps, eight shadowed
lights, 768 crowd members, 4,096 effect particles and lower fog detail. It remains visibly richer
than High, which uses 40 steps, six shadows, 384 crowd members, 2,048 effect particles and uniform
fog.

Extreme targets 16 ms of measured GPU work. Two consecutive over-budget samples step down a
bounded ladder of volumetric steps, shadow maps, and shaded resolution; recovery requires 120
samples below 12 ms so quality does not oscillate. The ladder runs from 64 steps, ten shadows and
full resolution to 16 steps, no shadows and 60% resolution. `FrameStats.degraded` says when a rung
below requested Extreme was used, and `FrameStats.effective_quality` names the closest effective
tier.

Adapters with timestamp queries use named asynchronous measurements for cull, shadow, opaque,
beams, lasers, bloom, composite and overlay. An adapter that does not deliver a completed sample
uses presented-frame intervals until the first GPU sample arrives. That fallback keeps Extreme
bounded without mislabelling a display interval as GPU time.

Quick Settings shows the cause at the control point: for an adaptive reduction it names the
requested and effective tiers and the 16 ms GPU budget. Capacity-only reductions, such as a scene
authoring more people or requesting more particles than fit, report requested and drawn counts
instead of the ambiguous
`Quality reduced` label alone.

Preferences written before quality schema 2 migrate `quality ultra` to Extreme, preserving the
old maximum picture. New files carry `quality_schema 2`, so a newly selected Ultra remains the new
bounded Ultra tier. Embedded Stage quality is desk-layout state, not portable show content; adding
Extreme to that wire/layout enum does not reinterpret an authored show.

Further options, roughly in order of value for effort:

**4. March at a lower resolution and upsample.** Volumetrics are low-frequency — that is what makes
this the standard trick. Render the beam pass at half resolution into its own target and composite
it up with a depth-aware filter. Close to a 4× saving on the dominant pass for a difference most
operators will not find. This is the single biggest Extreme win.

**5. Scale steps by what the beam actually covers.** A beam ten metres away crossing forty pixels
does not need the same march count as one filling the screen. Step count from the fragment's own
depth extent through the cone, bounded by the tier, keeps the near beam at full quality and makes
the far ones nearly free.

**6. Prefer timestamp feedback wherever the backend completes it.** Per-pass timing is implemented,
but a native backend may advertise queries without completing mapped samples during a short run.
The interval fallback is intentionally temporary and yields as soon as the first real sample is
collected.

**7. Cap the beam count that gets the expensive treatment.** Shadows are already ranked by radiance
and budgeted; march quality is not. The same ranking would let the twelve brightest beams have the
full treatment and the rest a cheaper one.

**8. Fixed frame-rate cap.** A Stage does not need to run at 120 Hz on a ProMotion display. Capping
the pane at 60 is a one-line change that halves the cost on this hardware, and it should probably
be a setting rather than a constant.

## Quality-split measurement and captures

Measured 2026-08-16 on Apple M5 Max / Metal / 4× MSAA with the release Visualizer, the same
deterministic 33-fixture Full 3D demo, and three-second forced-redraw runs:

| Requested tier | Command | Result |
| --- | --- | --- |
| High | `--demo --view full_3d --quality high --benchmark 3` | 60.0 fps; 16.64 ms median, 18.36 ms p95, 2.06 ms CPU p95; crowd 1,080/384; particles 220/220 |
| Ultra | `--demo --view full_3d --quality ultra --benchmark 3` | 60.0 fps; 16.67 ms median, 18.78 ms p95, 2.20 ms CPU p95; crowd 1,080/768; particles 220/220 |
| Extreme | `--demo --view full_3d --quality extreme --benchmark 3` | 60.0 fps; 16.60 ms median, 19.07 ms p95, 1.94 ms CPU p95; adaptive crowd 1,080/369; particles 220/220 |

The adapter again supplied no completed timestamp sample during these short runs, so the table
does not invent GPU time from presentation cadence. It does prove the authored cost separation:
Ultra draws twice High's crowd budget and uses 48/8 volumetric/shadow controls instead of 40/6;
Extreme requests the old 64/10 maximum and visibly adapts when the interval fallback sees no
16 ms headroom. Every run reports degradation because the demo deliberately authors 1,080 people,
above every current per-frame crowd budget; Quick Settings names that exact authored/drawn cause.

Representative settled captures were produced with the same commands plus `--capture-frames 60`
and `--capture` into `.artifacts/generated/tl-272-quality/{high,ultra,extreme}.png`. Each is
1,600 × 900 and the files are pairwise distinct. ImageMagick normalized RMSE is 0.0933 for
High/Ultra, 0.0952 for Ultra/Extreme, and 0.0345 for High/Extreme. The last pair is closer because
Extreme had already reduced itself on this machine; that is the intended effective-tier behavior,
not a claim that requested Extreme is cheaper than Ultra.

## Historical measured result before the tier split

Measured 2026-08-09 on Apple M5 Max / Metal / 4× MSAA, using the deterministic 32-fixture demo
scene and the real Visualizer window.

| Case | Command | Before | After |
| --- | --- | --- | --- |
| 3D Lines requested frames | `--demo --view lines_3d --quality ultra --benchmark 3` | 60.0 fps, 16.49 ms median, 18.68 ms p95, 0.56 ms CPU p95 | 60.5 fps, 16.54 ms median, 18.58 ms p95, 0.50 ms CPU p95; cull/shadow/bloom disabled by the pass plan |
| Full 3D Ultra | `--demo --view full_3d --quality ultra --benchmark 3` | 45.8 fps, 21.14 ms median, 28.80 ms p95 | 59.9 fps, 16.66 ms median, 18.63 ms p95; adaptive rung active on all 180 measured frames |

These measurements predate the Ultra/Extreme split: the rows labelled Ultra refer to the maximum
that is now named Extreme. The Metal adapter advertised timestamps but produced no completed mapped sample during these short
runs, so the GPU and pass columns truthfully remain absent. Unit decoding and host-GPU capture
tests cover the query path; the measured result above therefore used the documented frame-
interval fallback. For static 3D Lines, the benchmark is the conservative forced-redraw case; the
normal application gate issues no new renderer frame after the redraw identity is unchanged.

### Crowd budget evidence

Measured 2026-08-13 on the same Apple M5 Max / Metal / 4× MSAA renderer. The deterministic demo
now includes one 40 m × 20 m dense Crowd Area: 1,080 authored people, deliberately above both
quality budgets. Both commands held the 60 Hz presentation target and retained a deterministic
prefix of the authored population:

| Quality | Command | Result |
| --- | --- | --- |
| High | `--demo --view full_3d --quality high --benchmark 3` | 59.7 fps, 16.62 ms median, 17.91 ms p95, 3.23 ms CPU p95; 1,080 authored / 384 drawn |
| Ultra | `--demo --view full_3d --quality ultra --benchmark 3` | 60.0 fps, 16.61 ms median, 17.87 ms p95, 2.67 ms CPU p95; 1,080 authored / 369 drawn |

In this historical table High's fixed cap is 384 and old Ultra is current Extreme. Extreme begins
at 1,024 and follows the existing six-rung 16 ms controller;
this adapter again supplied no timestamp sample, so the conservative frame-interval fallback
settled on its 0.6 scale rung (`1,024 × 0.6²`, rounded to 369). All measured frames reported
degradation because authored demand exceeded the current person budget; that signal makes the
fallback visible rather than indicating a missed presentation deadline.

## Next optimizations

1. Render volumetrics into a dedicated lower-resolution target with depth-aware upsampling.
2. Render unlit views directly to the output and remove their remaining composite copy.
3. Tell an embedded renderer explicitly when its desk pane is occluded.

Items 1 and 2 are contained and testable — the existing headless capture harness can assert that an
unchanged scene produces no new frame, and that an outline view issues no compute pass. Item 3 is a
real piece of rendering work and wants its own measurements before and after, which is what the GPU
timer is for.
