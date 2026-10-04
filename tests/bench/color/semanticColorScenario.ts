import type { Locator, Page } from "@playwright/test";
import { replaceProgrammingSelection } from "../command-selection/programmingSelection";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import { expect } from "../core/fixtures";
import type { LightBench } from "../core/lightBench";
import type { FixtureDMXTarget } from "../output/fixtureDmxContract";
import { resolveFixtureDmxComponents } from "../output/fixtureDmxResolver";
import { BrowserPatch } from "../show-setup/patchScenario";

/**
 * Shared steps of the semantic Color scenarios (docs/testing/36-semantic-color-controls.md):
 * a fresh Color Intent show patched from the shipped library, the semantic Color dialog's own
 * Programmer writes, the accepted-frame colour report and the dialog's operator surfaces.
 */

export type ColorModel = "direct" | "intent";

export interface ColorRigEntry {
	number: number;
	name: string;
	manufacturer: string;
	profile: string;
	mode: string;
	/** `universe.address`. */
	address: string;
}

export interface ColorShow {
	id: string;
	/** Fixture id by fixture number. */
	ids: Record<number, string>;
}

/** A fresh show in the Color Intent model, patched with `rig`. */
export async function createColorIntentShow(
	api: ApiDriver,
	page: Page,
	desk: DeskDriver,
	label: string,
	rig: readonly ColorRigEntry[],
): Promise<ColorShow> {
	await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
	const show = await api.createShow<{ id: string }>({
		name: `SEMANTIC-COLOR ${label} ${crypto.randomUUID()}`,
	});
	await api.openShow(show.id, { transition: "hold_current" });
	await expect.poll(() => activeShowId(api)).toBe(show.id);
	if ((await colorModel(api, show.id)) !== "intent") await switchToIntent(api, show.id);
	const patch = new BrowserPatch(api, page, desk);
	for (const entry of rig) await patch.via.api.add(entry);
	return { id: show.id, ids: await fixtureIds(api) };
}

export async function fixtureIds(api: ApiDriver): Promise<Record<number, string>> {
	return Object.fromEntries(
		(await api.patch()).fixtures.map((fixture) => [fixture.fixture_number, fixture.fixture_id]),
	);
}

async function activeShowId(api: ApiDriver): Promise<string | null> {
	const bootstrap = await api.request<{ active_show: { id: string } | null }>(
		"GET",
		"/api/v2/bootstrap",
		undefined,
		false,
	);
	return bootstrap.active_show?.id ?? null;
}

interface AttributeConfigurationSnapshot {
	show_revision: number;
	object_revision: number;
	configuration: { color_model?: ColorModel | null };
}

function attributeConfiguration(api: ApiDriver, showId: string) {
	return api.request<AttributeConfigurationSnapshot>(
		"GET",
		"/api/v2/attribute-configuration",
		undefined,
		true,
		undefined,
		{ showId },
	);
}

async function colorModel(api: ApiDriver, showId: string): Promise<ColorModel> {
	return (await attributeConfiguration(api, showId)).configuration.color_model ?? "direct";
}

async function switchToIntent(api: ApiDriver, showId: string) {
	const snapshot = await attributeConfiguration(api, showId);
	await api.request(
		"POST",
		"/api/v2/attribute-configuration/update",
		{
			request_id: crypto.randomUUID(),
			expected_show_revision: snapshot.show_revision,
			expected_object_revision: snapshot.object_revision,
			patch: { color_model: "intent" },
		},
		true,
		undefined,
		{ showId },
	);
}

/** Whether the runtime publishes the semantic family pages (programming contract 1). */
export async function semanticPagesPublished(api: ApiDriver, fixtureIds: readonly string[]) {
	const pages = await api
		.request<{ semantic: boolean }>(
			"GET",
			`/api/v2/programming/family-encoder-pages?fixture_ids=${fixtureIds.join(",")}`,
		)
		.catch(() => null);
	return Boolean(pages?.semantic);
}

export const SEMANTIC_COLOR_GATE =
	"semantic programming contract is not enabled on this runtime (an older contract-0 build)";

// ---------------------------------------------------------------------------------------------
// Programmer

export async function selectFixtures(api: ApiDriver, showId: string, fixtureIds: readonly string[]) {
	await replaceProgrammingSelection(api, { surface: "api", showId, fixtures: fixtureIds });
}

export async function selectedFixtureIds(api: ApiDriver): Promise<string[]> {
	const programmers = await api.request<Array<{ session_id?: string; selected: string[] }>>(
		"GET",
		"/api/v2/programmers",
	);
	const current =
		programmers.find((entry) => entry.session_id === api.session?.session_id) ?? programmers[0];
	if (!current) throw new Error("No programmer");
	return current.selected;
}

export async function programmerValuesRevision(api: ApiDriver) {
	const snapshot = await api.request<{ projection: { revision: number } }>(
		"GET",
		"/api/v2/programmer/values/snapshot",
	);
	return snapshot.projection.revision;
}

export interface ProgrammerValue {
	fixture_id: string;
	attribute: string;
	value: { kind: string; value: unknown };
}

export async function programmerValues(api: ApiDriver): Promise<ProgrammerValue[]> {
	const snapshot = await api.request<{ projection: { fixture_values: ProgrammerValue[] } }>(
		"GET",
		"/api/v2/programmer/values/snapshot",
	);
	return snapshot.projection.fixture_values;
}

/** One revision-guarded Programmer values action. */
export async function valuesAction(api: ApiDriver, action: Record<string, unknown>) {
	const capture = await api.request<{ projection: { revision: number } }>(
		"GET",
		"/api/v2/programmer/capture-mode/snapshot",
	);
	return api.request<Record<string, unknown>>("POST", "/api/v2/programmer/values/actions", {
		request_id: crypto.randomUUID(),
		expected_revision: await programmerValuesRevision(api),
		expected_capture_mode_revision: capture.projection.revision,
		action,
	});
}

/** A complete semantic white: the start a fresh pick edits, exactly as the dialog's picker. */
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

export function scalarColorEdit(component: string, value: number) {
	return {
		kind: "scalar",
		component: { kind: "color", component },
		operation: { kind: "set", value: { kind: "value", value } },
	};
}

/** One atomic `component_edits` action on the whole colour of the given fixtures. */
export async function colorEdits(api: ApiDriver, fixtureIds: readonly string[], edits: unknown[]) {
	const outcome = await valuesAction(api, {
		type: "apply_intent",
		fixture_ids: fixtureIds,
		attribute: "color",
		operation: { type: "component_edits", edits },
		timing: { fade: false },
	});
	expect(outcome.status, JSON.stringify(outcome)).toBe("changed");
}

/**
 * The semantic Color dialog's write (programming contract 1): a whole colour from a white start,
 * edited by Hue (degrees) and Saturation (0..1), plus any further component edits.
 */
export async function programColor(
	api: ApiDriver,
	fixtureIds: readonly string[],
	color: { hue: number; saturation: number },
	extra: unknown[] = [],
) {
	await valuesAction(api, {
		type: "batch",
		mutations: fixtureIds.map((fixture_id) => ({
			type: "set_fixture",
			fixture_id,
			attribute: "color",
			value: SEMANTIC_WHITE,
			timing: { fade: false },
		})),
	});
	await colorEdits(api, fixtureIds, [
		scalarColorEdit("hue", color.hue),
		scalarColorEdit("saturation", color.saturation),
		...extra,
	]);
}

export async function setIntensity(api: ApiDriver, fixtureIds: readonly string[], level: number) {
	await valuesAction(api, {
		type: "batch",
		mutations: fixtureIds.map((fixture_id) => ({
			type: "set_fixture",
			fixture_id,
			attribute: "intensity",
			value: { kind: "normalized", value: level },
			timing: { fade: false },
		})),
	});
}

// ---------------------------------------------------------------------------------------------
// Accepted-frame colour report

export interface ColorIntentReport {
	color_model: ColorModel;
	heads: Array<{
		fixture_id: string;
		owner_id: string;
		fixture_number: number | null;
		quality: string;
		delta_uv: number | null;
		uv?: { status: string; clipped: boolean } | null;
	}>;
	accepted_frame?: { state: string } | null;
}

export function colorReport(api: ApiDriver, showId: string, fixtureIds: readonly string[]) {
	return api.request<ColorIntentReport>(
		"GET",
		`/api/v2/color-intent/report?fixtures=${fixtureIds.map(encodeURIComponent).join(",")}`,
		undefined,
		true,
		undefined,
		{ showId },
	);
}

/** The report of the output frame that was sent: tick once and read until it is accepted. */
export async function acceptedColorReport(
	api: ApiDriver,
	bench: LightBench,
	showId: string,
	fixtureIds: readonly string[],
): Promise<ColorIntentReport> {
	let report: ColorIntentReport | undefined;
	await expect
		.poll(async () => {
			await bench.tick(0);
			report = await colorReport(api, showId, fixtureIds);
			return report.accepted_frame?.state;
		})
		.toBe("accepted");
	if (!report) throw new Error("no accepted Color report");
	return report;
}

/** UV wording of the approximation and the Fixture Sheet (features/colorReport). */
export function uvWording(head: ColorIntentReport["heads"][number]): string | null {
	const uv = head.uv;
	if (!uv || uv.status === "not_requested") return null;
	if (uv.status === "unsupported") return "UV unavailable on this fixture";
	return uv.clipped ? "UV limited by the emitter" : "UV applied";
}

// ---------------------------------------------------------------------------------------------
// Operator UI

/** The open Color Special Dialog, compact (inside the encoder area) or the full modal. */
export function colorSpecialDialog(page: Page) {
	return page.getByRole("dialog", { name: "Color Special Dialog" });
}

/** The full modal's stack layer (title bar, close button and body). */
export function colorModalLayer(page: Page) {
	return page.locator(".ui-modal-stack-layer").filter({ has: colorSpecialDialog(page) });
}

/** The Color tab, then **Special Dialog**: the dialog opens compact or as the full modal. */
export async function openColorSpecialDialog(page: Page): Promise<Locator> {
	await page.getByRole("button", { name: /^Color( \d+ of \d+)?$/ }).first().click();
	await page.getByRole("button", { name: "Special Dialog", exact: true }).click();
	const dialog = colorSpecialDialog(page);
	await expect(dialog).toBeVisible();
	return dialog;
}

/** Opens the dialog and presses **Expand** when it opened compact; returns the modal layer. */
export async function openFullColorModal(page: Page, title = "Color"): Promise<Locator> {
	const opened = await openColorSpecialDialog(page);
	const expand = opened.getByRole("button", { name: "Expand", exact: true });
	if (await expand.count()) await expand.click();
	const layer = colorModalLayer(page);
	await expect(layer).toBeVisible();
	await expect(layer.getByRole("heading", { level: 2, name: title, exact: true })).toBeVisible();
	return layer;
}

export async function closeColorModal(layer: Locator) {
	await layer.getByRole("button", { name: "Close Special Dialog", exact: true }).click();
	await expect(layer).toBeHidden();
}

export function approximation(layer: Locator) {
	return layer.getByTestId("color-approximation");
}

export function approximationRow(results: Locator, fixtureId: string) {
	return results.locator(`tbody tr[data-fixture-id="${fixtureId}"]`);
}

// ---------------------------------------------------------------------------------------------
// DMX

/** The current output bytes of named channels of one fixture (head), after one bench tick. */
export async function channelBytes(
	api: ApiDriver,
	bench: LightBench,
	target: FixtureDMXTarget,
	names: readonly string[],
): Promise<Record<string, number>> {
	const resolved = resolveFixtureDmxComponents(
		await api.patch(),
		target,
		names.map((name) => [name, 0] as [string, number]),
	);
	const frame = await bench.tick(0);
	return Object.fromEntries(
		names.map((name, index) => {
			const { universe, address } = resolved[index];
			return [name, frame.universes.find((entry) => entry.universe === universe)?.slots[address - 1] ?? -1];
		}),
	);
}

// ---------------------------------------------------------------------------------------------
// Measured test profiles: no shipped profile carries measured colour data, so the scenarios save
// measured copies into the library the way the fixture editor does.

interface LibraryProfile {
	id: string;
	revision: number;
	manufacturer: string;
	name: string;
	modes: Array<{
		name: string;
		splits?: Array<{ number: number; footprint: number }>;
		heads: Array<{ id: string }>;
		channels: Array<{ id: string; attribute: string }>;
		// biome-ignore lint/suspicious/noExplicitAny: the colour-system schema is edited as JSON.
		color_systems?: Array<Record<string, any>>;
	}>;
}

async function libraryProfile(api: ApiDriver, manufacturer: string, name: string) {
	const library = await api.request<{ profiles: LibraryProfile[] }>(
		"GET",
		"/api/v2/fixture-library/profiles",
	);
	const profile = library.profiles.find(
		(candidate) => candidate.manufacturer === manufacturer && candidate.name === name,
	);
	if (!profile) throw new Error(`The fixture library has no ${manufacturer} ${name}`);
	return structuredClone(profile);
}

function srgbXyz(red: number, green: number, blue: number) {
	return {
		x: 0.4124564 * red + 0.3575761 * green + 0.1804375 * blue,
		y: 0.2126729 * red + 0.7151522 * green + 0.072175 * blue,
		z: 0.0193339 * red + 0.119192 * green + 0.9503041 * blue,
	};
}

const MEASURED = { status: "measured", revision: 1, source: "SEMANTIC-COLOR colorimeter" };
const IDENTITY = [
	[1, 0, 0],
	[0, 1, 0],
	[0, 0, 1],
];

async function saveCopy(api: ApiDriver, profile: LibraryProfile, name: string) {
	profile.id = crypto.randomUUID();
	profile.revision = 0;
	profile.manufacturer = "SEMANTIC-COLOR";
	profile.name = `${name} ${crypto.randomUUID().slice(0, 8)}`;
	await api.fixtureLibraryAction({ type: "save_profile", profile, expected_revision: 0 });
	return { manufacturer: profile.manufacturer, profile: profile.name };
}

/** Generic RGB LED (no UV emitter) with a measured sRGB-primary additive colour system. */
export async function saveMeasuredRgbProfile(api: ApiDriver) {
	const profile = await libraryProfile(api, "Generic", "RGB LED");
	for (const mode of profile.modes) {
		const emitter = (attribute: string, name: string, xyz: ReturnType<typeof srgbXyz>) => {
			const channel = mode.channels.find((candidate) => candidate.attribute === attribute);
			if (!channel) throw new Error(`${mode.name} has no ${attribute}`);
			return { channel_id: channel.id, name, xyz, maximum_level: 1, response_curve: 1, visible: true };
		};
		mode.color_systems = [
			{
				head_id: mode.heads[0].id,
				correction_matrix: IDENTITY,
				calibration: MEASURED,
				system: {
					type: "additive",
					emitters: [
						emitter("color.red", "Red", srgbXyz(1, 0, 0)),
						emitter("color.green", "Green", srgbXyz(0, 1, 0)),
						emitter("color.blue", "Blue", srgbXyz(0, 0, 1)),
					],
				},
			},
		];
	}
	return saveCopy(api, profile, "Measured RGB");
}

/**
 * A wheel-only fixture: Generic Dimmer plus the shipped Cameo AURO SPOT Z300 colour wheel with
 * measured slots, in the mode "Dimmer + wheel" (footprint 2).
 */
export async function saveMeasuredWheelSpot(api: ApiDriver) {
	const auro = await libraryProfile(api, "Cameo", "AURO SPOT Z300");
	const auroMode = auro.modes.find((mode) => mode.name === "17-Channel");
	const auroWheel = auroMode?.channels.find((channel) => channel.attribute === "color.wheel.1");
	const auroSystem = auroMode?.color_systems?.[0];
	if (!auroWheel || !auroSystem) throw new Error("AURO SPOT Z300 has no colour wheel");
	const slotColor: Record<string, [number, number, number]> = {
		open: [1, 1, 1],
		deep_red: [1, 0, 0],
		medium_blue: [0, 0, 1],
		deep_green: [0, 1, 0],
		yellow: [1, 1, 0],
		lavender: [0.7, 0.5, 1],
		amber_deep_orange: [1, 0.2, 0],
		cto_3200k: [1, 0.75, 0.5],
		congo_blue: [0.2, 0, 1],
	};
	const profile = await libraryProfile(api, "Generic", "Dimmer");
	profile.modes = profile.modes
		.filter((mode) => mode.name === "8-bit")
		.map((mode) => {
			const wheel = { ...structuredClone(auroWheel), id: crypto.randomUUID(), head_id: mode.heads[0].id, split: 1 };
			const system = structuredClone(auroSystem);
			system.head_id = mode.heads[0].id;
			system.system.channel_id = wheel.id;
			system.calibration = MEASURED;
			for (const slot of system.system.slots) {
				const color = slotColor[slot.semantic_id];
				if (!color) throw new Error(`No measurement for wheel slot ${slot.semantic_id}`);
				slot.measured_xyz = srgbXyz(...color);
			}
			return {
				...mode,
				name: "Dimmer + wheel",
				splits: [{ number: 1, footprint: 2 }],
				channels: [...mode.channels, wheel],
				color_systems: [system],
			};
		});
	return { ...(await saveCopy(api, profile, "Measured Wheel Spot")), mode: "Dimmer + wheel" };
}
