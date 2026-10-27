# Native Stage Performance

These scenarios are packaged-application acceptance only. Run them on every supported platform with a real GPU and an unlocked visible operator session. Browser Playwright, software rendering, and short development runs may diagnose failures but never satisfy these scenarios. Retain the generated `.artifacts/performance/stage/stage-visualization-timing.json`, raw observations and application/build identity with the result. Fixed-layout measurements and opt-in capture verification use separate reports; they cannot pass these delivery gates or the proposed display/GPU gates merely by completing.

## Evidence and TL-852 acceptance boundary

The existing 10 Hz source-delivery, 120 ms p95 and 200 ms hard/gap gates below remain required. Their legacy `FramePresented` event describes a helper submission; it does not establish when that drawable reached the display. Report source delivery, helper rendering, compositor work and actual display presentation separately.

The 10 Hz requirement is source delivery for the active workload, not a request to submit duplicate static pictures. A verified unchanged picture may remain resident while source/CPU input continues to advance.

The following are proposed TL-852 acceptance targets, not measured successes:

- On the declared reference hardware, sustain at least 59 actually displayed distinct Stage frames per second over a five-minute active window, with missed 60 Hz presentation opportunities at or below 1%, displayed-frame interval p95 at or below 18.5 ms and p99 at or below 33.4 ms. Declare the presentation timebase, scoped drawable identities and denominator; helper submissions, RAF callbacks and GPU completions cannot stand in for displayed frames.
- An internal 120 Hz display intentionally presenting Stage at 60 Hz has a 60 Hz target. Do not count the unused alternate 120 Hz refresh opportunities as missed Stage frames.
- Against a declared before build at a matched actual 60 Hz workload, reduce whole-Stage GPU time per second by at least 70% for Draft and 90% for Lines. Include all helper lanes and Control compositor work without double-counting. Sparse pass timestamps, lower submission rates or a short faster run do not prove these reductions. The percentages remain unproven until complete matched evidence exists.
- Measure both single 1920 × 1080 physical pixels and dual Live/Follow Preload 1920 × 540 each. Keep the total 2,073,600-pixel area, scene, camera, output workload, transport and scheduling comparable; record DPR and actual monitor refresh. Use real modeled fixtures and preserve articulation, selection, labels and visible aims.
- In a verified static settled picture, require zero recurring helper submissions and zero compositor acquire/submission admissions after settling. In a resident hidden interval, require the same zero recurring GPU work while input/CPU state continues to progress. Retain native interval boundaries and per-pane coverage; missing samples never mean zero. Reveal must retain pane/helper ownership, meet the TL-848 resident first-current-picture target of 100 ms and avoid replaying a stale frame backlog; the existing 200 ms delivery ceiling remains a separate hard limit. Actual whole-Stage GPU cost remains a separate measurement from these admission counters.

Run active, static, camera-moving, hidden and revealed conditions separately. Static savings cannot satisfy the active 500-instance target. Internal-display containment and an isolated private test desk are required for visible review on the reference host; do not change the operator's desk settings to arrange a measurement. Detailed instrumentation boundaries are in `docs/engineering/stage-gpu-measurement.md`.

## STAGE-PERF-001 — Default Stage latency and output isolation

Build the packaged desk through the authoritative build workflow, then run `npm run benchmark:packaged-stage -- 300 default-stage`. Record application identity, OS, native GPU/backend, drawable resolution, Control wireframe view, and show/look. Both Live and Follow Preload lanes must sustain the 10 Hz source cadence independently. Source-to-reported-native-frame p95 must be at most 120 ms; no changing frame or presentation gap may exceed 200 ms. Verify every presented Control frame is Draft quality in wireframe 3D or one of the five orthographic 2D views; full rendering belongs to Architect. Exercise native drawable resize/recovery, renderer teardown/recreation, a paused visualization consumer, show switching, and application suspension where supported. The five-minute Stage window must not introduce output deadline misses/send errors or regress scheduler p99 by more than 1 ms or 5 percent against the paired no-Stage window.

## STAGE-PERF-002 — 500-instance bounded delivery and resource release

Run `npm run benchmark:packaged-stage -- 300 stage-500` for the comparative gate and `npm run benchmark:packaged-stage -- 1800 stage-500` for retained resources. Confirm the generated scene contains exactly 500 fixture records and 500 physical instances. Apply the same latency, lane, lifecycle, Control wireframe admission, bounded latest-frame, and output-isolation gates as STAGE-PERF-001. During the 30-minute run, require an independent native Stage start/end clock and complete observation coverage, rather than timing the run from its first and last retained frames. Exclude the first Stage minute as warmup; resident growth must remain at or below 1 MiB per minute, native instance/draw-call ownership must not grow between the first and last five-minute windows, no degraded frame may be reported, and late CPU frame p95 must remain at or below 16.7 ms. A missing native GPU/backend field, locked session, stalled native presentation stream, non-packaged identity, retention overflow or missing early/late lane coverage fails acceptance rather than becoming an exception. The main-process RSS slope excludes separate WebView processes and GPU/driver allocations; retain renderer ownership evidence separately.

## STAGE-PERF-003 — Frozen 4K images from live low-cost views

Exercise **Render 4K** from Live and Follow Preload, and **Render 4K Image** in Architect, starting from live Draft/Lines. With Blender available, exercise both **Ultra · ToskLight** and **Ultimate · Blender** through the chooser and retain actual 3840 × 2160 PNGs from each engine. A probe, manifest export or chooser alone is not an actual render. With Blender absent/unusable, verify the native Ultra path and any actionable fallback notice. Keep each engine's result and limitations explicit.

Freeze scene, resolved fixture/effect/physics values, camera/view, local crowd amount and Detailed people preference, media pixels/availability and time before probing or choosing. Change the live show/look while the chooser or worker is pending and verify that the saved result still uses that frozen invocation. Native Ultra must use the separate full-resolution, full-budget capture renderer, rather than live Draft/adaptive reductions; live settings must remain unchanged. Preserve camera pose and vertical field with the disclosed 16:9 framing adjustment, and retain orthographic views as orthographic. Show chosen engine, dimensions, framing and saved path, with working Open/Reveal actions.

Verify progress, actionable failure, owned cancellation, cleanup and subsequent-job admission; never permit overlapping captures to contend across embedded panes. Native readback and Blender work must stay off the desk/render input thread. Ultimate refuses unsupported plan symbols/outlines and active laser scan transfer with an explicit Ultra alternative. Disclose procedural material finishes mapped to plain PBR, omitted grid/selection guides, native bloom differences and Cycles physical beam/source/fog/GI/AgX mapping; do not claim pixel or physical parity. Missing frozen media must be identified, with the frozen show fallback or black retained instead of fetching a later frame.

The opt-in packaged DOM driver proves button → chooser → owned native job → visible Saved state and a decoded job-associated PNG. It does not prove a physical pointer/keyboard interaction, human visual acceptance, or displayed 60 FPS. Run capture verification separately from performance acceptance because the independent 4K render may temporarily contend for GPU time.

## Runner notes

The supported scale is capped at 24 universes × 512. stage-500 patches 19 universes and canonical-demo 10; the 1,000-instance tiers (37 universes) are outside the cap. Set `LIGHT_PACKAGED_STAGE_DYNAMICS=1` so stage-500 carries the animated Dynamics workload that its 10 Hz cadence clause needs; without it the look is static. On a machine whose own desk already owns port 5000, set `LIGHT_PACKAGED_STAGE_PORT` to run beside it: the runner starts the bundled server on that port and stops only what it started. `LIGHT_PACKAGED_STAGE_CARGO_PROFILE=release` measures the release bundle, and `LIGHT_PACKAGED_STAGE_NETWORK_CAPTURE=1` records the desk's Art-Net and sACN output on loopback in both windows. The latest measured state is in `docs/engineering/engine-render-performance-series.md` (TL-553).

The requested duration is the Stage observation window. The paired no-Stage control window is capped at 300 seconds, so an 1800-second resource run has at most a five-minute control baseline, not a second 30-minute window. Resource evidence must cover the declared Stage interval after warmup and both first/last five-minute windows independently of whether new frames were submitted. Keep per-lane identities, observation loss/retention counts and actual coverage/gaps; bounded retained prefixes or missing terminal samples cannot pass a long-run gate. Current helper CPU fields are elapsed submission wall time from a source-deduplicated retained subset, not every 60 Hz frame or executing-thread CPU. GPU pass samples are not total GPU time per second. Report cold-start observations separately; steady coverage must span Stage start +60 seconds through Stage completion for RSS and each observed pane.


## STAGE-PERF-004 — Optional detailed people in Ultra and HQ images

Use an authored Crowd Area in both standing and sitting postures with an oblique
camera and a stage light that reveals body depth. Record the population, seed,
camera, crowd amount and renderer build. In Ultra, toggle **Detailed people**:
enabled must show volumetric heads, torsos and limbs with shaded skin/clothing
surfaces; disabled must restore the existing simple silhouette representation.
The option does not independently hide the audience: **Crowd amount** remains
its population/hide control. Check the same stable people occupy the same places
and keep their authored posture and human scale across toggles.

Restart the isolated renderer and verify that the local option persists. Verify
both standalone Quick Settings and embedded pane instructions without mutating
the portable show. Old local preferences that omit the setting must load safely.
Draft and Standard continue to omit crowds, High retains its simple figures, and
Lines retains its lightweight technical representation regardless of this option.

While still in live Draft, enable Detailed people and invoke the real 4K action.
After the invocation is frozen, change the live option while the engine chooser
or worker is pending. Render native **Ultra** and **Ultimate / Blender** separately
and decode actual 3840 × 2160 images: each must use the invocation-time detailed
choice, camera, crowd amount and deterministic population. Repeat with the
option disabled and with Crowd amount zero. Live Draft and the later live
preference must remain unchanged by capture completion or cancellation.

Retain images from both engines and the simple/detailed comparison. Verify body
depth from a side angle and show-light response rather than accepting a front
silhouette screenshot as proof. Dark clothing may remain mostly in shadow, but
lit surfaces must reveal volume without self-emission. Geometry/unit/transport
checks support this test; physical controls and human visual acceptance remain
separate. Run image tests outside performance measurement windows.
