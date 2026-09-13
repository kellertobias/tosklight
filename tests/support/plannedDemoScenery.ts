import type { FixtureProfile } from "../../apps/light-desktop/src/api/types";
import type { ApiDriver } from "../bench/core/api";
import { ensurePlannedDemoFixtureLibrary } from "./plannedDemoFixtureLibrary";
import { putPlannedDemoObject } from "./plannedDemoObjects";

export const PLANNED_DEMO_SCENERY_FIXTURES = 58;
export const PLANNED_DEMO_TOTAL_FIXTURE_RECORDS = 311;
export const PLANNED_DEMO_TOTAL_PHYSICAL_INSTANCES = 344;

type Point = { x: number; y: number; z: number };
type SceneryEntry = {
	number: number;
	name: string;
	profile: string;
	mode: string;
	layer: string;
	location: Point;
	rotation?: Point;
};

export async function installPlannedDemoScenery(
	api: ApiDriver,
	showId: string,
	layers: Readonly<Record<string, string>>,
	options: {
		progressive?: boolean;
		onItem?: () => Promise<void>;
		backCurtain?: { x: string; y: number; z: number };
	} = {},
) {
	await ensurePlannedDemoFixtureLibrary(api);
	const profiles = (await api.fixtureProfilesSnapshot())
		.profiles as FixtureProfile[];
	const before = await api.patch();
	const byVirtualNumber = new Map(
		before.fixtures.flatMap((fixture: any) =>
			fixture.virtual_fixture_number == null
				? []
				: [[fixture.virtual_fixture_number, fixture] as const],
		),
	);
	const fixtures = sceneryEntries(options.backCurtain).map((entry) => {
		const profile = profiles.find(
			(candidate) =>
				candidate.manufacturer === "Venue" && candidate.name === entry.profile,
		);
		const generated = Boolean((profile as any)?.scenery);
		const mode = generated
			? profile?.modes[0]
			: profile?.modes.find((candidate) => candidate.name === entry.mode);
		if (!profile || !mode)
			throw new Error(`Missing Venue profile ${entry.profile} / ${entry.mode}`);
		const scenerySize = generated
			? placedScenerySize((profile as any).scenery, entry.mode)
			: undefined;
		const existing = byVirtualNumber.get(entry.number);
		if (
			existing &&
			(existing.definition.manufacturer !== "Venue" ||
				existing.definition.model !== entry.profile ||
				existing.definition.mode !== mode.name)
		)
			throw new Error(
				`Venue fixture 0.${entry.number} is not the expected ${entry.profile} / ${entry.mode}`,
			);
		return {
			fixture_id: existing?.fixture_id ?? stableUuid(3, entry.number),
			fixture_number: null,
			virtual_fixture_number: entry.number,
			name: entry.name,
			profile_id: profile.id,
			profile_revision: profile.revision,
			mode_id: mode.id,
			split_patches: mode.splits.map((split) => ({
				split: split.number,
				universe: null,
				address: null,
			})),
			layer_id:
				layers[entry.layer] ??
				layers["Stage & Venue"] ??
				Object.values(layers)[0] ??
				"default",
			direct_control: null,
			location: millimetres(entry.location),
			scenery_size_metres: scenerySize,
			rotation: entry.rotation ?? { x: 0, y: 0, z: 0 },
			multipatch: [],
			move_in_black_enabled: true,
			move_in_black_delay_millis: 0,
			highlight_overrides: [],
		};
	});
	const batches = options.progressive
		? fixtures
				.filter(
					(fixture) => !byVirtualNumber.has(fixture.virtual_fixture_number),
				)
				.map((fixture) => [fixture])
		: [fixtures];
	for (const batch of batches) {
		const current = await api.patch();
		await api.request(
			"POST",
			"/api/v2/patch/fixtures",
			{
				request_id: crypto.randomUUID(),
				fixtures: batch,
				remove_fixture_ids: [],
				placements: [],
			},
			true,
			current.revision,
			{ showId },
		);
		await options.onItem?.();
	}
	await clearLegacyVenueObjects(api, showId);
	return fixtures;
}

function sceneryEntries(backCurtain?: {
	x: string;
	y: number;
	z: number;
}): SceneryEntry[] {
	// A truss run is one Venue object per two-metre span. The first span of each run keeps the
	// number it always had; the others follow the rest of the scenery, from 0.44.
	const trussRun = (
		name: string,
		y: number,
		z: number,
		first: number,
		rest: number,
	) =>
		[-3, -1, 1, 3].map((x, index) => ({
			number: index === 0 ? first : rest + index - 1,
			name: `${name} Truss Segment ${index + 1}`,
			profile: "Four-Point Truss",
			mode: "2 m",
			layer: "Trusses",
			location: { x, y, z },
		}));
	const trusses = [
		["Back", 4],
		["Mid", 0],
		["Front", -3],
	].flatMap(([name, y], row) =>
		trussRun(String(name), Number(y), 4.15, row + 1, 44 + row * 3),
	);
	// Five two-metre decks across gives the 28 stage profiles and 26 washes a credible
	// ten-metre stage. The old eight-metre deck put the outer fixtures exactly on its edge,
	// making correctly sized people read as giants beside a toy stage.
	const stage = Array.from({ length: 20 }, (_, index) => ({
		number: index + 4,
		name: `Stage Element ${index + 1}`,
		profile: "Stage Element 2 × 1 m",
		mode: "50 cm",
		layer: "Stage & Venue",
		location: {
			x: -4 + (index % 5) * 2,
			y: 0.5 + Math.floor(index / 5),
			z: 0,
		},
	}));
	const curtainRange = (backCurtain?.x ?? "-2.5 THRU 2.5")
		.split(/\s+THRU\s+/u)
		.map(Number);
	const curtains = [curtainRange[0], curtainRange[1] ?? curtainRange[0]].map(
		(x, index) => ({
				number: index + 24,
			name: `Back Curtain ${index + 1}`,
			profile: "Curtain 5 m",
			mode: "5 m",
			layer: "Stage & Venue",
			location: { x, y: backCurtain?.y ?? 4.35, z: backCurtain?.z ?? 2.0 },
		}),
	);
	const railings = [
		...[-3, -1, 1, 3].map((x, index) => ({
			number: index + 26,
			name: `Back Railing ${index + 1}`,
			profile: "Stage Railing 2 m",
			mode: "2 m",
			layer: "Stage & Venue",
			location: { x, y: 4.05, z: 1.35 },
		})),
		...[-1, 1].flatMap((side, sideIndex) =>
			[1, 3].map((y, index) => ({
				number: 30 + sideIndex * 2 + index,
				name: `${side < 0 ? "Left" : "Right"} Railing ${index + 1}`,
				profile: "Stage Railing 2 m",
				mode: "2 m",
				layer: "Stage & Venue",
				location: { x: side * 4.05, y, z: 1.35 },
				rotation: { x: 0, y: 0, z: 90 },
			})),
		),
	];
	const pipes = [-3, -1, 1, 3].map((x, index) => ({
		number: index + 34,
		name: `Vertical Pipe ${index + 1}`,
		profile: "One-Point Truss / Pipe",
		mode: "2.5 m",
		layer: "Trusses",
		location: { x, y: 4.15, z: 2.9 },
		rotation: { x: 0, y: 90, z: 0 },
	}));
	const crowd = {
		number: 38,
		name: "Dancefloor Crowd",
		profile: "Crowd Area",
		mode: "Dancing — Dense",
		layer: "Stage & Venue",
		location: { x: 0, y: -3, z: 0 },
	};
	const discoBall = {
		number: 39,
		name: "Audience Mirror Ball",
		profile: "Disco Ball 50 cm",
		mode: "50 cm",
		layer: "Stage & Venue",
		location: { x: 0, y: -3, z: 4.5 },
	};
	const audienceTrusses = [
		["Audience Front", -1.5],
		["Audience Rear", -4.5],
	].flatMap(([name, y], row) =>
		trussRun(String(name), Number(y), 4.2, 40 + row, 53 + row * 3),
	);
	const sideCurtains = [
		["Stage Left Curtain", -5.2],
		["Stage Right Curtain", 5.2],
	].map(([name, x], index) => ({
		number: 42 + index,
		name: String(name),
		profile: "Curtain 5 m",
		mode: "5 m",
		layer: "Stage & Venue",
		location: { x: Number(x), y: 2.0, z: 2.0 },
		rotation: { x: 0, y: 0, z: 90 },
	}));
	return [
		...trusses,
		...audienceTrusses,
		...stage,
		...curtains,
		...sideCurtains,
		...railings,
		...pipes,
		crowd,
		discoBall,
	];
}

/**
 * Clears the legacy standalone venue records.
 *
 * The demo's scenery is patched as Venue fixtures — a truss, a curtain and a deck are fixtures
 * with a visual-only patch policy, carrying their own geometry and their own place in the rig.
 * Writing a second, parallel `venue` record for each of them described the same object twice, and
 * the two could only drift. The object kind stays readable for shows that already have them.
 */
async function clearLegacyVenueObjects(api: ApiDriver, showId: string) {
	for (const object of await api.showObjects<any>(showId, "venue"))
		await api.deleteSeededShowObject(
			showId,
			"venue",
			object.id,
			object.revision,
		);
}

function millimetres(point: Point) {
	return {
		x: Math.round(point.x * 1_000),
		y: Math.round(point.y * 1_000),
		z: Math.round(point.z * 1_000),
	};
}

function stableUuid(namespace: number, value: number) {
	return `00000000-0000-4000-${namespace.toString(16).padStart(4, "0")}-${value
		.toString(16)
		.padStart(12, "0")}`;
}

/**
 * The size an entry asks for, in millimetres, held inside what the object can be built at.
 *
 * A scenery entry names its measurement where it used to name a mode — "2 m" of truss, a "50 cm"
 * deck — and that measurement belongs to whichever axis the profile says is adjustable.
 */
function placedScenerySize(scenery: any, measurement: string) {
	const metres = measurement.trim().endsWith("cm")
		? Number.parseFloat(measurement) / 100
		: Number.parseFloat(measurement);
	const size = { ...scenery.default_size_metres };
	if (Number.isFinite(metres)) {
		// Height where a profile is made to height, otherwise length.
		const axis = scenery.adjustable.height && !scenery.adjustable.width ? "y" : "x";
		size[axis] = metres;
	}
	return {
		x: Math.round(size.x * 1_000),
		y: Math.round(size.y * 1_000),
		z: Math.round(size.z * 1_000),
	};
}
