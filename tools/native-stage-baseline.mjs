import { evaluateBaselinePhases } from "./native-stage-baseline-phases.mjs";
/** A measurement report; this deliberately cannot pass the displayed-60-FPS acceptance gate. */
export function readNativeBaselineConfig(env) {
  const layout = env.LIGHT_STAGE_PACKAGED_BENCH_LAYOUT;
  if (!layout) {
    if (env.LIGHT_STAGE_PACKAGED_BENCH_TRANSPORT !== undefined) throw new Error("Baseline transport requires fixed layout");
    if (env.LIGHT_STAGE_PACKAGED_BENCH_PHASES) throw new Error("Baseline phases require fixed layout");
    return null;
  }
  if (!["single", "dual"].includes(layout)) throw new Error("Baseline layout must be single or dual");
  const quality = env.LIGHT_STAGE_PACKAGED_BENCH_QUALITY ?? "draft";
  const view = env.LIGHT_STAGE_PACKAGED_BENCH_VIEW ?? "lines_3d";
  const helperHz = env.LIGHT_STAGE_PACKAGED_BENCH_HELPER_HZ ?? "natural";
  const phases = env.LIGHT_STAGE_PACKAGED_BENCH_PHASES;
  const requestedTransport = env.LIGHT_STAGE_PACKAGED_BENCH_TRANSPORT ?? "auto";
  if (!["auto", "copy", "shared"].includes(requestedTransport)) throw new Error("Baseline transport must be copy, shared or auto");
  if (phases && !["static-camera-hidden", "animated-camera-hidden"].includes(phases)) throw new Error("Invalid baseline phases");
  if (phases && ((phases === "animated-camera-hidden") !== (env.LIGHT_PACKAGED_STAGE_DYNAMICS === "1"))) throw new Error("Phase mode must match the explicit Dynamics workload");
  if (!["draft", "standard", "high", "ultra"].includes(quality)) throw new Error("Invalid baseline quality");
  if (!["full_3d", "lines_3d"].includes(view)) throw new Error("Invalid baseline view");
  if (!["natural", "60"].includes(helperHz)) throw new Error("Matched baseline helper Hz must be 60 or absent");
  return { layout, quality, view, requestedTransport, ...(phases ? { phases } : {}), atmosphereAmount: view === "full_3d" ? 0.35 : 0, helperScheduling: helperHz, physicalPixelArea: 1920 * 1080,
    laneWidth: 1920, laneHeight: layout === "single" ? 1080 : 540,
    display: env.LIGHT_STAGE_PACKAGED_BENCH_DISPLAY ?? "internal" };
}

export function evaluateNativeBaseline(complete, metadata) {
  const { configuration } = metadata;
  const observations = complete.nativeStage?.gpuObservations;
  const rows = observations?.rows ?? [];
  const failures = [];
  if (!rows.length) failures.push("No raw helper submission observations");
  const requested = configuration.requestedTransport ?? "auto";
  const transportMatches = (transport) => requested === "auto" || transport === requested;
  if (!["auto", "copy", "shared"].includes(requested)) failures.push("Invalid declared baseline transport");
  const conditionRows = rows.filter((row) => row.width === configuration.laneWidth && row.height === configuration.laneHeight && row.quality === configuration.quality && row.mode === configuration.view);
  const matches = conditionRows.filter((row) => transportMatches(row.transport));
  if (conditionRows.some((row) => !transportMatches(row.transport))) failures.push("Helper transport does not match requested baseline transport");
  if (!matches.length) failures.push("No observations match declared physical pixels, quality and view");
  for (const followPreload of configuration.layout === "dual" ? [false, true] : [false])
    if (!matches.some((row) => row.followPreload === followPreload && row.gpuSampleCount > 0)) failures.push(`No accepted helper GPU samples for ${followPreload ? "Preload" : "Live"} lane`);
  if (configuration.layout === "dual" && new Set(matches.map((row) => row.paneId)).size < 2) failures.push("Dual layout did not observe both lanes");
  const compositorRows = observations?.compositor?.rows ?? [];
  for (const paneId of new Set(matches.map((row) => row.paneId)))
    if (!compositorRows.some((row) => row.samples > 0 && transportMatches(row.scope?.[3]?.transport) && row.scope?.[0] === paneId && row.scope?.[3]?.sourceSize?.[0] === configuration.laneWidth && row.scope?.[3]?.sourceSize?.[1] === configuration.laneHeight)) failures.push(`No compositor GPU samples for ready source in ${paneId}`);
  if (!compositorRows.length) failures.push("No independent compositor GPU samples");
  for (const counter of ["attachmentOverflow", "rowOverflow", "invalidFrames", "invalidGpuSamples"])
    if (observations?.counts?.[counter] > 0) failures.push(`Helper observation ${counter}`);
  const compositor = observations?.compositor;
  if (compositor?.overflow || compositor?.invalid || compositor?.totals?.gpuSampleOverflow || compositor?.totals?.gpuReadbackErrors || compositor?.totals?.errors) failures.push("Compositor observation loss or error");
  for (const [counter, count] of Object.entries(complete.nativeStage?.observerOverflow ?? {}))
    if (count > 0) failures.push(`Native retained observation ${counter} overflow`);
  if (complete.nativeStage?.hostObservationOverflow > 0 || complete.nativeStage?.collectorIntegrity?.passed === false)
    failures.push("Native collector retention integrity failed");
  failures.push(...(metadata.resources?.longRun?.failures ?? []));
  const output = metadata.outputComparison;
  if (output && (!output.boundedWindowGatePassed || output.stageWindowDeadlineMisses > 0 || output.stageWindowSendErrors > 0))
    failures.push("Stage output scheduler comparison failed");
  const declaredScopeCoverage = metadata.durationSeconds >= 1800
    ? longRunScopeCoverage(complete.nativeStage, rows, matches, configuration) : null;
  if (declaredScopeCoverage) failures.push(...declaredScopeCoverage.failures);
  const phases = configuration.phases ? evaluateBaselinePhases(complete, configuration, matches) : null;
  if (phases) failures.push(...phases.failures);
  return { schemaVersion: 1, kind: "native-stage-before-optimization-measurement", ...metadata,
    measurementSurface: "packaged-tauri-native-stage", failures,
    baselineMeasurementReady: failures.length === 0,
    acceptanceGateEnforced: false,
    displayed60Fps: { status: "unverified", reason: "Requires correlated platform display trace; submissions are not displayed frames" },
    gpuReduction: { status: "unverified", reason: "Sparse timing samples are not total GPU time per second or a matched-rate reduction" },
    measurementBoundaries: complete.nativeStage?.measurementBoundaries ?? null,
    gpuObservations: observations ?? null, ...(declaredScopeCoverage ? { declaredScopeCoverage } : {}), ...(phases ? { phases } : {}) };
}

function longRunScopeCoverage(native, rows, matches, configuration) {
  const startedAt = native?.stageStartedEpochMillis;
  const completedAt = native?.stageCompletedEpochMillis;
  const failures = [];
  const lanes = [];
  if (!Number.isFinite(startedAt) || !Number.isFinite(completedAt) || completedAt <= startedAt)
    return { failures: ["Declared native scope requires independent Stage bounds"], lanes };
  const inWindow = (row) => row.windowEpochSecond * 1000 <= completedAt && (row.windowEpochSecond + 1) * 1000 >= startedAt;
  const accepted = new Set(matches);
  if (rows.some((row) => inWindow(row) && !accepted.has(row)))
    failures.push("Native helper changed away from the declared quality, mode, physical pixels or transport during Stage");
  for (const followPreload of configuration.layout === "dual" ? [false, true] : [false]) {
    const laneRows = matches.filter((row) => inWindow(row) && row.followPreload === followPreload && row.helperSubmissionCount > 0);
    const paneIds = [...new Set(laneRows.map((row) => row.paneId))];
    if (paneIds.length !== 1) failures.push(`Declared scope has missing or changing ${followPreload ? "Preload" : "Live"} pane identity`);
    const seconds = [...new Set(laneRows.map((row) => row.windowEpochSecond))].sort((a, b) => a - b);
    const firstAt = seconds.length ? seconds[0] * 1000 : null;
    const lastAt = seconds.length ? (seconds.at(-1) + 1) * 1000 : null;
    const maximumGapMillis = seconds.reduce((maximum, second, index) => index ? Math.max(maximum, (second - seconds[index - 1] - 1) * 1000) : maximum, 0);
    const complete = firstAt !== null && firstAt <= startedAt + 5000 && lastAt >= completedAt - 5000 && maximumGapMillis <= 5000;
    lanes.push({ followPreload, paneIds, firstAt, lastAt, maximumGapMillis, complete });
    if (!complete) failures.push(`Declared native scope does not cover the full ${followPreload ? "Preload" : "Live"} Stage window`);
  }
  return { startedAt, completedAt, observationBucketMillis: 1000, coverageToleranceMillis: 5000, lanes, failures,
    boundary: "Declared helper submission scope coverage; not displayed frames or continuous GPU utilization." };
}
