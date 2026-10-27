import assert from "node:assert/strict";
import test from "node:test";
import { prepareStageDiagnosticReport, readStageDiagnosticLabel } from "./packaged-stage-diagnostic.mjs";

test("diagnostic labels are explicit, trimmed and bounded before launch", () => {
	assert.equal(readStageDiagnosticLabel({}), null);
	assert.equal(readStageDiagnosticLabel({ LIGHT_STAGE_BENCH_DIAGNOSTIC_LABEL: " \n " }), null);
	assert.equal(readStageDiagnosticLabel({ LIGHT_STAGE_BENCH_DIAGNOSTIC_LABEL: "  CPU stack profile  " }), "CPU stack profile");
	assert.equal(readStageDiagnosticLabel({ LIGHT_STAGE_BENCH_DIAGNOSTIC_LABEL: "x".repeat(128) }), "x".repeat(128));
	assert.throws(() => readStageDiagnosticLabel({ LIGHT_STAGE_BENCH_DIAGNOSTIC_LABEL: "x".repeat(129) }), /128/);
});

test("labeled valid runs retain observations, exit successfully and never publish acceptance", () => {
	const result = { baselineMeasurementReady: true, acceptanceGateEnforced: true,
		metrics: { p99: 7777 }, failures: [], outputComparison: { observed: true } };
	for (const fixedBaseline of [false, true]) {
		const outcome = prepareStageDiagnosticReport(result, { label: "sampled stacks", fixedBaseline });
		assert.equal(outcome.report.diagnosticOnly, true);
		assert.equal(outcome.report.diagnosticLabel, "sampled stacks");
		assert.equal(outcome.report.acceptanceGateEnforced, false);
		assert.equal(outcome.report.baselineMeasurementReady, false);
		assert.equal(outcome.report.metrics, result.metrics);
		assert.equal(outcome.report.outputComparison, result.outputComparison);
		assert.equal(outcome.report.failures, result.failures);
		assert.equal(outcome.writeCanonical, false);
		assert.equal(outcome.exitSuccessful, true);
	}
	assert.equal(result.acceptanceGateEnforced, true);
});

test("diagnostic labeling never hides actual errors or failed observed gates", () => {
	const result = { baselineMeasurementReady: false, acceptanceGateEnforced: false, failures: ["owned server died"] };
	const outcome = prepareStageDiagnosticReport(result, { label: "CPU", fixedBaseline: true, benchmarkFailure: true });
	assert.equal(outcome.exitSuccessful, false);
	assert.deepEqual(outcome.report.failures, ["owned server died"]);
	assert.equal(outcome.writeCanonical, false);
	assert.equal(prepareStageDiagnosticReport({ acceptanceGateEnforced: false }, { label: "CPU" }).exitSuccessful, false);
});

test("unlabeled reports preserve existing capture, fixed-baseline and ordinary behavior", () => {
	const result = { captureVerified: true, baselineMeasurementReady: true, acceptanceGateEnforced: false };
	const ordinary = prepareStageDiagnosticReport(result);
	assert.equal(ordinary.report, result);
	assert.equal(ordinary.writeCanonical, true);
	assert.equal(ordinary.exitSuccessful, false);
	assert.equal(prepareStageDiagnosticReport(result, { fixedBaseline: true }).exitSuccessful, true);
	const capture = prepareStageDiagnosticReport(result, { captureExercise: true });
	assert.equal(capture.writeCanonical, false);
	assert.equal(capture.exitSuccessful, true);
	const diagnosticCapture = prepareStageDiagnosticReport(result, { captureExercise: true, label: "offline diagnostic" });
	assert.equal(diagnosticCapture.report.captureVerified, true);
	assert.equal(diagnosticCapture.report.acceptanceGateEnforced, false);
	assert.equal(diagnosticCapture.writeCanonical, false);
	assert.equal(diagnosticCapture.exitSuccessful, true);
});
