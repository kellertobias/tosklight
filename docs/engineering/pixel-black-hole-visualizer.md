# Pixel Black Hole visualizer

Black Hole is an original procedural WGSL visualizer in the existing Pixel renderer. Its visual
reference is Eric Bruneton's [black_hole_shader](https://github.com/ebruneton/black_hole_shader).
The reference repository's `LICENSE`, checked on 2026-10-19, is BSD-3-Clause, copyright
2020 Eric Bruneton. That license permits redistribution subject to retaining its notice,
conditions and disclaimer, including in binary distribution materials, and prohibits implying
endorsement without permission.

This implementation incorporates **no upstream source code, textures, precomputed tables,
models, or other assets**. Only the README, project overview and license were consulted; the
implementation author was instructed to write an original shader using Pixel's existing
procedural helpers. No upstream dependency or third-party license obligation is introduced by
this visual reference. Any later import or port must separately audit the imported material and
ship its required notices with both source and binary distributions.

The renderer integrates the central-plane null-orbit equation `u'' = 1.5 u² - u` in units with
Schwarzschild horizon radius 1. A bounded, second-order angular integration follows each ray
and samples a procedural thin disc wherever that curved path crosses the disc plane. This
produces the rear-disc image and lower secondary image from the same emission model, while the
escaped direction samples a procedural star field. Captured paths suppress the background;
foreground disc emission can still cross the lower part of the shadow.

The disc has differential orbital motion, several scales of advected turbulence, sheared
filaments, radial temperature falloff, approximate Doppler beaming and gravitational colour
shift. Tone mapping retains the bright approaching side without a flat white disc. The sky uses
original procedural stars and faint dust; its stretched and repeated stellar images follow the
curved rays. No image assets or numerical tables are imported.

This remains a real-time approximation: the ray budget is finite, the disc is a thin procedural
surface, and the colour and bloom are artistic. It neither implements Bruneton's precomputed
beam tracing nor claims the reference's scientific accuracy. A landed beat still swells and
brightens the disc and ring; Pulse decay sets the settling time, and Beat pulse zero disables
this response. Motion continues in silence. Existing parameters, addresses and stored presets
remain unchanged.

Metal validation on Apple M5 Max: all visualizer render regressions, including curved rear-disc
coverage, moving disc detail, stopped rotation, pulse decay and held-beat behavior. A warmed
three-batch median of 20 frames per batch, with a synchronized readback after each batch, measured
0.93 ms/frame at 1920×1080 with the default preset and 1.22 ms/frame at maximum radius and pulse
strength viewed edge-on. These are isolated-renderer measurements, not whole-Pixel throughput
or a guarantee on another GPU.

Stable kind ID 56 is appended at default address `250/028`. Existing configuration assignments
are preserved; operators can add the kind to a free Visualizers slot. The first four visualizer
channels are Shadow radius, Disc speed, Beat pulse and Pulse decay. No persisted schema changes
are needed.
