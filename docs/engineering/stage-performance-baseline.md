# Stage performance evidence

The Stage performance collector records a repeatable engineering baseline for
the Default Stage and a deterministic large scene. It combines the existing
frontend Stage diagnostics with the authenticated runtime output and
visualization diagnostics.

## Current native acceptance boundary (TL-852)

The authoritative scenarios are `docs/testing/16-stage-performance.md`. Preserve the legacy 10 Hz source cadence, 120 ms source-to-reported-native-frame p95 and 200 ms hard delivery/gap ceilings. The protocol's `FramePresented` is a helper submission witness, not an on-glass presentation event. A passing delivery collector does not prove displayed 60 FPS.

TL-852 proposes a separate five-minute active display gate: at least 59 actually displayed distinct Stage FPS, missed 60 Hz opportunities at or below 1%, interval p95 at or below 18.5 ms and p99 at or below 33.4 ms. Declare drawable identity, presentation timebase and opportunity denominator. On the internal 120 Hz panel, the intentional Stage target is 60 Hz; alternate unused monitor refreshes are not misses. These display targets remain unproven without correlated platform presentation evidence.

Proposed GPU reductions are at least 70% for Draft and 90% for Lines against the recorded before build at matched actual 60 Hz, using whole-Stage GPU time per second. Include both helper lanes and Control compositing, account for other load and overlapping scopes, and retain comparable camera/scene/look/transport/physical pixels. Sparse timestamp sums and a submission cap cannot establish these percentages. Keep missing, short or unmatched evidence explicitly unverified. See `docs/engineering/stage-gpu-measurement.md` for instrument scope and trace limits.

The fixed matrix measures one 1920 × 1080 physical drawable or two 1920 × 540 Live/Follow Preload drawables: the same total pixel area. Record actual DPR, refresh, native GPU/backend, bundle/build identity and helper/compositor ownership. Private benchmark overrides can measure Full3d Draft as well as Lines; they do not change ordinary Control policy. The internal display is selected and containment verified before showing a benchmark window. Isolated HTTP/OSC ports and data/output directories preserve the operator's active desk and persistent settings.

Static, moving-camera, resident-hidden and revealed phases are distinct evidence. After a healthy static/hidden settling boundary, require zero recurring helper submissions and compositor acquire/submission admissions over verified interior native intervals with per-pane coverage. Missing telemetry, gaps and unknown boundary spans are not zero work. Keep helpers resident and CPU/input state current; on reveal show the latest state within the existing 200 ms ceiling without replaying old frames. Admission counters alone cannot prove whole-Stage zero GPU time. Neither idle savings nor a successful short phase substitutes for the active 500-instance display/GPU targets.

## Historical browser collector

The following browser workflow and its detailed historical workloads are diagnostic evidence. They do not establish current packaged/native capacity or TL-852 acceptance. The 1,000-instance profile below spans 37 universes and exceeds the current 24-universe supported output cap; it is not a supported-capacity claim.

Run the focused collector with:

```sh
npm run test:e2e -- tests/67-stage-performance-baseline.spec.ts --workers=1
```

The command writes JSON reports beneath:

```text
.artifacts/performance/stage/
```

Each report is also attached to its Playwright result. The report records:

- visualization request duration and payload size;
- source-to-settled-canvas timing and presentation gaps;
- scene-build, render, draw-call, triangle, geometry, texture, context, RAF,
  and browser-memory diagnostics;
- runtime output frames, packets, errors, deadline misses, timing maxima, and
  fixed-bucket scheduler-duration histograms;
- runtime visualization projections, projection time, payload bytes, source
  age, subscribers, and skipped source frames.

Each profile now records two paired bounded comparison windows:

1. a ten-frame three-second transition on a Desktop with no Stage surface; and
2. a ten-frame three-second transition with one Live Stage, one duplicate Live
   Stage, and one Follow Preload Stage.

The Stage window also covers all four render qualities, rapid quality
switching, simultaneous Live and Preload lanes with an independent Preload
change, one shared socket and one server subscriber per lane, stalled browser
message delivery, and (for the large profile) a separate authenticated
visualization WebSocket with its underlying TCP reader paused, WebGL context
recovery, 2D/3D renderer teardown, and duplicate-pane removal. Output
comparison fields derive a p99 for each bounded window by subtracting the
cumulative fixed-bucket histograms. The three-second windows provide more than
100 scheduler samples at the default 44 Hz rate, so p99 is not reduced to the
single slowest tick. The focused browser run enforces the same
1 ms-or-5-percent regression rule for those bounded windows, but does not
substitute them for the canonical five-minute packaged release gate.

The Default Stage profile measures the repository fixture unchanged. The large
profile deterministically extends the same fixture to 970 fixture records and
1,000 Stage instances, including at least 40 Showtec Sunstrip LED RGB fixtures,
a majority of higher-channel profile/beam/wash/Sunstrip fixtures, and a
separate fixed-dimmer control population. Added output fixtures are packed from
universe 101 onward without splitting a fixture across a universe. The final
channel need not be occupied; the report records profile/mode inventory,
universe count, and occupied slots.

The large profile creates and starts 20 production Dynamic instances before
measurement. Their non-overlapping target partitions cover every Dynamic-driven
fixture. Sunstrips and other color-capable fixtures receive intensity and color
lanes; moving beam/wash fixtures also receive pan and tilt lanes. Conventional
dimmers are intentionally excluded and retain a static intensity so the same
run contains a no-Dynamic control population.

The embedded Stage release target is 1,000 fixture instances for this exact
mixed profile. It becomes the supported ceiling only after the canonical
packaged, retained-resource, and supported-platform gates pass; it is not a
schema limit or a promise for 1,000 worst-case emitters. Any higher
fixture-count result belongs to a separately identified headless/output-only
profile with Stage disabled and is not Stage capacity evidence.

## Packaged native Stage collector

After `npm run build:open` has built the debug bundle and left its headless service
ready, run the packaged collector with a duration in seconds and an explicit
scene profile:

```sh
npm run benchmark:packaged-stage -- 300 default-stage
npm run benchmark:packaged-stage -- 300 stage-500
npm run benchmark:packaged-stage -- 300 large-stage
npm run benchmark:packaged-stage -- 1 improved-beam-spike
```

Without an isolated-port override, the runner stops the already-open development desk and service. For reference-host review use `LIGHT_PACKAGED_STAGE_PORT=5160` plus an unused `LIGHT_PACKAGED_STAGE_OSC_PORT`: this starts the bundled server beside the user desk and stops only owned processes. The isolated path launches the packaged executable directly; macOS restoration overrides are process-local arguments, not persistent user defaults. Both paths use a fresh benchmark data directory,
authenticates through the production session and Patch routes, and prepares the
requested profile before either measured Stage surface mounts. On macOS it
launches the `.app` through LaunchServices; on Windows and Linux it launches
the built debug desktop executable directly with the same benchmark
environment. `default-stage` records the seeded scene unchanged; `large-stage`
deterministically installs and verifies 970 fixture records, 1,000 Stage
instances, the declared fixture/mode inventory and DMX occupancy, and 20 active
Dynamic instances. The runner releases the benchmark-only preparation barrier only
after the profile is ready, then records a consolidated report under
`.artifacts/performance/stage/`.

The historical lifecycle exercise kept independent Live and Follow Preload views open,
cycled render qualities, exercised 2D/3D surfaces, resized the
shared drawable, tore down and recreated the live renderer helper, and drove
alternating Live and Preload values through the
production command and Preload lifecycle capabilities. Current lifecycle profiles preserve the Control Draft/wireframe admission rules; explicit fixed-layout matrix runs instead select one mode/quality and suppress lifecycle resize, show-switch and suspend exercises. Before either Stage
surface mounts, the runner drives a bounded no-Stage
output baseline capped at 300 seconds. The runner takes authenticated production diagnostics
snapshots before the baseline, at the Stage boundary, and after the Stage
window. It subtracts the fixed scheduler-duration histogram buckets, enforces
the 1 ms-or-5-percent p99 rule, and rejects Stage-window deadline misses or send
errors. During the Stage window it also performs two active-show round trips
through the production safe-blackout transition and, on POSIX reference hosts,
sends the desktop process `SIGSTOP` for one second followed by `SIGCONT`.
Lifecycle timestamps divide cadence and presentation measurements into
continuous active segments; the report separately requires the suspend/resume
cycle and all four show transitions to complete while the independently hosted
output scheduler remains within its deadline and p99 gates. During the packaged
large profile, the runner also opens the same authenticated raw visualization
WebSocket used by the focused browser collector, pauses its underlying TCP
reader, alternates lane resynchronization requests, and requires a bounded
queue replacement or send failure followed by zero final queue depth.

The Tauri bridge drains a bounded native-helper telemetry queue. The benchmark
retains at most one presentation sample for each source frame, quality, surface
size, and lifecycle transition per lane, so a long run cannot grow an
unbounded frontend history. The report is marked
`measurementSurface: packaged-tauri-native-stage` and enforces the 120 ms p95 and
200 ms hard changing-frame ceilings, the 200 ms presentation-gap ceiling, both
lane samples, Control-admitted view/quality, and native renderer restart/resize ownership for lifecycle runs. Explicit fixed-layout conditions record their selected mode/quality separately. It
also records the requested profile and verified fixture/instance counts,
native drawable dimensions, instance and draw-call counts, degraded frames,
and elapsed renderer/acquire wall time plus scoped sparse GPU pass timings from the production
renderer helper. These are neither executing-thread CPU nor displayed-frame timings. Process resident memory is sampled separately by the runner.

The `improved-beam-spike` profile is a synchronous packaged WebGL capability
experiment rather than a latency-duration run. It prepares the same 500-instance
scene and exercises the production Improved-beam shader, contributor budget,
surface light, CPU Raycaster first-opaque-hit shortening, and shadow
configuration. It then measures 60 synchronous frames with 500 fixture bodies
and eight 128-by-128 PCF soft-shadow maps. The final Apple-GPU run on 2026-07-28
used an arm64 Apple M5 Max host with 64 GiB of memory and accepted the capability
and visual sub-spike with a 3 ms p95 and 4 ms maximum after synchronizing every
measured frame with `gl.finish`.
Resolved-color receiver luminance moved from 0 to 213.8, the first opaque
occluder shortened the retained volume at 5.81 m before the 6.71 m receiver,
and the shadow probe darkened from 21.1 to 9.6. The shadow gate requires both
an eight-point absolute reduction and an occluded value no greater than 60%
of the unoccluded value. The WebView did not expose GPU timer queries, which
the report records explicitly rather than substituting CPU submission timing.
That acceptance requires the production
**Improved beams** path to retain one feathered volume per source and budget
surface lights, first-hit shortening, and shadow maps to the eight
highest-contributing directional sources with stable identity and hysteresis.
It does not complete the full Phase 0 gate without the canonical production
Default Stage and large-scene packaged runs. Windows and Linux packaged reports
remain separate release gates.

The duration argument specifies the Stage window; the no-Stage control window
is `min(duration, 300)` seconds. Thus `300` requests five minutes each, while
`1800` requests a five-minute control baseline and a 30-minute Stage window.
Use 300 seconds for canonical comparative latency/output runs and 1800 seconds
for the retained-resource run. Keep the JSON reports with the
completion evidence; a shorter development run does not replace either release
gate.

The 1800-second run additionally enforces the retained-resource gate. The
runner samples the `light-desktop` main-process resident set once per
second, excludes the first Stage minute as warmup, and rejects a fitted growth
slope above 1 MiB per minute. It compares the first and last five-minute
post-warmup render windows, rejects growth in native instance or draw-call
ownership per pane, rejects degraded frames, and requires the late-window CPU frame p95
to stay at or below 16.7 ms. This CPU value is elapsed submission wall time from a source-deduplicated retained subset; it does not measure every 60 Hz frame, executing-thread CPU, GPU completion or display latency. The sampled RSS covers only the `light-desktop` main process; it excludes separate WebView processes and GPU/driver allocations. Native frame telemetry supplies separate renderer ownership evidence, not a complete VRAM measurement.

The long-run evaluator must use independently recorded Stage start/end times,
not infer its window from the first/last retained render or memory sample. Require
the full declared Stage duration; report cold-start observations separately, then require complete RSS and per-pane render coverage from Stage start +60 seconds to Stage completion. Compare the first and last five-minute post-warmup windows and report actual sampled spans and gaps. Ordinary runs of at least 1800 seconds poll every 250 ms; short, phase and capture runs use 100 ms. The declared maximum duration is 3600 seconds. Histories retain bounded prefixes (32,000 host/timeline rows, 64,000 activity rows, 24,000 heartbeats and 100,000 source-deduplicated frames). GPU submission aggregates consume every drained native batch before source deduplication. Special phase/capture runs still fail if their higher poll rate exhausts a bound. Any rejected row, overflow or observation error
invalidates complete-run evidence: a retained prefix cannot become a passing
30-minute report. A static interval may legitimately have no new submissions,
but still requires resident/native ownership and coverage witnesses. A short or
truncated run, missing boundary clock, missing lane, or missing final observation
must remain failed/incomplete rather than pass on a few early samples.

The macOS user session must remain unlocked with ToskLight visible. The runner
reports a stalled native presentation stream as a failed cadence/presentation
gate rather than treating control timers or data delivery as packaged
performance evidence.

## Separate frozen 4K verification (TL-851)

Run each engine through an actual 3840 × 2160 render from frozen invocation-time
scene/values/media/camera/time and local preferences, starting with live Draft or
Lines. Native Ultra uses an isolated full-budget capture renderer and upgrades
3D outline/simple views to the full lighting path without changing live settings.
Keep orthographic views orthographic and disclose the 16:9 side-framing change.
With usable Blender, the operator chooses native Ultra or Blender/Cycles Ultimate;
without usable Blender, native Ultra remains available. Verify saved engine,
dimensions/framing/path, visible progress/failure, owned cancellation/cleanup and
subsequent-job admission. Keep the native and Ultimate PNGs and their job identities.

Ultimate's supported-feature guard and chooser disclosure are part of the result:
plan symbols/outlines and active laser scan transfer require Ultra; procedural
finishes, absent grid/selection guides, native bloom and physical beam/source/fog/
GI/AgX differences are explicit limitations. Freeze missing-media fallback and
availability too; a later server image cannot replace invocation-time pixels.
Do not claim full native/Cycles or physical parity.

The explicit isolated `live-ultra` packaged exercise drives actual DOM controls,
waits for accepted native Live readiness, chooses Ultra and retains Saved UI plus
a decoded, CRC-checked job-associated PNG. Its private dual/internal-display run
and output directory are separate from performance acceptance. It records output
counters during capture, but does not prove physical pointer/shortcut behavior,
visual acceptance or the 60 FPS/GPU targets; the 4K worker may compete for GPU time.

## Historical browser evidence boundary

This collector runs the production frontend in Chromium under Playwright. It
does **not** control or measure the packaged Tauri WebView. Every JSON report
therefore identifies its surface as `browser-playwright` and records:

```json
{
  "packagedWebView": {
    "controlled": false,
    "measured": false
  },
  "acceptanceGateEnforced": false
}
```

The separate packaged collector above owns Tauri WebView timing and resource
evidence. This Playwright collector remains browser-only; do not
reinterpret its Chromium numbers as packaged-WebView acceptance evidence.

The runtime diagnostics expose a cumulative fixed-bucket output scheduler
duration histogram. The collector subtracts paired snapshots and reports the
conservative upper bound of the bucket containing each window's p99; it does
not label latest or lifetime-maximum timing as a percentile.
The browser-delivery recovery case still stops `message` event delivery after
receipt. Separately, the large profile completes a real authenticated
visualization WebSocket handshake and then pauses its underlying TCP reader
while the real test output scheduler and large-scene stream continue. The
report requires bounded latest-value queue ownership plus a recorded queue
replacement or send timeout, with output deadlines remaining healthy. The
five-minute comparative output run, the 30-minute resource run, and
cross-platform GPU capability checks remain separate release gates. A runner
code path for a platform does not itself provide GPU or packaged-runtime
evidence; retain distinct reports from actual supported Windows, Apple Silicon
macOS, and Linux reference machines.


Native Stage lane demand retains a narrow volatile-polling exception to API rules: the
current visualization WebSocket carries resolved attribute snapshots/deltas and typed readouts,
not complete per-instance native raw vectors, mode identities, ownership masks and Point poses.
Until an equivalent accepted-native push contract exists, native Stage reads the existing DMX
snapshot at its existing source cadence. Only a helper following Preload requests its extra
lane; Live omits it. A renderer-local demand serial rejects both native and legacy overlay
responses from earlier display modes, including a double switch back to the original mode.
This saves unused Preload construction/serialization and legacy compatibility work. It does not
establish a cause for the sustained-run output deadline regression: current production family
adapters already use retained Pending publications, and the diagnostic captured no fresh
observational Dynamic projections. This change does not modify that publication contract or
output scheduling.


## Retained macOS results — 25 October 2026

These isolated internal-display runs preserve failed gates. All use animated
Stage-500, dual Live/Preload panes, 1920×1080 total physical pixels and a 60 Hz
helper ceiling. Other desks remained running; no builds, tests or profiling
competed with these measurement windows. Reports and raw samples are under
`.artifacts/performance/stage/` with the following report stems:

| Profile / active duration | Report suffix | Output p99 bucket, control → Stage | Added deadline misses | Result |
| --- | --- | --- | --- | --- |
| Debug Draft / 1,860 s | `2026-10-25T02-03-03-304Z` | 12 → 16 ms | 12 | Fail |
| Release Draft / 1,860 s | `2026-10-25T02-46-57-077Z` | 8 → 16 ms | 1 | Fail |
| Release Lines / 360 s | `2026-10-25T03-23-52-457Z` | 8 → 12 ms | 0 | Fail |
| Release Draft, macOS child priority / 45 s | `2026-10-25T03-42-50-923Z` | 8 → 12 ms | 0 | Fail |

Full report names are `packaged-tauri-stage-500-<suffix>.json`; corresponding
`.jsonl` files retain samples. Every listed control and Stage window had zero
send errors. The unchanged p99 allowance is max(1 ms, 5% of control), so zero
misses alone does not pass output isolation. Histogram buckets are conservative
bounds, not exact percentile measurements.

The sustained Draft runs passed complete retained-resource coverage and their
RSS/late CPU-wall gates. Release Draft had a main-process RSS slope of
−1,346 bytes/min and late sampled CPU-wall p95 of 5.611 ms. Release Lines had
late CPU-wall p95 of 1.696 ms; its shorter duration does not establish the
30-minute resource gate. Renderer CPU fields measure elapsed wall time.

The first three bundles were built from commit `0441d5800`. The short priority row retains its
host/server from that commit and uses renderer helper `51533c6d3` in the resealed
release bundle. The actual two embedded children ran at nice +5; desktop and
headless stayed at 0. This verifies the optional hint, without establishing that
CPU scheduling caused the failures or that the hint resolves them.

Displayed-frame timing, matched-rate whole-Stage GPU execution per second,
cross-platform evidence and human image acceptance remain open. Sparse helper
GPU samples and compositor submission counts cannot substitute for those gates.
No result here constitutes final acceptance of TL-846 or TL-852.


### Offline Metal interval accounting

`tools/stage-gpu-intervals.py` consumes an exported `metal-gpu-intervals` table.
Pass every verified owned renderer/compositor PID with repeated `--owned-pid`
arguments, together with the actual TOC duration (`--trace-duration-seconds`) or
one explicit common window (`--window-start-ns` and `--window-end-ns`). Requested
recording duration and export-tool wall time are not observation durations.

The parser resolves export-local XML identities and column mnemonics, clips
Active execution to that window, and unions duplicate, nested and overlapping
intervals per GPU. `activeWallMillisPerSecond` is the non-overlapping execution
interval metric. Channel unions are also reported, but their sum can exceed wall
time because channels execute concurrently. Raw duration sums double-count
nested/overlapping work. GPU names are display labels; canonical XML IDs separate
same-named devices within that export, without asserting identity across traces.

Missing selected-process execution, unknown selected states, unresolved refs,
ambiguous columns and invalid windows fail instead of becoming zero. Caller
proof of process ownership, lifetime, trace completeness and the exact workload
is still required. Attaching xctrace to one PID can capture other processes;
WindowServer and unrelated applications must remain outside the owned PID set.
The native compositor belongs to the desktop process, not WindowServer.

The retained historical static standalone trace has 162,988,873 ns of owned
execution union over 11.000456 seconds (14.817 ms/wall-second). It has no embedded
compositor evidence and cannot establish current Draft/Lines GPU savings.
Neither this metric nor Core Animation Requested Time/status fields establish
actual displayed frames. Keep display timing and human acceptance separate.


### Isolated server ownership correction

The older runner started headless and the app concurrently. Slow server startup
could cause the app to launch its bundled fallback, racing the original child
for the same port and truncating `light-headless.log`. The retained release
Draft/Lines logs and the 03:57 diagnostic contain bind errors. Preserve their
observed output failures, but backend PID/build and diagnostic-policy attribution
were not verified. In particular, the 03:57 bucket pass cannot provide CPU or
wake evidence: its log contains no such observations.

New isolated runs preflight the port, await the original child's readiness and
verify its unique listener PID before launching the app. They verify ownership
and executable SHA256 again at the end, and report `isolatedServerIdentity`.
The original child log is `owned-light-headless.log`; an app fallback cannot
truncate it. Missing ownership tooling, a foreign listener, child exit, timeout
or changed executable fails the run. Non-isolated workflows remain separate.
For opt-in CPU/wake investigation, set `LIGHT_OUTPUT_PHASE_DIAGNOSTICS=all` and
include `light_output=info,light_engine=info,light_headless_runtime=info` in
`RUST_LOG`; the normal default filter does not enable every diagnostic target.


### Fine timing and owned comparison

TL-872 preserves the original 20-bin histogram and adds an optional versioned
histogram with fixed runtime storage: 250 µs inclusive bins through 32 ms and
explicit wider tails. Older health JSON remains readable; missing, partial or
overflowed fine data is not invented. Wire generation follows the normal typed
contract workflow. Recording adds one bounded counter update under the existing
lock, with no per-tick allocation; a health snapshot copies about 1 KiB more.

The collector validates geometry, version and count conservation across resets
and both windows. It retains the old coarse comparison. Fine p99 is an interval,
so regression is bounded by Stage lower minus control upper through Stage upper
minus control lower. The unchanged allowance is max(1 ms, 5% of control). A pass
requires the regression upper bound to fit the smallest possible allowance;
a proven excess fails, and overlap remains indeterminate and cannot pass.

The clean owned 45 s Draft report `2026-10-25T04-35-46-060Z` proves a real failure:
control p99 is 7,501–7,750 µs, Stage p99 is 8,751–9,000 µs, and regression is
1,001–1,499 µs against the 1,000 µs allowance. Both had zero missed deadlines and
send errors. Coarse 8→12 ms remains visible. Backend/listener PID 3439 and its
SHA256 are verified unchanged at both endpoints in `isolatedServerIdentity`.
This result is not dismissed as a coarse-bucket artifact.

The earlier owned instrumented report `2026-10-25T04-09-02-052Z` has valid CPU
and wait observations: exact tick-wall p99 8.137→8.691 ms and timer-resume p99
3.846→3.793 ms. Some extra caller CPU accompanies increased wall time, but worker
CPU decreases. These observations do not establish process-total CPU growth,
saturation, parking or preemption as the cause. Instrumented short runs remain
separate from clean acceptance.

### Presentation calibration remains unavailable

The bounded Swift probe preserves requested time, callback arrival and SDK
`presentedTime` as separate fields. It validates internal-display containment,
layer attachment and process ownership; it never synthesizes a display sample.
Three retained attempts under `presentation-calibration-*` each completed one
GPU command but received no actual presentation callback before the timeout.
The first trace contains a matching CA request/IOSurface and no handler event.
Later runs verify visibility and layer attachment but still report actual time
zero. These are unsuccessful calibration experiments, not Stage FPS evidence.
The tool exits on its bounded failure and does not leave a monitor running.

### Owned CPU stack diagnostic

`2026-10-25T04-48-16-676Z` is explicitly a sampled diagnostic. The runner's
`LIGHT_STAGE_BENCH_DIAGNOSTIC_LABEL` marks the report `diagnosticOnly`, disables
acceptance flags and prevents canonical-report replacement. Observed failures
and the original exit decision remain intact. Empty labels preserve the normal
workflow; labels over 128 trimmed characters fail before launching anything.

Three-second `/usr/bin/sample` captures target only verified helper PID 4857
and headless PID 4588, with executable/parent/UID/start-time identity retained
before and after. The report verifies backend/listener ownership and SHA256 at
both endpoints. Raw stacks and identity files are under `.artifacts/tmp`, indexed
by `.artifacts/performance/stage/tl852-owned-cpu-stack-diagnostic.json`.
Waiting stacks and wall-sample counts are not CPU-utilization measurements.
The run retains two Stage deadline misses and a 3,751–4,249 µs fine-p99 regression;
profiling perturbs the target, so these observations are not clean acceptance.

Renderer stacks expose repeated runtime content hashing and scene-value decode
work. Server stacks also expose substantial full-diagnostics observation work.
These guide bounded experiments; neither establishes the cause of the earlier
clean regression. Preserve the clean failure and use a fresh uninstrumented
paired run after any change.

### Runtime content hashing experiment (TL-873)

The owned helper sample identifies content hashing inside preparation, full model
reconciliation and shadow-pose signatures. A release CPU-only benchmark compares
the identical production traversal with DefaultHasher and the workspace's
FxHasher, using 500 fixtures/emitters and three real GLB models (37 parts, 16,034
vertices). Seven alternating rounds of 256 iterations retain both sides in
`.artifacts/tmp/tl873-hash-before-after.json`. Median preparation elapsed time is
166.82→110.47 µs; asset fingerprint traversal is 84.44→18.88 µs. These are elapsed
CPU-work timings, not measured thread CPU, GPU savings or FPS acceptance.

The renderer now uses FxHasher only for ephemeral content fingerprints on 64-bit
targets; 32-bit targets retain DefaultHasher's 64-bit output. Field/geometry
traversal, current-scene ownership and same-slot/same-revision replacement checks
remain intact. No persisted/protocol/security hashes, visual quality or pacing
thresholds change. All 276 ordinary renderer tests and 19 explicit native GPU
tests pass. The first broad ignored-test invocation also selected the release-only
CPU benchmark in debug mode and correctly rejected that invocation; a GPU-only
invocation passed all 19. Private packaged output validation remains separate.

Counter-only benchmark observations now use `/api/v2/diagnostics/performance`,
whose typed output health is identical to full diagnostics. Full before/after
snapshots remain. This avoids repeatedly serializing programmer definitions;
it does not erase historical failures or establish their cause. FIFO read/window
closure, counter-reset handling and fine histograms are preserved by focused tests.


### Final clean release observations after TL-873

The 2026-10-25 final release bundle combines the runtime-hash change and the
counter-only observer. Both conditions ran serially on the internal display,
with no concurrent build, tests, CPU worker or profiler. Each used animated
Stage-500, dual 1920×540 physical panes, Draft quality and a requested 60 Hz
helper ceiling. The paired no-Stage control lasted 300 seconds. Original backend
PID/listener and SHA256 were verified at both endpoints. Raw reports and compact
summaries remain under canonical artifacts.

| Condition | Stage duration | Control → Stage output p99 | Regression | Output gate |
| --- | ---: | --- | --- | --- |
| Full 3D Draft | 1,860 s | 7,751–8,000 → 10,501–10,750 µs | 2,501–2,999 µs | **Fail**: allowance 1,000 µs |
| Lines | 360 s | 7,751–8,000 → 8,001–8,250 µs | 1–499 µs | Pass |

Reports are `packaged-tauri-stage-500-2026-10-25T05-06-19-539Z.json` and
`packaged-tauri-stage-500-2026-10-25T05-43-17-980Z.json`. Both windows in both
conditions recorded zero deadline misses, send errors and late intervals.
Draft still fails the p99 comparison; a short prior pass does not supersede it.
No scheduler policy or acceptance threshold was changed.

Draft passed the enforced 30-minute retained-resource gate with complete RSS
and both-lane coverage: RSS slope −15,012 bytes/min and late sampled CPU wall
p95 2.805 ms. Lines has complete six-minute observations, RSS slope +34,243
bytes/min and late CPU wall p95 1.581 ms, but its 30-minute resource gate was
not enforced. Its early/late resource windows overlap.

Draft recorded 223,115 helper and 222,859 compositor submissions; Lines recorded
43,152 and 43,151. Sparse helper GPU means were 16.35/15.44 ms for Draft
Live/Preload and 2.53/2.59 ms for Lines. Sparse timing is not whole-Stage GPU
usage; submission counts are not on-screen FPS. Sustained displayed 60 FPS and
70%/90% matched-rate total-GPU reductions remain unproved. These observations do
not isolate a root cause or establish a hash-only benefit.
