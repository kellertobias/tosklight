import assert from "node:assert/strict";
import test from "node:test";
import {
	latestProgrammerActionId,
	summarizeProgrammerActionTiming,
} from "./programmer-action-timing.mjs";

const settled = {
	action_id: 8,
	source: "websocket",
	action: "values",
	requires_output_frame: true,
	succeeded: true,
	acknowledgement_within_budget: true,
	output_within_budget: true,
};

test("evaluates only action records newer than the baseline", () => {
	const summary = summarizeProgrammerActionTiming(
		[{ ...settled, action_id: 7, output_within_budget: false }, settled],
		7,
	);
	assert.equal(summary.samples, 1);
	assert.equal(summary.passed, true);
	assert.equal(latestProgrammerActionId(summary.measurements), 8);
});

test("distinguishes an output frame that is still pending", () => {
	const summary = summarizeProgrammerActionTiming([
		{ ...settled, output_within_budget: null },
	]);
	assert.equal(summary.pendingOutput, 1);
	assert.equal(summary.failed, 1);
	assert.match(summary.failures[0], /first output frame is pending/);
});

test("reports failed server tick gates for ack and output", () => {
	const summary = summarizeProgrammerActionTiming([
		{
			...settled,
			acknowledgement_within_budget: false,
			output_within_budget: false,
		},
	]);
	assert.equal(summary.passed, false);
	assert.match(summary.failures[0], /ack exceeded tick budget/);
	assert.match(summary.failures[0], /first output frame exceeded tick budget/);
});

test("does not require an output frame for non-output actions", () => {
	const summary = summarizeProgrammerActionTiming([
		{
			...settled,
			action: "command_line_edit",
			requires_output_frame: false,
			output_within_budget: null,
		},
	]);
	assert.equal(summary.passed, true);
	assert.equal(summary.pendingOutput, 0);
});

test("cannot pass without a post-baseline action sample", () => {
	const summary = summarizeProgrammerActionTiming([settled], 8);
	assert.equal(summary.passed, false);
	assert.match(summary.failures[0], /required at least 1/);
});

test("requires the declared source, action, and frame-rate matrix", () => {
	const summary = summarizeProgrammerActionTiming(
		[
			{
				...settled,
				output_frame_hz: 44,
				budget_ticks: 2,
			},
		],
		0,
		{
			sources: ["http", "websocket"],
			actions: ["values", "undo"],
			frameRateBands: ["at-or-below-60", "above-60"],
		},
	);
	assert.equal(summary.passed, false);
	assert.deepEqual(summary.failures, [
		"missing required action source http",
		"missing required action undo",
		"missing required frame-rate band above-60",
	]);
});
