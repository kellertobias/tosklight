# Stage performance evidence

The Stage performance collector records a repeatable engineering baseline for
the Default Stage and a deterministic large scene. It combines the existing
frontend Stage diagnostics with the authenticated runtime output and
visualization diagnostics.

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

The runner stops the already-open development desk and service, launches the
exact debug Tauri application with an isolated fresh benchmark data directory,
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

The benchmark surface keeps independent Live and Follow Preload views open,
cycles every render quality, exercises 2D/3D native surfaces, resizes their
shared drawable, tears down and recreates the live renderer helper, and drives
alternating Live and Preload values through the
production command and Preload lifecycle capabilities. Before either Stage
surface mounts, it drives the same changes for an equal-duration no-Stage
output baseline. The runner takes authenticated production diagnostics
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
lane samples, all four qualities, and native renderer restart/resize ownership. It
also records the requested profile and verified fixture/instance counts,
native drawable dimensions, instance and draw-call counts, degraded frames,
and measured CPU acquisition and GPU submission time from the production
renderer helper. Process resident memory is sampled separately by the runner.

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

The duration argument applies independently to the no-Stage and Stage windows,
so `300` produces a five-minute baseline followed by a five-minute measured
Stage window. Use 300 seconds for the canonical comparative latency/output runs
and 1800 seconds for the retained-resource run. Keep the JSON reports with the
completion evidence; a shorter development run does not replace either release
gate.

The 1800-second run additionally enforces the retained-resource gate. The
runner samples the `light-desktop` main-process resident set once per
second, excludes the first Stage minute as warmup, and rejects a fitted growth
slope above 1 MiB per minute. It compares the first and last five-minute
post-warmup render windows, rejects growth in native instance or draw-call
ownership, rejects degraded frames, and requires the late-window CPU frame p95
to stay at or below 16.7 ms. Main-process resident memory does not claim
complete Metal driver allocation ownership; native frame telemetry is the
authoritative renderer evidence.

The macOS user session must remain unlocked with ToskLight visible. The runner
reports a stalled native presentation stream as a failed cadence/presentation
gate rather than treating control timers or data delivery as packaged
performance evidence.

## Evidence boundary

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
