import { expect } from "@playwright/test";
import { readPatchSnapshot } from "../../support/operator/patch";
import { replaceProgrammingSelection } from "../command-selection/programmingSelection";
import type { ApiDriver } from "../core/api";
import type { LightBench } from "../core/lightBench";
import { batchProgrammerValues } from "../programmer/programmerValues";

/**
 * Installed (per physical instance) calibration of patched fixtures, driven through the
 * authoritative Patch update route that the desk's Pan/Tilt and Color calibration dialogs use
 * (`POST /api/v2/patch/fixtures/{id}/update`), and observed on the logical DMX output.
 *
 * The rig is one Cameo AURO SPOT Z300 (20-Channel): Pan U16 on slots 1/2 across ±270°, Tilt U16
 * on slots 3/4 across ±135°, and a nominal Position physical graph, so semantic Position Angles
 * are fitted through the installed Position calibration.
 */
export const MOVER = {
	manufacturer: "Cameo",
	profile: "AURO SPOT Z300",
	mode: "20-Channel",
	footprint: 20,
} as const;

export const PAN_RANGE = { min: -270, max: 270 } as const;
export const TILT_RANGE = { min: -135, max: 135 } as const;

/** Literal JSON text, for payloads such as `1e999` that JSON.stringify cannot produce. */
export class RawJson {
	constructor(readonly text: string) {}
}

export interface RawResponse<T = unknown> {
	status: number;
	body: T;
}

/** An authenticated request that returns rejections instead of throwing on them. */
export async function rawRequest<T = any>(
	api: ApiDriver,
	method: string,
	path: string,
	body?: unknown,
	headers: Record<string, string> = {},
): Promise<RawResponse<T>> {
	if (!api.session) throw new Error("API session is not initialized");
	const response = await fetch(`${api.baseUrl}${path}`, {
		method,
		headers: {
			authorization: `Bearer ${api.session.token}`,
			...(body === undefined ? {} : { "content-type": "application/json" }),
			...headers,
		},
		// A `RawJson` body is sent verbatim, e.g. a number JSON.stringify cannot write.
		body: body === undefined ? undefined : body instanceof RawJson ? body.text : JSON.stringify(body),
	});
	const text = await response.text();
	let parsed: unknown = text;
	try {
		parsed = text ? JSON.parse(text) : null;
	} catch {
		// Non-JSON bodies (archives, plain errors) stay text.
	}
	return { status: response.status, body: parsed as T };
}

export interface MoverRig {
	showId: string;
	fixtureIds: string[];
}

export interface PatchedProfile {
	id: string;
	revision: number;
	modeId: string;
	footprint: number;
	/** Physical multi-patch copies patched directly after the root fixture. */
	copies?: number;
}

const INSTALLED_APPEARANCE = {
	light_source: { type: "profile_default" },
	color_temperature_kelvin: null,
	luminous_output_lumens: null,
	gel: { type: "open_white" },
	shaper_angles_degrees: [0, 0, 0, 0],
};

/** The library profile revision and mode one shipped fixture resolves to. */
export async function libraryProfile(
	api: ApiDriver,
	manufacturer: string,
	name: string,
	mode: string,
	footprint: number,
): Promise<PatchedProfile & { profile: Record<string, any> }> {
	const library = await api.request<{ profiles: Array<Record<string, any>> }>(
		"GET",
		"/api/v2/fixture-library/profiles",
	);
	const profile = library.profiles.find(
		(candidate) => candidate.manufacturer === manufacturer && candidate.name === name,
	);
	const found = profile?.modes.find((candidate: { name: string }) => candidate.name === mode);
	if (!profile || !found) throw new Error(`The library has no ${manufacturer} ${name} ${mode}`);
	return { id: profile.id, revision: profile.revision, modeId: found.id, footprint, profile };
}

/**
 * A fresh show with one fixture per entry patched back to back from 1.1, selected, and an empty
 * Programmer without fades.
 */
export async function arrangeFixtures(
	api: ApiDriver,
	bench: LightBench,
	label: string,
	entries: PatchedProfile[],
): Promise<MoverRig & { addresses: number[]; copyIds: string[][]; copyAddresses: number[][] }> {
	await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
	const show = await api.createShow<{ id: string }>({
		name: `FIXTURE-PHYSICAL ${label} ${crypto.randomUUID()}`,
	});
	await api.openShow(show.id, { transition: "hold_current" });
	const fixtureIds = entries.map(() => crypto.randomUUID());
	const addresses: number[] = [];
	const copyIds: string[][] = [];
	const copyAddresses: number[][] = [];
	let next = 1;
	for (const entry of entries) {
		addresses.push(next);
		next += entry.footprint;
		const copies = Array.from({ length: entry.copies ?? 0 }, () => crypto.randomUUID());
		copyIds.push(copies);
		copyAddresses.push(
			copies.map(() => {
				const address = next;
				next += entry.footprint;
				return address;
			}),
		);
	}
	const snapshot = await readPatchSnapshot(api, show.id);
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{
			request_id: crypto.randomUUID(),
			fixtures: entries.map((entry, index) => ({
				fixture_id: fixtureIds[index],
				fixture_number: index + 1,
				virtual_fixture_number: null,
				name: `Fixture ${index + 1}`,
				profile_id: entry.id,
				profile_revision: entry.revision,
				mode_id: entry.modeId,
				split_patches: [{ split: 1, universe: 1, address: addresses[index] }],
				layer_id: "default",
				direct_control: null,
				location: { x: index * 1000, y: 0, z: 0 },
				rotation: { x: 0, y: 0, z: 0 },
				multipatch: copyIds[index].map((id, copy) => ({
					id,
					name: `Fixture ${index + 1} copy ${copy + 1}`,
					split_patches: [{ split: 1, universe: 1, address: copyAddresses[index][copy] }],
					location: { x: index * 1000, y: (copy + 1) * 1000, z: 0 },
					rotation: { x: 0, y: 0, z: 0 },
					invert_pan: false,
					invert_tilt: false,
					bracket_angle: 0,
					shaper_angle: null,
					installed_appearance: INSTALLED_APPEARANCE,
				})),
				move_in_black_enabled: false,
				move_in_black_delay_millis: 0,
				highlight_overrides: [],
			})),
			remove_fixture_ids: [],
		},
		true,
		snapshot.patch_revision,
	);
	await replaceProgrammingSelection(api, { surface: "api", showId: show.id, fixtures: fixtureIds });
	await bench.tick(25);
	return { showId: show.id, fixtureIds, addresses, copyIds, copyAddresses };
}

/** A fresh show with `count` AURO SPOTs patched back to back from 1.1 and an empty Programmer. */
export async function arrangeMovers(
	api: ApiDriver,
	bench: LightBench,
	label: string,
	count = 1,
): Promise<MoverRig> {
	const mover = await libraryProfile(api, MOVER.manufacturer, MOVER.profile, MOVER.mode, MOVER.footprint);
	return arrangeFixtures(api, bench, label, Array.from({ length: count }, () => mover));
}

/** Whether the runtime publishes semantic pages of `family` for these fixtures. */
export async function publishesSemanticFamily(api: ApiDriver, fixtureIds: string[], family: string) {
	const pages = await api
		.request<{ semantic: boolean; families: Array<{ family: string }> }>(
			"GET",
			`/api/v2/programming/family-encoder-pages?fixture_ids=${fixtureIds.join(",")}`,
		)
		.catch(() => null);
	return Boolean(pages?.semantic && pages.families.some((group) => group.family === family));
}

/** Whether the runtime publishes semantic Position pages for these fixtures. */
export function publishesSemanticPosition(api: ApiDriver, fixtureIds: string[]) {
	return publishesSemanticFamily(api, fixtureIds, "position");
}

export async function patchFixture(api: ApiDriver, showId: string, fixtureId: string) {
	const snapshot = await readPatchSnapshot(api, showId);
	const fixture = snapshot.fixtures.find((candidate) => candidate.fixture_id === fixtureId);
	if (!fixture) throw new Error(`Fixture ${fixtureId} is absent from the patch`);
	return { snapshot, fixture: fixture as typeof fixture & Record<string, any> };
}

/** The Patch projection of one referenced profile mode, with its server-derived identities. */
export async function referencedMode(api: ApiDriver, showId: string, profile: PatchedProfile) {
	const snapshot = await readPatchSnapshot(api, showId);
	const revision = snapshot.profile_revisions.find(
		(candidate) => candidate.profile_id === profile.id && candidate.profile_revision === profile.revision,
	);
	const mode = revision?.referenced_modes.find((candidate) => candidate.mode_id === profile.modeId);
	if (!mode) throw new Error(`The patch does not reference ${profile.id} mode ${profile.modeId}`);
	return mode;
}

/**
 * One sparse installed-settings update of the root (`copyId` null) or one multi-patch copy,
 * exactly as the desk's Patch dialogs send it, with the current revisions.
 */
export async function updateInstalledFixture(
	api: ApiDriver,
	showId: string,
	fixtureId: string,
	action: Record<string, unknown>,
	copyId: string | null = null,
	requestId: string = crypto.randomUUID(),
	encode: (body: Record<string, unknown>) => unknown = (body) => body,
): Promise<RawResponse> {
	const send = await prepareInstalledUpdate(api, showId, fixtureId, action, copyId, requestId, encode);
	return send();
}

/**
 * Freezes one installed-settings request against the current revisions; every call of the
 * returned function sends exactly the same request again (a replay).
 */
export async function prepareInstalledUpdate(
	api: ApiDriver,
	showId: string,
	fixtureId: string,
	action: Record<string, unknown>,
	copyId: string | null = null,
	requestId: string = crypto.randomUUID(),
	encode: (body: Record<string, unknown>) => unknown = (body) => body,
): Promise<() => Promise<RawResponse>> {
	const { snapshot, fixture } = await patchFixture(api, showId, fixtureId);
	const body = encode({
		request_id: requestId,
		expected_fixture_revision: fixture.fixture_revision,
		expected_patch_revision: snapshot.patch_revision,
		expected_show_revision: snapshot.show_revision,
		multipatch_instance_id: copyId,
		...action,
	});
	return () =>
		rawRequest(api, "POST", `/api/v2/patch/fixtures/${fixtureId}/update`, body, {
			"if-match": String(snapshot.patch_revision),
			"x-tosk-show": showId,
		});
}

export function positionCalibration(
	panZero: number,
	tiltZero: number,
	extra: Record<string, unknown> = {},
) {
	return {
		revision: 1,
		quality: "estimated",
		source: "FIXTURE-INSTALLATION bench survey",
		pan_zero_degrees: panZero,
		tilt_zero_degrees: tiltZero,
		...extra,
	};
}

/** Programs semantic Position Angles (degrees) for every fixture, without a fade. */
export async function programAngles(
	api: ApiDriver,
	showId: string,
	fixtureIds: string[],
	pan: number,
	tilt: number,
) {
	await batchProgrammerValues(api, {
		surface: "api",
		showId,
		mutations: fixtureIds.map((fixtureId) => ({
			action: "set_fixture",
			fixtureId,
			attribute: "position",
			value: {
				kind: "position",
				value: {
					kind: "angles",
					pan_degrees: { kind: "value", value: pan },
					tilt_degrees: { kind: "value", value: tilt },
				},
			} as never,
			timing: { fade: false, fadeMillis: null, delayMillis: null },
		})),
	});
}

/** Programs one semantic Zoom opening (degrees, Beam convention) for every fixture. */
export async function programZoom(api: ApiDriver, showId: string, fixtureIds: string[], degrees: number) {
	await batchProgrammerValues(api, {
		surface: "api",
		showId,
		mutations: fixtureIds.map((fixtureId) => ({
			action: "set_fixture",
			fixtureId,
			attribute: "zoom",
			value: {
				kind: "zoom",
				value: { opening_degrees: { kind: "value", value: degrees }, convention: "beam" },
			} as never,
			timing: { fade: false, fadeMillis: null, delayMillis: null },
		})),
	});
}

/** One logical DMX byte on universe 1. */
export async function emittedSlot(api: ApiDriver, address: number) {
	const snapshot = await api.request<{ universes: Array<{ universe: number; slots: number[] }> }>(
		"GET",
		"/api/v2/output/dmx",
		undefined,
		false,
	);
	return snapshot.universes.find((candidate) => candidate.universe === 1)?.slots[address - 1] ?? 0;
}

/** The U16 Pan and Tilt words one mover currently emits on universe 1. */
export async function emittedPanTilt(api: ApiDriver, start: number) {
	const snapshot = await api.request<{ universes: Array<{ universe: number; slots: number[] }> }>(
		"GET",
		"/api/v2/output/dmx",
		undefined,
		false,
	);
	const slots = snapshot.universes.find((candidate) => candidate.universe === 1)?.slots ?? [];
	const word = (offset: number) =>
		(slots[start - 1 + offset] ?? 0) * 256 + (slots[start + offset] ?? 0);
	return { pan: word(0), tilt: word(2) };
}

/** The raw U16 word a linear axis emits for one mechanical angle. */
export function u16For(range: { min: number; max: number }, degrees: number) {
	return Math.round(((degrees - range.min) / (range.max - range.min)) * 65535);
}

/** Polls the emitted words after each manual clock frame until they match within ±1 raw step. */
export async function expectPanTilt(
	api: ApiDriver,
	bench: LightBench,
	start: number,
	expected: { pan: number; tilt: number },
) {
	await expect
		.poll(async () => {
			await bench.tick(25);
			const actual = await emittedPanTilt(api, start);
			return Math.abs(actual.pan - expected.pan) <= 1 && Math.abs(actual.tilt - expected.tilt) <= 1
				? "match"
				: JSON.stringify({ actual, expected });
		})
		.toBe("match");
}
