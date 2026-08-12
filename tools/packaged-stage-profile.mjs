export const PACKAGED_STAGE_PROFILES = Object.freeze({
	"default-stage": Object.freeze({
		label: "Default Stage control",
		tier: "default-stage-control",
		targetHz: null,
		blocking: true,
	}),
	"stage-500": Object.freeze({
		label: "500-instance mixed Stage",
		tier: "stage-500",
		targetHz: 10,
		blocking: true,
		expectedScene: Object.freeze({
			fixtureRecords: 500,
			fixtureInstances: 500,
		}),
	}),
	"canonical-demo": Object.freeze({
		label:
			"Canonical demo (231 controls / 264 records / 306 physical instances)",
		tier: "realistic-demo",
		targetHz: null,
		blocking: true,
		expectedScene: Object.freeze({
			fixtureRecords: 264,
			fixtureInstances: 306,
		}),
	}),
	"large-stage": Object.freeze({
		label: "Interactive large (970 controls / 1,000 physical instances)",
		tier: "interactive-large",
		targetHz: 100,
		blocking: false,
	}),
	"supported-scale": Object.freeze({
		label: "Supported scale (970 controls / 1,000 physical instances at 60 Hz)",
		tier: "supported-scale",
		targetHz: 60,
		blocking: true,
		expectedScene: Object.freeze({
			fixtureRecords: 970,
			fixtureInstances: 1_000,
		}),
	}),
	"improved-beam-spike": Object.freeze({
		label: "Improved-beam capability spike",
		tier: "improved-beam-spike",
		targetHz: null,
		blocking: true,
	}),
});

export const CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS = Object.freeze([
	{ name: "ACL Chase", kind: "physical", playbackNumber: 17 },
	{ name: "Show Wash Waterfall", kind: "virtual", playbackNumber: 1024 },
	{ name: "Show Profile Circle", kind: "virtual", playbackNumber: 1019 },
	{ name: "Show Profile PWM", kind: "virtual", playbackNumber: 1001 },
	{ name: "Show LED Random", kind: "virtual", playbackNumber: 1014 },
	{ name: "Show LED Random Strobe", kind: "virtual", playbackNumber: 1030 },
	{ name: "Sunstrip Rain", kind: "virtual", playbackNumber: 1029 },
	{ name: "Aux Show Profile Circle", kind: "virtual", playbackNumber: 1021 },
	{ name: "Aux Show Profile PWM", kind: "virtual", playbackNumber: 1004 },
	{ name: "Aux Show Wash Waterfall", kind: "virtual", playbackNumber: 1026 },
	{ name: "Aux Show Wash Random", kind: "virtual", playbackNumber: 1011 },
	{ name: "Aux Show LED Sinus", kind: "virtual", playbackNumber: 1018 },
]);

export function packagedStageProfile(profile) {
	const definition = PACKAGED_STAGE_PROFILES[profile];
	if (definition) return definition;
	throw new Error(
		`packaged Stage profile must be ${Object.keys(PACKAGED_STAGE_PROFILES)
			.map((candidate) => `\`${candidate}\``)
			.join(", ")}`,
	);
}

export function packagedStageSceneFailures(profile, scene) {
	const expected = packagedStageProfile(profile).expectedScene;
	if (!expected) return [];
	const failures = [];
	if (scene.fixtureRecords !== expected.fixtureRecords)
		failures.push(
			`${profile} resolved ${scene.fixtureRecords} fixture records; expected ${expected.fixtureRecords}`,
		);
	if (scene.fixtureInstances !== expected.fixtureInstances)
		failures.push(
			`${profile} resolved ${scene.fixtureInstances} physical instances; expected ${expected.fixtureInstances}`,
		);
	return failures;
}

export function packagedStageControlDurationSeconds(durationSeconds) {
	return Math.min(durationSeconds, 300);
}
