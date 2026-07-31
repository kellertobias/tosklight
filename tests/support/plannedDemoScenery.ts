import type { FixtureProfile } from "../../apps/light-desktop/src/api/types";
import type { ApiDriver } from "../bench/core/api";
import { ensurePlannedDemoFixtureLibrary } from "./plannedDemoFixtureLibrary";

export const PLANNED_DEMO_SCENERY_FIXTURES = 33;
export const PLANNED_DEMO_TOTAL_FIXTURE_RECORDS = 295;
export const PLANNED_DEMO_TOTAL_PHYSICAL_INSTANCES = 343;

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
	const fixtures = sceneryEntries().map((entry) => {
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
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{
			request_id: crypto.randomUUID(),
			fixtures,
			remove_fixture_ids: [],
			placements: [],
		},
		true,
		before.revision,
		{ showId },
	);
	return fixtures;
}

function sceneryEntries(): SceneryEntry[] {
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
	const stage = Array.from({ length: 16 }, (_, index) => ({
		number: index + 4,
		name: `Stage Element ${index + 1}`,
		profile: "Stage Element 2 × 1 m",
		mode: "50 cm",
		layer: "Stage & Venue",
		location: {
			x: -3 + (index % 4) * 2,
			y: 0.5 + Math.floor(index / 4),
			z: 0,
		},
	}));
	const curtains = [-2.5, 2.5].map((x, index) => ({
		number: index + 20,
		name: `Back Curtain ${index + 1}`,
		profile: "Curtain 5 m",
		mode: "5 m",
		layer: "Stage & Venue",
		location: { x, y: 4.35, z: 2.5 },
	}));
	const railings = [
		...[-3, -1, 1, 3].map((x, index) => ({
			number: index + 22,
			name: `Back Railing ${index + 1}`,
			profile: "One-Point Truss / Pipe",
			mode: "2 m",
			layer: "Stage & Venue",
			location: { x, y: 4.05, z: 1.35 },
		})),
		...[-1, 1].flatMap((side, sideIndex) =>
			[1, 3].map((y, index) => ({
				number: 26 + sideIndex * 2 + index,
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
		number: index + 30,
		name: `Vertical Pipe ${index + 1}`,
		profile: "One-Point Truss / Pipe",
		mode: "2.5 m",
		layer: "Trusses",
		location: { x, y: 4.15, z: 2.9 },
		rotation: { x: 0, y: 90, z: 0 },
	}));
	return [...trusses, ...stage, ...curtains, ...railings, ...pipes];
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
