# Native Stage Performance

These scenarios are packaged-application acceptance only. Run them on every supported platform with a real GPU and an unlocked visible operator session. Browser Playwright, software rendering, and short development runs may diagnose failures but never satisfy these scenarios. Retain the generated `.artifacts/performance/stage/stage-visualization-timing.json` with the result.

## STAGE-PERF-001 — Default Stage latency and output isolation

Build the packaged desk through the authoritative build workflow, then run `npm run benchmark:packaged-stage -- 300 default-stage`. Record application identity, OS, GPU/backend, resolution, quality modes, and show/look. Both Live and Follow Preload lanes must sustain the 10 Hz source cadence independently. Source-to-settled-canvas p95 must be at most 120 ms; no changing frame or presentation gap may exceed 200 ms. Exercise every quality, native context loss/recovery, renderer teardown/recreation, a paused visualization consumer, show switching, and application suspension where supported. The five-minute Stage window must not introduce output deadline misses/send errors or regress scheduler p99 by more than 1 ms or 5 percent against the paired no-Stage window.

## STAGE-PERF-002 — 500-instance bounded delivery and resource release

Run `npm run benchmark:packaged-stage -- 300 stage-500` for the comparative gate and `npm run benchmark:packaged-stage -- 1800 stage-500` for retained resources. Confirm the generated scene contains exactly 500 fixture records and 500 physical instances. Apply the same latency, lane, lifecycle, quality, bounded latest-frame, and output-isolation gates as STAGE-PERF-001. During the 30-minute run, exclude the first Stage minute as warmup; resident growth must remain at or below 1 MiB per minute, WebGL geometry/texture ownership must not grow between the first and last five-minute windows, and late CPU submission p95 must remain at or below 16.7 ms. A missing GPU/backend field, locked/background-throttled session, or non-packaged identity fails acceptance rather than becoming an exception.
