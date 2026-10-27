const finiteCamera = (camera) => Array.isArray(camera) && camera.length === 6 && camera.every(Number.isFinite);
const sorted = (panes) => (panes ?? []).map((pane) => `${pane.paneId}\0${pane.attachmentId}`).sort();

/** Phase completion is measurement evidence only, never a GPU reduction/display acceptance pass. */
export function evaluateBaselinePhases(complete, configuration, matchingRows) {
  const names = [configuration.phases === "animated-camera-hidden" ? "animated" : "static", "camera", "hidden", "revealed"];
  const evidence = complete.nativeStage?.phases;
  const failures = [];
  const rows = evidence?.rows;
  if (evidence?.status !== "completed" || evidence?.error || !Array.isArray(rows) || rows.length !== 4)
    return { measurementStatus: "incomplete", evidence: evidence ?? null, failures: ["Baseline phase exercise did not complete"] };
  let previousEnd = 0;
  for (let index = 0; index < names.length; index++) {
    const row = rows[index];
    if (row.phase !== names[index] || row.status !== "completed" || !Number.isFinite(row.startedEpochMillis) ||
        !Number.isFinite(row.finishedEpochMillis) || row.startedEpochMillis < previousEnd || row.finishedEpochMillis - row.startedEpochMillis < 10_000)
      failures.push(`Invalid or truncated ${names[index]} baseline phase`);
    previousEnd = row.finishedEpochMillis;
  }
  const camera = rows[1];
  if (!finiteCamera(camera.cameraBefore) || !finiteCamera(camera.cameraAfter) ||
      camera.cameraBefore.every((value, index) => value === camera.cameraAfter[index]) || !(camera.cameraRequests > 0))
    failures.push("Baseline camera motion has no changed native camera witness");
  const visibility = evidence.nativeVisibility;
  if (!visibility?.hiddenVerified || !visibility?.revealedVerified ||
      !Number.isSafeInteger(visibility.hiddenEpochMicros) || !Number.isSafeInteger(visibility.revealedEpochMicros) ||
      visibility.revealedEpochMicros - visibility.hiddenEpochMicros < 10_000_000 ||
      visibility.hiddenEpochMicros / 1000 < rows[2].startedEpochMillis ||
      visibility.revealedEpochMicros / 1000 > rows[2].finishedEpochMillis)
    failures.push("Baseline has no verified ten-second native hidden interval");
  const before = evidence.attachmentsBeforeHidden ?? [];
  const required = configuration.layout === "dual" ? 2 : 1;
  if (before.length !== required || new Set(sorted(before)).size !== required ||
      before.some((pane) => !pane.paneId?.startsWith("main:") || !pane.attachmentId) ||
      JSON.stringify(sorted(before)) !== JSON.stringify(sorted(evidence.attachmentsAfterReveal)))
    failures.push("Baseline hide/reveal did not retain the same owned pane attachments");
  for (const pane of before) {
    const heartbeats = (complete.nativeStage?.heartbeats ?? []).filter((beat) => beat.paneId === pane.paneId && beat.attachmentId === pane.attachmentId && beat.attached);
    if (!heartbeats.some((beat) => !beat.visible && beat.receivedEpochMicros >= visibility?.hiddenEpochMicros && beat.receivedEpochMicros <= visibility?.revealedEpochMicros))
      failures.push(`No resident hidden heartbeat for ${pane.paneId}`);
    if (!heartbeats.some((beat) => beat.visible && beat.receivedEpochMicros >= visibility?.revealedEpochMicros && beat.receivedEpochMicros / 1000 <= rows[3].finishedEpochMillis))
      failures.push(`No resident revealed heartbeat for ${pane.paneId}`);
  }
  const summaries = rows.map((phase) => {
    // Whole one-second helper buckets avoid crediting edge frames to the wrong phase.
    const covered = matchingRows.filter((row) => row.windowEpochSecond * 1000 >= phase.startedEpochMillis && (row.windowEpochSecond + 1) * 1000 <= phase.finishedEpochMillis);
    const paneIds = new Set(matchingRows.map((row) => row.paneId));
    const compositorRows = (complete.nativeStage?.gpuObservations?.compositor?.rows ?? []).filter((row) =>
      paneIds.has(row.scope?.[0]) && row.scope?.[2] * 1000 >= phase.startedEpochMillis &&
      (row.scope?.[2] + 1) * 1000 <= phase.finishedEpochMillis);
    return { phase: phase.phase, wholeSecondBuckets: covered.length,
      compositor: {
        counters: phase.compositorCounters ?? null,
        gpuSamplesByScope: compositorRows,
        sampledGpuCount: compositorRows.reduce((sum, row) => sum + row.samples, 0),
        sampledGpuMicros: compositorRows.reduce((sum, row) => sum + row.microsSum, 0),
        boundary: "Scoped sparse compositor GPU samples by submission epoch; not total GPU time/s or displayed frames",
      },
      helperSubmissions: covered.reduce((sum, row) => sum + row.helperSubmissionCount, 0),
      sampledGpuCount: covered.reduce((sum, row) => sum + row.gpuSampleCount, 0),
      sampledGpuMicros: covered.reduce((sum, row) => sum + row.gpuSampleMicrosSum, 0),
      boundary: "Whole-bucket helper submissions and sparse timing samples; not total GPU time/s or displayed FPS" };
  });
  return { measurementStatus: failures.length ? "incomplete" : "ready-for-review", evidence, summaries, failures,
    steadyHiddenCompositor: evaluateSteadyHiddenCompositor(evidence),
    steadyStaticCompositor: evaluateSteadyStaticCompositor(evidence, complete.nativeStage?.heartbeats ?? []) };
}

/** Exclude unknown startup spans and the first250ms after verified native hide. */
export function evaluateSteadyHiddenCompositor(evidence) {
  const retained = evidence?.hiddenCompositorIntervals;
  const visibility = evidence?.nativeVisibility;
  const expected = evidence?.attachmentsBeforeHidden ?? [];
  const unverified = (reason) => ({ status: "unverified", reason,
    boundary: "Native compositor counter intervals only; zero counters are not proof of zero GPU utilization" });
  if (!retained || retained.invalid || retained.overflow || !Array.isArray(retained.rows) || retained.rows.length > 240)
    return unverified("Bounded hidden compositor intervals are missing, invalid or overflowed");
  if (!visibility?.hiddenVerified || !visibility?.revealedVerified || !Number.isSafeInteger(visibility.hiddenEpochMicros) || !Number.isSafeInteger(visibility.revealedEpochMicros) || !expected.length)
    return unverified("Verified native hidden boundaries or owned pane identities are missing");
  if (expected.length > 2 || new Set(sorted(expected)).size !== expected.length || expected.some((pane) => !pane.paneId?.startsWith("main:") || !pane.attachmentId))
    return unverified("Owned hidden pane identities are invalid");
  return evaluateSteadyCompositorIntervals(retained, expected, visibility.hiddenEpochMicros, visibility.revealedEpochMicros);
}

export function evaluateSteadyStaticCompositor(evidence, heartbeats = []) {
  const phase = evidence?.rows?.find((row) => row.phase === "static" && row.status === "completed");
  const expected = evidence?.attachmentsDuringStatic ?? [];
  const unverified = (reason) => ({ status: "unverified", reason,
    boundary: "Static needs a healthy unchanged-state idle witness per owned lane; missing evidence never establishes zero work" });
  if (!phase || !expected.length || expected.length > 2 || new Set(sorted(expected)).size !== expected.length)
    return unverified("Static phase or complete owned identities are missing");
  const lanes = [];
  for (const pane of expected) {
    const witness = heartbeats.filter((beat) => beat.paneId === pane.paneId && beat.attachmentId === pane.attachmentId &&
      beat.attached === true && beat.visible === true && beat.drawHealthy === true && beat.idleUnchanged === true &&
      [beat.sequence, beat.sourceFrame, beat.lastPresentedSequence, beat.epochMicros, beat.receivedEpochMicros].every((value) => Number.isSafeInteger(value) && value > 0) &&
      beat.epochMicros >= phase.startedEpochMillis * 1_000 && beat.epochMicros <= phase.finishedEpochMillis * 1_000 &&
      beat.receivedEpochMicros >= beat.epochMicros && beat.receivedEpochMicros <= phase.finishedEpochMillis * 1_000)
      .sort((a, b) => a.receivedEpochMicros - b.receivedEpochMicros)[0];
    if (!witness) return unverified(`No healthy native idleUnchanged witness for ${pane.paneId}`);
    const result = evaluateSteadyCompositorIntervals(evidence.staticCompositorIntervals, [pane],
      witness.receivedEpochMicros, phase.finishedEpochMillis * 1_000);
    if (!result.lanes?.length) return unverified(result.reason);
    lanes.push({ ...result.lanes[0], idleWitness: witness, eligibleStartEpochMicros: result.eligibleStartEpochMicros });
  }
  return { status: lanes.every((lane) => lane.submissions === 0 && lane.acquireWallMicros === 0)
      ? "zero-submissions-and-acquire-wall-observed" : "work-observed",
    settlingExcludedMicros: 250_000, endEpochMicros: phase.finishedEpochMillis * 1_000, lanes,
    zeroAcquireAdmissionsObserved: lanes.every((lane) => lane.potentialAcquireAttemptsUpperBound === 0),
    boundary: "Per-lane first healthy unchanged-state idle heartbeat arrival plus 250ms; wholly interior native intervals and actual coverage only. No GPU utilization or total GPU-time claim." };
}

function evaluateSteadyCompositorIntervals(retained, expected, start, finish) {
  const unverified = (reason) => ({ status: "unverified", reason,
    boundary: "Native compositor counter intervals only; zero counters are not proof of zero GPU utilization" });
  if (!retained || retained.invalid || retained.overflow || !Array.isArray(retained.rows) || retained.rows.length > 240)
    return unverified("Bounded compositor intervals are missing, invalid or overflowed");
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(finish) || finish <= start || !expected.length ||
      expected.length > 2 || new Set(sorted(expected)).size !== expected.length || expected.some((pane) => !pane.paneId?.startsWith("main:") || !pane.attachmentId))
    return unverified("Phase boundaries or owned pane identities are invalid");
  const eligibleStart = start + 250_000;
  const lanes = [];
  for (const pane of expected) {
    const rows = retained.rows.filter((row) => row.paneId === pane.paneId && row.attachmentId === pane.attachmentId &&
      Number.isSafeInteger(row.startObservedEpochMicros) && Number.isSafeInteger(row.endObservedEpochMicros) &&
      row.startObservedEpochMicros >= eligibleStart && row.endObservedEpochMicros <= finish && row.endObservedEpochMicros > row.startObservedEpochMicros);
    if (!rows.length) return unverified(`No wholly interior native intervals for ${pane.paneId}`);
    let previous = null;
    let covered = 0;
    for (const row of rows) {
      if (previous !== null && row.startObservedEpochMicros < previous) return unverified("Native compositor intervals overlap or are reordered");
      for (const name of ["drawAttempts", "submissions", "acquireWallMicros", "occluded", "timeouts", "reconfigures", "errors"])
        if (!Number.isSafeInteger(row.counters?.[name]) || row.counters[name] < 0) return unverified("Native compositor interval counters are invalid");
      const skipNames = ["cleanSkips", "hiddenSkips", "busySkips", "pacedSkips"];
      const known = skipNames.every((name) => Number.isSafeInteger(row.counters[name]) && row.counters[name] >= 0);
      const expectedUpper = known ? row.counters.drawAttempts - skipNames.reduce((total, name) => total + row.counters[name], 0) : null;
      if ((expectedUpper !== null && expectedUpper < 0) || row.potentialAcquireAttemptsUpperBound !== expectedUpper)
        return unverified("Compositor acquisition admission accounting is inconsistent");
      previous = row.endObservedEpochMicros;
      covered += row.endObservedEpochMicros - row.startObservedEpochMicros;
    }
    const sum = (name) => rows.reduce((total, row) => total + row.counters[name], 0);
    const upperKnown = rows.every((row) => Number.isSafeInteger(row.potentialAcquireAttemptsUpperBound) && row.potentialAcquireAttemptsUpperBound >= 0);
    lanes.push({ ...pane, intervals: rows.length, firstObservedEpochMicros: rows[0].startObservedEpochMicros,
      lastObservedEpochMicros: rows.at(-1).endObservedEpochMicros, coveredMicros: covered,
      eligibleWindowMicros: Math.max(0, finish - eligibleStart),
      cleanSkips: rows.every((row) => Number.isSafeInteger(row.counters.cleanSkips)) ? sum("cleanSkips") : null,
      hiddenSkips: rows.every((row) => Number.isSafeInteger(row.counters.hiddenSkips)) ? sum("hiddenSkips") : null,
      busySkips: rows.every((row) => Number.isSafeInteger(row.counters.busySkips)) ? sum("busySkips") : null,
      pacedSkips: rows.every((row) => Number.isSafeInteger(row.counters.pacedSkips)) ? sum("pacedSkips") : null,
      drawAttempts: sum("drawAttempts"), submissions: sum("submissions"),
      acquireWallMicros: sum("acquireWallMicros"), occluded: sum("occluded"), timeouts: sum("timeouts"),
      errors: sum("errors"), reconfigures: sum("reconfigures"),
      potentialAcquireAttemptsUpperBound: upperKnown ? rows.reduce((total, row) => total + row.potentialAcquireAttemptsUpperBound, 0) : null,
    });
  }
  return { status: lanes.every((lane) => lane.submissions === 0 && lane.acquireWallMicros === 0)
      ? "zero-submissions-and-acquire-wall-observed" : "work-observed",
    settlingExcludedMicros: 250_000, eligibleStartEpochMicros: eligibleStart, endEpochMicros: finish, lanes,
    zeroAcquireAdmissionsObserved: lanes.every((lane) => lane.potentialAcquireAttemptsUpperBound === 0),
    boundary: "Wholly interior native compositor counter intervals with actual per-pane coverage. Zero acquire wall time can be quantized; zero acquisition admissions require the skip-accounted upper bound. No GPU utilization or total GPU-time claim." };
}
