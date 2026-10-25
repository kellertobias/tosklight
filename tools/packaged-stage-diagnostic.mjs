/** Explicit opt-in diagnostics never replace canonical acceptance evidence. */
export function readStageDiagnosticLabel(environment = process.env) {
	const value = environment.LIGHT_STAGE_BENCH_DIAGNOSTIC_LABEL;
	if (value === undefined) return null;
	if (typeof value !== "string") throw new Error("LIGHT_STAGE_BENCH_DIAGNOSTIC_LABEL must be text");
	const label = value.trim();
	if (label.length > 128) throw new Error("LIGHT_STAGE_BENCH_DIAGNOSTIC_LABEL must be at most 128 characters after trimming");
	return label || null;
}

export function prepareStageDiagnosticReport(result, {
	label = null,
	captureExercise = false,
	fixedBaseline = false,
	benchmarkFailure = false,
} = {}) {
	// Preserve the original evaluation for exit status before disabling acceptance claims.
	// Instrumentation does not make genuine collection/evaluation errors successful.
	const exitSuccessful = !benchmarkFailure && Boolean(captureExercise
		? result.captureVerified
		: fixedBaseline ? result.baselineMeasurementReady : result.acceptanceGateEnforced);
	return {
		report: label === null ? result : {
			...result,
			diagnosticOnly: true,
			diagnosticLabel: label,
			baselineMeasurementReady: false,
			acceptanceGateEnforced: false,
		},
		writeCanonical: !captureExercise && label === null,
		exitSuccessful,
	};
}
