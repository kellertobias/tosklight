# Visualizer GPU cost: where it goes and what to do about it

Two complaints, and they have almost nothing in common:

- **Draft, and the outline view, cost far more than what they draw.** An outline view is a few
  thousand lines. It should be nearly free and is not.
- **Ultra pins the GPU at 100%.** It looks right, and it costs everything.

The first is waste. The second is a budget that is not bounded by anything. They need different
answers, so they are listed separately, cheapest and most certain first.

## What the renderer does per frame today

Every frame runs the same pipeline regardless of what is in the picture: light cull (compute) →
shadow atlas → opaque → beams → lasers → bloom extract → two blur passes → composite → overlay.
`Renderer::render` in `crates/viz/render/src/renderer/frame.rs` is the whole order.

Two things about that shape matter for both complaints:

- **It redraws whether or not anything changed.** A Stage looking at a rig nobody is touching costs
  the same as one mid-cue.
- **The passes are not conditional on the view.** A view that simulates no light still runs the
  cull pass, still allocates and clears the HDR and bloom targets, and still runs a full-screen
  composite.

## Cheap and certain: stop drawing frames nobody asked for

**1. Redraw only when something changed.** The biggest single win, and it costs no picture quality
at all. The renderer already has the two facts it needs: `SceneValues.frame` is a monotonic counter
bumped only when an emitter value actually changed, and the camera is known. A frame where the
value counter, the camera, the view and the size are all unchanged is a frame identical to the one
already on screen.

Careful with what is genuinely animated: gobo rotation, prism rotation, persistence of vision and
the Ultra fog all move on a clock rather than on a value change, so "nothing changed" has to mean
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

## Ultra: give it a budget it cannot exceed

Ultra is 64 ray-march steps per beam fragment, with a 3D noise lookup at every step, plus up to ten
shadow-mapped lights. Nothing in that scales with how many beams are on screen or how large the
pane is, which is why it saturates: a rig with sixty beams across a full-screen Stage is asking for
roughly sixty times the work of one beam, and it gets it.

Options, roughly in order of value for effort:

**4. March at a lower resolution and upsample.** Volumetrics are low-frequency — that is what makes
this the standard trick. Render the beam pass at half resolution into its own target and composite
it up with a depth-aware filter. Close to a 4× saving on the dominant pass for a difference most
operators will not find. This is the single biggest Ultra win.

**5. Scale steps by what the beam actually covers.** A beam ten metres away crossing forty pixels
does not need the same march count as one filling the screen. Step count from the fragment's own
depth extent through the cone, bounded by the tier, keeps the near beam at full quality and makes
the far ones nearly free.

**6. Make the tier a target rather than a constant.** Measure the frame — the GPU timer is already
there in `crates/viz/render/src/timing.rs` and already reports `gpu_micros` — and trim march steps
and shadow count to hold a frame budget. Ultra becomes "spend up to 16 ms making this as good as
possible" instead of "do 64 steps and take as long as that takes". This also fixes the same problem
on a weaker GPU, where today Ultra is simply unusable rather than degraded.

**7. Cap the beam count that gets the expensive treatment.** Shadows are already ranked by radiance
and budgeted; march quality is not. The same ranking would let the twelve brightest beams have the
full treatment and the rest a cheaper one.

**8. Fixed frame-rate cap.** A Stage does not need to run at 120 Hz on a ProMotion display. Capping
the pane at 60 is a one-line change that halves the cost on this hardware, and it should probably
be a setting rather than a constant.

## What I would do first

1. Redraw-on-change plus the time-driven exception (helps every view, most of all the cheap ones).
2. Skip cull/shadow/bloom/composite for views that simulate no light.
3. Half-resolution beam march with depth-aware upsample (the Ultra win).

Items 1 and 2 are contained and testable — the existing headless capture harness can assert that an
unchanged scene produces no new frame, and that an outline view issues no compute pass. Item 3 is a
real piece of rendering work and wants its own measurements before and after, which is what the GPU
timer is for.

## What is not yet measured

None of the above is instrumented per pass today. `FrameStats` reports total `gpu_micros`, draw
calls, instances and lights, but not the split between the beam pass and everything else. Before
committing to item 3 it is worth adding per-pass timestamps — the timer already supports opening
and closing writes, and it would turn "the beam pass is presumably dominant" into a number.
