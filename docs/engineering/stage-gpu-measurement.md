# Stage GPU measurement

TL-847 supplies before/after measurement for TL-846. This instrumentation does not change normal Control picture policy or establish human acceptance. Existing packaged acceptance remains in `docs/testing/16-stage-performance.md`.

## Boundaries

- A source update, helper submission, completed GPU timing sample, compositor submission and displayed drawable are different events. The legacy protocol name `FramePresented` observes helper submission only.
- Each helper GPU sample has an attachment-local identity and captured rendering scope. Consume repeated IDs and reject late samples from another mode, quality, scene or size. Do not credit a rejected sample later.
- Report helper and Control compositor timings separately. GPU timestamps cover instrumented passes, not the complete desktop or display engine. Sparse sample sums are not total GPU time per second and cannot prove reduction percentages.
- CPU measurements are elapsed wall time. Acquire wait is recorded separately; executing-thread CPU is unavailable.
- Every unavailable measurement stays explicit. Missing GPU/display evidence never means zero cost. A zero-sample lane cannot complete a GPU baseline.
- The target is sustained displayed 60 FPS, with proposed matched-rate GPU reductions of at least 70 percent for Draft and 90 percent for Lines. Submission caps alone do not establish those reductions.

## Fixed physical-pixel runs

Use an isolated worktree and packaged bundle. Preserve the user's active desk with explicit unused HTTP and OSC ports. macOS benchmark windows construct hidden, move to the built-in display and verify containment before showing; they refuse ambiguous placement. The internal display must fit the physical window.

```sh
LIGHT_PACKAGED_STAGE_PORT=5160 \
LIGHT_PACKAGED_STAGE_OSC_PORT=19520 \
LIGHT_STAGE_PACKAGED_BENCH_LAYOUT=single \
LIGHT_STAGE_PACKAGED_BENCH_VIEW=lines_3d \
LIGHT_STAGE_PACKAGED_BENCH_QUALITY=draft \
LIGHT_STAGE_PACKAGED_BENCH_DISPLAY=internal \
node tools/run-packaged-stage-benchmark.mjs 30 canonical-demo
```

`single` uses one 1920×1080 physical drawable. `dual` uses Live and Follow Preload at 1920×540 each, keeping total physical pixel area equal. `full_3d` and `lines_3d` choose the actual renderer mode only inside the benchmark subtree. Draft, Standard, High and Ultra use their actual renderer settings. The full-render benchmark uses a fixed haze amount of 0.35; Lines uses zero. This override is enforced only with an absolute benchmark report path plus validated fixed layout. The ordinary Control path retains its wireframe/Draft contract.

Set `LIGHT_STAGE_PACKAGED_BENCH_TRANSPORT=copy` or `shared` to measure one transport explicitly. The default `auto` retains capability negotiation and prefers Shared where usable, with Copy fallback. An explicit request never falls back: unavailable desk/helper support fails the run. These overrides require a fixed layout and an absolute native benchmark report path; ordinary desks retain normal negotiation. The report records `configuration.requestedTransport` and requires matching actual helper and compositor transport evidence before marking the measurement ready. Keep Copy and Shared results separate.

Run both `canonical-demo` and `stage-500`; set `LIGHT_PACKAGED_STAGE_DYNAMICS=1` for animated stage-500 (19 universes). Record the actual inventory rather than assuming an older count. Retain camera/look, DPR, physical dimensions, transport, GPU/backend, OS, bundle hash, other load, monitor refresh and thermal state with each result.

Leave `LIGHT_STAGE_PACKAGED_BENCH_HELPER_HZ` unset for natural scheduling. Set it to `60` for a controlled matched-rate condition; it only takes effect with passive benchmark reporting enabled. Keep natural and matched runs separate. If throughput is below 60, report the shortfall rather than inventing a 60 FPS baseline.

Fixed runs suppress lifecycle resize/release/show-switch/suspend exercises. They produce measurement reports with `acceptanceGateEnforced:false`; they do not replace the full packaged acceptance scenarios. Raw observations and reports remain under canonical `.artifacts/performance/stage`. Storage is bounded and overflows invalidate the baseline.

## Platform display trace

`tools/stage-gpu-trace.mjs` attaches only to an explicitly identified, current-user-owned isolated ToskLight native process. It checks executable/start-time identity before and after capture. Its recorder/export subprocesses have bounded TERM-to-KILL cleanup; it never signals the target application.

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
node tools/stage-gpu-trace.mjs --pid PID \
  --executable /absolute/path/to/light-desktop --owned-isolated \
  --build COMMIT --window 'internal single 1920x1080 physical Draft Lines' --duration 10
```

Inspect exported schemas before deriving displayed FPS. Full-display compositor swap surfaces are not the application's drawable surfaces. Correlate the target PID, drawable/surface identity and actual presentation event; do not substitute queue completion or global monitor refresh.

On the initial M5 Max / macOS 26.6.2 / Xcode 27.1 discovery trace, CoreAnimation supplied 31 PID-attributed drawable-presented-handler events across only half a second. Status semantics and exported presentation timebase remain unresolved. That discovery does not establish sustained displayed 60 FPS. Later accepted performance evidence must resolve those limits or use another declared platform measurement.

## Static, camera and resident-hidden supplement

After the active before-state matrix, collect four additional Stage-500 conditions: `lines_3d` and `full_3d`, each under natural and controlled-60 helper scheduling, using the dual layout. Leave `LIGHT_PACKAGED_STAGE_DYNAMICS` unset so this supplemental rig is static. The canonical demo runner always starts its twelve benchmark playbacks; it is not a static workload switch.

```sh
LIGHT_PACKAGED_STAGE_PORT=5160 \
LIGHT_PACKAGED_STAGE_OSC_PORT=19520 \
LIGHT_STAGE_PACKAGED_BENCH_LAYOUT=dual \
LIGHT_STAGE_PACKAGED_BENCH_VIEW=lines_3d \
LIGHT_STAGE_PACKAGED_BENCH_QUALITY=draft \
LIGHT_STAGE_PACKAGED_BENCH_DISPLAY=internal \
LIGHT_STAGE_PACKAGED_BENCH_PHASES=static-camera-hidden \
node tools/run-packaged-stage-benchmark.mjs 50 stage-500
```

The opt-in phase exercise records ten seconds each of static view, bounded programmatic camera orbit, resident hidden window, and revealed view. Forty seconds is the minimum requested Stage duration; the runner waits for phase completion rather than truncating native hide/placement/IPC overhead. Fifty seconds leaves collection margin. The exercise has a sixty-second collection bound and records an explicit failure if it cannot finish. Ordinary fixed runs retain their existing behavior.

The private fixed-dual bridge reserves native 3D ownership for its first pane ID and allocates the second without that exclusive flag; both still request the same actual 3D picture. The owner ID remains stable across helper close/reopen so native camera diagnostics stay available and another pane cannot steal ownership. This scoped allocation exception lasts only for the fixed benchmark. Single-pane and ordinary Control allocation policies are unchanged.

Hiding uses a guarded native command scoped to the main benchmark window, an absolute report path, validated fixed layout, explicit phase opt-in, static Stage-500, and the minimum duration. It hides the existing window without unmounting Stage or restarting helpers. A Rust timer owns the hidden interval, so hidden-WebView timer throttling cannot leave the window waiting for JavaScript to reveal it. The command verifies visibility changes, revalidates internal-display placement before reveal, and never requests focus. Native hidden/revealed timestamps and the same before/after pane attachment identities are retained.

While hidden, native collection drains the same telemetry queues every 100 ms into at most 120 batches and 16 MiB. The frontend pauses its regular collector after its in-flight drain completes, then ingests those native batches through the same aggregators before resuming. Existing global queue bounds are unchanged. Overflow, failed visibility, missing resident heartbeats, unchanged camera witnesses, or recreated attachments invalidate phase evidence.

Final reports retain explicit phase boundaries and whole-second helper submission/sparse-GPU summaries. The raw JSONL also records verified native visibility boundaries. These observations are report-ready baseline evidence, not proof of displayed 60 FPS, total GPU time per second, near-zero hidden GPU work, or percentage savings. The offline evaluator checks completion and resident witnesses separately from final performance acceptance; phase results remain pending until that evaluation runs. Actual hidden cost, including continued helper work, must be reported honestly.

Each phase also snapshots compositor aggregate counters before and after its work, independently of helper submissions. Retained deltas include draw attempts, successful submissions, occluded/time-out outcomes, reconfiguration/errors, CPU wall time and acquire wall time. They describe arrivals at the collector: polling is nominally 100 ms, and each snapshot retains its actual observation timestamp and age so delayed boundaries are visible rather than assumed precise. Native hidden batches are ingested before the hidden phase's ending snapshot. The phase summary separately groups sparse compositor GPU samples by their captured pane/attachment/render scope and submission epoch, keeping their cost distinct from helper GPU samples. These diagnostic counter deltas cannot establish on-glass FPS or total GPU utilization.


## Positive presentation-time calibration, 2026-10-25

A bounded single-layer Swift probe now releases all strong drawable references
after commit inside an autorelease pool. The prior retained-drawable experiments
remain unsuccessful. The new `presentation-calibration-sLtVMu` experiment has
60 SDK callbacks/completions and 55 positive `presentedTime` values. Its retained
`gpu-trace-X43MFy/analysis/presentation-calibration-summary.json` and joined
witnesses identify exactly 55 status-2 CA handler rows, drawable IDs 5–59,
matching those positive SDK values. Five SDK zero times remain unclassified
not-presented/skipped outcomes, with no corresponding trace rows.

For this Xcode 27.1 export path, the handler field labeled `Requested Time`
reconstructs actual host nanoseconds as `rawAtTime * denominator / numerator +
epochMachTicks`. All 55 joined values match the SDK actual time within 0.92 ns.
This is an empirical calibration of this export/timebase, not a portable schema
assumption. Handler callback timestamps and the roughly 8.333 ms `delta` field
are distinct from actual presentation time and source frame intervals.

Request targets are float32-quantized at this uptime: the 28 scheduled rows can
differ from SDK double-precision targets by about ±27.5 ms. They cannot support
precise latency claims. Immediate requests use a sentinel. Command-buffer labels
join each traced request to its SDK sequence and backing IOSurface.

The collector retains an error: the probe exited with its strict 60-positive
criterion unmet before final process identity verification. The raw trace is
useful calibration evidence, not a successful collection or Stage FPS result.
No probe IOSurface joins directly to display-swap/hardware-layer surfaces. A
single-layer calibration also does not resolve overlapping drawable IDs across
two Control layers. Later measurements must prove layer attribution and coverage;
PID or nearest-time matching alone is insufficient.

## Physical UI service limitation

The unique-bundle-ID private clone `run-5d798f26953f` verified its private
server, expected show, native panes and cache identity on the internal display.
The CUA service timed out selecting that app after about 7,130 seconds, returning
no accessibility result. No physical pointer or shortcut action was confirmed.
The coordinator independently ended after 15 minutes, revoked its launch token
and cleaned its own children. Both app/server PIDs and loopback 5162 were verified
absent afterward. This is an automation-service limitation; physical capture
acceptance remains open. Existing successful packaged/native image evidence
must not be relabeled as physical input coverage.


## Draft/Lines diagnostic sampling cadence (TL-875)

Draft and Lines now admit a GPU timing sample at most once per second, with one
frame-start decision shared by every pass. Rendering still follows its existing
cadence. Outstanding readbacks, slot completion and nonblocking device polls
continue between timing samples. Standard, High and Ultra retain their previous
every-available-frame timing policy; Ultra's adaptive input is unchanged.

In a benchmark with an absolute native report path, set
`LIGHT_STAGE_GPU_TIMING_CADENCE=full` or `1hz` explicitly. Other values fail with
an actionable error. Without a benchmark report the override is ignored.
Fixed-layout runner configuration retains the **requested** override; it does
not prove that an older binary applied it. For a paired comparison, retain native
startup policy witnesses and actual sampling coverage, and match the declared
policy. An absent historical field means unknown, rather than today's default.

Actual helper coverage is available through the existing opt-in pass-profile
JSONL `timing_coverage` field. Native compositor drains include `timingCoverage`;
the frontend retains the latest cumulative snapshot per pane/attachment, bounded
to 64 owners. Repeated drains are not added together. Coverage distinguishes
completed readbacks from usable timestamp samples; inconsistent/regressing
snapshots fail validation. Historical missing coverage remains unavailable.
These are owner-lifetime timing counters, including different quality modes if
the operator switched views. They are not displayed-frame counts or a per-mode
rate unless the measured condition stayed fixed.

## First whole-owned-GPU before/after diagnostic

Receipt: `.artifacts/performance/stage/tl874-before-after-owned-gpu-diagnostic.json`.
The frozen before build and the cbcbd612a-era after build each ran the same
animated Stage-500 program, dual 1920×540 physical panes on the internal display,
with requested helper 60 Hz. Each retained an owned 10-second Metal trace and
exact app/server/two-helper hashes and PID/start identity at both endpoints.
Both collectors succeeded; both runners retained their output comparison failures.
This comparison predates TL-875's sampling changes.

In the common relative [1,10)-second trace interval, combined owned Active
execution union was 986.627 ms per wall second before and 684.120 after, an
observed 30.66% reduction. Per-lane whole-second helper submission means rose
from 34.25/34.875 to 60.0/60.125. The intervals exclude WindowServer and other
processes and account for overlapping GPU channels/processes through union.
Channel sums, sparse frame timings and summed process totals are different metrics.

**This is not the 70% matched-rate acceptance result.** Achieved cadence differs;
individual animated poses, thermal state and background load are not proven
identical. One nine-second interval per build does not establish sustained
behavior or power/energy savings. Parent CA handler IDs overlap across layers;
no exact per-pane join was found, so the roughly 119 combined positive events/s
cannot certify per-pane displayed FPS. Raw traces remain `gpu-trace-WfrqWt`
(before) and `gpu-trace-uUxaUM` (after); limitations and failures are retained.


### Actual default sampling coverage in the final packaged build

The full release host/helper/headless bundle built from `b0eab21b8` was measured
with `LIGHT_STAGE_GPU_TIMING_CADENCE` unset. Diagnostic report
`packaged-tauri-stage-500-2026-10-25T09-01-50-577Z.json` and compact receipt
`tl875-default-coverage-summary.json` retain the actual applied policy. Both
helpers and both compositors reported `oneHz`, `benchmarkOverride=false`, and
45 admitted, completed, valid samples over approximately 2,650 frames each.
This confirms sample admission rather than merely retaining a requested setting.

The 45-second control and 45-second Stage windows both had fine output p99
8,001–8,250 µs, with conservative regression −249–249 µs and no comparison
failures. Opt-in pass profiling was enabled, so this is a diagnostic pass and
cannot replace a clean sustained acceptance run.

### Public API boundary for actual per-pane presentation

In the pinned wgpu 30 source, `src/api/surface_texture.rs:10–25` publicly exposes
the texture while keeping backend presentation detail private. The Metal HAL's
`src/metal/mod.rs:653–675` retains the acquired drawable in private fields and
only exposes texture borrowing. `src/api/queue.rs:306–322` documents submission
completion, while `Queue::present` at lines 377–380 exposes no presentation
callback. A public texture handle is not the corresponding acquired drawable.

Obtaining another drawable from the layer would observe a different picture.
Exact per-pane SDK presentation callbacks require a separately planned upstream
hook or native drawable ownership/import architecture; private-layout casts or
method swizzling are not a measurement substitute. The calibrated single-layer
probe and ambiguous aggregate CA events do not close this acceptance gap.
