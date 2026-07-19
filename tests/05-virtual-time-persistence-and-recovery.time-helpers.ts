import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import type { DeskDriver } from "../apps/control-ui/e2e/bench/desk";
import { expect } from "../apps/control-ui/e2e/bench/fixtures";
import type { LightBench } from "../apps/control-ui/e2e/bench/lightBench";
import type { OscHardware } from "../apps/control-ui/e2e/bench/protocols";
import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { normalized, programmer } from "./support/catalog";

export const FIXED_NOW = "2020-01-01T00:00:00Z";
export type HardwareState = {
	hardware?: OscHardware;
	hardwareClientId?: string;
};

export async function assertZeroTicks(
	api: ApiDriver,
	bench: LightBench,
): Promise<void> {
	const before = behaviorTimestamps(await programmer(api));
	const osc = await bench.osc();
	const deskAlias = api.session!.desk.osc_alias;
	const pageFeedback = `/light/${deskAlias}/feedback/page`;
	const clientId = `time-001-${crypto.randomUUID()}`;
	try {
		await osc.subscribe(clientId, deskAlias);
		// Subscription feedback is a full asynchronous burst. Drain it before marking the two
		// explicitly clocked cycles so UDP delivery already in flight cannot be misattributed.
		await new Promise<void>((resolve) => setTimeout(resolve, 75));
		const bursts: Array<{
			now: string;
			artnetSequence: number;
			sacnSequence: number;
		}> = [];
		for (let call = 0; call < 2; call += 1) {
			const artnetMark = bench.artnet.mark();
			const sacnMark = bench.sacn.mark();
			const oscMark = osc.mark();
			const frame = await bench.tick(0);
			expect(frame).toMatchObject({ now: FIXED_NOW, packets_sent: 2 });
			const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", 1);
			const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", 101);
			await osc.expectAfter(oscMark, pageFeedback);
			await new Promise<void>((resolve) => setTimeout(resolve, 20));
			expect(
				bench.artnet.packets
					.slice(artnetMark)
					.filter(
						(packet) => packet.protocol === "artnet" && packet.universe === 1,
					),
			).toHaveLength(1);
			expect(
				bench.sacn.packets
					.slice(sacnMark)
					.filter(
						(packet) => packet.protocol === "sacn" && packet.universe === 101,
					),
			).toHaveLength(1);
			expect(
				osc.messages
					.slice(oscMark)
					.filter((message) => message.address === pageFeedback),
			).toHaveLength(1);
			expect(Array.from(artnet.slots.slice(0, 12))).toEqual([
				128,
				...Array(11).fill(0),
			]);
			expect(Array.from(sacn.slots.slice(0, 12))).toEqual([
				128,
				...Array(11).fill(0),
			]);
			bursts.push({
				now: frame.now,
				artnetSequence: artnet.sequence,
				sacnSequence: sacn.sequence,
			});
		}
		expect(bursts.map((burst) => burst.now)).toEqual([FIXED_NOW, FIXED_NOW]);
		expectSequenceIncrement(bursts[0].artnetSequence, bursts[1].artnetSequence);
		expectSequenceIncrement(bursts[0].sacnSequence, bursts[1].sacnSequence);
		expect(behaviorTimestamps(await programmer(api))).toEqual(before);
	} finally {
		await osc.send("/light/unsubscribe", [clientId]).catch(() => undefined);
		await osc.close();
	}
}

export async function connectHardware(
	api: ApiDriver,
	bench: LightBench,
	state: HardwareState,
	prefix: string,
): Promise<void> {
	state.hardware = await bench.osc();
	state.hardwareClientId = `${prefix}-${crypto.randomUUID()}`;
	await state.hardware.subscribe(
		state.hardwareClientId,
		api.session!.desk.osc_alias,
	);
	await expect
		.poll(
			async () =>
				(await api.request<any>("GET", "/api/v1/bootstrap", undefined, false))
					.hardware_connected,
		)
		.toBe(true);
}

export async function disconnectHardware(
	api: ApiDriver,
	state: HardwareState,
): Promise<void> {
	if (!state.hardware || !state.hardwareClientId) return;
	await state.hardware.send("/light/unsubscribe", [state.hardwareClientId]);
	await expect
		.poll(
			async () =>
				(await api.request<any>("GET", "/api/v1/bootstrap", undefined, false))
					.hardware_connected,
		)
		.toBe(false);
	await state.hardware.close();
}
export async function assertFadeBoundaries(
	api: ApiDriver,
	bench: LightBench,
	fixtureId: string,
): Promise<void> {
	const increments = [0, 1, 1_498, 1, 1, 1_498, 1, 1];
	const checkpoints = [0, 1, 1_499, 1_500, 1_501, 2_999, 3_000, 3_001];
	const levels: number[] = [];
	for (let index = 0; index < increments.length; index += 1) {
		const artnetMark = bench.artnet.mark();
		const sacnMark = bench.sacn.mark();
		const frame = await bench.tick(increments[index]);
		const level = await visualizationLevel(api, fixtureId, "intensity");
		const expectedLevel = Math.min(checkpoints[index] / 3_000, 1);
		const expectedByte = Math.round(expectedLevel * 255);
		levels.push(level);
		expect(level).toBeCloseTo(expectedLevel, 6);
		expect(slot(frame, 1)).toBe(expectedByte);
		const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", 1);
		const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", 101);
		expect(artnet.slots[0]).toBe(expectedByte);
		expect(sacn.slots[0]).toBe(expectedByte);
	}
	expect(levels).toEqual([...levels].sort((left, right) => left - right));
	expect(levels[3]).toBeCloseTo(0.5, 8);
	expect(Math.round(levels[3] * 255)).toBe(128);
	expect(levels[6]).toBe(1);
	expect(levels[7]).toBe(1);
}

export async function expectEncoderTarget(
	page: Page,
	percent: number,
): Promise<void> {
	const encoder = page.locator(".vertical-touch-fader-stack").filter({
		hasText: "Enc 1 · Dimmer",
	});
	await expect(encoder.locator(".vertical-touch-fader > strong")).toHaveText(
		`${percent}%`,
	);
}

export async function expectFixtureSheetDimmer(
	page: Page,
	fixtureNumber: number,
	percent: number,
): Promise<void> {
	await expect(
		fixtureRow(page, fixtureNumber).getByRole("cell").nth(2),
	).toContainText(`${percent}%`);
}

export async function recordFirstCuelistThroughUi(
	api: ApiDriver,
	page: Page,
): Promise<any> {
	await openBuiltIn(page, "Cuelists");
	await page.locator(".global-store-button").click();
	await expect(page.locator(".global-store-button")).toHaveText("REC ARMED");
	await page.locator(".cuelist-card").first().click();
	await expect
		.poll(async () => {
			const playbacks = await api.request<any>("GET", "/api/v1/playbacks");
			return playbacks.pool.some((definition: any) => definition.number === 1);
		})
		.toBe(true);
	const playbacks = await api.request<any>("GET", "/api/v1/playbacks");
	const definition = playbacks.pool.find(
		(candidate: any) => candidate.number === 1,
	);
	const cueList = playbacks.cue_lists.find(
		(candidate: any) => candidate.id === definition?.target?.cue_list_id,
	);
	expect(cueList?.cues).toHaveLength(1);
	return cueList.cues[0];
}

export async function assertCueReplayBoundaries(
	api: ApiDriver,
	bench: LightBench,
	page: Page,
	desk: DeskDriver,
	targets: Array<{ fixtureId: string; number: number; slot: number }>,
): Promise<void> {
	const increments = [0, 1, 1_498, 1, 1, 1_498, 1, 1];
	const checkpoints = [0, 1, 1_499, 1_500, 1_501, 2_999, 3_000, 3_001];
	const observed = new Map<string, number[]>();
	for (let index = 0; index < increments.length; index += 1) {
		const artnetMark = bench.artnet.mark();
		const sacnMark = bench.sacn.mark();
		const frame = await bench.tick(increments[index]);
		const expectedLevel = Math.min(checkpoints[index] / 3_000, 1);
		const expectedByte = Math.round(expectedLevel * 255);
		const expectedPercent = Math.round(expectedLevel * 100);
		for (const target of targets) {
			const level = await visualizationLevel(
				api,
				target.fixtureId,
				"intensity",
			);
			observed.set(target.fixtureId, [
				...(observed.get(target.fixtureId) ?? []),
				level,
			]);
			expect(level).toBeCloseTo(expectedLevel, 6);
			expect(slot(frame, target.slot)).toBe(expectedByte);
			await expectFixtureSheetDimmer(page, target.number, expectedPercent);
		}
		const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", 1);
		const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", 101);
		for (const target of targets) {
			expect(artnet.slots[target.slot - 1]).toBe(expectedByte);
			expect(sacn.slots[target.slot - 1]).toBe(expectedByte);
		}
		await desk.recordStep(
			`CUE FADE · ${checkpoints[index]} ms · ${expectedPercent}%`,
			`Fixture Sheet, resolved engine value, logical DMX, Art-Net, and sACN agree at ${expectedPercent}% (${expectedByte}/255).`,
		);
	}
	for (const levels of observed.values()) {
		expect(levels).toEqual([...levels].sort((left, right) => left - right));
		expect(levels[3]).toBeCloseTo(0.5, 8);
		expect(levels[6]).toBe(1);
		expect(levels[7]).toBe(1);
	}
}

export async function setProgrammerFadeThroughUi(
	api: ApiDriver,
	page: Page,
	seconds: number,
): Promise<void> {
	await page
		.locator(".hardware-control-summary")
		.getByRole("button")
		.filter({ hasText: "Prog Fade" })
		.click();
	const dialog = page
		.locator(".direct-value-modal")
		.filter({ hasText: "Prog. Fade" });
	await expect(dialog).toBeVisible();
	await page.keyboard.type(String(seconds));
	await page.keyboard.press("Enter");
	await expect(dialog).toBeHidden();
	await expect
		.poll(async () => {
			const response = await api.request<any>("GET", "/api/v1/configuration");
			return (response.configuration ?? response).programmer_fade_millis;
		})
		.toBe(seconds * 1_000);
}
export async function openBuiltIn(page: Page, name: string): Promise<void> {
	const entry = page.locator(".dock-entry").filter({ hasText: name }).first();
	if (!(await entry.isVisible()))
		await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
	await expect(entry).toBeVisible();
	await entry.click();
}

export async function openFixtures(page: Page): Promise<void> {
	await openBuiltIn(page, "Fixtures");
	await expect(page.locator(".fixture-window")).toBeVisible();
}

export function fixtureRow(page: Page, number: number) {
	return page
		.locator(".fixture-window .ui-data-table-row:not(.header)")
		.filter({
			has: page.getByRole("cell", { name: String(number), exact: true }),
		})
		.first();
}

export async function openShiftedWindow(
	page: Page,
	key: string,
	windowSelector: string,
): Promise<void> {
	if (await page.locator(windowSelector).isVisible()) return;
	const shift = page.getByRole("button", { name: "SHIFT", exact: true });
	if (!(await shift.isVisible())) await page.locator(".mode-toggle").click();
	await shift.click();
	await page.getByRole("button", { name: key, exact: true }).click();
	await expect(page.locator(windowSelector)).toBeVisible();
}

export async function openGroups(page: Page): Promise<void> {
	await openShiftedWindow(page, "1", ".group-pool-window");
}

export async function openCuelistPool(page: Page): Promise<void> {
	await openShiftedWindow(page, "4", ".cuelist-pool-window");
}

export function groupCard(page: Page, number: number) {
	return page.locator(".group-pool-window .group-card").nth(number - 1);
}

export async function setDimmerByTouch(
	page: Page,
	value: number,
): Promise<void> {
	const encoder = page
		.locator(".vertical-touch-fader-stack")
		.filter({ hasText: "Enc 1 · Dimmer" });
	await encoder.getByRole("button", { name: "Set value" }).click();
	const dialog = page.getByRole("dialog", { name: "Enc 1 · Dimmer value" });
	await expect(dialog).toBeVisible();
	await page.keyboard.type(String(value));
	await page.keyboard.press("Enter");
	await expect(dialog).toBeHidden();
}

export async function setProgrammerFade(
	api: ApiDriver,
	millis: number,
	sequenceMasterFadeMillis?: number,
): Promise<void> {
	const response = await api.request<any>("GET", "/api/v1/configuration");
	const configuration = response.configuration ?? response;
	await api.request("PUT", "/api/v1/configuration", {
		...configuration,
		programmer_fade_millis: millis,
		...(sequenceMasterFadeMillis == null
			? {}
			: { sequence_master_fade_millis: sequenceMasterFadeMillis }),
	});
}

export async function setSpeedGroups(
	api: ApiDriver,
	speedGroups: number[],
): Promise<void> {
	const response = await api.request<any>("GET", "/api/v1/configuration");
	const configuration = response.configuration ?? response;
	await api.request("PUT", "/api/v1/configuration", {
		...configuration,
		speed_groups_bpm: speedGroups,
		sequence_master_fade_millis: 0,
	});
}

export function behaviorTimestamps(state: any): unknown {
	return {
		last_activity: state.last_activity,
		values: state.values.map((value: any) => value.changed_at),
		groups: Object.fromEntries(
			Object.entries(state.group_values ?? {}).map(
				([group, attributes]: [string, any]) => [
					group,
					Object.fromEntries(
						Object.entries(attributes).map(
							([attribute, value]: [string, any]) => [
								attribute,
								value.changed_at,
							],
						),
					),
				],
			),
		),
	};
}

export function expectSequenceIncrement(before: number, after: number): void {
	expect(after).toBe(before >= 255 ? 1 : before + 1);
}

export function slot(
	frame: { universes: Array<{ universe: number; slots: number[] }> },
	address: number,
): number | undefined {
	return frame.universes.find((universe) => universe.universe === 1)?.slots[
		address - 1
	];
}

export async function visualizationLevel(
	api: ApiDriver,
	fixtureId: string,
	attribute: string,
): Promise<number> {
	const visualization = await api.request<any>("GET", "/api/v1/visualization");
	return (
		normalized(
			visualization.values.find(
				(item: any) =>
					item.fixture_id === fixtureId && item.attribute === attribute,
			)?.value,
		) ?? 0
	);
}
