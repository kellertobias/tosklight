import assert from "node:assert/strict";
import test from "node:test";
import { summarizeHeadlessStress } from "./run-headless-stress-benchmark.mjs";

function report(targetMet = true) {
	return {
		required_floor_met: null,
		process_resources: { resident_bytes: 512 * 1024 * 1024 },
		scenarios: [{
			workload_tier: "headless_stress",
			release_blocking: false,
			active_ui_surfaces: [],
			visualization_enabled: false,
			fixture_count: 2_000,
			physical_instance_count: 2_000,
			dynamic_definition_count: 20,
			dynamic_lane_attributes: [
				"intensity", "color.red", "color.green", "color.blue", "pan", "tilt",
			],
			dynamic_excluded_fixture_count: 920,
			universes: 74,
			met_configured_rate: targetMet,
			fixture_inventory: { total_slots: 37_720 },
			frame_rate: {
				average_completed_hz: targetMet ? 60 : 55,
				minimum_one_second_completed_hz: targetMet ? 60 : 54,
				required_minimum_hz: 60,
			},
			deadline: {
				dropped_ticks: targetMet ? 0 : 5,
				deferred_ticks: 1,
				deadline_misses: 2,
			},
		}],
	};
}

test("summarizes valid headless evidence", () => {
	const summary = summarizeHeadlessStress(report(), 2_000);
	assert.equal(summary.valid, true);
	assert.equal(summary.targetMet, true);
	assert.equal(summary.dynamicCount, 20);
	assert.equal(summary.occupiedSlots, 37_720);
	assert.equal(summary.residentBytes, 512 * 1024 * 1024);
});

test("keeps a valid 60 Hz miss informational", () => {
	const summary = summarizeHeadlessStress(report(false), 2_000);
	assert.equal(summary.valid, true);
	assert.equal(summary.targetMet, false);
});

test("rejects reports that accidentally enable visualization", () => {
	const invalid = report();
	invalid.scenarios[0].visualization_enabled = true;
	assert.equal(summarizeHeadlessStress(invalid, 2_000).valid, false);
});
