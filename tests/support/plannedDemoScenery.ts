import type { FixtureProfile } from "../../apps/light-desktop/src/api/types";
import type { ApiDriver } from "../bench/core/api";
import { ensurePlannedDemoFixtureLibrary } from "./plannedDemoFixtureLibrary";
import { putPlannedDemoObject } from "./plannedDemoObjects";

export const PLANNED_DEMO_SCENERY_FIXTURES = 43;
export const PLANNED_DEMO_TOTAL_FIXTURE_RECORDS = 296;
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
	multipatches?: Array<{ name: string; location: Point; rotation?: Point }>;
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
		const mode = profile?.modes.find(
			(candidate) => candidate.name === entry.mode,
		);
		if (!profile || !mode)
			throw new Error(`Missing Venue profile ${entry.profile} / ${entry.mode}`);
		const existing = byVirtualNumber.get(entry.number);
		if (
			existing &&
			(existing.definition.manufacturer !== "Venue" ||
				existing.definition.model !== entry.profile ||
				existing.definition.mode !== entry.mode)
		)
			throw new Error(
				`Venue fixture 0.${entry.number} is not the expected ${entry.profile} / ${entry.mode}`,
			);
		const multipatch = (entry.multipatches ?? []).map((instance, index) => ({
			id:
				existing?.multipatch?.[index]?.id ??
				stableUuid(4, entry.number * 100 + index + 1),
			name: instance.name,
			split_patches: mode.splits.map((split) => ({
				split: split.number,
				universe: null,
				address: null,
			})),
			location: millimetres(instance.location),
			rotation: instance.rotation ?? { x: 0, y: 0, z: 0 },
		}));
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
			rotation: entry.rotation ?? { x: 0, y: 0, z: 0 },
			multipatch,
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
	await installVenueObjects(api, showId, sceneryEntries(options.backCurtain));
	return fixtures;
}

function sceneryEntries(backCurtain?: {
	x: string;
	y: number;
	z: number;
}): SceneryEntry[] {
	const trusses = [
		["Back", 4],
		["Mid", 0],
		["Front", -3],
	].map(([name, y], row) => ({
		number: row + 1,
		name: `${name} Truss Segment 1`,
		profile: "Four-Point Truss",
		mode: "2 m",
		layer: "Trusses",
		location: { x: -3, y: Number(y), z: 4.15 },
		multipatches: [-1, 1, 3].map((x, index) => ({
			name: `${name} Truss Segment ${index + 2}`,
			location: { x, y: Number(y), z: 4.15 },
		})),
	}));
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
			profile: "One-Point Truss / Pipe",
			mode: "2 m",
			layer: "Stage & Venue",
			location: { x, y: 4.05, z: 1.35 },
		})),
		...[-1, 1].flatMap((side, sideIndex) =>
			[1, 3].map((y, index) => ({
				number: 30 + sideIndex * 2 + index,
				name: `${side < 0 ? "Left" : "Right"} Railing ${index + 1}`,
				profile: "One-Point Truss / Pipe",
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
	].map(([name, y], row) => ({
		number: 40 + row,
		name: `${name} Truss Segment 1`,
		profile: "Four-Point Truss",
		mode: "2 m",
		layer: "Trusses",
		location: { x: -3, y: Number(y), z: 4.2 },
		multipatches: [-1, 1, 3].map((x, index) => ({
			name: `${name} Truss Segment ${index + 2}`,
			location: { x, y: Number(y), z: 4.2 },
		})),
	}));
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

async function installVenueObjects(
	api: ApiDriver,
	showId: string,
	entries: readonly SceneryEntry[],
) {
	for (const object of await api.showObjects<any>(showId, "venue"))
		await api.deleteSeededShowObject(
			showId,
			"venue",
			object.id,
			object.revision,
		);
	for (const entry of entries) {
		if (entry.profile === "Crowd Area") continue;
		const instances = [
			{ name: entry.name, location: entry.location, rotation: entry.rotation },
			...(entry.multipatches ?? []),
		];
		for (const [index, instance] of instances.entries()) {
			const id = `planned-demo-venue-${entry.number}-${index + 1}`;
			await putPlannedDemoObject(api, showId, "venue", id, {
				id,
				name: instance.name,
				kind: venueKind(entry),
				position: instance.location,
				rotation_degrees: instance.rotation ?? { x: 0, y: 0, z: 0 },
				size: venueSize(entry),
				chords: entry.profile === "Four-Point Truss" ? 4 : undefined,
			});
		}
	}
}

function venueKind(entry: SceneryEntry) {
	if (entry.profile === "Disco Ball 50 cm") return "mirror_ball";
	if (entry.profile.startsWith("Curtain")) return "curtain";
	if (entry.name.includes("Railing")) return "railing";
	if (entry.profile.includes("Truss") || entry.profile.includes("Pipe"))
		return "truss";
	if (entry.profile.startsWith("Stage Element")) return "riser";
	return "prop";
}

function venueSize(entry: SceneryEntry): Point {
	if (entry.profile === "Disco Ball 50 cm") return { x: 0.5, y: 0.5, z: 0.75 };
	if (
		entry.name === "Stage Left Curtain" ||
		entry.name === "Stage Right Curtain"
	)
		return { x: 0.12, y: 5, z: 5 };
	if (entry.profile.startsWith("Curtain")) return { x: 5, y: 0.12, z: 5 };
	if (entry.profile.startsWith("Stage Element")) return { x: 2, y: 1, z: 0.5 };
	if (entry.profile.includes("Truss") || entry.profile.includes("Pipe"))
		return { x: Number.parseFloat(entry.mode), y: 0.3, z: 0.3 };
	return { x: 1, y: 1, z: 1 };
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
