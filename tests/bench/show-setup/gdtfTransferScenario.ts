import { strFromU8, strToU8, unzipSync, zipSync } from "fflate";
import { readPatchSnapshot } from "../../support/operator/patch";
import type { ApiDriver } from "../core/api";
import { type RawResponse, rawRequest } from "./installedCalibrationScenario";

/**
 * Fixture profile ↔ GDTF/MVR transfers through the desk's own routes: show MVR export
 * (`GET /api/v2/shows/{id}/mvr` and its `/preview`), GDTF preview/import
 * (`POST /api/v2/fixture-library/gdtf/preview`, `POST …/profiles/{id}/update`) and MVR import
 * preview (`POST /api/v2/mvr/imports/preview`). Archives are inspected as the ZIPs they are.
 */

type Json = Record<string, any>;

function channel(
	head: string,
	attribute: string,
	resolution: string,
	secondarySlots: number[],
	defaultRaw: number,
	range: [number, number],
	functions: Json[],
): Json {
	return {
		id: crypto.randomUUID(),
		head_id: head,
		split: 1,
		fixture_attribute: attribute,
		attribute,
		canonical_transform: "identity",
		resolution,
		secondary_slots: secondarySlots,
		default_raw: defaultRaw,
		highlight_raw: 0,
		physical_min: range[0],
		physical_max: range[1],
		unit: "degrees",
		invert: false,
		snap: false,
		reacts_to_virtual_intensity: false,
		virtual_intensity_inverted: false,
		reacts_to_sequence_master: false,
		reacts_to_group_master: false,
		reacts_to_grand_master: false,
		behavior: "controlled",
		functions,
	};
}

function continuous(name: string, attribute: string, from: number, to: number, physical: [number, number]): Json {
	return {
		id: crypto.randomUUID(),
		name,
		dmx_from: from,
		dmx_to: to,
		attribute,
		priority: 0,
		behavior: { type: "continuous", physical_min: physical[0], physical_max: physical[1], unit: "degrees" },
	};
}

/**
 * The FIXTURE-GDTF-001/002 profile: U16 Pan on slots 1 and 3 (fine byte separated by a spare
 * slot), default 32769, directed +540° → −540°; U8 Zoom on slot 2 with 4°→50° on raw 0–99, an
 * unassigned gap 100–149 and a descending 50°→4° function on 150–255.
 */
export function gdtfProbeProfile(name: string): Json {
	const head = crypto.randomUUID();
	return {
		schema_version: 2,
		id: crypto.randomUUID(),
		revision: 1,
		manufacturer: "E2E Physical",
		name,
		short_name: "",
		fixture_type: "other",
		patch_policy: "dmx",
		notes: "",
		photograph_asset: null,
		stage_icon_asset: null,
		model_asset: null,
		model_units: "auto",
		physical: {
			width_millimetres: null,
			height_millimetres: null,
			depth_millimetres: null,
			weight_kilograms: null,
			power_watts: null,
			connectors: "",
			light_source: "",
			color_rendering_index: null,
			lens: "",
		},
		optics: {},
		geometry: { nodes: [], emitters: [] },
		modes: [
			{
				id: crypto.randomUUID(),
				name: "Precise",
				notes: "",
				splits: [{ number: 1, footprint: 3 }],
				heads: [{ id: head, name: "Main", master_shared: true }],
				channels: [
					channel(head, "pan", "u16", [3], 32769, [-540, 540], [continuous("Pan", "pan", 0, 65535, [540, -540])]),
					channel(head, "zoom", "u8", [], 0, [4, 50], [
						continuous("Zoom", "zoom", 0, 99, [4, 50]),
						continuous("Zoom wide", "zoom", 150, 255, [50, 4]),
					]),
				],
				color_systems: [],
				control_actions: [],
				emitter_heads: [],
				geometry: { nodes: [], emitters: [] },
			},
		],
		hazardous: false,
		direct_control_protocols: [],
		signal_loss_policy: { type: "hold_last" },
		reserved_source: null,
	};
}

/** A fresh, opened show. */
export async function openFreshShow(api: ApiDriver, label: string): Promise<string> {
	const show = await api.createShow<{ id: string }>({ name: `FIXTURE-GDTF ${label} ${crypto.randomUUID()}` });
	await api.openShow(show.id, { transition: "hold_current" });
	return show.id;
}

/** Patches `count` fixtures of one profile revision back to back on universe 1. */
export async function patchProfile(
	api: ApiDriver,
	showId: string,
	profile: { id: string; revision: number; modeId: string; footprint: number },
	count: number,
	firstNumber = 1,
) {
	const snapshot = await readPatchSnapshot(api, showId);
	const fixtures = Array.from({ length: count }, (_, index) => {
		const slot = firstNumber - 1 + index;
		const perUniverse = Math.floor(512 / profile.footprint);
		return {
			fixture_id: crypto.randomUUID(),
			fixture_number: firstNumber + index,
			virtual_fixture_number: null,
			name: `Physical ${firstNumber + index}`,
			profile_id: profile.id,
			profile_revision: profile.revision,
			mode_id: profile.modeId,
			split_patches: [
				{
					split: 1,
					universe: 1 + Math.floor(slot / perUniverse),
					address: (slot % perUniverse) * profile.footprint + 1,
				},
			],
			layer_id: "default",
			direct_control: null,
			location: { x: index * 100, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
			multipatch: [],
			move_in_black_enabled: false,
			move_in_black_delay_millis: 0,
			highlight_overrides: [],
		};
	});
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{ request_id: crypto.randomUUID(), fixtures, remove_fixture_ids: [] },
		true,
		snapshot.patch_revision,
		{ showId },
	);
	return fixtures.map((fixture) => fixture.fixture_id);
}

export interface MvrExportPreview {
	fixtures: number;
	embedded_profiles: number;
	missing_profiles: string[];
	warnings: string[];
}

export function mvrExportPreview(api: ApiDriver, showId: string) {
	return api.request<MvrExportPreview>("GET", `/api/v2/shows/${showId}/mvr/preview`);
}

/** The exported MVR of one show, as its raw bytes and its ZIP members. */
export async function exportMvr(api: ApiDriver, showId: string) {
	if (!api.session) throw new Error("API session is not initialized");
	const response = await fetch(`${api.baseUrl}/api/v2/shows/${showId}/mvr`, {
		headers: { authorization: `Bearer ${api.session.token}` },
	});
	if (!response.ok) throw new Error(`MVR export returned ${response.status}: ${await response.text()}`);
	const bytes = new Uint8Array(await response.arrayBuffer());
	return { bytes, members: unzipSync(bytes) };
}

export function gdtfMembers(members: Record<string, Uint8Array>) {
	return Object.entries(members).filter(([name]) => name.toLowerCase().endsWith(".gdtf"));
}

export function gdtfDescription(archive: Uint8Array): string {
	return strFromU8(unzipSync(archive)["description.xml"]);
}

/** The same GDTF with another FixtureTypeID, so it can be imported beside its origin profile. */
export function withFixtureTypeId(archive: Uint8Array, fixtureTypeId: string): Uint8Array {
	const members = unzipSync(archive);
	const xml = strFromU8(members["description.xml"]).replace(
		/FixtureTypeID="[^"]*"/u,
		`FixtureTypeID="${fixtureTypeId}"`,
	);
	return zipSync({ ...members, "description.xml": strToU8(xml) });
}

/** The same GDTF with one more archive member, as a manufacturer's richer original would carry. */
export function withExtraMember(archive: Uint8Array, name: string, text: string): Uint8Array {
	return zipSync({ ...unzipSync(archive), [name]: strToU8(text) });
}

export function base64(bytes: Uint8Array) {
	return Buffer.from(bytes).toString("base64");
}

export function previewGdtf(api: ApiDriver, archive: Uint8Array): Promise<RawResponse<Json>> {
	return rawRequest(api, "POST", "/api/v2/fixture-library/gdtf/preview", { source_base64: base64(archive) });
}

export function importGdtf(
	api: ApiDriver,
	profileId: string,
	archive: Uint8Array,
	options: { requestId?: string; expectedRevision?: number; attributeMappings?: unknown[] } = {},
): Promise<RawResponse<Json>> {
	return rawRequest(api, "POST", `/api/v2/fixture-library/profiles/${profileId}/update`, {
		request_id: options.requestId ?? crypto.randomUUID(),
		expected_revision: options.expectedRevision ?? 0,
		source_base64: base64(archive),
		attribute_mappings: options.attributeMappings ?? [],
	});
}

export async function previewMvrImport(api: ApiDriver, archive: Uint8Array): Promise<RawResponse<Json>> {
	if (!api.session) throw new Error("API session is not initialized");
	const response = await fetch(`${api.baseUrl}/api/v2/mvr/imports/preview`, {
		method: "POST",
		headers: { authorization: `Bearer ${api.session.token}`, "content-type": "application/zip" },
		body: Buffer.from(archive),
	});
	const text = await response.text();
	return { status: response.status, body: text ? JSON.parse(text) : null };
}

/** Every revision number of every library profile, as a stable signature. */
export async function librarySignature(api: ApiDriver) {
	const { profiles } = await api.fixtureProfilesSnapshot();
	return (profiles as Json[]).map((profile) => `${profile.id}:${profile.revision}`).sort();
}

/** Exported package bytes of one profile revision. */
export async function exportPackage(api: ApiDriver, id: string, revision: number) {
	if (!api.session) throw new Error("API session is not initialized");
	const response = await fetch(
		`${api.baseUrl}/api/v2/fixture-library/profiles/${id}/revisions/${revision}/package`,
		{ headers: { authorization: `Bearer ${api.session.token}` } },
	);
	if (!response.ok) throw new Error(`Package export returned ${response.status}: ${await response.text()}`);
	return new Uint8Array(await response.arrayBuffer());
}

export function packageProfile(archive: Uint8Array): Json {
	return JSON.parse(strFromU8(unzipSync(archive)["fixture.json"])).profile;
}

export async function deleteProfileRevision(api: ApiDriver, id: string, revision: number) {
	await api.fixtureLibraryAction({ type: "delete_profile_revision", profile_id: id, revision });
}

export async function importPackage(api: ApiDriver, archive: Uint8Array) {
	return api.fixtureLibraryAction({
		type: "import_package",
		package_base64: base64(archive),
		attribute_mappings: [],
	});
}
