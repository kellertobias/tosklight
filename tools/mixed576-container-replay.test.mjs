import assert from "node:assert/strict";
import test from "node:test";
import { options, summarize } from "./mixed576-container-replay.mjs";

test("options default to the free profile and bound the frame count", () => {
	assert.deepEqual(options([]), { profile: "free", frames: 128, label: "free-128", compare: null, skipBuild: false });
	assert.equal(options(["--profile", "ci", "--frames", "64"]).label, "ci-64");
	assert.throws(() => options(["--profile", "fast"]), /profile must be one of/);
	assert.throws(() => options(["--frames", "0"]), /--frames/);
	assert.throws(() => options(["--frames", "257"]), /--frames/);
	assert.throws(() => options(["--bogus"]), /unknown option/);
});

test("summaries report per-frame costs and digest every fingerprint", () => {
	const entry = (workers) => ({
		workers, warmup: 2, measured: 8, transaction_total_ns: 80e6, whole_loop_including_warmup_and_fingerprints_ns: 100e6,
		process_cpu: { available: 1, total_seconds: 0.12 }, family_phase_order: ["prepare_families"], family_phase_total_us: [16000],
	});
	const report = { per_frame_fingerprints: [{ dmx: "a" }], cases: [entry(1), entry(2)] };
	const summary = summarize(report);
	assert.equal(summary.cases[0].transaction_ms, 10);
	assert.equal(summary.cases[0].cpu_ms_per_frame, 12);
	assert.equal(summary.cases[0].wall_ms_per_frame, 10);
	assert.equal(summary.cases[1].family_ms.prepare_families, 2);
	assert.notEqual(summarize({ ...report, per_frame_fingerprints: [{ dmx: "b" }] }).fingerprints_sha256, summary.fingerprints_sha256);
	assert.equal(summarize({ ...report, cases: [{ ...entry(1), process_cpu: { available: 0 } }] }).cases[0].cpu_ms_per_frame, null);
});
