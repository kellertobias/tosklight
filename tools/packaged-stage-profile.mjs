export const PACKAGED_STAGE_PROFILES = Object.freeze({
	"default-stage": Object.freeze({
		label: "Default Stage control",
		tier: "default-stage-control",
		targetHz: null,
		blocking: true,
	}),
	"canonical-demo": Object.freeze({
		label: "Canonical demo (262 controls / 301 physical instances)",
		tier: "realistic-demo",
		targetHz: null,
		blocking: true,
		expectedScene: Object.freeze({
			fixtureRecords: 262,
			fixtureInstances: 301,
		}),
	}),
	"large-stage": Object.freeze({
		label: "Interactive large (970 controls / 1,000 physical instances)",
		tier: "interactive-large",
		targetHz: 100,
		blocking: false,
	}),
	"improved-beam-spike": Object.freeze({
		label: "Improved-beam capability spike",
		tier: "improved-beam-spike",
		targetHz: null,
		blocking: true,
	}),
});

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
