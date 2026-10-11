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
			"Canonical demo (312 records / 345 physical instances)",
		tier: "realistic-demo",
		targetHz: null,
		blocking: true,
		expectedScene: Object.freeze({
			fixtureRecords: 312,
			fixtureInstances: 345,
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
	{ name: "ACL Chase", kind: "physical", surface_id: "main", page: 1, position: 107 },
	{ name: "Wash Show Waterfall", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 424 },
	{ name: "Beam Show Circle", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 419 },
	{ name: "Beam Show PWM", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 401 },
	{ name: "LED Show Random", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 414 },
	{ name: "LED Show Random Strobe", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 430 },
	{ name: "Sunstrip Rain", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 429 },
	{ name: "Beam Auxiliary Show Circle", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 421 },
	{ name: "Beam Auxiliary Show PWM", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 404 },
	{ name: "Wash Auxiliary Show Waterfall", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 426 },
	{ name: "Wash Auxiliary Show Random", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 411 },
	{ name: "LED Auxiliary Show Sinus", kind: "virtual", surface_id: "pane:busking-playbacks", page: 1, position: 418 },
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
