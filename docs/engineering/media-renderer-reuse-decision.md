# Media renderer: reuse or separate

Slice 2 requires evaluating the existing GPU stack before writing a renderer, and recording the
decision with reasons. This is that record.

**Decision: a separate Media render adapter, reusing the workspace's vetted GPU dependency set
and the `PresentationSurface` seam pattern — not the `viz-render` crate itself.** No shared GPU
kernel is extracted yet; extraction follows callers, not speculation.

## What was evaluated

`crates/viz/render` (≈4,800 lines of Rust plus nine WGSL shaders), `crates/viz/surface`,
`crates/viz/scene`, `crates/viz/snapshot`, and `apps/viz-renderer`, at baseline
`adbc624d5b90929e564be16e5fa3e49d5540da79`.

## Why not reuse `viz-render`

`viz-render` is a 3D lighting-visualizer renderer. Its documented contract is: take a
presentation-surface adapter, a semantic scene, its live values, and a view configuration, and
present one image. Its passes are `beam`, `cull`, `laser`, `lines`, `overlay`, `post`, `shadow`,
and `surface`; its public surface is cameras, rays, picking, semantic lights, gobo slots, emitter
poses, and scene geometry.

The Media renderer composites 2D layers: one textured quad per layer, a mask texture with its own
independent transform, an ordered effect chain, per-layer intermediate targets when a mask or
effect needs one, then master tint, dimmer, and flip/mirror over the finished composite. It has no
scene, no camera, no lights, no shadows, no picking, and no gobos. Essentially none of
`viz-render`'s passes or public types carry over, so depending on it would mean pulling a large
3D pipeline in to use its device setup.

Two structural mismatches also matter:

* **One surface per renderer.** `Gpu` owns a single optional surface and one swapchain
  configuration. A Media Server process hosts *N* outputs, each with its own surface, monitor,
  render clock, and failure isolation — a slow or disconnected output must not stop another from
  presenting. That is a different ownership shape, not a parameter.
* **One global present-mode choice.** `preferred_present_mode` picks Mailbox, else Fifo. Media's
  presentation mode is per-output configuration (`DisplaySynchronized`, `FixedFps`, diagnostic
  `Unlocked`), chosen from each surface's real capabilities, with the measured presentation
  cadence recorded rather than assumed.

## What is reused

* **The dependency set.** `wgpu`, `winit`, `bytemuck`, `glam`, and `pollster` are already pinned
  in the workspace and already build and run on macOS, Windows, and Linux here. Media takes those
  same versions rather than introducing a second graphics stack.
* **The `PresentationSurface` seam.** Taking a surface adapter instead of owning a window is the
  right shape, and it is what lets an output be a monitor window, an off-screen target, or a test
  harness without the renderer knowing. Media defines its own trait of the same shape rather than
  importing `viz-render`'s, so neither product constrains the other's surface lifecycle.
* **`GpuTimer`'s approach.** Timestamps written around a frame's passes, resolved into a buffer,
  and read back a frame later so nothing waits for the device — this is exactly how Media's
  per-stage render measurements should work, and how CITP preview readback stays off the critical
  path. The pattern is reused; the code is not shared until both sides prove identical types.
* **`viz-surface`'s cross-process shared-surface work** is the reference to return to if the
  Media renderer ever needs to hand a texture to another process. It is not needed for the first
  release.

## When to revisit

Extract a shared GPU kernel only after the Media render adapter exists and the two sides
demonstrably want the same thing — most plausibly device/adapter selection, capability
validation, and GPU timing. Adding the Media adapter first, identifying identical stable
behavior, then extracting, then keeping separate orchestration is the repository's stated rule for
shared Rust, and it applies here.
