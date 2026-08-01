import assert from "node:assert/strict";
import test from "node:test";
import {
	CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS,
	PACKAGED_STAGE_PROFILES,
	packagedStageControlDurationSeconds,
	packagedStageProfile,
	packagedStageSceneFailures,
} from "./packaged-stage-profile.mjs";

test("long resource runs retain a five-minute no-Stage control", () => {
	assert.equal(packagedStageControlDurationSeconds(30), 30);
	assert.equal(packagedStageControlDurationSeconds(300), 300);
	assert.equal(packagedStageControlDurationSeconds(1_800), 300);
});

test("canonical-demo identifies the shipped realistic release workload", () => {
	assert.deepEqual(PACKAGED_STAGE_PROFILES["canonical-demo"], {
		label:
			"Canonical demo (231 controls / 264 records / 306 physical instances)",
		tier: "realistic-demo",
		targetHz: null,
		blocking: true,
		expectedScene: {
			fixtureRecords: 264,
			fixtureInstances: 306,
		},
	});
	assert.deepEqual(
		packagedStageSceneFailures("canonical-demo", {
			fixtureRecords: 264,
			fixtureInstances: 306,
		}),
		[],
	);
});

test("canonical-demo rejects either wrong control or physical-instance count", () => {
	assert.deepEqual(
		packagedStageSceneFailures("canonical-demo", {
			fixtureRecords: 263,
			fixtureInstances: 305,
		}),
		[
			"canonical-demo resolved 263 fixture records; expected 264",
			"canonical-demo resolved 305 physical instances; expected 306",
		],
	);
});

test("supported-scale fixes the packaged operator contract at 1,000 instances and 60 Hz", () => {
	assert.deepEqual(PACKAGED_STAGE_PROFILES["supported-scale"], {
		label: "Supported scale (970 controls / 1,000 physical instances at 60 Hz)",
		tier: "supported-scale",
		targetHz: 60,
		blocking: true,
		expectedScene: {
			fixtureRecords: 970,
			fixtureInstances: 1_000,
		},
	});
	assert.deepEqual(
		packagedStageSceneFailures("supported-scale", {
			fixtureRecords: 970,
			fixtureInstances: 1_000,
		}),
		[],
	);
});

test("canonical-demo benchmark look has one physical and eleven virtual assignments", () => {
	assert.equal(CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.length, 12);
	assert.equal(
		CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.filter(
			(assignment) => assignment.kind === "physical",
		).length,
		1,
	);
	assert.equal(
		new Set(
			CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.map(
				(assignment) => `${assignment.kind}:${assignment.playbackNumber}`,
			),
		).size,
		CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.length,
	);
});

test("packaged profile errors enumerate canonical-demo with the other supported profiles", () => {
	assert.throws(
		() => packagedStageProfile("unknown"),
		/error.*`default-stage`, `canonical-demo`, `large-stage`, `supported-scale`, `improved-beam-spike`/i,
	);
});
