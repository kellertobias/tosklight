import type { FixtureProfile } from "../../apps/light-desktop/src/api/types";
import type { ApiDriver } from "../bench/core/api";
import { ensurePlannedDemoFixtureLibrary } from "./plannedDemoFixtureLibrary";
import {
	type DemoFixtureManifestEntry,
	PLANNED_DEMO_CONTROL_FIXTURES,
	PLANNED_DEMO_FIXTURES,
	PLANNED_DEMO_PHYSICAL_INSTANCES,
} from "./plannedDemoManifest";

interface Point {
	x: number;
	y: number;
	z: number;
}

interface PatchCursor {
	universe: number;
	address: number;
}

interface PlannedDemoPatchResult {
	fixtures: any[];
	fixtureRecords: number;
	physicalInstances: number;
	firstUniverse: number;
	lastUniverse: number;
	occupiedSlots: number;
}

export async function installPlannedDemoPatch(
	api: ApiDriver,
	showId: string,
	layers: Readonly<Record<string, string>>,
): Promise<PlannedDemoPatchResult> {
	await ensurePlannedDemoFixtureLibrary(api);
	const library = await api.fixtureProfilesSnapshot();
	const profiles = library.profiles as FixtureProfile[];
	const built = createPlannedDemoPatchInputs(profiles, layers);
	const { fixtures } = built;
	const before = await api.patch();
	const expectedNumbers = new Set(
		PLANNED_DEMO_FIXTURES.map((fixture) => fixture.number),
	);
	const existingByNumber = new Map(
		before.fixtures.flatMap((fixture: any) =>
			fixture.fixture_number == null
				? []
				: [[fixture.fixture_number, fixture] as const],
		),
	);
	const manifestByNumber = new Map(
		PLANNED_DEMO_FIXTURES.map((fixture) => [fixture.number, fixture]),
	);
	const adopted = fixtures.map((fixture) => {
		const existing = existingByNumber.get(fixture.fixture_number);
		const expected = manifestByNumber.get(fixture.fixture_number);
		return {
			...fixture,
			fixture_id:
				existing && expected && matchesManifestProfile(existing, expected)
					? existing.fixture_id
					: fixture.fixture_id,
		};
	});
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{
			request_id: crypto.randomUUID(),
			fixtures: adopted,
			remove_fixture_ids: before.fixtures.flatMap((fixture: any) => {
				const expected = manifestByNumber.get(fixture.fixture_number);
				return fixture.fixture_number != null &&
					(!expectedNumbers.has(fixture.fixture_number) ||
						(expected && !matchesManifestProfile(fixture, expected)))
					? [fixture.fixture_id]
					: [];
			}),
			placements: [],
		},
		true,
		before.revision,
		{ showId },
	);
	const after = await api.patch();
	const lightingFixtures = after.fixtures.filter((fixture: any) =>
		expectedNumbers.has(fixture.fixture_number),
	);
	const physicalInstances = lightingFixtures.reduce(
		(count: number, fixture: any) =>
			count + 1 + (fixture.multipatch?.length ?? 0),
		0,
	);
	if (lightingFixtures.length !== PLANNED_DEMO_CONTROL_FIXTURES)
		throw new Error(
			`Plan 76 patch has ${lightingFixtures.length} controls; expected 262`,
		);
	if (physicalInstances !== PLANNED_DEMO_PHYSICAL_INSTANCES)
		throw new Error(
			`Plan 76 patch has ${physicalInstances} physical instances; expected 301`,
		);
	return {
		...built,
		fixtures: lightingFixtures,
		fixtureRecords: lightingFixtures.length,
		physicalInstances,
	};
}

function matchesManifestProfile(
	fixture: any,
	expected: DemoFixtureManifestEntry,
) {
	return (
		fixture.definition.manufacturer === expected.profile.manufacturer &&
		fixture.definition.model === expected.profile.name &&
		fixture.definition.mode === expected.profile.mode
	);
}

export function createPlannedDemoPatchInputs(
	profiles: FixtureProfile[],
	layers: Readonly<Record<string, string>>,
): PlannedDemoPatchResult {
	const cursor = { universe: 1, address: 1 };
	let occupiedSlots = 0;
	const fixtures = [...PLANNED_DEMO_FIXTURES]
		.sort((left, right) => patchPriority(left) - patchPriority(right))
		.map((entry) => {
		const profile = resolveProfile(profiles, entry);
		const mode = profile.modes.find(
			(candidate) => candidate.name === entry.profile.mode,
		);
		if (!mode)
			throw new Error(
				`Missing demo mode ${entry.profile.name} / ${entry.profile.mode}`,
			);
		const placement = placementFor(entry);
		const primaryPatches = allocateSplits(mode.splits, cursor);
		occupiedSlots += footprint(mode.splits);
		const multipatch = Array.from(
			{ length: entry.multipatches },
			(_, index) => {
				const instance = placement.multipatches[index];
				if (!instance)
					throw new Error(
						`${entry.name} is missing physical placement ${index + 2}`,
					);
				const splitPatches = allocateSplits(mode.splits, cursor);
				occupiedSlots += footprint(mode.splits);
				return {
					id: stableUuid(2, entry.number * 100 + index + 1),
					name: `${entry.name} ${index + 2}`,
					split_patches: splitPatches,
					location: millimetres(instance.location),
					rotation: instance.rotation,
				};
			},
		);
		return {
			fixture_id: stableUuid(1, entry.number),
			fixture_number: entry.number,
			virtual_fixture_number: null,
			name: entry.name,
			profile_id: profile.id,
			profile_revision: profile.revision,
			mode_id: mode.id,
			split_patches: primaryPatches,
			layer_id: layerFor(entry, layers),
			direct_control: null,
			location: millimetres(placement.primary.location),
			rotation: placement.primary.rotation,
			multipatch,
			move_in_black_enabled: true,
			move_in_black_delay_millis: 0,
			highlight_overrides: [],
		};
		});
	const physicalInstances = fixtures.reduce(
		(count, fixture) => count + 1 + fixture.multipatch.length,
		0,
	);
	return {
		fixtures,
		fixtureRecords: fixtures.length,
		physicalInstances,
		firstUniverse: 1,
		lastUniverse: cursor.universe,
		occupiedSlots,
	};
}

function resolveProfile(
	profiles: FixtureProfile[],
	entry: DemoFixtureManifestEntry,
) {
	const found = profiles.find(
		(candidate) =>
			candidate.manufacturer === entry.profile.manufacturer &&
			candidate.name === entry.profile.name,
	);
	if (!found)
		throw new Error(
			`Missing demo profile ${entry.profile.manufacturer} / ${entry.profile.name}`,
		);
	return found;
}

function allocateSplits(
	splits: readonly { number: number; footprint: number }[],
	cursor: PatchCursor,
) {
	return splits.map((split) => {
		if (split.footprint === 0)
			return { split: split.number, universe: null, address: null };
		if (cursor.address + split.footprint - 1 > 512) {
			cursor.universe++;
			cursor.address = 1;
		}
		const patch = {
			split: split.number,
			universe: cursor.universe,
			address: cursor.address,
		};
		cursor.address += split.footprint;
		return patch;
	});
}

function footprint(splits: readonly { footprint: number }[]) {
	return splits.reduce((total, split) => total + split.footprint, 0);
}

function layerFor(
	entry: DemoFixtureManifestEntry,
	layers: Readonly<Record<string, string>>,
) {
	const name = entry.name;
	const location =
		entry.location === "stage"
			? "Stage"
			: entry.location === "audience"
				? "Audience"
				: "Auxilliary";
	const preferred = name.startsWith("Fresnel")
		? "Front Lights"
		: name.startsWith("Static Profile")
			? "Front Profiles"
			: entry.roles.includes("All ACLs") || entry.roles.includes("Blinders")
				? "ACLs & Blinder"
				: entry.family === "profile"
					? `Profile ${location}`
					: entry.family === "wash"
						? `Wash ${location}`
						: entry.family === "led" || entry.roles.includes("Sunstrips")
							? `LED PAR ${location}`
							: "Stage & Venue";
	return (
		layers[preferred] ??
		layers["Stage & Venue"] ??
		Object.values(layers)[0] ??
		"default"
	);
}

function patchPriority(entry: DemoFixtureManifestEntry) {
	return entry.name.startsWith("Fresnel") ? 0 : 1;
}

function placementFor(entry: DemoFixtureManifestEntry) {
	if (entry.roles.includes("All ACLs")) return aclPlacement(entry);
	if (entry.roles.includes("House Lights")) return housePlacement();
	const location = ordinaryLocation(entry);
	return {
		primary: { location, rotation: ordinaryRotation(entry, location) },
		multipatches: [] as Array<{ location: Point; rotation: Point }>,
	};
}

function ordinaryLocation(entry: DemoFixtureManifestEntry): Point {
	const index = familyIndex(entry);
	if (entry.family === "profile" && entry.location === "stage")
		return trussLine(
			index < 16 ? index : index - 16,
			index < 16 ? 16 : 12,
			index < 16 ? 4 : 0,
		);
	if (entry.family === "wash" && entry.location === "stage")
		return trussLine(
			index < 15 ? index : index - 15,
			index < 15 ? 15 : 11,
			index < 15 ? 4 : 0,
		);
	if (entry.location === "audience") {
		const columns = entry.family === "led" ? 10 : 4;
		return grid(index, columns, 0.8, -4, -5.5, 5.5);
	}
	if (entry.location === "aux") return grid(index, 4, 0.7, -1, 6, 0.3);
	if (entry.family === "led") {
		const centers = [-3, -1, 1, 3];
		const center = centers[Math.floor(index / 4)] ?? 0;
		return { x: center + ((index % 4) - 1.5) * 0.2, y: 2.4, z: 0.25 };
	}
	if (entry.roles.includes("Sunstrips"))
		return {
			x: -1.5 + Math.floor(index / 2),
			y: 4.15,
			z: 1.7 + (index % 2) * 1.15,
		};
	if (entry.roles.includes("Blinders"))
		return { x: index ? 2 : -2, y: -3, z: 4 };
	if (entry.roles.includes("Hazers"))
		return { x: index ? 3.5 : -3.5, y: 3.5, z: 0.2 };
	if (entry.name.startsWith("Fresnel")) {
		const fresnelIndex = entry.number - 1;
		return {
			x:
				fresnelIndex < 4
					? spread(fresnelIndex, 4, -4, -3)
					: spread(fresnelIndex - 4, 4, 3, 4),
			y: -3,
			z: 4,
		};
	}
	return { x: spread(index, 7, -3, 3), y: entry.number < 14 ? 0 : -3, z: 4 };
}

function ordinaryRotation(
	entry: DemoFixtureManifestEntry,
	origin: Point,
): Point {
	if (entry.roles.includes("Sunstrips")) return { x: 0, y: 90, z: 0 };
	if (entry.location === "audience")
		return aimAt(origin, { x: origin.x, y: origin.y, z: 0 });
	if (entry.location === "aux") return { x: 0, y: 0, z: 0 };
	return aimAt(origin, { x: 0, y: 1, z: 0 });
}

function aclPlacement(entry: DemoFixtureManifestEntry) {
	const number = entry.number - 600;
	const y = number === 3 ? 0 : number === 4 ? -3 : 4;
	const split = number !== 1;
	const positions = split
		? [
				...Array.from({ length: 4 }, (_, index) => -3.8 + index * 0.27),
				...Array.from({ length: 4 }, (_, index) => 3 + index * 0.27),
			]
		: Array.from({ length: 8 }, (_, index) => spread(index, 8, -0.7, 0.7));
	const targets =
		number === 1 || number === 3
			? Array.from({ length: 8 }, (_, index) => spread(index, 8, -4, 4))
			: [
					...Array.from({ length: 4 }, (_, index) => spread(index, 4, -1, 0.5)),
					...Array.from({ length: 4 }, (_, index) => spread(index, 4, -0.5, 1)),
				];
	const instances = positions.map((x, index) => {
		const location = { x, y, z: 4.4 };
		return {
			location,
			rotation: aimAt(location, { x: targets[index], y: 1, z: 0 }),
		};
	});
	return { primary: instances[0], multipatches: instances.slice(1) };
}

function housePlacement() {
	const instances = Array.from({ length: 12 }, (_, index) => {
		const location = {
			x: ((index % 3) - 1) * 2.2,
			y: -5 - Math.floor(index / 3) * 1.2,
			z: 5,
		};
		return {
			location,
			rotation: aimAt(location, { x: location.x, y: location.y, z: 0 }),
		};
	});
	return { primary: instances[0], multipatches: instances.slice(1) };
}

function familyIndex(entry: DemoFixtureManifestEntry) {
	return PLANNED_DEMO_FIXTURES.filter(
		(candidate) =>
			candidate.family === entry.family &&
			candidate.location === entry.location,
	).indexOf(entry);
}

function stageIndex(entry: DemoFixtureManifestEntry) {
	return familyIndex(entry);
}

function stageBackCount(entry: DemoFixtureManifestEntry) {
	return entry.family === "profile" ? 16 : entry.family === "wash" ? 15 : 0;
}

function trussLine(index: number, count: number, y: number): Point {
	return { x: spread(index, count, -4, 4), y, z: 4 };
}

function grid(
	index: number,
	columns: number,
	spacing: number,
	y: number,
	xOffset: number,
	z: number,
): Point {
	return {
		x: xOffset + (index % columns) * spacing,
		y: y - Math.floor(index / columns) * spacing,
		z,
	};
}

function spread(index: number, count: number, first: number, last: number) {
	return count <= 1 ? first : first + ((last - first) * index) / (count - 1);
}

function aimAt(origin: Point, target: Point): Point {
	const dx = target.x - origin.x;
	const dy = target.y - origin.y;
	const dz = target.z - origin.z;
	const distance = Math.hypot(dx, dy, dz) || 1;
	return {
		x: (Math.atan2(dy, -dz) * 180) / Math.PI,
		y: (Math.asin(Math.max(-1, Math.min(1, dx / distance))) * 180) / Math.PI,
		z: 0,
	};
}

function millimetres(point: Point) {
	return {
		x: Math.round(point.x * 1_000),
		y: Math.round(point.y * 1_000),
		z: Math.round(point.z * 1_000),
	};
}

function stableUuid(namespace: number, value: number) {
	return `00000000-0000-4${namespace.toString(16).padStart(3, "0")}-8000-${value.toString(16).padStart(12, "0")}`;
}
