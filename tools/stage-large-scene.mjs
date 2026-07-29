export const LARGE_STAGE_FIXTURE_RECORDS = 970;
export const LARGE_STAGE_FIXTURE_INSTANCES = 1_000;
export const LARGE_STAGE_FIRST_UNIVERSE = 101;
export const LARGE_STAGE_DYNAMIC_INSTANCES = 20;

export const LARGE_STAGE_MANIFEST = [
	{
		key: "dls",
		category: "moving",
		manufacturer: "ROBE",
		name: "Robin DLS Profile",
		mode: "Mode 1",
		records: 140,
		multipatches: 0,
		dynamic: true,
	},
	{
		key: "wash",
		category: "moving",
		manufacturer: "ROBE",
		name: "Robin 600X LEDWash",
		mode: "Mode 1",
		records: 180,
		multipatches: 0,
		dynamic: true,
	},
	{
		key: "sunstrip",
		category: "sunstrip",
		manufacturer: "Showtec",
		name: "Sunstrip LED RGB 42206",
		mode: "30 Channel",
		records: 40,
		multipatches: 0,
		dynamic: true,
	},
	{
		key: "beam",
		category: "moving",
		manufacturer: "ROBE",
		name: "Robin LEDBeam 150",
		mode: "Mode 1 – Standard 16-bit",
		records: 180,
		multipatches: 0,
		dynamic: true,
	},
	{
		key: "dimmer",
		category: "static",
		manufacturer: "Generic",
		name: "Dimmer",
		mode: "8-bit",
		records: 410,
		multipatches: 30,
		dynamic: false,
	},
	{
		key: "stage",
		category: "venue",
		manufacturer: "Venue",
		name: "Stage Element 2 × 1 m",
		mode: "40 cm",
		records: 8,
		multipatches: 0,
		dynamic: false,
	},
	{
		key: "stairs",
		category: "venue",
		manufacturer: "Venue",
		name: "Stage Stairs",
		mode: "40 cm",
		records: 4,
		multipatches: 0,
		dynamic: false,
	},
	{
		key: "truss",
		category: "venue",
		manufacturer: "Venue",
		name: "Four-Point Truss",
		mode: "4 m",
		records: 4,
		multipatches: 0,
		dynamic: false,
	},
	{
		key: "curtain",
		category: "venue",
		manufacturer: "Venue",
		name: "Curtain 5 m",
		mode: "5 m",
		records: 4,
		multipatches: 0,
		dynamic: false,
	},
];

export function createDeterministicLargeStageInputs(
	_seedFixtures,
	profiles,
	layerId = "default",
) {
	if (!Array.isArray(profiles) || profiles.length === 0)
		throw new Error("Large Stage requires the fixture profile catalog");
	const resolved = LARGE_STAGE_MANIFEST.map((entry) => ({
		...entry,
		profile: resolveProfile(profiles, entry),
	}));
	const fixtures = [];
	const dynamicFixtureIds = [];
	const staticControlFixtureIds = [];
	const categoryCounts = {
		sunstrip: 0,
		moving: 0,
		static: 0,
		venue: 0,
	};
	const inventory = [];
	let fixtureIndex = 0;
	for (const entry of resolved) {
		const mode = entry.profile.modes.find(
			(candidate) => candidate.name === entry.mode,
		);
		const footprint = mode.splits.reduce(
			(total, split) => total + split.footprint,
			0,
		);
		inventory.push({
			key: entry.key,
			manufacturer: entry.manufacturer,
			name: entry.name,
			mode: entry.mode,
			records: entry.records,
			instances: entry.records + entry.multipatches,
			footprint,
			patchedSlots: footprint * (entry.records + entry.multipatches),
			dynamic: entry.dynamic,
		});
		for (let index = 0; index < entry.records; index++) {
			fixtureIndex++;
			const fixture = fixtureInput({
				entry,
				mode,
				index,
				fixtureIndex,
				layerId,
				withMultipatch: index < entry.multipatches,
			});
			fixtures.push(fixture);
			categoryCounts[entry.category] += 1 + fixture.multipatch.length;
			if (entry.dynamic) dynamicFixtureIds.push(fixture.fixture_id);
			else if (entry.category === "static")
				staticControlFixtureIds.push(fixture.fixture_id);
		}
	}
	const patch = packPatchInstances(fixtures, resolved);
	const fixtureInstances = countFixtureInstances(fixtures);
	if (
		fixtures.length !== LARGE_STAGE_FIXTURE_RECORDS ||
		fixtureInstances !== LARGE_STAGE_FIXTURE_INSTANCES
	)
		throw new Error(
			`Large Stage manifest resolved to ${fixtures.length} records and ${fixtureInstances} instances`,
		);
	return {
		fixtures,
		addedFixtureRecords: fixtures.length,
		addedMultipatchInstances: fixtureInstances - fixtures.length,
		categoryCounts,
		dynamicFixtureIds,
		staticControlFixtureIds,
		inventory,
		patch,
	};
}

export function countFixtureInstances(fixtures) {
	return fixtures.reduce(
		(total, fixture) => total + 1 + fixture.multipatch.length,
		0,
	);
}

function resolveProfile(profiles, entry) {
	const profile = profiles.find(
		(candidate) =>
			candidate.manufacturer === entry.manufacturer &&
			candidate.name === entry.name,
	);
	if (!profile)
		throw new Error(
			`Large Stage fixture profile is missing: ${entry.manufacturer} ${entry.name}`,
		);
	if (!profile.modes.some((mode) => mode.name === entry.mode))
		throw new Error(
			`Large Stage fixture mode is missing: ${entry.manufacturer} ${entry.name} / ${entry.mode}`,
		);
	return profile;
}

function fixtureInput({
	entry,
	mode,
	index,
	fixtureIndex,
	layerId,
	withMultipatch,
}) {
	const visual = mode.splits.every((split) => split.footprint === 0);
	const column = index % 30;
	const row = Math.floor(index / 30);
	const location =
		entry.category === "venue"
			? venueLocation(entry.key, index)
			: {
					x: (column * 2 - 29) * 300,
					y: entry.category === "static" ? -2_000 : 2_000 + (row % 4) * 600,
					z: -Math.floor(row / 4) * 800,
				};
	return {
		fixture_id: deterministicUuid("1", fixtureIndex),
		fixture_number: visual ? null : 100 + fixtureIndex,
		virtual_fixture_number: visual ? 100 + fixtureIndex : null,
		name: `Stage capacity ${entry.name} ${String(index + 1).padStart(3, "0")}`,
		profile_id: entry.profile.id,
		profile_revision: entry.profile.revision,
		mode_id: mode.id,
		split_patches: mode.splits.map((split) => ({
			split: split.number,
			universe: null,
			address: null,
		})),
		layer_id: layerId,
		direct_control: null,
		location,
		rotation: { x: 0, y: 0, z: 0 },
		multipatch: withMultipatch
			? [
					{
						id: deterministicUuid("2", fixtureIndex),
						name: `Stage capacity ${entry.name} ${String(index + 1).padStart(3, "0")} multipatch`,
						split_patches: mode.splits.map((split) => ({
							split: split.number,
							universe: null,
							address: null,
						})),
						location: { ...location, x: location.x + 150 },
						rotation: { x: 0, y: 0, z: 0 },
					},
				]
			: [],
		move_in_black_enabled: !visual,
		move_in_black_delay_millis: 0,
		highlight_overrides: [],
	};
}

function packPatchInstances(fixtures, resolved) {
	const modes = new Map(
		resolved.map((entry) => {
			const mode = entry.profile.modes.find(
				(candidate) => candidate.name === entry.mode,
			);
			return [`${entry.profile.id}:${entry.profile.revision}:${mode.id}`, mode];
		}),
	);
	const items = [];
	for (const fixture of fixtures) {
		const mode = modes.get(
			`${fixture.profile_id}:${fixture.profile_revision}:${fixture.mode_id}`,
		);
		const footprint = mode.splits.reduce(
			(total, split) => total + split.footprint,
			0,
		);
		if (footprint === 0) continue;
		items.push({ fixture, instance: fixture, mode, footprint });
		for (const multipatch of fixture.multipatch)
			items.push({ fixture, instance: multipatch, mode, footprint });
	}
	items.sort(
		(left, right) =>
			right.footprint - left.footprint ||
			left.fixture.fixture_id.localeCompare(right.fixture.fixture_id) ||
			left.instance.name.localeCompare(right.instance.name),
	);
	const bins = [];
	for (const item of items) {
		let bin = bins.find((candidate) => candidate.used + item.footprint <= 512);
		if (!bin) {
			bin = {
				universe: LARGE_STAGE_FIRST_UNIVERSE + bins.length,
				used: 0,
			};
			bins.push(bin);
		}
		let address = bin.used + 1;
		item.instance.split_patches = item.mode.splits.map((split) => {
			const assignment = {
				split: split.number,
				universe: bin.universe,
				address,
			};
			address += split.footprint;
			return assignment;
		});
		bin.used += item.footprint;
	}
	return {
		firstUniverse: bins[0]?.universe ?? null,
		lastUniverse: bins.at(-1)?.universe ?? null,
		universeCount: bins.length,
		occupiedSlots: bins.reduce((total, bin) => total + bin.used, 0),
		occupiedByUniverse: Object.fromEntries(
			bins.map((bin) => [bin.universe, bin.used]),
		),
	};
}

function venueLocation(key, index) {
	const x = (index % 4) * 2_000 - 3_000;
	if (key === "stage") return { x, y: Math.floor(index / 4) * 1_000, z: 0 };
	if (key === "stairs") return { x, y: -1_500, z: 0 };
	if (key === "truss") return { x, y: 3_000, z: 4_500 };
	return { x, y: 4_000, z: 2_500 };
}

function deterministicUuid(namespace, value) {
	return `${namespace}0000000-0000-4000-8000-${value
		.toString(16)
		.padStart(12, "0")}`;
}
