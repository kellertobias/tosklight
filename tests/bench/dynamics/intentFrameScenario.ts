import { expect } from "@playwright/test";
import { HttpPresetRecordingTransport } from "../../../apps/light-desktop/src/api/PresetRecordingTransport";
import { replaceProgrammingSelection } from "../command-selection/programmingSelection";
import type { ApiDriver } from "../core/api";
import { requireSemanticContract } from "../core/semanticContract";
import { recallPreset } from "../groups-presets/presetRecall";
import { clearProgrammerValues } from "../programmer/programmerValues";
import { readPatchSnapshot } from "../../support/operator/patch";

/**
 * API-level helpers of docs/testing/32-intention-programming-frame-contract.md: a fresh show with
 * shipped library fixtures, typed Programmer values written as the desk writes them, Presets,
 * Preload and Undo. Dynamics, Playbacks and published frames live in `intentFrameOutput.ts`.
 */

export interface LibraryFixture {
	readonly manufacturer: string;
	readonly profile: string;
	readonly mode: string;
}

export interface RigFixture extends LibraryFixture {
	readonly number: number;
	readonly address: string;
	/** Stage location in millimetres; the origin when absent. */
	readonly location?: { x: number; y: number; z: number };
}

export interface IntentRig {
	readonly showId: string;
	readonly ids: Record<number, string>;
}

/** Cameo AURO SPOT Z300: a calibrated mover (nominal Position graph) with a colour wheel. */
export const SPOT = { manufacturer: "Cameo", profile: "AURO SPOT Z300", mode: "20-Channel" } as const;
/** ROBE Robin 300 LEDWash: a different mover, Pan 450° / Tilt 300°, used as the replacement. */
export const WASH = { manufacturer: "ROBE", profile: "Robin 300 LEDWash", mode: "Mode 3" } as const;
export const POINT = { manufacturer: "ToskLight", profile: "3D Point", mode: "Full 16 bit" } as const;
/** Dimmer-first Generic LEDs: Dimmer, then the colour controls, one byte each. */
export const RGB = { manufacturer: "Generic", profile: "RGB LED", mode: "DRGB 8-bit dimmer first" } as const;
export const RGBW = { manufacturer: "Generic", profile: "RGBW LED", mode: "DRGBW 8-bit dimmer first" } as const;
export const CMY = { manufacturer: "Generic", profile: "CMY LED", mode: "DCMY 8-bit dimmer first" } as const;

const GATE =
	"semantic programming contract is not enabled on this runtime (an older contract-0 server; run npm run test:e2e-semantic)";

/** A fresh, open show with zero Programmer and Cue fades and the given library fixtures. */
export async function arrangeRig(
	api: ApiDriver,
	label: string,
	fixtures: readonly RigFixture[],
): Promise<IntentRig> {
	await api.request("PUT", "/api/v2/configuration", {
		programmer_fade_millis: 0,
		sequence_master_fade_millis: 0,
	});
	const show = await api.createShow<{ id: string }>({
		name: `INTENT-FRAME ${label} ${crypto.randomUUID()}`,
	});
	await api.openShow(show.id, { transition: "hold_current" });
	await expect.poll(() => activeShow(api).then((active) => active?.id)).toBe(show.id);
	await patchFixtures(
		api,
		await Promise.all(fixtures.map((fixture) => fixtureInput(api, crypto.randomUUID(), fixture))),
	);
	const ids = Object.fromEntries(
		(await api.patch()).fixtures.map((fixture) => [fixture.fixture_number, fixture.fixture_id]),
	) as Record<number, string>;
	for (const fixture of fixtures)
		if (!ids[fixture.number]) throw new Error(`Fixture ${fixture.number} was not patched`);
	return { showId: show.id, ids };
}

/** Skips (or, in the e2e-semantic project, fails) unless the runtime publishes semantic pages. */
export async function requireSemanticFamilies(api: ApiDriver, fixtureIds: readonly string[]) {
	const pages = await api
		.request<{ semantic: boolean }>(
			"GET",
			`/api/v2/programming/family-encoder-pages?fixture_ids=${fixtureIds.join(",")}`,
		)
		.catch(() => null);
	requireSemanticContract(Boolean(pages?.semantic), GATE);
}

export async function activeShow(api: ApiDriver) {
	const bootstrap = await api.request<{ active_show: { id: string; revision: number } | null }>(
		"GET",
		"/api/v2/bootstrap",
	);
	return bootstrap.active_show;
}

/** The Patch row of one library fixture; reusing `fixtureId` replaces that fixture's type. */
export async function fixtureInput(api: ApiDriver, fixtureId: string, fixture: RigFixture) {
	const library = await api.request<{
		profiles: Array<{
			id: string;
			revision: number;
			manufacturer: string;
			name: string;
			modes: Array<{ id: string; name: string }>;
		}>;
	}>("GET", "/api/v2/fixture-library/profiles");
	const profile = library.profiles.find(
		(candidate) =>
			candidate.manufacturer === fixture.manufacturer && candidate.name === fixture.profile,
	);
	const mode = profile?.modes.find((candidate) => candidate.name === fixture.mode);
	if (!profile || !mode)
		throw new Error(`The library has no ${fixture.manufacturer} ${fixture.profile} ${fixture.mode}`);
	const [universe, address] = fixture.address.split(".").map(Number);
	return {
		fixture_id: fixtureId,
		fixture_number: fixture.number,
		virtual_fixture_number: null,
		name: `${fixture.profile} ${fixture.number}`,
		profile_id: profile.id,
		profile_revision: profile.revision,
		mode_id: mode.id,
		split_patches: [{ split: 1, universe, address }],
		layer_id: "default",
		direct_control: null,
		location: fixture.location ?? { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		multipatch: [],
		move_in_black_enabled: false,
		move_in_black_delay_millis: 0,
		highlight_overrides: [],
	};
}

export async function patchFixtures(api: ApiDriver, fixtures: unknown[], remove: string[] = []) {
	const snapshot = await readPatchSnapshot(api);
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{ request_id: crypto.randomUUID(), fixtures, remove_fixture_ids: remove },
		true,
		snapshot.patch_revision,
	);
}

/** Replaces one fixture's type in place: same fixture identity, number and address. */
export async function replaceFixture(api: ApiDriver, fixtureId: string, fixture: RigFixture) {
	await patchFixtures(api, [await fixtureInput(api, fixtureId, fixture)]);
}

export function select(api: ApiDriver, rig: IntentRig, fixtureIds: readonly string[]) {
	return replaceProgrammingSelection(api, {
		surface: "api",
		showId: rig.showId,
		fixtures: fixtureIds,
	});
}

export function clearProgrammer(api: ApiDriver, rig: IntentRig) {
	return clearProgrammerValues(api, { surface: "api", showId: rig.showId });
}

// ---------------------------------------------------------------------------------------------
// Programmer values (raw wire actions, as the desk sends them)

type Lane = "normal" | "preload";

async function laneRevision(api: ApiDriver, lane: Lane) {
	const snapshot = await api.request<{ projection: { revision: number } }>(
		"GET",
		lane === "normal"
			? "/api/v2/programmer/values/snapshot"
			: "/api/v2/programmer/preload-values/snapshot",
	);
	return snapshot.projection.revision;
}

export function programmerRevision(api: ApiDriver) {
	return laneRevision(api, "normal");
}

/** One Programmer values action on the normal or the Preload lane. */
export async function valuesAction(
	api: ApiDriver,
	action: Record<string, unknown>,
	lane: Lane = "normal",
) {
	const capture = await api.request<{ projection: { revision: number } }>(
		"GET",
		"/api/v2/programmer/capture-mode/snapshot",
	);
	return api.request<{ status: string } & Record<string, unknown>>(
		"POST",
		lane === "normal"
			? "/api/v2/programmer/values/actions"
			: "/api/v2/programmer/preload-values/actions",
		{
			request_id: crypto.randomUUID(),
			expected_revision: await laneRevision(api, lane),
			expected_capture_mode_revision: capture.projection.revision,
			action,
		},
	);
}

const scalar = (value: number) => ({ kind: "value", value });

export function angles(pan: number, tilt: number) {
	return {
		kind: "position",
		value: { kind: "angles", pan_degrees: scalar(pan), tilt_degrees: scalar(tilt) },
	};
}

export function pointTarget(pointId: string) {
	return {
		kind: "position",
		value: {
			kind: "target",
			reference: { kind: "point", point_id: pointId },
			offset_metres: [scalar(0), scalar(0), scalar(0)],
		},
	};
}

/** The same whole value on each fixture, applied as one batch. */
export async function setEach(
	api: ApiDriver,
	fixtureIds: readonly string[],
	attribute: string,
	value: unknown,
	lane: Lane = "normal",
) {
	const outcome = await valuesAction(
		api,
		{
			type: "batch",
			mutations: fixtureIds.map((fixture_id) => ({
				type: "set_fixture",
				fixture_id,
				attribute,
				value,
				timing: { fade: false },
			})),
		},
		lane,
	);
	expect(outcome.status, JSON.stringify(outcome)).toBe("changed");
}

export function setAngles(
	api: ApiDriver,
	fixtureIds: readonly string[],
	pan: number,
	tilt: number,
	lane: Lane = "normal",
) {
	return setEach(api, fixtureIds, "position", angles(pan, tilt), lane);
}

export function setIntensity(api: ApiDriver, fixtureIds: readonly string[], level: number) {
	return setEach(api, fixtureIds, "intensity", { kind: "normalized", value: level });
}

/** An ordinary encoder edit of one Position component (degrees, or metres for a Target axis). */
export function editPosition(
	api: ApiDriver,
	fixtureIds: readonly string[],
	component: "pan" | "tilt" | "target_x",
	value: number,
	operation: "set" | "relative" = "set",
	lane: Lane = "normal",
) {
	return valuesAction(
		api,
		{
			type: "apply_intent",
			fixture_ids: fixtureIds,
			attribute: "position",
			operation: {
				type: "component_edits",
				edits: [
					{
						kind: "scalar",
						component: { kind: component },
						operation:
							operation === "set" ? { kind: "set", value: scalar(value) } : { kind: "relative", value },
					},
				],
			},
			timing: { fade: false },
		},
		lane,
	);
}

/** A complete semantic white: the start the Color dialog edits from. */
const SEMANTIC_WHITE = {
	kind: "color_program",
	value: {
		kind: "semantic",
		intent: {
			base_xyz: { x: 0.95047, y: 1, z: 1.08883 },
			recipe: { version: 1, rgb: [1, 1, 1], amber: 0, approximate: false },
			white_blend: 0,
			white_target: { kelvin: 6500, duv: 0 },
			uv: { amount: 0 },
			relative_output: 1,
			allocation: "preserve_recipe",
		},
	},
};

export function colorEdit(component: string, value: number) {
	return {
		kind: "scalar",
		component: { kind: "color", component },
		operation: { kind: "set", value: scalar(value) },
	};
}

/** The Color dialog's write: a whole semantic white, then the given component edits at once. */
export async function programColor(api: ApiDriver, fixtureIds: readonly string[], edits: unknown[]) {
	await setEach(api, fixtureIds, "color", SEMANTIC_WHITE);
	const outcome = await valuesAction(api, {
		type: "apply_intent",
		fixture_ids: fixtureIds,
		attribute: "color",
		operation: { type: "component_edits", edits },
		timing: { fade: false },
	});
	expect(outcome.status, JSON.stringify(outcome)).toBe("changed");
}

export interface ProgrammerValue {
	fixture_id: string;
	attribute: string;
	value: { kind: string; value: { kind: string } & Record<string, unknown> };
}

export async function programmerValues(api: ApiDriver, attribute?: string) {
	const snapshot = await api.request<{ projection: { fixture_values?: ProgrammerValue[] } }>(
		"GET",
		"/api/v2/programmer/values/snapshot",
	);
	return (snapshot.projection.fixture_values ?? []).filter(
		(value) => attribute === undefined || value.attribute === attribute,
	);
}

/** The Programmer's Undo, as the desk's UNDO key sends it. */
export async function undo(api: ApiDriver, rig: IntentRig) {
	const session = api.session;
	if (!session) throw new Error("API session is not initialized");
	return api.request<{ changed: boolean }>(
		"GET",
		"/api/v2/programmer-undo/actions",
		undefined,
		true,
		undefined,
		{ showId: rig.showId, deskId: session.desk.id },
	);
}

// ---------------------------------------------------------------------------------------------
// Presets

const FAMILY_NUMBER = { Color: 2, Position: 3 } as const;
type Family = keyof typeof FAMILY_NUMBER;

export async function recordPreset(api: ApiDriver, rig: IntentRig, family: Family, number: number) {
	const before = await api.showObject(rig.showId, "preset", `${FAMILY_NUMBER[family]}.${number}`);
	const session = api.session;
	if (!session) throw new Error("API session is not initialized");
	return new HttpPresetRecordingTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
	}).record(rig.showId, {
		requestId: crypto.randomUUID(),
		address: { family, number },
		name: `${family} ${number}`,
		mode: "overwrite",
		expectedObjectRevision: before?.revision ?? 0,
	});
}

export function recall(api: ApiDriver, rig: IntentRig, family: Family, number: number) {
	return recallPreset(api, {
		surface: "api",
		showId: rig.showId,
		preset: { objectId: `${FAMILY_NUMBER[family]}.${number}`, family, number },
	});
}

export async function presetBody<T = Record<string, unknown>>(
	api: ApiDriver,
	rig: IntentRig,
	family: Family,
	number: number,
) {
	const preset = await api.showObject<T>(rig.showId, "preset", `${FAMILY_NUMBER[family]}.${number}`);
	if (!preset) throw new Error(`${family} ${number} is not stored`);
	return preset.body;
}

/** Records the Programmer as a new Cue onto a pool Playback through the command line. */
export async function recordPlayback(api: ApiDriver, playback: number) {
	const outcome = await api.executeCommandLineRaw(`RECORD PBK ${playback}`);
	expect(outcome, JSON.stringify(outcome)).toMatchObject({ outcome: "accepted" });
}

export interface StoredCue {
	changes: Array<{ fixture_id: string; attribute: string; value: unknown }>;
	group_changes?: Array<{ group_id: string; attribute: string; value: unknown }>;
}

/** Every stored Cue of the show, Cuelist by Cuelist. */
export async function storedCues(api: ApiDriver, rig: IntentRig) {
	const lists = await api.request<{ objects: Array<{ body: { cues: StoredCue[] } }> }>(
		"GET",
		"/api/v2/objects/cue_list",
		undefined,
		true,
		undefined,
		{ showId: rig.showId },
	);
	return lists.objects.flatMap((list) => list.body.cues);
}
