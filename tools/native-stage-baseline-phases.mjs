const names = ["static", "camera", "hidden", "revealed"];
const finiteCamera = (camera) => Array.isArray(camera) && camera.length === 6 && camera.every(Number.isFinite);
const sorted = (panes) => (panes ?? []).map((pane) => `${pane.paneId}\0${pane.attachmentId}`).sort();

/** Phase completion is measurement evidence only, never a GPU reduction/display acceptance pass. */
export function evaluateBaselinePhases(complete, configuration, matchingRows) {
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
  return { measurementStatus: failures.length ? "incomplete" : "ready-for-review", evidence, summaries, failures };
}
