import type { Meta, StoryObj } from "@storybook/react-vite";
import { type PropsWithChildren, useMemo } from "react";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import type { FixtureProfile, PatchLayer, VersionedObject } from "../api/types";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "../components/setup/fixtureProfileModel";
import { fixtureTypeIconAsset } from "../components/setup/fixtureTypeIconAssets";
import { FixtureLibraryProvider } from "../features/fixtureLibrary/FixtureLibraryContext";
import type {
	PatchFixtureProjection,
	PatchMutationOutcome,
	PatchProfileRevision,
	PatchSnapshot,
} from "../features/patch/contracts";
import { PatchViewProvider } from "../features/patch/PatchContext";
import type {
	PatchEventStream,
	PatchTransport,
} from "../features/patch/transport";
import { PatchWindow } from "./PatchWindow";

const meta = {
	title: "ToskLight/Windows/Patch",
	component: PatchWindow,
	tags: ["autodocs"],
	parameters: { layout: "fullscreen" },
	decorators: [
		(Story) => (
			<ApplicationStateHarness>
				<div style={{ height: 680, minWidth: 920 }}>
					<Story />
				</div>
			</ApplicationStateHarness>
		),
	],
} satisfies Meta<typeof PatchWindow>;
export default meta;
type Story = StoryObj<typeof meta>;

export const EmptyPatch: Story = {
	args: { active: true },
	decorators: [
		(Story) => (
			<PatchStoryHarness fixtures={[]}>
				<Story />
			</PatchStoryHarness>
		),
	],
};

export const FilledPatch: Story = {
	args: { active: true },
	decorators: [
		(Story) => (
			<PatchStoryHarness fixtures={fixtureProjections}>
				<Story />
			</PatchStoryHarness>
		),
	],
};

const showId = "10000000-0000-0000-0000-000000000001";
const fresnelProfile = storyProfile({
	id: "20000000-0000-0000-0000-000000000001",
	modeId: "30000000-0000-0000-0000-000000000001",
	manufacturer: "ToskLight Demo",
	name: "Front Fresnel",
	type: "fresnel",
	footprint: 1,
});
const washProfile = storyProfile({
	id: "20000000-0000-0000-0000-000000000002",
	modeId: "30000000-0000-0000-0000-000000000002",
	manufacturer: "ToskLight Demo",
	name: "Aurora Wash",
	type: "led wash moving light",
	footprint: 10,
});
const blinderProfile = storyProfile({
	id: "20000000-0000-0000-0000-000000000003",
	modeId: "30000000-0000-0000-0000-000000000003",
	manufacturer: "ToskLight Demo",
	name: "Audience Blinder 2 × 2",
	type: "blinder",
	footprint: 4,
});
const aclProfile = storyProfile({
	id: "20000000-0000-0000-0000-000000000004",
	modeId: "30000000-0000-0000-0000-000000000004",
	manufacturer: "ToskLight Demo",
	name: "ACL",
	type: "acl",
	footprint: 1,
});
const profiles = [fresnelProfile, washProfile, blinderProfile, aclProfile];
const definitions = fixtureDefinitionsFromProfiles(profiles);
const fixtureProjections: PatchFixtureProjection[] = [
	fixtureProjection(fresnelProfile, 101, "Front Fresnel 1", 1, 1, "front"),
	fixtureProjection(fresnelProfile, 102, "Front Fresnel 2", 1, 2, "front"),
	fixtureProjection(fresnelProfile, 103, "Front Fresnel 3", 1, 3, "front"),
	fixtureProjection(fresnelProfile, 104, "Front Fresnel 4", 1, 4, "front"),
	fixtureProjection(washProfile, 201, "Front Wash 1", 1, 21, "front"),
	fixtureProjection(washProfile, 202, "Front Wash 2", 1, 31, "front"),
	fixtureProjection(washProfile, 203, "Front Wash 3", 1, 41, "front"),
	fixtureProjection(washProfile, 204, "Front Wash 4", 1, 51, "front"),
	{
		...fixtureProjection(
			blinderProfile,
			301,
			"Front Blinder 1",
			1,
			201,
			"front",
		),
		multipatch: [
			{
				id: "front-blinder-2",
				name: "Front Blinder 2",
				splitPatches: [{ split: 1, universe: 1, address: 205 }],
				location: { x: -800, y: 0, z: 4500 },
				rotation: { x: 0, y: 0, z: 0 },
			},
			{
				id: "front-blinder-3",
				name: "Front Blinder 3",
				splitPatches: [{ split: 1, universe: 1, address: 209 }],
				location: { x: 800, y: 0, z: 4500 },
				rotation: { x: 0, y: 0, z: 0 },
			},
			{
				id: "front-blinder-4",
				name: "Front Blinder 4",
				splitPatches: [{ split: 1, universe: 1, address: 213 }],
				location: { x: 2400, y: 0, z: 4500 },
				rotation: { x: 0, y: 0, z: 0 },
			},
		],
	},
	{
		...fixtureProjection(aclProfile, 401, "Stage ACL 1", 2, 1, "stage"),
		multipatch: Array.from({ length: 7 }, (_, index) => ({
			id: `stage-acl-${index + 2}`,
			name: `Stage ACL ${index + 2}`,
			splitPatches: [{ split: 1, universe: null, address: null }],
			location: { x: (index - 3) * 700, y: 1800, z: 3200 },
			rotation: { x: 0, y: 0, z: 0 },
		})),
	},
];

function PatchStoryHarness({
	fixtures,
	children,
}: PropsWithChildren<{ fixtures: PatchFixtureProjection[] }>) {
	const transport = useMemo(
		() => new StoryPatchTransport(fixtures),
		[fixtures],
	);
	return (
		<FixtureLibraryProvider library={storyLibrary}>
			<PatchViewProvider
				showId={showId}
				initialFixtures={[]}
				definitions={definitions}
				transport={transport}
			>
				{children}
			</PatchViewProvider>
		</FixtureLibraryProvider>
	);
}

const layers: VersionedObject<PatchLayer>[] = [
	versionedLayer("front", "Front truss", 1),
	versionedLayer("stage", "Stage", 2),
	versionedLayer("floor", "Floor package", 3),
];

const storyLibrary = {
	fixtureLibrary: definitions,
	fixtureProfiles: profiles,
	fixtureProfileWarnings: [],
	patchLayers: layers,
	unresolvedMvrFixtures: [],
	savePatchLayer: async () => true,
	saveFixtureProfile: async (profile: FixtureProfile) => profile,
	deleteFixtureProfile: async () => undefined,
	fixtureProfileRevisions: async () => [],
	saveFixtureProfileSourceGdtf: async () => true,
	importFixturePackage: async () => washProfile,
	exportFixturePackage: async () => new Blob(),
};

class StoryPatchTransport implements PatchTransport {
	constructor(private readonly fixtures: PatchFixtureProjection[]) {}
	async snapshot(): Promise<PatchSnapshot> {
		return {
			showId,
			showRevision: 7,
			patchRevision: 12,
			cursor: 12,
			fixtures: this.fixtures,
			profileRevisions: profiles.map(profileRevision),
		};
	}
	async patchFixtures(): Promise<PatchMutationOutcome> {
		throw new Error("Story Patch transport is read only");
	}
	subscribe(): PatchEventStream {
		return { repair: () => undefined, close: () => undefined };
	}
}

function storyProfile({
	id,
	modeId,
	manufacturer,
	name,
	type,
	footprint,
}: {
	id: string;
	modeId: string;
	manufacturer: string;
	name: string;
	type: string;
	footprint: number;
}) {
	const profile = blankFixtureProfile();
	profile.id = id;
	profile.revision = 1;
	profile.manufacturer = manufacturer;
	profile.name = name;
	profile.short_name = name;
	profile.fixture_type = type;
	profile.stage_icon_asset = fixtureTypeIconAsset(type);
	profile.modes[0].id = modeId;
	profile.modes[0].name = `${footprint} channel`;
	profile.modes[0].splits = [{ number: 1, footprint }];
	return profile;
}

function fixtureProjection(
	profile: FixtureProfile,
	fixtureNumber: number,
	name: string,
	universe: number,
	address: number,
	layerId: string,
): PatchFixtureProjection {
	return {
		fixtureId: `40000000-0000-0000-0000-${String(fixtureNumber).padStart(12, "0")}`,
		fixtureRevision: 1,
		fixtureNumber,
		virtualFixtureNumber: null,
		name,
		profileId: profile.id,
		profileRevision: profile.revision,
		modeId: profile.modes[0].id,
		splitPatches: [{ split: 1, universe, address }],
		layerId,
		directControl: null,
		location: { x: (fixtureNumber % 2 ? -1 : 1) * 2200, y: 0, z: 4500 },
		rotation: { x: 0, y: 0, z: 0 },
		logicalHeads: [],
		multipatch: [],
		moveInBlackEnabled: true,
		moveInBlackDelayMillis: fixtureNumber === 201 ? 800 : 0,
		highlightOverrides: [],
	};
}

function profileRevision(profile: FixtureProfile): PatchProfileRevision {
	return {
		profileId: profile.id,
		profileRevision: profile.revision,
		contentDigest: `storybook-${profile.id}`,
		manufacturer: profile.manufacturer,
		name: profile.name,
		fixtureType: profile.fixture_type,
		patchPolicy: "dmx",
		referencedModes: profile.modes.map((mode) => ({
			modeId: mode.id,
			name: mode.name,
			splits: mode.splits.map((split) => ({
				split: split.number,
				footprint: split.footprint,
			})),
		})),
		profileSnapshot: profile,
	};
}

function versionedLayer(
	id: string,
	name: string,
	order: number,
): VersionedObject<PatchLayer> {
	return {
		kind: "patch_layer",
		id,
		body: { id, name, order },
		revision: 1,
		updated_at: "2026-07-26T12:00:00Z",
	};
}
