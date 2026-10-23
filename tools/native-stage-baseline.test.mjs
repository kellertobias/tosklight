import assert from "node:assert/strict";
import test from "node:test";
import { readNativeBaselineConfig, evaluateNativeBaseline } from "./native-stage-baseline.mjs";

test("baseline configuration preserves normal runs and declares fixed pixel budget", () => {
  assert.equal(readNativeBaselineConfig({}), null);
  assert.deepEqual(readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "dual" }), {
    layout: "dual", quality: "draft", view: "lines_3d", helperScheduling: "natural",
    physicalPixelArea: 2073600, laneWidth: 1920, laneHeight: 540, display: "internal" });
  assert.throws(() => readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "all" }));
  assert.throws(() => readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "single", LIGHT_STAGE_PACKAGED_BENCH_HELPER_HZ: "120" }));
});

test("successful baseline observations cannot assert displayed FPS or savings", () => {
  const configuration = readNativeBaselineConfig({ LIGHT_STAGE_PACKAGED_BENCH_LAYOUT: "single" });
  const complete = { nativeStage: { gpuObservations: { rows: [{ paneId: "live", followPreload: false, gpuSampleCount: 1, width: 1920, height: 1080, mode: "lines_3d", quality: "draft" }], counts: {}, compositor: { rows: [{}], totals: {} } } } };
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
  const complete = { nativeStage: { gpuObservations: { rows: [{ paneId: "live", followPreload: false, gpuSampleCount: 0, width: 1920, height: 1080, mode: "lines_3d", quality: "draft" }], counts: {}, compositor: { rows: [{}], totals: {} } } } };
  assert.equal(evaluateNativeBaseline(complete, { configuration }).baselineMeasurementReady, false);
});
