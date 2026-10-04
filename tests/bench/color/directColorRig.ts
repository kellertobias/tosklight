import { expect, type Locator, type Page } from "@playwright/test";
import { replaceProgrammingSelection } from "../command-selection/programmingSelection";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { LightBench } from "../core/lightBench";
import { BrowserPatch } from "../show-setup/patchScenario";

/**
 * Bench of docs/testing/37-direct-color-pages.md (TL-554): the DIRECT-COLOR-001 rig of
 * tests/112-color-intent.spec.ts (two verified ROOT PAR 6 heads A1/A2 and a verified Lustr B),
 * plus a verified fixture with unknown emitter appearance (Martin ELP CL, E) and a desk-saved
 * library copy of it with more than eight native colour controls (X1/X2) for the overflow.
 */

export const SEMANTIC_GATE =
	"semantic programming contract is not enabled on this runtime (run npm run test:e2e-semantic)";

export const DIRECT_RIG = [
	{ number: 101, name: "Par A1", manufacturer: "Cameo", profile: "ROOT PAR 6", mode: "D7CH — Delay Off, virtual dimmer", address: "1.101" },
	{ number: 102, name: "Par A2", manufacturer: "Cameo", profile: "ROOT PAR 6", mode: "D7CH — Delay Off, virtual dimmer", address: "1.111" },
	{ number: 103, name: "Lustr B", manufacturer: "ETC", profile: "Source Four LED Series 2 Lustr", mode: "Direct", address: "1.121" },
	{ number: 104, name: "ELP E", manufacturer: "Martin", profile: "ELP CL Profile", mode: "10-Channel", address: "1.141" },
] as const;

export const OVERFLOW_PROFILE = { manufacturer: "DIRECT-COLOR", name: "Overflow Wash", mode: "Overflow" } as const;

export interface DirectShow {
	id: string;
	ids: Record<number, string>;
}

export const SEMANTIC_RED = semantic({ x: 0.4124564, y: 0.2126729, z: 0.0193339 }, [1, 0, 0]);
export const SEMANTIC_GREEN = semantic({ x: 0.3575761, y: 0.7151522, z: 0.119192 }, [0, 1, 0]);
export const SEMANTIC_BLUE = semantic({ x: 0.1804375, y: 0.072175, z: 0.9503041 }, [0, 0, 1]);

function semantic(base_xyz: { x: number; y: number; z: number }, rgb: [number, number, number]) {
	return {
		kind: "color_program",
		value: {
			kind: "semantic",
			intent: {
				base_xyz,
				recipe: { version: 1, rgb, amber: 0, approximate: false },
				white_blend: 0,
				white_target: { kelvin: 6500, duv: 0 },
				uv: { amount: 0 },
				relative_output: 1,
				allocation: "preserve_recipe",
			},
		},
	};
}

/** A new semantic-Color (intent) show with the DIRECT rig; `overflow` adds X1/X2 (201, 202). */
export async function createDirectShow(
	api: ApiDriver,
	page: Page,
	desk: DeskDriver,
	options: { overflow?: boolean } = {},
): Promise<DirectShow> {
	await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
	const show = await api.createShow<{ id: string }>({ name: `DIRECT-COLOR ${crypto.randomUUID()}` });
	await api.openShow(show.id, { transition: "hold_current" });
	await expect.poll(() => activeShowId(api)).toBe(show.id);
	await useIntentColor(api, show.id, Boolean(options.overflow));
	if (options.overflow) await saveOverflowProfile(api);
	const patch = new BrowserPatch(api, page, desk);
	for (const rig of DIRECT_RIG) await patch.via.api.add({ ...rig });
	if (options.overflow)
		for (const number of [201, 202])
			await patch.via.api.add({
				number,
				name: `Overflow X${number - 200}`,
				manufacturer: OVERFLOW_PROFILE.manufacturer,
				profile: OVERFLOW_PROFILE.name,
				mode: OVERFLOW_PROFILE.mode,
				address: `2.${(number - 201) * 20 + 1}`,
			});
	const ids = Object.fromEntries(
		(await api.patch()).fixtures.map((fixture) => [fixture.fixture_number, fixture.fixture_id]),
	);
	return { id: show.id, ids };
}

async function activeShowId(api: ApiDriver): Promise<string | null> {
	const bootstrap = await api.request<{ active_show: { id: string } | null }>("GET", "/api/v2/bootstrap", undefined, false);
	return bootstrap.active_show?.id ?? null;
}

/** Selects the semantic Color model; `colorScene` also creates the ELP's Color Scene macro. */
async function useIntentColor(api: ApiDriver, showId: string, colorScene: boolean) {
	const read = () =>
		api.request<{
			show_revision: number;
			object_revision: number;
			configuration: {
				color_model?: string | null;
				custom_attributes: Array<{ id: string }>;
				placements: Array<{ attribute: string; encoder_group: string; encoder_page: number; encoder_slot: number }>;
			};
		}>(
			"GET",
			"/api/v2/attribute-configuration",
			undefined,
			true,
			undefined,
			{ showId },
		);
	const snapshot = await read();
	const custom = snapshot.configuration.custom_attributes ?? [];
	const placements = snapshot.configuration.placements ?? [];
	const freePage =
		Math.max(0, ...placements.filter((entry) => entry.encoder_group === "control").map((entry) => entry.encoder_page)) + 1;
	const scene = colorScene && !custom.some((descriptor) => descriptor.id === COLOR_SCENE);
	if (snapshot.configuration.color_model === "intent" && !scene) return;
	await api.request(
		"POST",
		"/api/v2/attribute-configuration/update",
		{
			request_id: crypto.randomUUID(),
			expected_show_revision: snapshot.show_revision,
			expected_object_revision: snapshot.object_revision,
			patch: {
				color_model: "intent",
				...(scene
					? {
							custom_attributes: [
								...custom,
								{
									id: COLOR_SCENE,
									label: "Color Scene",
									value_type: "indexed",
									display_unit: null,
									physical_unit: null,
									normalized_bounds: null,
									domain_bounds: null,
									cyclic: false,
									recordable: false,
									lifecycle: "active",
								},
							],
							placements: [
								...placements,
								{ attribute: COLOR_SCENE, encoder_group: "control", encoder_page: freePage, encoder_slot: 1 },
							],
						}
					: {}),
			},
		},
		true,
		undefined,
		{ showId },
	);
}

const COLOR_SCENE = "fixture.color_scene";

interface LibraryChannel {
	id: string;
	attribute: string;
	fixture_attribute?: string;
	split: number;
	head_id: string;
	// biome-ignore lint/suspicious/noExplicitAny: the channel schema is edited as JSON.
	[key: string]: any;
}

/**
 * The shipped Martin ELP CL Profile (a verified native layout: Temperature, R, G, B, Lime, Amber,
 * Color Scene) saved as a desk library copy whose single head has eleven native colour controls:
 * eight continuous ones first (pages 3/4), then a continuous White, the Color Scene macro and a
 * colour wheel taken from the shipped AURO SPOT Z300 (the overflow of the full Color dialog).
 */
async function saveOverflowProfile(api: ApiDriver) {
	type Profile = {
		id: string;
		revision: number;
		manufacturer: string;
		name: string;
		// biome-ignore lint/suspicious/noExplicitAny: the profile schema is edited as JSON.
		modes: Array<Record<string, any> & { name: string; channels: LibraryChannel[] }>;
	};
	const library = await api.request<{ profiles: Profile[] }>("GET", "/api/v2/fixture-library/profiles");
	if (library.profiles.some((profile) => profile.manufacturer === OVERFLOW_PROFILE.manufacturer)) return;
	const find = (manufacturer: string, name: string) => {
		const profile = library.profiles.find((entry) => entry.manufacturer === manufacturer && entry.name === name);
		if (!profile) throw new Error(`The fixture library has no ${manufacturer} ${name}`);
		return structuredClone(profile);
	};
	const elp = find("Martin", "ELP CL Profile");
	const auro = find("Cameo", "AURO SPOT Z300");
	const wheel = auro.modes.flatMap((mode) => mode.channels).find((channel) => channel.attribute === "color.wheel.1");
	const mode = elp.modes.find((entry) => entry.name === "10-Channel");
	if (!wheel || !mode) throw new Error("the overflow profile sources changed");
	const head = mode.heads[0].id as string;
	const path = mode.color_physical.paths[0];
	const byAttribute = (attribute: string) => {
		const channel = mode.channels.find((entry) => entry.attribute === attribute);
		if (!channel) throw new Error(`ELP CL has no ${attribute}`);
		return channel;
	};
	const copy = (source: LibraryChannel, attribute: string): LibraryChannel => ({
		...structuredClone(source),
		id: crypto.randomUUID(),
		head_id: head,
		split: 1,
		attribute,
		fixture_attribute: attribute,
		functions: (source.functions ?? []).map((fn: Record<string, unknown>) => ({
			...structuredClone(fn),
			id: crypto.randomUUID(),
			attribute,
		})),
	});
	const lime = byAttribute("color.lime");
	const cyan = copy(lime, "color.cyan");
	const indigo = copy(lime, "color.indigo");
	const white = copy(lime, "color.white");
	const colorWheel = copy(wheel, "color.wheel.1");
	// The ELP's dimmer fade time is not a canonical attribute of a new show: leave it out.
	const channels = [
		...mode.channels.filter((channel) => channel.attribute !== "fixture.dimmer_fade_time"),
		cyan,
		indigo,
		white,
		colorWheel,
	];
	const controlOf = (attribute: string) => byAttribute(attribute).id;
	path.controls = [
		controlOf("color.temperature"),
		controlOf("color.red"),
		controlOf("color.green"),
		controlOf("color.blue"),
		controlOf("color.lime"),
		controlOf("color.amber"),
		cyan.id,
		indigo.id,
		white.id,
		controlOf("fixture.color_scene"),
		colorWheel.id,
	];
	const profile = {
		...elp,
		id: crypto.randomUUID(),
		revision: 0,
		manufacturer: OVERFLOW_PROFILE.manufacturer,
		name: OVERFLOW_PROFILE.name,
		modes: [
			{
				...mode,
				id: crypto.randomUUID(),
				name: OVERFLOW_PROFILE.mode,
				splits: [{ number: 1, footprint: channels.length }],
				channels,
			},
		],
	};
	await api.fixtureLibraryAction({ type: "save_profile", profile, expected_revision: 0 } as never);
}

export async function select(api: ApiDriver, show: DirectShow, fixtureIds: readonly string[]) {
	await replaceProgrammingSelection(api, { surface: "api", showId: show.id, fixtures: [...fixtureIds] });
}

export async function programmerRevision(api: ApiDriver) {
	const snapshot = await api.request<{ projection: { revision: number } }>("GET", "/api/v2/programmer/values/snapshot");
	return snapshot.projection.revision;
}

export interface ProgrammerColor {
	fixture_id: string;
	attribute: string;
	value: {
		kind: string;
		value: {
			kind: "direct" | "semantic";
			recipe?: { channels: Array<{ channel_id: string; raw: number }> };
			// biome-ignore lint/suspicious/noExplicitAny: semantic intents are compared as JSON.
			intent?: any;
		};
	};
}

export async function colorValues(api: ApiDriver): Promise<ProgrammerColor[]> {
	const snapshot = await api.request<{ projection: { fixture_values: ProgrammerColor[] } }>(
		"GET",
		"/api/v2/programmer/values/snapshot",
	);
	return snapshot.projection.fixture_values.filter((value) => value.attribute === "color");
}

export async function colorOf(api: ApiDriver, fixtureId: string) {
	return (await colorValues(api)).find((value) => value.fixture_id === fixtureId)?.value.value ?? null;
}

/** One Programmer values action, as the HTTP integrator path sends it. */
export async function valuesAction(api: ApiDriver, action: Record<string, unknown>) {
	const capture = await api.request<{ projection: { revision: number } }>("GET", "/api/v2/programmer/capture-mode/snapshot");
	return api.request<Record<string, unknown>>("POST", "/api/v2/programmer/values/actions", {
		request_id: crypto.randomUUID(),
		expected_revision: await programmerRevision(api),
		expected_capture_mode_revision: capture.projection.revision,
		action,
	});
}

export async function programSemantic(api: ApiDriver, fixtureIds: readonly string[], value: unknown) {
	for (const fixture_id of fixtureIds)
		await valuesAction(api, { type: "set_fixture", fixture_id, attribute: "color", value, timing: {} });
}

export interface NativeControl {
	id: string;
	label: string;
	channel_id: string;
	raw_max: number;
	functions: Array<{ function_id: string; label: string; raw_from: number; raw_to: number; continuous: boolean }>;
}

export interface NativePages {
	semantic: boolean;
	unavailable?: string | null;
	reference?: {
		fixture_id: string;
		fixture_number: number | null;
		fixture_name: string;
		head_id: string;
		head_name: string;
		chosen: boolean;
	} | null;
	candidates: Array<{ fixture_id: string; head_id: string; fixture_number: number | null; fixture_name: string; head_name: string }>;
	pages: Array<{ number: number; controls: Array<NativeControl | null> }>;
	overflow: NativeControl[];
	values?: { controls: Array<{ channel_id: string; raw: number }> } | null;
	fixtures: Array<{ fixture_id: string; replay: string }>;
}

export function nativePages(api: ApiDriver, fixtureIds: readonly string[], reference?: string) {
	return api.request<NativePages>(
		"GET",
		`/api/v2/programming/color/native-pages?fixture_ids=${fixtureIds.join(",")}${reference ? `&reference=${reference}` : ""}`,
	);
}

export async function semanticPublished(api: ApiDriver, fixtureIds: readonly string[]) {
	const pages = await api
		.request<{ semantic: boolean }>("GET", `/api/v2/programming/family-encoder-pages?fixture_ids=${fixtureIds.join(",")}`)
		.catch(() => null);
	return Boolean(pages?.semantic);
}

/** One Direct edit of `control` (first function), relative by `value` native integers. */
export function nativeEdit(
	fixtureIds: readonly string[],
	control: NativeControl,
	value: number,
	options: { reference?: { fixture_id: string; head_id: string }; undoGroup?: string; displayedSource?: { lane: string; lease: number } } = {},
) {
	return {
		type: "apply_intent",
		fixture_ids: [...fixtureIds],
		attribute: "color",
		operation: {
			type: "component_edits",
			edits: [
				{
					kind: "native",
					binding: { channel_id: control.channel_id, function_id: control.functions[0].function_id },
					operation: { kind: "relative", value },
				},
			],
		},
		undo_group: options.undoGroup ?? null,
		timing: {},
		...(options.reference ? { native_reference: options.reference } : {}),
		...(options.displayedSource ? { displayed_source: options.displayedSource } : {}),
	};
}

/** One semantic White Blend set on the whole colour of `fixtureIds`. */
export function whiteBlendEdit(fixtureIds: readonly string[], value: number, explicitStart?: [number, number, number]) {
	return {
		type: "apply_intent",
		fixture_ids: [...fixtureIds],
		attribute: "color",
		operation: {
			type: "component_edits",
			edits: [
				{
					kind: "scalar",
					component: { kind: "color", component: "white_blend" },
					operation: { kind: "set", value: { kind: "value", value } },
				},
			],
		},
		timing: { fade: false },
		...(explicitStart ? { explicit_color_start: { rgb: explicitStart } } : {}),
	};
}

export interface DirectReport {
	heads: Array<{
		fixture_id: string;
		direct?: { replay: string; compatibility?: string | null; uv?: string | null; limitations: string[] } | null;
	}>;
	accepted_frame?: { state: string } | null;
}

export async function acceptedReport(api: ApiDriver, bench: LightBench, fixtureIds: readonly string[]) {
	let report: DirectReport | undefined;
	await expect
		.poll(async () => {
			await bench.tick(25);
			report = await api.request<DirectReport>("GET", `/api/v2/color-intent/report?fixtures=${fixtureIds.join(",")}`);
			return report.accepted_frame?.state;
		})
		.toBe("accepted");
	if (!report) throw new Error("no accepted Color report");
	return report;
}

export function directOf(report: DirectReport, fixtureId: string) {
	return report.heads.find((head) => head.fixture_id === fixtureId)?.direct ?? null;
}

/** DMX of one patched range of universe `universe` (1-based address, `count` slots). */
export async function dmx(bench: LightBench, universe: number, address: number, count: number) {
	const frame = await bench.tick(0);
	const slots = frame.universes.find((entry) => entry.universe === universe)?.slots ?? [];
	return slots.slice(address - 1, address - 1 + count);
}

// ---------------------------------------------------------------------------------------------
// Operator UI

/** The Color family button; a multi-page family names its page, e.g. `Color 3 of 4`. */
export function colorFamily(page: Page) {
	return page.getByRole("button", { name: /^Color( \d+ of \d+)?$/ });
}

/** Makes Color the active family without paging it (touching the active family pages it). */
export async function showColor(page: Page) {
	const family = colorFamily(page);
	await expect(family).toBeVisible();
	if (!/\bis-active\b/.test((await family.getAttribute("class")) ?? "")) await family.click();
	await expect(family).toHaveClass(/\bis-active\b/);
}

/** Pages the active Color family to `target` by touching its family button (it wraps). */
export async function pageColorTo(page: Page, target: number) {
	await showColor(page);
	const family = colorFamily(page);
	for (let attempt = 0; attempt < 8; attempt += 1) {
		const current = /(\d+) of \d+/.exec((await family.getAttribute("aria-label")) ?? "")?.[1];
		if (current === String(target)) break;
		await family.click();
	}
	await expect(family).toHaveAccessibleName(new RegExp(`^Color ${target} of \\d+$`));
}

/** One software encoder of the lower encoder area, by its slot. */
export function encoder(page: Page, slot: number) {
	return page.locator(".parameter-surfaces").getByRole("group", { name: new RegExp(`^Enc ${slot} · `) });
}

/** One touch detent: a tap on the upper (positive) or lower (negative) third of an encoder. */
export async function detent(encoderGroup: Locator, direction: 1 | -1) {
	await encoderGroup.locator(direction > 0 ? ".touch-encoder-tap-positive" : ".touch-encoder-tap-negative").click();
}

/** The desk configuration's Color presentation (Advanced: semantic pages 1 and 2). */
export async function usePresentation(api: ApiDriver, presentation: "advanced" | "easy_rgbw") {
	await api.request("POST", "/api/v2/configuration/update", {
		request_id: crypto.randomUUID(),
		patch: { color_presentation: presentation },
	});
}

/** Records the values action of every `component_edits` Color frame the desk sends. */
export function recordColorActions(page: Page) {
	// biome-ignore lint/suspicious/noExplicitAny: action frames are compared as JSON.
	const sent: any[] = [];
	page.on("websocket", (socket) =>
		socket.on("framesent", ({ payload }) => {
			const text = String(payload);
			if (!text.includes("component_edits") || !text.includes('"attribute":"color"')) return;
			try {
				sent.push(JSON.parse(text)?.action?.request?.action ?? text);
			} catch {
				sent.push(text);
			}
		}),
	);
	page.on("request", (request) => {
		const body = request.postData();
		if (request.method() === "POST" && body?.includes("component_edits") && body.includes('"attribute":"color"'))
			sent.push(JSON.parse(body)?.action ?? body);
	});
	return sent;
}

/** The full (expanded) Color modal. */
export async function openColorDialog(page: Page): Promise<Locator> {
	await showColor(page);
	await page.getByRole("button", { name: "Special Dialog", exact: true }).click();
	const opened = page.getByRole("dialog", { name: "Color Special Dialog" });
	await expect(opened).toBeVisible();
	const expand = opened.getByRole("button", { name: "Expand", exact: true });
	if (await expand.count()) await expand.click();
	const dialog = page.locator(".ui-modal-stack-layer").filter({ has: page.getByRole("dialog", { name: "Color Special Dialog" }) });
	await expect(dialog).toBeVisible();
	return dialog;
}

export async function closeDialog(dialog: Locator) {
	const close = dialog.getByRole("button", { name: "Close modal", exact: true });
	if (await close.count()) await close.click();
	else await dialog.page().keyboard.press("Escape");
	await expect(dialog).toBeHidden();
}

/** Anything the desk would raise loudly: a toast, an alert or an alert dialog. */
export function loudFeedback(page: Page) {
	return page.locator('[role="alert"], [role="alertdialog"], .toast, [data-sonner-toast], .ui-toast');
}

/** A sent Direct edit without its per-request identity (request, revision, gesture, lease). */
// biome-ignore lint/suspicious/noExplicitAny: action frames are compared as JSON.
export function directEditOf(action: any) {
	const { request_id, expected_revision, undo_group, displayed_source, ...edit } = action ?? {};
	return edit;
}
