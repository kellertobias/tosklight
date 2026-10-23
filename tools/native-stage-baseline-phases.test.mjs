import assert from "node:assert/strict";
import test from "node:test";
import { evaluateBaselinePhases } from "./native-stage-baseline-phases.mjs";

function fixture() {
  const names = ["static", "camera", "hidden", "revealed"];
  const identities = [{ paneId: "main:live", attachmentId: "one" }, { paneId: "main:preload", attachmentId: "two" }];
  return { nativeStage: { phases: { status: "completed", measurementStatus: "pending", error: null,
    rows: names.map((phase, index) => ({ phase, status: "completed", startedEpochMillis: 100_000 + index * 10_000, finishedEpochMillis: 110_000 + index * 10_000,
      ...(phase === "camera" ? { cameraRequests: 100, cameraBefore: [0,0,0,0,0,1], cameraAfter: [1,0,0,0,0,1] } : {}) })),
    nativeVisibility: { hiddenVerified: true, revealedVerified: true, hiddenEpochMicros: 120_000_000, revealedEpochMicros: 130_000_000 },
    attachmentsBeforeHidden: identities, attachmentsAfterReveal: identities,
  }, heartbeats: identities.flatMap((id) => [ { ...id, attached: true, visible: false, receivedEpochMicros: 125_000_000 }, { ...id, attached: true, visible: true, receivedEpochMicros: 135_000_000 } ]) } };
}
test("phase completion preserves measurement-only boundary and real resident witnesses", () => {
  const report = evaluateBaselinePhases(fixture(), { layout: "dual" }, []);
  assert.equal(report.measurementStatus, "ready-for-review");
  assert.deepEqual(report.failures, []);
  assert.ok(report.summaries.every((row) => row.boundary.includes("not total GPU")));
});
test("missing/truncated/native-unverified/reattached phases fail rather than imply idle", () => {
  for (const corrupt of [
    (f) => { f.nativeStage.phases.rows[0].finishedEpochMillis--; },
    (f) => { f.nativeStage.phases.nativeVisibility.hiddenVerified = false; },
    (f) => { f.nativeStage.phases.attachmentsAfterReveal = []; },
    (f) => { f.nativeStage.phases.rows[1].cameraAfter = [0,0,0,0,0,1]; },
    (f) => { f.nativeStage.heartbeats = []; },
  ]) {
    const f = fixture(); corrupt(f);
    assert.equal(evaluateBaselinePhases(f, { layout: "dual" }, []).measurementStatus, "incomplete");
  }
  assert.equal(evaluateBaselinePhases({}, { layout: "dual" }, []).measurementStatus, "incomplete");
});

test("compositor GPU samples use their scoped submission epoch, independently of helper counters", () => {
  const f = fixture();
  f.nativeStage.phases.rows[2].compositorCounters = { values: { drawAttempts: 100, submissions: 5, occluded: 95, acquireWallMicros: 800 }, nominalPollMillis: 100 };
  f.nativeStage.gpuObservations = { compositor: { rows: [
    { scope: ["main:live", "one", 125, { generation: 1 }], samples: 2, microsSum: 16, microsMax: 9 },
    { scope: ["main:live", "one", 119, { generation: 1 }], samples: 1, microsSum: 99, microsMax: 99 },
    { scope: ["foreign:live", "foreign", 125, { generation: 1 }], samples: 1, microsSum: 999, microsMax: 999 },
  ] } };
  const result = evaluateBaselinePhases(f, { layout: "dual" }, [{ paneId: "main:live", windowEpochSecond: 125, helperSubmissionCount: 1000, gpuSampleCount: 1, gpuSampleMicrosSum: 20 }]);
  const hidden = result.summaries.find((row) => row.phase === "hidden");
  assert.equal(hidden.compositor.sampledGpuCount, 2);
  assert.equal(hidden.compositor.sampledGpuMicros, 16);
  assert.equal(hidden.compositor.counters.values.occluded, 95);
  assert.equal(hidden.helperSubmissions, 1000);
});
