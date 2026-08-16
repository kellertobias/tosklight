import type {
	FixtureDefinition,
	FixtureProfile,
	InstalledFixtureAppearance,
	PatchedFixture,
	SplitPatch,
} from "../../api/types";
import { fixtureDefinitionFromProfileMode } from "../../components/setup/fixtureProfileModel";
import type {
	PatchFixtureProjection,
	PatchFixtureWrite,
	PatchInstalledFixtureAppearance,
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
	const splitPatches = fixture.split_patches?.length
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
		internal_bindings: {},
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		multipatch: [],
		group_masters_enabled: true,
		grand_master_enabled: true,
		invert_pan: false,
		invert_tilt: false,
		bracket_angle: 0,
		shaper_angle: null,
		installed_appearance: defaultInstalledFixtureAppearance(),
		move_in_black_enabled: true,
		move_in_black_delay_millis: 0,
		highlight_overrides: {},
		freeze_targets: [],
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
	const splitPatches = fixture.split_patches?.length
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
			internalBindings: {
				library: fixture.internal_bindings?.library ?? null,
				output: fixture.internal_bindings?.output ?? null,
			},
			location: fixture.location ?? { x: 0, y: 0, z: 0 },
			rotation: fixture.rotation ?? { x: 0, y: 0, z: 0 },
			multipatch: (fixture.multipatch ?? []).map((instance) => ({
				id: instance.id,
				name: instance.name,
				splitPatches: instance.split_patches?.length
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
				invertPan: instance.invert_pan ?? false,
				invertTilt: instance.invert_tilt ?? false,
				bracketAngle: instance.bracket_angle ?? 0,
				shaperAngle: instance.shaper_angle ?? null,
				installedAppearance: patchAppearance(instance.installed_appearance),
			})),
			groupMastersEnabled: fixture.group_masters_enabled ?? true,
			grandMasterEnabled: fixture.grand_master_enabled ?? true,
			invertPan: fixture.invert_pan ?? false,
			invertTilt: fixture.invert_tilt ?? false,
			bracketAngle: fixture.bracket_angle ?? 0,
			shaperAngle: fixture.shaper_angle ?? null,
			installedAppearance: patchAppearance(fixture.installed_appearance),
			moveInBlackEnabled: fixture.move_in_black_enabled ?? true,
			moveInBlackDelayMillis: fixture.move_in_black_delay_millis ?? 0,
			highlightOverrides: Object.entries(fixture.highlight_overrides ?? {}).map(
				([channelId, rawValue]) => ({ channelId, rawValue }),
			),
		},
	};
}

/**
 * Builds a complete definition from the exact-revision profile snapshot carried in the patch
 * projection. This is the authoritative fallback when the live fixture library does not hold the
 * revision a fixture was patched at, so programmer-surface readers still see full head parameters,
 * channels, and control actions instead of the parameter-less synthetic definition.
 */
function definitionFromSnapshot(
	profile: PatchProfileRevision,
	modeId: string,
): FixtureDefinition | null {
	const snapshot = profile.profileSnapshot;
	if (!snapshot) return null;
	const mode =
		snapshot.modes.find((candidate) => candidate.id === modeId) ??
		snapshot.modes[0];
	if (!mode) return null;
	try {
		return fixtureDefinitionFromProfileMode(snapshot, mode);
	} catch {
		return null;
	}
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
		definitionFromSnapshot(profile, projection.modeId) ??
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
		internal_bindings: {
			library: projection.internalBindings?.library ?? null,
			output: projection.internalBindings?.output ?? null,
		},
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
				invert_pan: instance.invertPan ?? false,
				invert_tilt: instance.invertTilt ?? false,
				bracket_angle: instance.bracketAngle ?? 0,
				shaper_angle: instance.shaperAngle ?? null,
				installed_appearance: fixtureAppearance(instance.installedAppearance),
			};
		}),
		group_masters_enabled: projection.groupMastersEnabled ?? true,
		grand_master_enabled: projection.grandMasterEnabled ?? true,
		invert_pan: projection.invertPan ?? false,
		invert_tilt: projection.invertTilt ?? false,
		bracket_angle: projection.bracketAngle ?? 0,
		shaper_angle: projection.shaperAngle ?? null,
		installed_appearance: fixtureAppearance(projection.installedAppearance),
		move_in_black_enabled: projection.moveInBlackEnabled,
		move_in_black_delay_millis: projection.moveInBlackDelayMillis,
		highlight_overrides: Object.fromEntries(
			projection.highlightOverrides.map((override) => [
				override.channelId,
				override.rawValue,
			]),
		),
		freeze_targets: (projection.freezeTargets ?? []).map((target) => ({
			fixture_id: target.fixtureId,
			full: target.full,
			families: [...target.families],
		})),
	};
}

export function defaultInstalledFixtureAppearance(): InstalledFixtureAppearance {
	return {
		light_source: { type: "profile_default" },
		color_temperature_kelvin: null,
		luminous_output_lumens: null,
		gel: { type: "open_white" },
		shaper_angles_degrees: [0, 0, 0, 0],
	};
}

function patchAppearance(
	appearance: InstalledFixtureAppearance | undefined,
): PatchInstalledFixtureAppearance {
	const resolved = appearance ?? defaultInstalledFixtureAppearance();
	return {
		lightSource: { ...resolved.light_source },
		colorTemperatureKelvin: resolved.color_temperature_kelvin,
		luminousOutputLumens: resolved.luminous_output_lumens,
		gel:
			resolved.gel.type === "built_in"
				? {
						type: "built_in",
						catalogId: resolved.gel.catalog_id,
						entryId: resolved.gel.entry_id,
						embeddedFallback: {
							number: resolved.gel.embedded_fallback.number,
							name: resolved.gel.embedded_fallback.name,
							displaySrgb: resolved.gel.embedded_fallback.display_srgb,
							visualizerSrgb: resolved.gel.embedded_fallback.visualizer_srgb,
						},
					}
				: resolved.gel.type === "custom"
					? {
							type: "custom",
							name: resolved.gel.name,
							colorSrgb: resolved.gel.color_srgb,
							note: resolved.gel.note,
						}
					: { type: "open_white" },
		shaperAnglesDegrees: [...resolved.shaper_angles_degrees],
	};
}

function fixtureAppearance(
	appearance: PatchInstalledFixtureAppearance | undefined,
): InstalledFixtureAppearance {
	const resolved = appearance ?? patchAppearance(undefined);
	return {
		light_source: { ...resolved.lightSource },
		color_temperature_kelvin: resolved.colorTemperatureKelvin,
		luminous_output_lumens: resolved.luminousOutputLumens,
		gel:
			resolved.gel.type === "built_in"
				? {
						type: "built_in",
						catalog_id: resolved.gel.catalogId,
						entry_id: resolved.gel.entryId,
						embedded_fallback: {
							number: resolved.gel.embeddedFallback.number,
							name: resolved.gel.embeddedFallback.name,
							display_srgb: resolved.gel.embeddedFallback.displaySrgb,
							visualizer_srgb: resolved.gel.embeddedFallback.visualizerSrgb,
						},
					}
				: resolved.gel.type === "custom"
					? {
							type: "custom",
							name: resolved.gel.name,
							color_srgb: resolved.gel.colorSrgb,
							note: resolved.gel.note,
						}
					: { type: "open_white" },
		shaper_angles_degrees: [...resolved.shaperAnglesDegrees],
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
		definition.profile_snapshot?.modes.find(
			(mode) => mode.id === definition.mode_id,
		)?.splits[0]?.number ?? 1
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
