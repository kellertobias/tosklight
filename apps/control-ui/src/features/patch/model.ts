import type {
	FixtureDefinition,
	FixtureProfile,
	PatchedFixture,
	SplitPatch,
} from "../../api/types";
import type {
	PatchFixtureProjection,
	PatchFixtureWrite,
	PatchProfileRevision,
} from "./contracts";

export interface PatchFixtureCandidate {
	input: PatchFixtureWrite;
	fixture: PatchedFixture;
}

export interface NewPatchFixture {
	name: string;
	fixture_number: number | null;
	virtual_fixture_number?: number | null;
	definition: FixtureDefinition;
	universe: number | null;
	address: number | null;
	split_patches?: SplitPatch[];
	layer_id?: string;
}

export type PatchDefinitionResolver = (
	profileId: string,
	profileRevision: number,
	modeId: string,
) => FixtureDefinition | null;

export function createPatchDefinitionResolver(
	definitions: readonly FixtureDefinition[],
): PatchDefinitionResolver {
	const byReference = new Map<string, FixtureDefinition>();
	for (const definition of definitions) {
		const reference = definitionReference(definition);
		if (reference) byReference.set(referenceKey(...reference), definition);
	}
	return (profileId, profileRevision, modeId) =>
		byReference.get(referenceKey(profileId, profileRevision, modeId)) ?? null;
}

export function newPatchFixtureCandidate(
	fixture: NewPatchFixture,
): PatchFixtureCandidate {
	const fixtureId = crypto.randomUUID();
	const splitPatches =
		fixture.split_patches?.length
			? fixture.split_patches
			: [
					{
						split: primarySplit(fixture.definition),
						universe: fixture.universe,
						address: fixture.address,
					},
				];
	const optimistic: PatchedFixture = {
		fixture_id: fixtureId,
		fixture_number: fixture.fixture_number,
		virtual_fixture_number: fixture.virtual_fixture_number ?? null,
		name: fixture.name,
		definition: fixture.definition,
		universe: fixture.universe,
		address: fixture.address,
		split_patches: splitPatches,
		layer_id: fixture.layer_id ?? "default",
		direct_control: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		multipatch: [],
		move_in_black_enabled: true,
		move_in_black_delay_millis: 0,
		highlight_overrides: {},
	};
	return patchedFixtureCandidate(optimistic);
}

export function changedPatchFixtureCandidate(
	fixture: PatchedFixture,
	changes: Partial<PatchedFixture>,
): PatchFixtureCandidate {
	return patchedFixtureCandidate({ ...fixture, ...changes });
}

export function patchedFixtureCandidate(
	fixture: PatchedFixture,
): PatchFixtureCandidate {
	const reference = definitionReference(fixture.definition);
	if (!reference)
		throw new Error(
			"Fixture " +
				(fixture.name || fixture.fixture_id) +
				" has no immutable profile and mode reference",
		);
	const [profileId, profileRevision, modeId] = reference;
	const splitPatches =
		fixture.split_patches?.length
			? fixture.split_patches
			: [
					{
						split: primarySplit(fixture.definition),
						universe: fixture.universe,
						address: fixture.address,
					},
				];
	return {
		fixture,
		input: {
			fixtureId: fixture.fixture_id,
			fixtureNumber: fixture.fixture_number ?? null,
			virtualFixtureNumber: fixture.virtual_fixture_number ?? null,
			name: fixture.name || fixture.definition.name,
			profileId,
			profileRevision,
			modeId,
			splitPatches,
			layerId: fixture.layer_id || "default",
			directControl: fixture.direct_control
				? {
						protocol: fixture.direct_control.protocol,
						ipAddress: fixture.direct_control.ip_address,
						port: fixture.direct_control.port,
					}
				: null,
			location: fixture.location ?? { x: 0, y: 0, z: 0 },
			rotation: fixture.rotation ?? { x: 0, y: 0, z: 0 },
			multipatch: (fixture.multipatch ?? []).map((instance) => ({
				id: instance.id,
				name: instance.name,
				splitPatches:
					instance.split_patches?.length
						? instance.split_patches
						: [
								{
									split: primarySplit(fixture.definition),
									universe: instance.universe,
									address: instance.address,
								},
							],
				location: instance.location,
				rotation: instance.rotation,
			})),
			moveInBlackEnabled: fixture.move_in_black_enabled ?? true,
			moveInBlackDelayMillis:
				fixture.move_in_black_delay_millis ?? 0,
			highlightOverrides: Object.entries(
				fixture.highlight_overrides ?? {},
			).map(([channelId, rawValue]) => ({ channelId, rawValue })),
		},
	};
}

export function projectionToPatchedFixture(
	projection: PatchFixtureProjection,
	profile: PatchProfileRevision,
	resolveDefinition: PatchDefinitionResolver,
	fallback?: PatchedFixture,
): PatchedFixture {
	const definition =
		resolveDefinition(
			projection.profileId,
			projection.profileRevision,
			projection.modeId,
		) ??
		(matchingDefinition(fallback?.definition, projection)
			? fallback?.definition
			: null) ??
		syntheticDefinition(profile, projection.modeId);
	const primary =
		projection.splitPatches.find((split) => split.split === 1) ??
		projection.splitPatches[0];
	return {
		fixture_id: projection.fixtureId,
		fixture_number: projection.fixtureNumber,
		virtual_fixture_number: projection.virtualFixtureNumber,
		name: projection.name,
		definition,
		universe: primary?.universe ?? null,
		address: primary?.address ?? null,
		split_patches: [...projection.splitPatches],
		layer_id: projection.layerId,
		direct_control: projection.directControl
			? {
					protocol: projection.directControl.protocol,
					ip_address: projection.directControl.ipAddress,
					port: projection.directControl.port,
				}
			: null,
		location: projection.location,
		rotation: projection.rotation,
		logical_heads: projection.logicalHeads.map((head) => ({
			profile_head_id: head.profileHeadId,
			fixture_id: head.fixtureId,
			head_index: head.headIndex,
		})),
		multipatch: projection.multipatch.map((instance) => {
			const instancePrimary =
				instance.splitPatches.find((split) => split.split === 1) ??
				instance.splitPatches[0];
			return {
				id: instance.id,
				name: instance.name,
				universe: instancePrimary?.universe ?? null,
				address: instancePrimary?.address ?? null,
				split_patches: [...instance.splitPatches],
				location: instance.location,
				rotation: instance.rotation,
			};
		}),
		move_in_black_enabled: projection.moveInBlackEnabled,
		move_in_black_delay_millis: projection.moveInBlackDelayMillis,
		highlight_overrides: Object.fromEntries(
			projection.highlightOverrides.map((override) => [
				override.channelId,
				override.rawValue,
			]),
		),
	};
}

function definitionReference(
	definition: FixtureDefinition,
): [string, number, string] | null {
	const profileId = definition.profile_id ?? definition.profile_snapshot?.id;
	const profileRevision =
		definition.profile_snapshot?.revision ?? definition.revision;
	const modeId =
		definition.mode_id ??
		definition.profile_snapshot?.modes.find(
			(mode) => mode.name === definition.mode,
		)?.id;
	return profileId && modeId ? [profileId, profileRevision, modeId] : null;
}

function referenceKey(
	profileId: string,
	profileRevision: number,
	modeId: string,
) {
	return profileId + ":" + profileRevision + ":" + modeId;
}

function matchingDefinition(
	definition: FixtureDefinition | undefined,
	projection: PatchFixtureProjection,
) {
	const reference = definition && definitionReference(definition);
	return Boolean(
		reference &&
			reference[0] === projection.profileId &&
			reference[1] === projection.profileRevision &&
			reference[2] === projection.modeId,
	);
}

function primarySplit(definition: FixtureDefinition): number {
	return (
		definition.profile_snapshot?.modes
			.find((mode) => mode.id === definition.mode_id)
			?.splits[0]?.number ?? 1
	);
}

function syntheticDefinition(
	profile: PatchProfileRevision,
	modeId: string,
): FixtureDefinition {
	const mode =
		profile.referencedModes.find((candidate) => candidate.modeId === modeId) ??
		profile.referencedModes[0];
	if (!mode)
		throw new Error(
			"Patch snapshot profile " + profile.profileId + " has no referenced mode",
		);
	const fixtureProfile: FixtureProfile = {
		schema_version: 2,
		id: profile.profileId,
		revision: profile.profileRevision,
		manufacturer: profile.manufacturer,
		name: profile.name,
		short_name: profile.name,
		fixture_type: profile.fixtureType,
		patch_policy: profile.patchPolicy,
		notes: "",
		photograph_asset: null,
		stage_icon_asset: null,
		model_asset: null,
		physical: {
			width_millimetres: null,
			height_millimetres: null,
			depth_millimetres: null,
			weight_kilograms: null,
			power_watts: null,
		},
		modes: [
			{
				id: mode.modeId,
				name: mode.name,
				notes: "",
				splits: mode.splits.map((split) => ({
					number: split.split,
					footprint: split.footprint,
				})),
				heads: [],
				channels: [],
				color_systems: [],
				control_actions: [],
				geometry: { nodes: [], emitters: [] },
			},
		],
		hazardous: false,
		direct_control_protocols: [],
		signal_loss_policy: { type: "hold_last" },
		reserved_source: null,
	};
	return {
		schema_version: 2,
		id: profile.profileId,
		revision: profile.profileRevision,
		manufacturer: profile.manufacturer,
		device_type: profile.fixtureType,
		name: profile.name,
		model: profile.name,
		mode: mode.name,
		footprint: mode.splits[0]?.footprint ?? 0,
		heads: [],
		color_calibration: null,
		physical: {},
		model_asset: null,
		icon_asset: null,
		hazardous: false,
		direct_control_protocols: [],
		signal_loss_policy: { type: "hold_last" },
		safe_values: {},
		profile_id: profile.profileId,
		mode_id: mode.modeId,
		profile_snapshot: fixtureProfile,
	};
}
