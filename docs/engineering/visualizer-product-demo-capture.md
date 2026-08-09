# Visualizer product-demo capture

## Decision

Extend the existing headless `viz-capture` frame exporter into an offline, frame-indexed demo
renderer. Feed it an authoritative playback timeline, render without window or UI chrome, and mux
the resulting numbered frames with audio only after rendering.

This is the only candidate that makes resolution, camera, playback time and presentation time
independent of desktop composition and machine speed. It also leaves the desk UI completely free
for a separate screen recording or scripted UI capture. The current exporter is already most of
this path: it uses the production renderer and scene projection, accepts an explicit resolution and
fixed time step, reads the GPU result back to RGBA, and writes numbered PNG files. Its missing part
is authoritative show playback. Today it applies one static scripted preview look before rendering
every frame.

The product-demo master should be **3840 x 2160 at 60 progressive frames per second**, rendered in
an sRGB working image and encoded to Rec.709. A 1920 x 1080 at 60 fps proof is the first gate; 30
fps may be derived for distribution, never used as an implicit capture-clock fallback. The master
contains the Visualizer image only. Status bars, Quick Settings and pointer state are excluded;
intentional titles or callouts belong to the video composition.

## Candidate comparison

| Path | Platforms | Image quality | Timing and latency | Operational tradeoff |
| --- | --- | --- | --- | --- |
| Capture the dedicated Visualizer window | Any platform with desktop capture | Includes compositor scaling, occlusion and possibly window chrome; resolution depends on the display | Live and low latency, but variable frame pacing and dropped/duplicated frames couple the result to GPU and display load | Fast for an informal recording; moving, covering or resizing the window changes the take and the operator cannot use it safely |
| Native capture on a physical or virtual display | ScreenCaptureKit on macOS; separate Windows/Linux implementations and virtual-display setup | Can capture clean pixels at a controlled display size, but still passes through a window server and colour-management path | Live, with platform-specific buffering; playback and capture clocks still have to be reconciled | Useful for recording the interactive Visualizer UI, but requires per-platform permissions, display setup and recovery from notifications/focus changes |
| Live renderer texture stream | A backend per ecosystem, such as Syphon/Spout or an encoded network stream | Can avoid chrome and compositor resampling; encoding or transport may add chroma loss | Lowest live latency, but delivery backpressure and encoder cadence become part of the render loop | Appropriate for broadcast/live output; adds native dependencies and failure modes to the Visualizer and can compromise interactive performance |
| Offline headless frame export | Existing cross-platform `wgpu` renderer; no display server | Exact requested dimensions, production shaders, no chrome, lossless intermediate frames | No live-latency promise; frame index is the clock, so a slow frame delays the job instead of damaging the video | Repeatable, scriptable and isolated from the desk UI; storage and later encoding are explicit costs. This is the recommended product-demo path |

Generic desktop capture is therefore a convenience fallback, not acceptance evidence for a
repeatable product demo. Native/window capture remains valuable for a separate operator-UI track,
which can be edited beside the clean Visualizer render.

The platform comparison is grounded in the supported system APIs: Apple ScreenCaptureKit can
filter displays, applications and windows and delivers timed media buffers; Windows Graphics
Capture acquires display/window frames through a system-mediated capture session; and the XDG
ScreenCast portal exposes monitor, window and virtual-monitor sources over PipeWire, including a
hidden-cursor mode. See the official [Apple ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit),
[Windows screen-capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture),
and [XDG ScreenCast portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)
documentation. Those APIs make native capture viable; they do not remove its dependence on a live
window-server clock, which is why it is not the recommended clean master.

## Playback synchronization contract

The capture clock is a rational frame rate, not wall time. At 60 fps, output frame `n` represents
presentation time `n / 60` seconds. Rendering may run faster or slower than real time without
changing that timestamp or skipping a frame.

The authoritative input should be a versioned demo timeline produced from the show playback path,
not packets recorded opportunistically from Art-Net or sACN. It must pin:

- the immutable show/revision identity and fixture-profile revisions;
- the exact frame rate and total frame count;
- playback commands at frame-indexed times, including cue starts, releases and jumps;
- the resulting universe values for every output frame, or enough deterministic engine state to
  reproduce them with a manual clock; and
- an optional 48 kHz audio asset plus its sample offset relative to frame zero.

The safest first boundary is a recorded authoritative universe timeline: a producer drives the
real playback engine on a manual clock, writes the complete resolved universe state for each frame,
and `viz-capture` decodes that state. This keeps HTP/LTP, fades, tracking and output policies owned
by the lighting engine while keeping the renderer independent of a running desk and a live network.
A later in-process engine driver may replace the file boundary if it proves simpler without
duplicating playback semantics.

Camera state is timeline input too. It may be a fixed authored camera, a frame-indexed sequence, or
the DMX Visualizer Camera fixture; it must never be sampled from a window the operator happened to
leave open. Audio is muxed after the image sequence using the shared zero point, constant 60 fps
timestamps and 48 kHz samples. The mux step must fail on a duration mismatch instead of stretching
one track silently.

## Quality and performance requirements

- Every requested frame is written once with a zero-padded index; no gaps, duplicates or
  wall-clock-derived timestamps are allowed.
- The production render path, quality tier, haze, exposure, camera and fixture packages are pinned
  in the capture manifest and reported beside the output.
- The lossless intermediate is RGBA PNG until a higher-throughput lossless sequence is justified.
  Delivery encoding converts explicitly to Rec.709 and preserves a constant frame rate.
- Two runs from the same revision and manifest must have identical dimensions and frame count. The
  existing GPU tolerance remains the image threshold: every differing byte is at most one value
  and fewer than 0.1% of bytes may differ.
- The first performance gate is sustained 1920 x 1080 at 60 fps at least 0.5 times real time on
  the reference development Mac, with no missing frame. The 4K60 master may render offline at any
  speed, but must report total render time, peak storage and failed readbacks.
- Renderer or adapter absence, a dark/empty rig, missing playback input, profile mismatch and mux
  duration mismatch are hard errors. None may produce a plausible but incomplete video.

## Implementation plan

1. Define a small versioned capture manifest and timeline format with show hash, fixture-profile
   identities, rational frame rate, total frames, camera, renderer settings, universe snapshots and
   optional audio alignment. Put generated manifests and captures under `.artifacts`.
2. Split the current static `scripted_look` from `viz-capture` behind an input source. Retain it as
   an explicit still-look mode; add timeline playback that supplies a complete frame before each
   call to `Renderer::capture`.
3. Make frame number the sole render timestamp. Advance persistence and physical fixture motion by
   the exact rational step and settle only when the manifest requests a pre-roll.
4. Add a chrome-free default overlay and explicit opt-in diagnostic overlay. Record view, camera,
   quality, haze, exposure and renderer adapter in a machine-readable sidecar.
5. Add a separate deterministic encode/mux command that consumes the numbered sequence and optional
   audio, emits constant-frame-rate Rec.709 output, and verifies frame and audio duration before
   success.
6. Keep the interactive `--capture` PNG and native display-capture options for inspection and UI
   footage. They do not become inputs to the clean demo master.

## Proof plan

1. Generate the shipped demo show and capture a 10-second 1080p60 timeline containing a fade, a
   tracked cue, pan/tilt motion, colour change and blackout. Assert 600 consecutive frames and
   inspect known frame indices before, during and after each transition.
2. Run that job twice from clean output directories. Compare dimensions, frame indices, manifest
   hash and pixels against the documented one-value/0.1% GPU tolerance.
3. Capture the same timeline at 4K60. Record throughput, total duration, peak intermediate storage
   and adapter identity; visually inspect fine beams, gobos, gradients, text-free edges and extreme
   camera zoom.
4. Mux a 48 kHz reference click at frame zero and a second marker on an exact later frame. Verify
   both markers at the intended video timestamps and reject one-frame or one-sample duration drift.
5. Run the interactive desk and editor during offline capture. Verify their windows remain usable,
   never appear in the frames, and closing or covering them does not change the image sequence.
6. Run negative cases for a missing GPU, mismatched show revision, missing universe frame, dark
   timeline, failed readback and audio-duration mismatch. Each must stop with an actionable error
   and no success marker.

Passing this proof promotes offline frame export from a strong still-image harness to the supported
product-demo capture path. Until then, the existing command is evidence for renderer quality and
determinism only; it is not evidence that cue playback and audio are synchronized.
