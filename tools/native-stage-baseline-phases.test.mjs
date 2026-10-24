import assert from "node:assert/strict";
import test from "node:test";
import { evaluateBaselinePhases, evaluateSteadyHiddenCompositor, evaluateSteadyStaticCompositor } from "./native-stage-baseline-phases.mjs";

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


function interiorEvidence() {
  const evidence = fixture().nativeStage.phases;
  const counters = (submissions = 0) => ({ drawAttempts: 4, submissions, acquireWallMicros: submissions * 20,
    occluded: 0, timeouts: 0, reconfigures: 0, errors: 0, cleanSkips: 0, hiddenSkips: 4 - submissions, busySkips: 0, pacedSkips: 0 });
  evidence.hiddenCompositorIntervals = { invalid: 0, overflow: 0, limit: 240, rows: evidence.attachmentsBeforeHidden.flatMap((pane) => [
    { ...pane, startObservedEpochMicros: null, endObservedEpochMicros: 120_100_000, counters: counters(4), potentialAcquireAttemptsUpperBound: 4 },
    { ...pane, startObservedEpochMicros: 120_100_000, endObservedEpochMicros: 120_300_000, counters: counters(2), potentialAcquireAttemptsUpperBound: 2 },
    { ...pane, startObservedEpochMicros: 120_300_000, endObservedEpochMicros: 120_400_000, counters: counters(), potentialAcquireAttemptsUpperBound: 0 },
    { ...pane, startObservedEpochMicros: 120_400_000, endObservedEpochMicros: 120_500_000, counters: counters(), potentialAcquireAttemptsUpperBound: 0 },
  ]) };
  return evidence;
}
test("steady hidden excludes unknown and settling spans, exposes actual per-pane coverage and no GPU-zero claim", () => {
  const report = evaluateSteadyHiddenCompositor(interiorEvidence());
  assert.equal(report.status, "zero-submissions-and-acquire-wall-observed");
  assert.equal(report.zeroAcquireAdmissionsObserved, true);
  assert.equal(report.lanes.length, 2);
  assert.ok(report.lanes.every((lane) => lane.coveredMicros === 200_000 && lane.intervals === 2 && lane.submissions === 0));
  assert.equal(report.lanes[0].eligibleWindowMicros, 9_750_000);
  assert.ok(report.boundary.includes("No GPU utilization"));
});
test("real interior work is reported and insufficient or corrupted evidence remains unverified", () => {
  const work = interiorEvidence();
  const row = work.hiddenCompositorIntervals.rows[2];
  row.counters.submissions = 1; row.counters.acquireWallMicros = 10; row.counters.hiddenSkips--;
  row.potentialAcquireAttemptsUpperBound = 1;
  assert.equal(evaluateSteadyHiddenCompositor(work).status, "work-observed");
  for (const corrupt of [
    (e) => { e.hiddenCompositorIntervals.rows = e.hiddenCompositorIntervals.rows.filter((row) => row.paneId === "main:live"); },
    (e) => { e.hiddenCompositorIntervals.overflow = 1; },
    (e) => { e.hiddenCompositorIntervals.invalid = 1; },
    (e) => { e.hiddenCompositorIntervals.rows[2].potentialAcquireAttemptsUpperBound = 2; },
    (e) => { e.hiddenCompositorIntervals.rows[3].startObservedEpochMicros--; },
  ]) {
    const evidence = interiorEvidence(); corrupt(evidence);
    assert.equal(evaluateSteadyHiddenCompositor(evidence).status, "unverified");
  }
  assert.equal(evaluateSteadyHiddenCompositor(fixture().nativeStage.phases).status, "unverified");
});

test("steady static uses native interior spans, separate identities, and actual coverage", () => {
  const evidence = interiorEvidence();
  evidence.staticCompositorIntervals = structuredClone(evidence.hiddenCompositorIntervals);
  evidence.attachmentsDuringStatic = evidence.attachmentsBeforeHidden;
  for (const row of evidence.staticCompositorIntervals.rows) {
    if (row.startObservedEpochMicros !== null) row.startObservedEpochMicros -= 20_000_000;
    row.endObservedEpochMicros -= 20_000_000;
    row.counters.cleanSkips = row.counters.hiddenSkips;
    row.counters.hiddenSkips = 0;
  }
  const report = evaluateSteadyStaticCompositor(evidence);
  assert.equal(report.status, "zero-submissions-and-acquire-wall-observed");
  assert.equal(report.zeroAcquireAdmissionsObserved, true);
  assert.equal(report.eligibleStartEpochMicros, 100_250_000);
  assert.ok(report.lanes.every((lane) => lane.coveredMicros === 200_000 && lane.cleanSkips === 8 && lane.hiddenSkips === 0));
  assert.ok(report.boundary.includes("No GPU utilization"));
  evidence.staticCompositorIntervals.rows[2].counters.submissions++;
  assert.equal(evaluateSteadyStaticCompositor(evidence).status, "work-observed");
  evidence.staticCompositorIntervals.overflow++;
  assert.equal(evaluateSteadyStaticCompositor(evidence).status, "unverified");
  assert.equal(evaluateSteadyStaticCompositor(fixture().nativeStage.phases).status, "unverified");
});

test("static identity discovery without a known positive native interval cannot imply idle", () => {
  const evidence = fixture().nativeStage.phases;
  evidence.attachmentsDuringStatic = evidence.attachmentsBeforeHidden;
  evidence.staticCompositorIntervals = { invalid: 0, overflow: 0, rows: evidence.attachmentsDuringStatic.map((pane) => ({
    ...pane, startObservedEpochMicros: null, endObservedEpochMicros: 100_800_000, counters: { submissions: 0 },
  })) };
  assert.equal(evaluateSteadyStaticCompositor(evidence).status, "unverified");
});
