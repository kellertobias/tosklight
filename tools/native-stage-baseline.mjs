/** A measurement report; this deliberately cannot pass the displayed-60-FPS acceptance gate. */
export function readNativeBaselineConfig(env) {
  const layout = env.LIGHT_STAGE_PACKAGED_BENCH_LAYOUT;
  if (!layout) return null;
  if (!["single", "dual"].includes(layout)) throw new Error("Baseline layout must be single or dual");
  const quality = env.LIGHT_STAGE_PACKAGED_BENCH_QUALITY ?? "draft";
  const view = env.LIGHT_STAGE_PACKAGED_BENCH_VIEW ?? "lines_3d";
  const helperHz = env.LIGHT_STAGE_PACKAGED_BENCH_HELPER_HZ ?? "natural";
  if (!["draft", "standard", "high", "ultra"].includes(quality)) throw new Error("Invalid baseline quality");
  if (!["full_3d", "lines_3d"].includes(view)) throw new Error("Invalid baseline view");
  if (!["natural", "60"].includes(helperHz)) throw new Error("Matched baseline helper Hz must be 60 or absent");
  return { layout, quality, view, helperScheduling: helperHz, physicalPixelArea: 1920 * 1080,
    laneWidth: 1920, laneHeight: layout === "single" ? 1080 : 540,
    display: env.LIGHT_STAGE_PACKAGED_BENCH_DISPLAY ?? "internal" };
}

export function evaluateNativeBaseline(complete, metadata) {
  const { configuration } = metadata;
  const observations = complete.nativeStage?.gpuObservations;
  const rows = observations?.rows ?? [];
  const failures = [];
  if (!rows.length) failures.push("No raw helper submission observations");
  const matches = rows.filter((row) => row.width === configuration.laneWidth && row.height === configuration.laneHeight && row.quality === configuration.quality && row.mode === configuration.view);
  if (!matches.length) failures.push("No observations match declared physical pixels, quality and view");
  for (const followPreload of configuration.layout === "dual" ? [false, true] : [false])
    if (!matches.some((row) => row.followPreload === followPreload && row.gpuSampleCount > 0)) failures.push(`No accepted helper GPU samples for ${followPreload ? "Preload" : "Live"} lane`);
  if (configuration.layout === "dual" && new Set(matches.map((row) => row.paneId)).size < 2) failures.push("Dual layout did not observe both lanes");
  const compositorRows = observations?.compositor?.rows ?? [];
  for (const paneId of new Set(matches.map((row) => row.paneId)))
    if (!compositorRows.some((row) => row.samples > 0 && row.scope?.[0] === paneId && row.scope?.[3]?.sourceSize?.[0] === configuration.laneWidth && row.scope?.[3]?.sourceSize?.[1] === configuration.laneHeight)) failures.push(`No compositor GPU samples for ready source in ${paneId}`);
  if (!compositorRows.length) failures.push("No independent compositor GPU samples");
  for (const counter of ["attachmentOverflow", "rowOverflow", "invalidFrames", "invalidGpuSamples"])
    if (observations?.counts?.[counter] > 0) failures.push(`Helper observation ${counter}`);
  const compositor = observations?.compositor;
  if (compositor?.overflow || compositor?.invalid || compositor?.totals?.gpuSampleOverflow || compositor?.totals?.gpuReadbackErrors || compositor?.totals?.errors) failures.push("Compositor observation loss or error");
  return { schemaVersion: 1, kind: "native-stage-before-optimization-measurement", ...metadata,
    measurementSurface: "packaged-tauri-native-stage", failures,
    baselineMeasurementReady: failures.length === 0,
    acceptanceGateEnforced: false,
    displayed60Fps: { status: "unverified", reason: "Requires correlated platform display trace; submissions are not displayed frames" },
    gpuReduction: { status: "unverified", reason: "Before-state only; sparse timing samples are not total GPU time per second" },
    measurementBoundaries: complete.nativeStage?.measurementBoundaries ?? null,
    gpuObservations: observations ?? null };
}
