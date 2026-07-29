import assert from "node:assert/strict";
import test from "node:test";
import {
	PACKAGED_STAGE_PROFILES,
	packagedStageProfile,
	packagedStageSceneFailures,
} from "./packaged-stage-profile.mjs";

test("canonical-demo identifies the shipped realistic release workload", () => {
	assert.deepEqual(PACKAGED_STAGE_PROFILES["canonical-demo"], {
		label: "Canonical demo (262 controls / 301 physical instances)",
		tier: "realistic-demo",
		targetHz: null,
		blocking: true,
		expectedScene: {
			fixtureRecords: 262,
			fixtureInstances: 301,
		},
	});
	assert.deepEqual(
		packagedStageSceneFailures("canonical-demo", {
			fixtureRecords: 262,
			fixtureInstances: 301,
		}),
		[],
	);
});

test("canonical-demo rejects either wrong control or physical-instance count", () => {
	assert.deepEqual(
		packagedStageSceneFailures("canonical-demo", {
			fixtureRecords: 261,
			fixtureInstances: 300,
		}),
		[
			"canonical-demo resolved 261 fixture records; expected 262",
			"canonical-demo resolved 300 physical instances; expected 301",
		],
	);
});

test("packaged profile errors enumerate canonical-demo with the other supported profiles", () => {
	assert.throws(
		() => packagedStageProfile("unknown"),
		/error.*`default-stage`, `canonical-demo`, `large-stage`, `improved-beam-spike`/i,
	);
});
