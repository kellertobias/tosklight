import { fixtureTypeIconAsset } from "../../../light-desktop/src/components/setup/fixtureTypeIconAssets";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "../../../light-desktop/src/components/setup/fixtureProfileModel";
import type {
	PatchedFixture,
	VisualizationSnapshot,
} from "../../../light-desktop/src/api/types";
import type { PresetCard } from "../../../light-desktop/src/features/presetRecording/presetCards";
import type { Stage3dFixture } from "../../../light-desktop/src/windows/stage3dScene";
import type { FixtureSheetRow } from "../../../light-desktop/src/windows/fixtureSheetProjection";
import type { FixtureStepPresenter } from "../../../light-desktop/src/windows/fixtureSheetStep";
import type { Group } from "../../../light-desktop/src/windows/groupsWindow/model";
import { ICON_CATALOG_GROUPS } from "../../src/common/controls/iconCatalog";

const profileIcon = fixtureTypeIconAsset("profile dimmer lamp");
const washIcon = fixtureTypeIconAsset("led wash moving light");
const blinderIcon = fixtureTypeIconAsset("blinder");
const hazerIcon = fixtureTypeIconAsset("hazer");

function catalogIcon(groupId: string, label: string) {
	const icon = ICON_CATALOG_GROUPS.find(
		(group) => group.id === groupId,
	)?.icons.find((candidate) => candidate.label === label);
	if (!icon) throw new Error(`Missing ${groupId} icon: ${label}`);
	return icon.value;
}

const positionIcons = {
	blind: catalogIcon("position-beam", "Up Fan Out"),
	center: catalogIcon("position", "Left Right In"),
	cross1: catalogIcon("position", "Down Cross 1"),
	cross2: catalogIcon("position", "Down Cross 2"),
	down: catalogIcon("position", "Down"),
	overAudience: catalogIcon("position", "Up Fan Out"),
};

const fixtureSheetDefaults = {
	beam: "Open",
	childFixtureIds: [] as string[],
	color: "#ffffff",
	colorLabel: "Open White",
	dimmer: 0,
	focus: "Sharp",
	indented: false,
	limitingGroups: [],
	parentFixtureId: "",
	pan: 50,
	patch: "U1.1",
	positionLabel: "Center",
	preloadColor: null,
	preloadDimmer: null,
	preloadPan: null,
	preloadTilt: null,
	sources: {
		beam: "default" as const,
		color: "default" as const,
		dimmer: "default" as const,
		focus: "default" as const,
		position: "default" as const,
	},
	targetKind: "fixture" as const,
	tilt: 50,
	type: "Fixture",
};

export const marketingFixtureSheetRows = [
	{
		...fixtureSheetDefaults,
		id: "101",
		fixtureId: "fixture-101",
		name: "Front Profile SL",
		fixtureType: "ETC · Source Four LED Series 3",
		icon: profileIcon,
		patch: "U1.1",
		dimmer: 72,
		color: "#f6c985",
		colorLabel: "Warm White",
		pan: 42,
		tilt: 38,
		positionLabel: "Lectern",
		sources: {
			...fixtureSheetDefaults.sources,
			dimmer: "programmer" as const,
			color: "programmer" as const,
			position: "programmer" as const,
		},
	},
	{
		...fixtureSheetDefaults,
		id: "102",
		fixtureId: "fixture-102",
		name: "Front Profile SR",
		fixtureType: "ETC · Source Four LED Series 3",
		icon: profileIcon,
		patch: "U1.31",
		dimmer: 68,
		color: "#f6c985",
		colorLabel: "Warm White",
		pan: 58,
		tilt: 38,
		positionLabel: "Lectern",
		sources: {
			...fixtureSheetDefaults.sources,
			dimmer: "programmer" as const,
			color: "programmer" as const,
			position: "programmer" as const,
		},
	},
	{
		...fixtureSheetDefaults,
		id: "201",
		fixtureId: "fixture-201",
		name: "Stage Wash SL",
		fixtureType: "Robe · Tetra2",
		icon: washIcon,
		patch: "U1.101",
		dimmer: 86,
		color: "#1bd6ec",
		colorLabel: "Cyan",
		pan: 35,
		tilt: 54,
		positionLabel: "Downstage fan",
		beam: "Wash",
		focus: "Medium",
		sources: {
			...fixtureSheetDefaults.sources,
			dimmer: "programmer" as const,
			color: "programmer" as const,
			position: "programmer" as const,
			beam: "playback" as const,
			focus: "playback" as const,
		},
	},
	{
		...fixtureSheetDefaults,
		id: "202",
		fixtureId: "fixture-202",
		name: "Stage Wash SR",
		fixtureType: "Robe · Tetra2",
		icon: washIcon,
		patch: "U1.161",
		dimmer: 78,
		color: "#e24bdb",
		colorLabel: "Magenta",
		pan: 65,
		tilt: 54,
		positionLabel: "Downstage fan",
		beam: "Wash",
		focus: "Medium",
		preloadDimmer: 92,
		preloadColor: "#6bbcff",
		preloadPan: 62,
		preloadTilt: 48,
		sources: {
			...fixtureSheetDefaults.sources,
			dimmer: "programmer" as const,
			color: "programmer" as const,
			position: "default" as const,
			beam: "playback" as const,
			focus: "playback" as const,
		},
	},
	{
		...fixtureSheetDefaults,
		id: "301",
		fixtureId: "fixture-301",
		name: "Back Wash SL",
		fixtureType: "Astera · AX9",
		icon: washIcon,
		patch: "U2.1",
		dimmer: 56,
		color: "#8a5cf6",
		colorLabel: "Purple",
		positionLabel: "Back truss",
		beam: "Wide",
		focus: "Soft",
		sources: {
			...fixtureSheetDefaults.sources,
			dimmer: "programmer" as const,
			color: "programmer" as const,
			beam: "playback" as const,
		},
	},
	{
		...fixtureSheetDefaults,
		id: "302",
		fixtureId: "fixture-302",
		name: "Back Wash SR",
		fixtureType: "Astera · AX9",
		icon: washIcon,
		patch: "U2.21",
		dimmer: 56,
		color: "#285bd8",
		colorLabel: "Dark Blue",
		positionLabel: "Back truss",
		beam: "Wide",
		focus: "Soft",
		sources: {
			...fixtureSheetDefaults.sources,
			dimmer: "programmer" as const,
			color: "programmer" as const,
			beam: "playback" as const,
		},
	},
	{
		...fixtureSheetDefaults,
		id: "401",
		fixtureId: "fixture-401",
		name: "Audience Blinder",
		fixtureType: "Generic · Blinder 2 × 2",
		icon: blinderIcon,
		patch: "U3.1",
		dimmer: 24,
		colorLabel: "Warm White",
		positionLabel: "Audience",
		beam: "Flood",
		focus: "Soft",
		sources: {
			...fixtureSheetDefaults.sources,
			dimmer: "programmer" as const,
		},
	},
	{
		...fixtureSheetDefaults,
		id: "501",
		fixtureId: "fixture-501",
		name: "Stage Hazer",
		fixtureType: "Generic · Hazer",
		icon: hazerIcon,
		patch: "U4.1",
		dimmer: 35,
		colorLabel: "—",
		positionLabel: "Upstage",
		beam: "Haze",
		focus: "—",
		sources: {
			...fixtureSheetDefaults.sources,
			dimmer: "programmer" as const,
		},
	},
] satisfies FixtureSheetRow[];

export const marketingFixtureSheetSelectedFixtureIds = new Set([
	"fixture-101",
	"fixture-102",
]);

export const marketingFixtureSheetPresentStep: FixtureStepPresenter = (
	row,
) => ({
	base: row.fixtureId === "fixture-101" || row.fixtureId === "fixture-102",
	containedBase: false,
	current: row.fixtureId === "fixture-202",
	containedCurrent: row.fixtureId === "fixture-201",
});

function group(
	id: string,
	name: string,
	fixtures: string[],
	options: Partial<Group["body"]> = {},
): Group {
	return {
		kind: "group",
		id,
		revision: 3,
		updated_at: "2026-07-26T12:00:00Z",
		body: {
			name,
			fixtures,
			programming: {},
			derived_from: null,
			frozen_from: null,
			...options,
		},
	};
}

export const marketingGroups = [
	group(
		"1",
		"All Fixtures",
		marketingFixtureSheetRows.map((row) => row.fixtureId),
		{ color: "#d98236", icon: profileIcon },
	),
	group("2", "Front Profiles", ["fixture-101", "fixture-102"], {
		color: "#e09645",
		icon: profileIcon,
	}),
	group(
		"4",
		"Stage Washes",
		["fixture-201", "fixture-202", "fixture-301", "fixture-302"],
		{
			color: "#cf7530",
			icon: washIcon,
		},
	),
	group("6", "Back Lights", ["fixture-301", "fixture-302"], {
		color: "#d98236",
		icon: washIcon,
	}),
	group("8", "Audience", ["fixture-401"], {
		color: "#e8a85f",
		icon: blinderIcon,
	}),
] satisfies Group[];

export const marketingGroupCards = Array.from(
	{ length: 200 },
	(_, index) =>
		marketingGroups.find((candidate) => candidate.id === String(index + 1)) ??
		null,
);

export const marketingKnownFixtureIds = new Set(
	marketingFixtureSheetRows.map((row) => row.fixtureId),
);

type PresetFixture = {
	name: string;
	color: string;
	icon?: string;
};

function presets(
	family: "Color" | "Position",
	fixtures: readonly PresetFixture[],
): PresetCard[] {
	return fixtures.map((fixture, index) => ({
		id: `${family === "Color" ? 2 : 3}.${index + 1}`,
		revision: 2,
		body: {
			name: fixture.name,
			number: index + 1,
			family,
			values: {
				"fixture-101": {
					...(family === "Color"
						? { color: fixture.color }
						: { position: fixture.name }),
				},
			},
			color: fixture.color,
			icon: fixture.icon ?? "●",
		},
	}));
}

export const marketingColorPresets = presets("Color", [
	{ name: "Red", color: "#ff2b35" },
	{ name: "Orange", color: "#ff7a1a" },
	{ name: "Yellow", color: "#ffd21f" },
	{ name: "Lime", color: "#a8e42c" },
	{ name: "Green", color: "#27c85a" },
	{ name: "Teal", color: "#18a68d" },
	{ name: "Cyan", color: "#1bd6ec" },
	{ name: "Light Blue", color: "#6bbcff" },
	{ name: "Dark Blue", color: "#285bd8" },
	{ name: "Purple", color: "#8a5cf6" },
	{ name: "Magenta", color: "#e24bdb" },
	{ name: "White", color: "#ffffff" },
	{ name: "Warm White", color: "#f6c985" },
]);

export const marketingColorPresetCards = Array.from(
	{ length: 200 },
	(_, index) =>
		marketingColorPresets.find(
			(candidate) => candidate.body.number === index + 1,
		) ?? null,
);

export const marketingPositionPresets = presets("Position", [
	{ name: "Down", color: "#9b8cff", icon: positionIcons.down },
	{ name: "Cross 1", color: "#8a7bec", icon: positionIcons.cross1 },
	{ name: "Cross 2", color: "#7669dc", icon: positionIcons.cross2 },
	{
		name: "Over Audience",
		color: "#aa9cf7",
		icon: positionIcons.overAudience,
	},
	{ name: "Blind", color: "#c1b7ff", icon: positionIcons.blind },
	{ name: "Center", color: "#9386e8", icon: positionIcons.center },
]);

export const marketingPositionPresetCards = Array.from(
	{ length: 200 },
	(_, index) =>
		marketingPositionPresets.find(
			(candidate) => candidate.body.number === index + 1,
		) ?? null,
);

function lightingFixture(
	id: string,
	number: number,
	name: string,
	deviceType: string,
): PatchedFixture {
	return {
		fixture_id: id,
		fixture_number: number,
		name,
		universe: number < 200 ? 1 : 2,
		address: ((number - 1) % 20) * 20 + 1,
		logical_heads: [],
		definition: {
			name,
			manufacturer: number < 200 ? "ETC" : "Robe",
			model: name,
			device_type: deviceType,
			footprint: 16,
			heads: [
				{
					parameters: [
						{ attribute: "intensity", components: [{ offset: 0 }] },
						{ attribute: "pan", components: [{ offset: 1 }] },
						{ attribute: "tilt", components: [{ offset: 2 }] },
						{ attribute: "color.red", components: [{ offset: 3 }] },
						{ attribute: "color.green", components: [{ offset: 4 }] },
						{ attribute: "color.blue", components: [{ offset: 5 }] },
					],
				},
			],
		},
	} as unknown as PatchedFixture;
}

function trussFixture(
	id: string,
	number: number,
	name: string,
	widthMillimetres: number,
): PatchedFixture {
	const profile = blankFixtureProfile();
	profile.id = `marketing-${id}`;
	profile.manufacturer = "Venue";
	profile.name = name;
	profile.short_name = name;
	profile.fixture_type = "truss";
	profile.patch_policy = "visual_only";
	profile.physical.width_millimetres = widthMillimetres;
	profile.physical.height_millimetres = 260;
	profile.physical.depth_millimetres = 260;
	profile.modes[0].id = `${id}-10m`;
	profile.modes[0].name = "10 m";
	profile.modes[0].splits = [{ number: 1, footprint: 0 }];
	profile.modes[0].heads = [];
	profile.modes[0].channels = [];
	profile.modes[0].geometry.emitters = [];
	const definition = fixtureDefinitionsFromProfiles([profile])[0];
	return {
		fixture_id: id,
		fixture_number: number,
		name,
		universe: null,
		address: null,
		definition,
		logical_heads: [],
	} as PatchedFixture;
}

const frontProfiles = Array.from({ length: 4 }, (_, index) =>
	lightingFixture(
		`front-profile-${index + 1}`,
		101 + index,
		`Front Profile ${index + 1}`,
		"profile",
	),
);
const backProfiles = Array.from({ length: 8 }, (_, index) =>
	lightingFixture(
		`back-profile-${index + 1}`,
		201 + index,
		`Back Profile ${index + 1}`,
		"profile moving light",
	),
);
const backWashes = Array.from({ length: 7 }, (_, index) =>
	lightingFixture(
		`back-wash-${index + 1}`,
		301 + index,
		`Back Wash ${index + 1}`,
		"led wash moving light",
	),
);
const frontTruss = trussFixture("front-truss", 0.1, "Front Truss", 10_000);
const backTruss = trussFixture("back-truss", 0.2, "Back Truss", 10_000);

function stageItem(
	fixture: PatchedFixture,
	index: number,
	x: number,
	y: number,
	z: number,
): Stage3dFixture {
	return {
		fixture,
		index,
		position: {
			x,
			y,
			z,
			rotationX: 0,
			rotationY: 0,
			rotationZ: 0,
		},
	};
}

export const marketingStage3dFixtures: Stage3dFixture[] = [
	stageItem(frontTruss, 0, 0, 0.8, 4.8),
	stageItem(backTruss, 1, 0, 6.2, 5.2),
	...frontProfiles.map((fixture, index) =>
		stageItem(fixture, index + 2, -3.6 + index * 2.4, 0.8, 4.5),
	),
	...backProfiles.map((fixture, index) =>
		stageItem(fixture, index + 6, -4.2 + index * 1.2, 6.15, 4.85),
	),
	...backWashes.map((fixture, index) =>
		stageItem(fixture, index + 14, -3.6 + index * 1.2, 5.75, 4.45),
	),
];

function normalized(value: number) {
	return { kind: "normalized" as const, value };
}

function fixtureValues(
	fixture: PatchedFixture,
	color: [number, number, number],
	index: number,
) {
	return [
		{
			fixture_id: fixture.fixture_id,
			attribute: "intensity",
			value: normalized(0.66 + (index % 4) * 0.07),
		},
		{
			fixture_id: fixture.fixture_id,
			attribute: "pan",
			value: normalized(0.38 + (index % 5) * 0.06),
		},
		{
			fixture_id: fixture.fixture_id,
			attribute: "tilt",
			value: normalized(0.38 + (index % 3) * 0.05),
		},
		{
			fixture_id: fixture.fixture_id,
			attribute: "color.red",
			value: normalized(color[0]),
		},
		{
			fixture_id: fixture.fixture_id,
			attribute: "color.green",
			value: normalized(color[1]),
		},
		{
			fixture_id: fixture.fixture_id,
			attribute: "color.blue",
			value: normalized(color[2]),
		},
	];
}

export const marketingStageVisualization: VisualizationSnapshot = {
	revision: 12,
	generated_at: "2026-07-26T12:00:00Z",
	grand_master: 1,
	blackout: false,
	values: [
		...frontProfiles.flatMap((fixture, index) =>
			fixtureValues(fixture, [1, 0.78, 0.52], index),
		),
		...backProfiles.flatMap((fixture, index) =>
			fixtureValues(fixture, [0.88, 0.08, 0.72], index),
		),
		...backWashes.flatMap((fixture, index) =>
			fixtureValues(fixture, [0.04, 0.82, 0.94], index),
		),
	],
};
