import assert from "node:assert/strict";
import test from "node:test";
import { summarizeSustainedOutput } from "./run-sustained-output-benchmark.mjs";

function report(frameRate = {}, deadline = {}) {
	return {
		required_floor_met: true,
		scenarios: [
			{
				profile: "hard_floor",
				met_configured_rate: true,
				fixture_count: 4_148,
				fixture_inventory: {
					manufacturer_fixture_slots: 4_288,
					rgb_par_fill_slots: 12_096,
					total_slots: 16_384,
				},
				frame_rate: {
					average_completed_hz: 100,
					minimum_one_second_completed_hz: 100,
					required_minimum_hz: 100,
					one_second_windows: 120,
					windows_below_minimum: 0,
					...frameRate,
				},
				deadline: {
					dropped_ticks: 0,
					deferred_ticks: 0,
					deadline_misses: 0,
					...deadline,
				},
			},
		],
	};
}

test("summarizes the explicit sustained frame-rate evidence", () => {
	assert.deepEqual(summarizeSustainedOutput(report()), {
		passed: true,
		averageHz: 100,
		minimumHz: 100,
		requiredHz: 100,
		windows: 120,
		windowsBelowMinimum: 0,
		dropped: 0,
		deferred: 0,
		deadlineMisses: 0,
		fixtureCount: 4_148,
		manufacturerFixtureSlots: 4_288,
		rgbParFillSlots: 12_096,
		totalSlots: 16_384,
	});
});

test("does not present a failed hard-floor gate as passing", () => {
	const failed = report({
		minimum_one_second_completed_hz: 99,
		windows_below_minimum: 1,
	});
	failed.required_floor_met = false;
	failed.scenarios[0].met_configured_rate = false;
	assert.equal(summarizeSustainedOutput(failed).passed, false);
});
