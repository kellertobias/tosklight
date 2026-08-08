import type {
	PatchedFixture,
	VisualizationSnapshot,
} from "../../../light-desktop/src/api/types";
import type { Stage3dFixture } from "../../../light-desktop/src/windows/stage3dScene";
import type {
	StageFixturePresentation,
	StageLayoutModel,
	StageOptionsModel,
} from "../../../light-desktop/src/windows/stageWindow/types";
import type { StageSelectionModel } from "../../../light-desktop/src/windows/stageWindow/useStageSelection";

export const stagePresentations: StageFixturePresentation[] = [
	{
		fixtureId: "front-left",
		fixtureNumber: 1,
		name: "Front Fresnel Left",
		icon: null,
		color: "#ffd9a8",
		dimmer: 78,
		pan: 0.38,
		tilt: 0.66,
	},
	{
		fixtureId: "front-right",
		fixtureNumber: 2,
		name: "Front Fresnel Right",
		icon: null,
		color: "#ffd9a8",
		dimmer: 62,
		pan: 0.62,
		tilt: 0.66,
	},
	{
		fixtureId: "wash-left",
		fixtureNumber: 201,
		name: "LED Wash Left",
		icon: null,
		color: "#3b8cff",
		dimmer: 86,
		pan: 0.44,
		tilt: 0.48,
	},
	{
		fixtureId: "wash-right",
		fixtureNumber: 202,
		name: "LED Wash Right",
		icon: null,
		color: "#c65cff",
		dimmer: 72,
		pan: 0.56,
		tilt: 0.48,
	},
	{
		fixtureId: "profile",
		fixtureNumber: 101,
		name: "Center Profile",
		icon: null,
		color: "#ffffff",
		dimmer: 92,
		pan: 0.5,
		tilt: 0.34,
	},
];

export const stageLayout: StageLayoutModel = {
	positions: {
		"front-left": { x: 18, y: 20, rotation: -20 },
		"front-right": { x: 74, y: 20, rotation: 20 },
		"wash-left": { x: 30, y: 58, rotation: -8 },
		"wash-right": { x: 62, y: 58, rotation: 8 },
		profile: { x: 46, y: 38, rotation: 0 },
	},
	positions3d: {},
	positions2dConfig: {
		provenance: "automatic",
		projection: "front_to_back",
	},
};

export const stageOptions: StageOptionsModel = {
	mode: "select",
	setMode: () => undefined,
	view: "2d",
	setView: () => undefined,
	side2d: "top" as const,
	setSide2d: () => undefined,
	followPreload: false,
	toggleFollowPreload: () => undefined,
	groupsVisible: true,
	showSelection: true,
	showFloorGrid: true,
	environmentBrightness: 1,
};

export const stageSelection: StageSelectionModel = {
	fixtureIds: ["profile"],
	fixtureIdSet: new Set(["profile"]),
	firstFixtureId: "profile",
	applyFixtureGesture: async () => null,
	replaceFixtureIds: async () => null,
	clear: async () => null,
};

const fixtures = stagePresentations.map((presentation, index) => ({
	fixture_id: presentation.fixtureId,
	fixture_number: Number(presentation.fixtureNumber),
	name: presentation.name,
	universe: 1,
	address: index * 10 + 1,
	logical_heads: [],
	definition: {
		name: presentation.name,
		device_type: index > 1 ? "moving_head" : "fresnel",
		footprint: 4,
		heads: [
			{
				parameters: [
					{ attribute: "intensity", components: [{ offset: 0 }] },
					{ attribute: "pan", components: [{ offset: 1 }] },
					{ attribute: "tilt", components: [{ offset: 2 }] },
				],
			},
		],
	},
})) as unknown as PatchedFixture[];

export const stage3dFixtures: Stage3dFixture[] = fixtures.map(
	(fixture, index) => ({
		fixture,
		index,
		position: {
			x: (index - 2) * 1.8,
			y: index < 2 ? 1 : index < 4 ? 4.6 : 2.8,
			z: index < 2 ? 3.8 : index < 4 ? 3.2 : 4.5,
			rotationX: index < 2 ? -15 : 0,
			rotationY: 0,
			rotationZ: 0,
		},
	}),
);

export const stageVisualization: VisualizationSnapshot = {
	revision: 8,
	generated_at: "2026-07-26T12:00:00Z",
	grand_master: 1,
	blackout: false,
	values: fixtures.flatMap((fixture, index) => [
		{
			fixture_id: fixture.fixture_id,
			attribute: "intensity",
			value: {
				kind: "normalized" as const,
				value: [0.78, 0.62, 0.86, 0.72, 0.92][index],
			},
		},
		{
			fixture_id: fixture.fixture_id,
			attribute: "pan",
			value: { kind: "normalized" as const, value: 0.42 + index * 0.04 },
		},
		{
			fixture_id: fixture.fixture_id,
			attribute: "tilt",
			value: { kind: "normalized" as const, value: 0.48 },
		},
	]),
};
