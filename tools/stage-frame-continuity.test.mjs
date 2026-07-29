import assert from "node:assert/strict";
import test from "node:test";
import {
	changingPresentationGaps,
	laneSourceCadenceGaps,
	latestChangingFrameDidNotSettle,
} from "./stage-frame-continuity.mjs";

test("a superseded interpolation still contributes its first visible canvas submission", () => {
	const frames = [
		frame(867, 0, 88, 88),
		frame(868, 100, 86, 86),
		frame(869, 200, 17, null),
		frame(870, 300, 42, 111),
	];

	assert.deepEqual(changingPresentationGaps(frames), [98, 31, 125]);
});

test("an old deactivated scope does not fail the final active-lane settlement gate", () => {
	const frames = [
		{
			...frame(1, 0, null, null),
			receivedAt: 1_000,
			scopeActivation: 1,
		},
		{
			...frame(2, 1_000, null, null),
			receivedAt: 2_000,
			scopeActivation: 2,
		},
	];

	assert.equal(
		latestChangingFrameDidNotSettle(frames, new Date(2_150).toISOString()),
		false,
	);
	assert.equal(
		latestChangingFrameDidNotSettle(frames, new Date(2_250).toISOString()),
		true,
	);
});

test("source cadence restarts after an intentional application suspension", () => {
	const frames = [
		frame(1, 0, 5, 5),
		frame(2, 100, 5, 5),
		frame(3, 1_200, 5, 5),
	];
	const applicationSuspend = {
		finishedAt: new Date(2_000).toISOString(),
	};

	assert.deepEqual(laneSourceCadenceGaps(frames, applicationSuspend), [100]);
});

function frame(sourceFrame, sourceOffset, firstLatency, settledLatency) {
	const sourceAt = 1_000 + sourceOffset;
	return {
		lane: "preload",
		showId: "11111111-1111-4111-8111-111111111111",
		scopeActivation: 1,
		visibilitySegment: 0,
		sourceFrame,
		sourceGeneratedAt: new Date(sourceAt).toISOString(),
		receivedAt: sourceAt + 2,
		firstCanvasSubmittedAt:
			firstLatency === null ? null : sourceAt + firstLatency,
		settledCanvasSubmittedAt:
			settledLatency === null ? null : sourceAt + settledLatency,
		visibleChanged: true,
	};
}
