import assert from "node:assert/strict";
import test from "node:test";
import { readNativeBaselineConfig, evaluateNativeBaseline } from "./native-stage-baseline.mjs";

test("baseline configuration preserves normal runs and declares fixed pixel budget", () => {
  assert.equal(readNativeBaselineConfig({}), null);
  assert.deepEqual(readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "dual" }), {
    layout: "dual", requestedTransport: "auto", quality: "draft", view: "lines_3d", atmosphereAmount: 0, helperScheduling: "natural",
    physicalPixelArea: 2073600, laneWidth: 1920, laneHeight: 540, display: "internal" });
  assert.throws(() => readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "all" }));
  assert.throws(() => readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "single", LIGHT_STAGE_PACKAGED_BENCH_HELPER_HZ: "120" }));
});

test("successful baseline observations cannot assert displayed FPS or savings", () => {
  const configuration = readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "single" });
  const complete = { nativeStage: { gpuObservations: { rows: [{ paneId: "live", followPreload: false, gpuSampleCount: 1, width: 1920, height: 1080, mode: "lines_3d", quality: "draft" }], counts: {}, compositor: { rows: [{ samples: 1, scope: ["live", "one", 1, { sourceSize: [1920, 1080] }] }], totals: {} } } } };
  const report = evaluateNativeBaseline(complete, { configuration });
  assert.equal(report.baselineMeasurementReady, true);
  assert.equal(report.acceptanceGateEnforced, false);
  assert.equal(report.displayed60Fps.status, "unverified");
  assert.equal(report.gpuReduction.status, "unverified");
});

test("missing lane, pixel mismatch and telemetry loss fail the measurement gate", () => {
  const configuration = readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "dual" });
  const complete = { nativeStage: { gpuObservations: { rows: [{ paneId: "live", followPreload: false, gpuSampleCount: 1, width: 1920, height: 1080, mode: "lines_3d", quality: "draft" }], counts: { rowOverflow: 1 }, compositor: { rows: [], invalid: 1, totals: {} } } } };
  const report = evaluateNativeBaseline(complete, { configuration });
  assert.equal(report.baselineMeasurementReady, false);
  assert.equal(report.failures.length, 7);
});

test("submission-only telemetry cannot complete a GPU baseline", () => {
  const configuration = readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "single" });
  const complete = { nativeStage: { gpuObservations: { rows: [{ paneId: "live", followPreload: false, gpuSampleCount: 0, width: 1920, height: 1080, mode: "lines_3d", quality: "draft" }], counts: {}, compositor: { rows: [{ samples: 1, scope: ["live", "one", 1, { sourceSize: [1920, 1080] }] }], totals: {} } } } };
  assert.equal(evaluateNativeBaseline(complete, { configuration }).baselineMeasurementReady, false);
});

test("phase supplement requires explicit fixed and static configuration", () => {
  assert.throws(() => readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_PHASES: "static-camera-hidden" }));
  assert.throws(() => readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "dual", LIGHT_STAGE_PACKAGED_BENCH_PHASES: "unknown" }));
  assert.throws(() => readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "dual", LIGHT_STAGE_PACKAGED_BENCH_PHASES: "static-camera-hidden", LIGHT_PACKAGED_STAGE_DYNAMICS: "1" }));
  assert.equal(readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "dual", LIGHT_STAGE_PACKAGED_BENCH_PHASES: "static-camera-hidden" }).phases, "static-camera-hidden");
});

test("animated suspension requires Dynamics and remains distinct from static evidence", () => {
  const env = { LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "dual", LIGHT_STAGE_PACKAGED_BENCH_PHASES: "animated-camera-hidden" };
  assert.throws(() => readNativeBaselineConfig(env));
  assert.equal(readNativeBaselineConfig({ ...env, LIGHT_PACKAGED_STAGE_DYNAMICS: "1" }).phases, "animated-camera-hidden");
});

test("transport overrides require fixed layout and reject unknown conditions before launch", () => {
  for (const transport of ["", "bogus", "Shared"])
    assert.throws(() => readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "single", LIGHT_STAGE_PACKAGED_BENCH_TRANSPORT: transport }));
  assert.throws(() => readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_TRANSPORT: "copy" }));
  for (const requestedTransport of ["auto", "copy", "shared"])
    assert.equal(readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "single", LIGHT_STAGE_PACKAGED_BENCH_TRANSPORT: requestedTransport }).requestedTransport, requestedTransport);
});

test("explicit transport requires matching helper and compositor evidence", () => {
  const configuration = readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "single", LIGHT_STAGE_PACKAGED_BENCH_TRANSPORT: "copy" });
  const helper = { paneId: "live", followPreload: false, gpuSampleCount: 1, width: 1920, height: 1080, mode: "lines_3d", quality: "draft", transport: "copy" };
  const compositor = { samples: 1, scope: ["live", "one", 1, { sourceSize: [1920, 1080], transport: "copy" }] };
  const complete = { nativeStage: { gpuObservations: { rows: [helper], counts: {}, compositor: { rows: [compositor], totals: {} } } } };
  assert.equal(evaluateNativeBaseline(complete, { configuration }).baselineMeasurementReady, true);
  helper.transport = "shared";
  assert.equal(evaluateNativeBaseline(complete, { configuration }).baselineMeasurementReady, false);
  helper.transport = "copy";
  compositor.scope[3].transport = "shared";
  assert.equal(evaluateNativeBaseline(complete, { configuration }).baselineMeasurementReady, false);
  delete helper.transport;
  assert.equal(evaluateNativeBaseline(complete, { configuration }).baselineMeasurementReady, false);
});


test("fixed native reports retain resource evidence and reject truncated collectors or output regressions", () => {
  const configuration = readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "single" });
  const complete = { nativeStage: { gpuObservations: { rows: [{ paneId: "live", followPreload: false, gpuSampleCount: 1, width: 1920, height: 1080, mode: "lines_3d", quality: "draft" }], counts: {}, compositor: { rows: [{ samples: 1, scope: ["live", "one", 1, { sourceSize: [1920, 1080] }] }], totals: {} } } } };
  const resources = { processMemory: { measurement: "main-process RSS", samples: [{ recordedAt: 1, residentBytes: 100 }] }, longRun: { enforced: true, failures: [] } };
  const outputComparison = { boundedWindowGatePassed: true, stageWindowDeadlineMisses: 0, stageWindowSendErrors: 0 };
  const metadata = { configuration, resources, outputComparison };
  assert.equal(evaluateNativeBaseline(complete, metadata).resources, resources);
  assert.equal(evaluateNativeBaseline(complete, metadata).baselineMeasurementReady, true);
  for (const field of ["timeline", "frames"]) {
    complete.nativeStage.observerOverflow = { [field]: 1 };
    assert.equal(evaluateNativeBaseline(complete, metadata).baselineMeasurementReady, false);
  }
  delete complete.nativeStage.observerOverflow;
  complete.nativeStage.hostObservationOverflow = 1;
  assert.equal(evaluateNativeBaseline(complete, metadata).baselineMeasurementReady, false);
  delete complete.nativeStage.hostObservationOverflow;
  complete.nativeStage.collectorIntegrity = { passed: false };
  assert.equal(evaluateNativeBaseline(complete, metadata).baselineMeasurementReady, false);
  delete complete.nativeStage.collectorIntegrity;
  resources.longRun.failures = ["RSS coverage missing"];
  assert.ok(evaluateNativeBaseline(complete, metadata).failures.includes("RSS coverage missing"));
  resources.longRun.failures = [];
  for (const output of [
    { ...outputComparison, boundedWindowGatePassed: false },
    { ...outputComparison, stageWindowDeadlineMisses: 1 },
    { ...outputComparison, stageWindowSendErrors: 1 },
  ]) assert.equal(evaluateNativeBaseline(complete, { ...metadata, outputComparison: output }).baselineMeasurementReady, false);
  assert.equal(evaluateNativeBaseline(complete, metadata).acceptanceGateEnforced, false);
});


test("long native scope must stay declared and cover each lane throughout Stage", () => {
  const configuration = readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "dual" });
  const rows = [];
  for (const followPreload of [false, true]) for (let second = 1000; second <= 2860; second++)
    rows.push({ paneId: followPreload ? "preload" : "live", attachmentId: followPreload ? "2" : "1", followPreload,
      gpuSampleCount: 1, helperSubmissionCount: 60, width: 1920, height: 540, mode: "lines_3d", quality: "draft", transport: "shared", windowEpochSecond: second });
  const complete = { nativeStage: { stageStartedEpochMillis: 1_000_000, stageCompletedEpochMillis: 2_860_000,
    gpuObservations: { rows, counts: {}, compositor: { rows: ["live", "preload"].map((pane) => ({ samples: 1, scope: [pane, "one", 1, { sourceSize: [1920, 540] }] })), totals: {} } } } };
  const metadata = { configuration, durationSeconds: 1860 };
  assert.equal(evaluateNativeBaseline(complete, metadata).baselineMeasurementReady, true);
  const coldRows = rows.filter((row) => row.windowEpochSecond >= 1017);
  complete.nativeStage.gpuObservations.rows = coldRows;
  assert.equal(evaluateNativeBaseline(complete, metadata).baselineMeasurementReady, true);
  complete.nativeStage.gpuObservations.rows = rows;
  const final = rows.at(-1);
  for (const [field, value] of [["quality", "ultra"], ["mode", "full_3d"], ["height", 1080]]) {
    const original = final[field]; final[field] = value;
    const bad = evaluateNativeBaseline(complete, metadata);
    assert.equal(bad.baselineMeasurementReady, false);
    assert.match(bad.failures.join("\n"), /changed away/u);
    final[field] = original;
  }
  complete.nativeStage.gpuObservations.rows = rows.filter((row) => !row.followPreload || row.windowEpochSecond < 1900);
  assert.equal(evaluateNativeBaseline(complete, metadata).baselineMeasurementReady, false);
  complete.nativeStage.gpuObservations.rows = rows.filter((row) => !row.followPreload || row.windowEpochSecond < 1900 || row.windowEpochSecond > 1910);
  assert.equal(evaluateNativeBaseline(complete, metadata).baselineMeasurementReady, false);
});
