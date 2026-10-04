import { expect, type Locator, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import { rawRequest } from "./installedCalibrationScenario";

/**
 * The desk's Fixture Library profile editor (Setup → Open Fixture Library), against the real
 * library of the bench server. Profiles are seeded through `save_profile` so every case starts
 * from one exactly known Zoom function; everything the scenario authors happens in the editor.
 */

export interface SeededProfile {
	id: string;
	manufacturer: string;
	name: string;
	modeId: string;
	channelId: string;
	functionId: string;
}

export type JsonProfile = Record<string, any>;

/**
 * One U8 Zoom channel whose only Continuous function runs raw 0–255 from 44° down to 8°, with no
 * optional calibration: the FIXTURE-MAPPING-001 starting point.
 */
export function zoomProbeProfile(name: string, overrides: { physicalMapping?: unknown } = {}) {
	const head = crypto.randomUUID();
	const modeId = crypto.randomUUID();
	const channelId = crypto.randomUUID();
	const functionId = crypto.randomUUID();
	const fn: Record<string, unknown> = {
		id: functionId,
		name: "Zoom",
		dmx_from: 0,
		dmx_to: 255,
		attribute: "zoom",
		priority: 0,
		behavior: { type: "continuous", physical_min: 44, physical_max: 8, unit: "degrees" },
	};
	if (overrides.physicalMapping !== undefined) fn.physical_mapping = overrides.physicalMapping;
	const profile: JsonProfile = {
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
				id: modeId,
				name: "Default",
				notes: "",
				splits: [{ number: 1, footprint: 1 }],
				heads: [{ id: head, name: "Main", master_shared: true }],
				channels: [
					{
						id: channelId,
						head_id: head,
						split: 1,
						fixture_attribute: "zoom",
						attribute: "zoom",
						canonical_transform: "identity",
						resolution: "u8",
						secondary_slots: [],
						default_raw: 0,
						highlight_raw: 0,
						physical_min: 8,
						physical_max: 44,
						unit: "degrees",
						invert: false,
						snap: false,
						reacts_to_virtual_intensity: false,
						virtual_intensity_inverted: false,
						reacts_to_sequence_master: false,
						reacts_to_group_master: false,
						reacts_to_grand_master: false,
						behavior: "controlled",
						functions: [fn],
					},
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
	return {
		profile,
		seeded: {
			id: profile.id as string,
			manufacturer: profile.manufacturer as string,
			name,
			modeId,
			channelId,
			functionId,
		} satisfies SeededProfile,
	};
}

export async function saveProfile(api: ApiDriver, profile: JsonProfile, expectedRevision = 0) {
	return api.fixtureLibraryAction<{ profile_id: string; revision: number }>({
		type: "save_profile",
		profile,
		expected_revision: expectedRevision,
	});
}

/**
 * Saves a copy of a shipped profile under a new identity after `mutate` changed it. Shipped
 * packages may use descriptors this desk's attribute registry does not map; such channels are
 * re-addressed as plain Control so the copy can be published (they are not under test).
 */
export async function saveShippedCopy(
	api: ApiDriver,
	shipped: JsonProfile,
	name: string,
	mutate: (profile: JsonProfile) => void,
): Promise<JsonProfile> {
	const profile = structuredClone(shipped);
	profile.id = crypto.randomUUID();
	profile.name = name;
	profile.revision = 1;
	mutate(profile);
	for (let attempt = 0; attempt < 3; attempt += 1) {
		const response = await rawRequest(api, "POST", "/api/v2/fixture-library", {
			request_id: crypto.randomUUID(),
			action: { type: "save_profile", profile, expected_revision: 0 },
		});
		if (response.status === 200) return profile;
		const unmapped = /canonical descriptors for (.+?) in Show/u.exec(String(response.body?.error ?? ""));
		if (!unmapped) throw new Error(`Saving ${name} failed: ${JSON.stringify(response.body)}`);
		const names = new Set(unmapped[1].split(/,\s*/u));
		for (const mode of profile.modes)
			for (const channel of mode.channels) {
				if (names.has(channel.attribute)) channel.attribute = "control";
				if (names.has(channel.fixture_attribute)) channel.fixture_attribute = "control";
				for (const fn of channel.functions) if (names.has(fn.attribute)) fn.attribute = "control";
			}
	}
	throw new Error(`Saving ${name} kept failing`);
}

/** Every stored revision of one profile, newest last. */
export async function profileRevisions(api: ApiDriver, id: string): Promise<JsonProfile[]> {
	const revisions = await api.fixtureProfileRevisions<JsonProfile>(id);
	return [...revisions].sort((left, right) => left.revision - right.revision);
}

export async function latestProfile(api: ApiDriver, id: string): Promise<JsonProfile> {
	const revisions = await profileRevisions(api, id);
	const latest = revisions.at(-1);
	if (!latest) throw new Error(`Profile ${id} has no revisions`);
	return latest;
}

export function zoomFunction(profile: JsonProfile) {
	return profile.modes[0].channels[0].functions[0];
}

export async function openFixtureLibrary(page: Page, desk: DeskDriver, baseUrl: string) {
	await desk.open(baseUrl);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
	await page.getByRole("button", { name: "Open Fixture Library", exact: true }).click();
	const library = page.getByRole("dialog", { name: "Fixture Library" });
	await expect(library).toBeVisible();
	return library;
}

/** Fixture Library → profile → Edit fixture, from an open library. */
export async function editProfile(page: Page, profile: SeededProfile): Promise<Locator> {
	await page.getByPlaceholder("Search manufacturer, fixture, mode, or type").fill(profile.name);
	await page.getByRole("button", { name: new RegExp(profile.name) }).first().click();
	await page.getByRole("button", { name: "Edit fixture", exact: true }).click();
	const editor = page.getByRole("dialog", { name: "Edit fixture profile" });
	await expect(editor).toBeVisible();
	return editor;
}

/** Modes → Default → Channels → Zoom mapping → Details for Zoom; returns the mapping dialog. */
export async function openZoomDetails(page: Page, editor: Locator): Promise<Locator> {
	await editor.getByRole("tab", { name: "Modes", exact: true }).click();
	await page.getByRole("button", { name: "Edit channels for Default", exact: true }).click();
	const mode = page.getByRole("dialog", { name: "Edit Default mode" });
	await mode.getByRole("tab", { name: "Channels", exact: true }).click();
	await mode.getByRole("button", { name: "Edit zoom mapping", exact: true }).click();
	const mapping = page.getByRole("dialog", { name: "Zoom mapping" });
	await expect(mapping).toBeVisible();
	await mapping.getByRole("button", { name: "Details for Zoom", exact: true }).click();
	await expect(calibrationSection(mapping)).toBeVisible();
	return mapping;
}

export function calibrationSection(mapping: Locator) {
	return mapping.getByRole("region", { name: "Physical mapping calibration" });
}

/** Chooses one option of a labelled desk select inside `container`. */
export async function chooseSelect(container: Locator, label: string, option: string) {
	await container.getByRole("button", { name: label, exact: true }).click();
	await container.page().getByRole("option", { name: option, exact: true }).click();
}

/** Replaces the value of a labelled numeric or text field and commits it. */
export async function setField(container: Locator, label: string, value: string) {
	const field = container.getByRole("textbox", { name: label, exact: true });
	await field.fill(value);
	await field.press("Tab");
}

export async function closeMappingAndMode(page: Page) {
	await page.getByRole("button", { name: "Close channel mapping", exact: true }).click();
	const mode = page.getByRole("dialog", { name: "Edit Default mode" });
	await mode.getByRole("button", { name: "Close mode editor", exact: true }).click();
	await expect(mode).toBeHidden();
}

/** Save fixture, confirming the new immutable revision. */
export async function saveNewRevision(page: Page, editor: Locator) {
	await editor.getByRole("button", { name: "Save fixture", exact: true }).click();
	await page
		.getByRole("alertdialog", { name: "Create a new fixture revision?" })
		.getByRole("button", { name: "Save and create revision" })
		.click();
	await expect(editor).toBeHidden();
}

/** Whether the document itself scrolls (page-level overflow) at the current viewport. */
export async function pageOverflows(page: Page) {
	return page.evaluate(
		() =>
			document.documentElement.scrollHeight > window.innerHeight + 1 ||
			document.documentElement.scrollWidth > window.innerWidth + 1,
	);
}

/** `inner` lies completely inside `outer` (both visible). */
export async function containedIn(inner: Locator, outer: Locator) {
	const [a, b] = await Promise.all([inner.boundingBox(), outer.boundingBox()]);
	if (!a || !b) return false;
	return (
		a.x >= b.x - 1 &&
		a.y >= b.y - 1 &&
		a.x + a.width <= b.x + b.width + 1 &&
		a.y + a.height <= b.y + b.height + 1
	);
}
