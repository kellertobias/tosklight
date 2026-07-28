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
- runtime output frames, packets, errors, deadline misses, and timing maxima;
- runtime visualization projections, projection time, payload bytes, source
  age, subscribers, and skipped source frames.

The Default Stage profile measures the repository fixture unchanged. The large
profile deterministically extends the same fixture to 470 fixture records and
500 Stage instances. Added fixtures and multipatches retain the representative
profile and mode mix but are deliberately unpatched, so they increase Stage
projection/rendering work without consuming DMX addresses or increasing output
routes.

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

The repository currently has no automation bridge into the packaged macOS
WebView. Use `npm run open` for the real packaged operator path and complete the
required visual review there. Do not treat the Chromium numbers as packaged
WebView acceptance evidence.

The current runtime diagnostics expose cumulative counters and timing maxima,
not a per-frame output-latency distribution. Consequently the collector reports
the available output counters honestly and does not invent an output p99.
Latency thresholds remain informational until a packaged-WebView control seam
and the required sampler exist.
