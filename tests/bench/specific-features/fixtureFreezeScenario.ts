import { expect, type Locator, type Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import type { LightBench, TestShow } from "../core/lightBench";
import type { OscHardware } from "../core/protocols";
import { BrowserDynamics } from "../dynamics/dynamicScenario";
import {
	createCueList,
	definition,
	playbackAt,
	poolAction,
	saveSlot,
} from "../playbacks/playback-configuration/api";
import {
	batchProgrammerValues,
	setProgrammerFixtureValue,
} from "../programmer/programmerValues";
import { BrowserPatch } from "../show-setup/patchScenario";

/**
 * docs/testing/17-fixture-freeze.md bench: one Group holding three intensity fixtures, each driven by
 * a different source (Programmer, Cue, Dynamic), a multi-head colour/position wash and a
 * single-head colour PAR, under a Group Master and Grand Master below Full with Blackout off.
 *
 * Universe 1 layout (Art-Net to the bench receiver and the logical frame):
 * - U1.1 Dimmer 1 (Programmer), U1.2 Dimmer 2 (Cue), U1.3 Dimmer 3 (Dynamic)
 * - U1.101-125 ROBE Robin 600X LEDWash Mode 6, Fixture 101: Pan 101/102, Tilt 103/104, zone 1-3
 *   RGBW 107-118, Zoom 121/122, Shutter 123, Intensity 124/125 (all on the Master). Shutter belongs
 *   to the Intensity family (2026-10-05); Zoom is the Beam source.
 * - U1.201-206 Cameo ROOT PAR 6 virtual-dimmer mode, Fixture 201: R G B W A UV
 */
export const FREEZE_GROUP = "10";
export const DIMMER_SLOTS = [1, 2, 3] as const;
export const WASH_MASTER_SLOTS = [101, 102, 103, 104, 124, 125] as const;
export const WASH_HEAD_COLOR_SLOTS = Array.from(
	{ length: 12 },
	(_, index) => 107 + index,
);
export const PAR_SLOTS = [201, 202, 203, 204, 205, 206] as const;

const IMMEDIATE = { fade: false, fadeMillis: null, delayMillis: null } as const;

/** A complete semantic white, the start a fresh Color pick edits (programming contract 1). */
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

export interface FreezeRig {
	showId: string;
	dimmers: [string, string, string];
	wash: string;
	washHeads: string[];
	par: string;
	cueListId: string;
	cuePlayback: number;
	groupMasterPlayback: number;
}

export interface FreezeFrame {
	/** Logical frame slots of universe 1, index = DMX address - 1. */
	logical: number[];
	/** The Art-Net packet the bench receiver got for the same frame. */
	physical: number[];
}

export type FreezeFamilyName = "intensity" | "color" | "position" | "beam";

export interface FreezeTarget {
	fixture_id: string;
	full: boolean;
	families: FreezeFamilyName[];
}

/** Builds the Full/Partial Freeze rig on the bench's active twelve-dimmer show. */
export async function arrangeFreezeRig(
	{
		api,
		bench,
		desk,
		page,
		show,
	}: {
		api: ApiDriver;
		bench: LightBench;
		desk: DeskDriver;
		page: Page;
		show: TestShow;
	},
): Promise<FreezeRig> {
	await api.request("PUT", "/api/v2/configuration", {
		programmer_fade_millis: 0,
	});
	const patch = new BrowserPatch(api, page, desk);
	await patch.via.api.add({
		number: 101,
		name: "Wash 101",
		manufacturer: "ROBE",
		profile: "Robin 600X LEDWash",
		mode: "Mode 6",
		address: "1.101",
	});
	await patch.via.api.add({
		number: 201,
		name: "Par 201",
		manufacturer: "Cameo",
		profile: "ROOT PAR 6",
		mode: "D7CH — Delay Off, virtual dimmer",
		address: "1.201",
	});
	const fixtures = await patchFixtures(api);
	const wash = fixtures.find((fixture) => fixture.fixture_number === 101);
	const par = fixtures.find((fixture) => fixture.fixture_number === 201);
	if (!wash || !par) throw new Error("The Freeze rig fixtures were not patched");
	const washHeads = wash.logical_heads.map((head) => head.fixture_id);
	expect(washHeads).toHaveLength(3);
	const dimmers = show.fixtureIds.slice(0, 3) as [string, string, string];

	await api.executeCommandLine("1 THRU 3 + 101 + 201");
	await api.executeCommandLine(`RECORD GROUP ${FREEZE_GROUP}`);

	// Programmer contributions: Dimmer 1, the wash Master and PAR intensity, colour and Position.
	await setIntensity(api, show.id, dimmers[0], 0.6);
	await setIntensity(api, show.id, wash.fixture_id, 0.8);
	await setIntensity(api, show.id, par.fixture_id, 0.8);
	await programColor(api, [...washHeads, par.fixture_id], 120);
	await programAngles(api, show.id, wash.fixture_id, 30, 20);
	await setZoom(api, show.id, wash.fixture_id, 20);

	// Cue contribution: Dimmer 2 from a running Cue List.
	const cueListId = await createCueList(
		api,
		{ 2: dimmers[1] },
		"Freeze Cue",
		[0.5, 0.9],
		0,
		0,
		[2],
		false,
	);
	await saveSlot(
		api,
		1,
		1,
		definition(1, "Freeze Cue", {
			type: "cue_list",
			cue_list_id: cueListId,
		} as never),
	);
	await saveSlot(
		api,
		1,
		2,
		definition(2, "Freeze Group Master", {
			type: "group",
			group_id: FREEZE_GROUP,
		}),
	);
	const cuePlayback = (await playbackAt(api, 1, 1)).body.number;
	const groupMasterPlayback = (await playbackAt(api, 1, 2)).body.number;
	await poolAction(api, cuePlayback, "go");

	// Dynamic contribution: Dimmer 3 from a running intensity Dynamic.
	await api.executeCommandLine("3");
	const dynamics = new BrowserDynamics(api, () => show.id);
	await dynamics.apply(
		await dynamics.create({
			pool: 1,
			name: "Freeze Pulse",
			cycleMillis: 2_000,
			gridAngleDegrees: 0,
			lanes: [
				{
					attribute: "intensity",
					keyframes: [
						[0, 0.2],
						[0.5, 1],
					],
				},
			],
		}),
	);

	// Masters below Full, Blackout off.
	await setGroupMaster(api, groupMasterPlayback, 0.8);
	await setGlobalMaster(api, { grand_master: 0.9, blackout: false });
	await bench.tick(3_000);
	return {
		showId: show.id,
		dimmers,
		wash: wash.fixture_id,
		washHeads,
		par: par.fixture_id,
		cueListId,
		cuePlayback,
		groupMasterPlayback,
	};
}

/** Changes every contributing source and every master after a Freeze, Blackout on. */
export async function changeEverySource(
	api: ApiDriver,
	bench: LightBench,
	rig: FreezeRig,
): Promise<void> {
	await setIntensity(api, rig.showId, rig.dimmers[0], 0.1);
	await setIntensity(api, rig.showId, rig.wash, 0.3);
	await setIntensity(api, rig.showId, rig.par, 0.3);
	await programColor(api, [...rig.washHeads, rig.par], 240);
	await programAngles(api, rig.showId, rig.wash, -60, -20);
	await setZoom(api, rig.showId, rig.wash, 40);
	await poolAction(api, rig.cuePlayback, "go");
	await setGroupMaster(api, rig.groupMasterPlayback, 0.3);
	await setGlobalMaster(api, { grand_master: 0.4, blackout: true });
	// The Dynamic keeps running and the Cue fade completes.
	await bench.tick(1_300);
}

export async function setIntensity(
	api: ApiDriver,
	showId: string,
	fixtureId: string,
	value: number,
) {
	await setProgrammerFixtureValue(api, {
		surface: "api",
		showId,
		fixtureId,
		attribute: "intensity",
		value: { kind: "normalized", value },
		timing: IMMEDIATE,
	});
}

/** Beam source: the wash Master's Zoom (U1.121/122), a semantic opening in degrees. */
export async function setZoom(
	api: ApiDriver,
	showId: string,
	fixtureId: string,
	degrees: number,
) {
	await setProgrammerFixtureValue(api, {
		surface: "api",
		showId,
		fixtureId,
		attribute: "zoom",
		value: {
			kind: "zoom",
			value: { opening_degrees: { kind: "value", value: degrees }, convention: "beam" },
		} as never,
		timing: IMMEDIATE,
	});
}

export async function programAngles(
	api: ApiDriver,
	showId: string,
	fixtureId: string,
	pan: number,
	tilt: number,
) {
	await batchProgrammerValues(api, {
		surface: "api",
		showId,
		mutations: [
			{
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
				timing: IMMEDIATE,
			},
		],
	});
}

/** The semantic Color write: a white start edited to a fully saturated hue. */
export async function programColor(
	api: ApiDriver,
	fixtureIds: string[],
	hueDegrees: number,
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
	const outcome = await valuesAction(api, {
		type: "apply_intent",
		fixture_ids: fixtureIds,
		attribute: "color",
		operation: {
			type: "component_edits",
			edits: [
				scalarColorEdit("hue", hueDegrees),
				scalarColorEdit("saturation", 1),
			],
		},
		timing: { fade: false },
	});
	expect(outcome.status, JSON.stringify(outcome)).toBe("changed");
}

function scalarColorEdit(component: string, value: number) {
	return {
		kind: "scalar",
		component: { kind: "color", component },
		operation: { kind: "set", value: { kind: "value", value } },
	};
}

async function valuesAction(api: ApiDriver, action: Record<string, unknown>) {
	const [capture, values] = await Promise.all([
		api.request<{ projection: { revision: number } }>(
			"GET",
			"/api/v2/programmer/capture-mode/snapshot",
		),
		api.request<{ projection: { revision: number } }>(
			"GET",
			"/api/v2/programmer/values/snapshot",
		),
	]);
	return api.request<Record<string, unknown>>(
		"POST",
		"/api/v2/programmer/values/actions",
		{
			request_id: crypto.randomUUID(),
			expected_revision: values.projection.revision,
			expected_capture_mode_revision: capture.projection.revision,
			action,
		},
	);
}

/** Moves a Group Master fader; a fresh fader first picks up the master at Full. */
export async function setGroupMaster(
	api: ApiDriver,
	playback: number,
	value: number,
) {
	await poolAction(api, playback, "master", { value: 1 });
	await poolAction(api, playback, "master", { value });
}

export async function setGlobalMaster(
	api: ApiDriver,
	input: { grand_master?: number; blackout?: boolean },
) {
	await api.request(
		"POST",
		"/api/v2/output-runtime/global-master/actions",
		input,
	);
}

/** Advances the manual clock one frame and returns both the logical and the Art-Net output. */
export async function outputFrame(
	bench: LightBench,
	millis = 25,
): Promise<FreezeFrame> {
	const mark = bench.artnet.mark();
	const frame = await bench.tick(millis);
	const logical =
		frame.universes.find((universe) => universe.universe === 1)?.slots ?? [];
	const packet = await bench.artnet.nextAfter(mark, "artnet", 1);
	return { logical: [...logical], physical: Array.from(packet.slots) };
}

/** The physical Art-Net bytes at the given 1-based addresses, after checking the logical frame. */
export function slots(
	frame: FreezeFrame,
	addresses: readonly number[],
): number[] {
	const physical = addresses.map((address) => frame.physical[address - 1]);
	expect(
		addresses.map((address) => frame.logical[address - 1]),
		"the Art-Net packet carries the logical frame",
	).toEqual(physical);
	return physical;
}

interface PatchFixtureWire {
	fixture_id: string;
	fixture_number: number;
	fixture_revision: number;
	name: string;
	logical_heads: Array<{ fixture_id: string }>;
	freeze_targets?: FreezeTarget[];
}

export async function patchFixtures(api: ApiDriver) {
	return (
		await api.request<{ fixtures: PatchFixtureWire[]; patch_revision: number }>(
			"GET",
			"/api/v2/patch",
		)
	).fixtures;
}

export async function patchRevision(api: ApiDriver) {
	return (
		await api.request<{ patch_revision: number }>("GET", "/api/v2/patch")
	).patch_revision;
}

/** Every stored Freeze target, keyed by the target fixture or head id, in a stable order. */
export async function freezeTargets(
	api: ApiDriver,
): Promise<Record<string, Omit<FreezeTarget, "fixture_id">>> {
	const targets = (await patchFixtures(api)).flatMap(
		(fixture) => fixture.freeze_targets ?? [],
	);
	return Object.fromEntries(
		targets
			.sort((left, right) => left.fixture_id.localeCompare(right.fixture_id))
			.map(({ fixture_id, full, families }) => [
				fixture_id,
				{ full, families: [...families].sort() },
			]),
	);
}

/** Semantic visualization values of the given fixtures, keyed `fixture/attribute`. */
export async function visualizationValues(
	api: ApiDriver,
	fixtureIds: readonly string[],
	attributes: readonly string[],
) {
	const visualization = await api.request<{
		values: Array<{ fixture_id: string; attribute: string; value: unknown }>;
	}>("GET", "/api/v2/output/visualization");
	return Object.fromEntries(
		visualization.values
			.filter(
				(entry) =>
					fixtureIds.includes(entry.fixture_id) &&
					attributes.includes(entry.attribute),
			)
			.map((entry) => [`${entry.fixture_id}/${entry.attribute}`, entry.value])
			.sort(([left], [right]) => String(left).localeCompare(String(right))),
	);
}

/** Programmer intensity values by fixture id, as the Programmer stores them. */
export async function programmerIntensities(api: ApiDriver) {
	const programmers = await api.request<
		Array<{
			session_id: string;
			values: Array<{
				fixture_id: string;
				attribute: string;
				value: { value?: number } | number;
			}>;
		}>
	>("GET", "/api/v2/programmers");
	const own =
		programmers.find(
			(programmer) => programmer.session_id === api.session?.session_id,
		) ?? programmers[0];
	return Object.fromEntries(
		(own?.values ?? [])
			.filter((value) => value.attribute === "intensity")
			.map((value) => [
				value.fixture_id,
				typeof value.value === "number" ? value.value : value.value.value,
			]),
	);
}

/** The visible Fixture Sheet and its per-row Freeze status. */
export class FixtureSheetFreeze {
	constructor(private readonly page: Page) {}

	async open(): Promise<void> {
		const entry = this.page
			.locator(".dock-entry")
			.filter({ hasText: "Fixtures" })
			.first();
		if (!(await entry.isVisible()))
			await this.page
				.getByRole("button", { name: "Desktops / Built-ins", exact: true })
				.click();
		await entry.click();
		await expect(this.page.locator(".fixture-window")).toBeVisible();
	}

	row(name: string): Locator {
		return this.page
			.locator(".fixture-window .ui-data-table-row:not(.header)")
			.filter({
				has: this.page.getByText(name, { exact: true }),
			});
	}

	status(name: string): Locator {
		return this.row(name).locator(".fixture-freeze-status");
	}
}

/** The visible software command keypad, with Shift held on the desk keyboard like an operator. */
export class FreezeKeypad {
	constructor(
		private readonly page: Page,
		private readonly desk: DeskDriver,
	) {}

	get commandLine(): Locator {
		return this.page.getByRole("textbox", { name: "Command line", exact: true });
	}

	key(key: string): Locator {
		if (key === "ESC")
			return this.page.locator(".command-escape:visible").first();
		return this.page.locator(`[data-keypad-key="${key}"]:visible`).first();
	}

	async escape(): Promise<void> {
		await this.desk.click(this.key("ESC"));
		await expect(this.commandLine).toHaveValue(/^(?:FIXTURE|GROUP)$/);
	}

	async press(...keys: string[]): Promise<void> {
		for (const key of keys) await this.desk.click(this.key(key));
	}

	/** Holds SHIFT, presses `keys`, and releases SHIFT. */
	async shifted(...keys: string[]): Promise<void> {
		await this.page.keyboard.down("Shift");
		try {
			await this.press(...keys);
		} finally {
			await this.page.keyboard.up("Shift");
		}
	}

	/** `[^CLR]` (`[^CLR][^CLR]` for Unfreeze), the selection keys, family keys, then `[ENT]`. */
	async enter(
		operation: "FREEZE" | "UNFREEZE",
		selection: string[],
		families: Array<"1" | "2" | "3" | "4"> = [],
		visible?: string,
	): Promise<void> {
		await this.escape();
		await this.shifted(...(operation === "FREEZE" ? ["CLR"] : ["CLR", "CLR"]));
		await expect(this.commandLine).toHaveValue(operation);
		await this.press(...selection);
		if (families.length) await this.shifted(...families);
		if (visible) await expect(this.commandLine).toHaveValue(visible);
		await this.press("ENT");
		await expect(this.commandLine).toHaveValue(/^(?:FIXTURE|GROUP)$/);
	}

	async undo(): Promise<void> {
		await this.shifted("ESC");
	}
}

/** OSC programmer keys on one subscribed controller path. */
export class FreezeOscKeys {
	constructor(
		private readonly hardware: Pick<OscHardware, "send">,
		private readonly alias: string,
	) {}

	/**
	 * One key edge. OSC is UDP and the desk handles datagrams independently, so a controller's
	 * keys are spaced like a human's presses rather than sent back to back.
	 */
	async key(action: string, pressed = true): Promise<void> {
		await this.hardware.send(`/light/${this.alias}/programmer/${action}`, [
			pressed,
		]);
		await new Promise<void>((resolve) => setTimeout(resolve, 40));
	}

	async keys(...actions: string[]): Promise<void> {
		for (const action of actions) await this.key(action);
	}
}

export async function commandLineText(api: ApiDriver) {
	return (await api.getCommandLine()).commandLine.text;
}

export async function cueListBody(api: ApiDriver, showId: string, id: string) {
	return (await api.showObject<Record<string, unknown>>(showId, "cue_list", id))
		?.body;
}
