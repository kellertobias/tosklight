import assert from "node:assert/strict";
import test from "node:test";
import { buildProductDemoEditFilters } from "./product-demo-edit-graph.mjs";

test("normalizes mixed concat and crossfade outputs to the canonical timebase", () => {
	const clips = [
		clip("a", 0, 4_000, 100, 0),
		clip("b", 4_000, 8_000, 100, 15),
		clip("c", 8_000, 12_000, 100, 0),
		clip("d", 12_000, 16_000, 100, 0),
	];
	const filters = buildProductDemoEditFilters(
		{ fps: 25, transitionFrames: 15, totalFrames: 370 },
		clips,
		{ delogoEnable: "0" },
	);
	assert.equal(
		filters.filter((filter) => filter.includes("settb=expr=1/25")).length,
		7,
	);
	assert.ok(filters.some((filter) => filter.includes("concat=n=2:v=1:a=0[m1]")));
	assert.ok(filters.some((filter) => filter.includes("[m1]fps=25,settb=expr=1/25[x1]")));
	assert.ok(filters.some((filter) => filter.includes("xfade=transition=fade") && filter.endsWith("[m2]")));
	assert.ok(filters.some((filter) => filter.includes("[m2]fps=25,settb=expr=1/25[x2]")));
});

function clip(id, sourceStartMillis, sourceEndMillis, frames, transitionFramesAfter) {
	return {
		id,
		sourceStartMillis,
		sourceEndMillis,
		frames,
		transitionFramesAfter,
	};
}
