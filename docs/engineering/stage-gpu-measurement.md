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

`single` uses one 1920×1080 physical drawable. `dual` uses Live and Follow Preload at 1920×540 each, keeping total physical pixel area equal. `full_3d` and `lines_3d` choose the actual renderer mode only inside the benchmark subtree. Draft, Standard, High and Ultra use their actual renderer settings. The ordinary Control path retains its wireframe/Draft contract.

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
