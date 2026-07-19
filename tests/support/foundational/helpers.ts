import {
	type ApiDriver,
	commandLineRequiresLegacyCompatibility,
} from "../../../apps/control-ui/e2e/bench/api";
import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import type { Page } from "../../../apps/control-ui/node_modules/@playwright/test/index.js";
import { loadCanonicalCopy } from "../catalog";

export interface VersionedObject<T = Record<string, any>> {
	kind: string;
	id: string;
	revision: number;
	body: T;
}

interface ShowEntry {
	id: string;
	name: string;
}

export interface ProgrammerState {
	selected: string[];
	selection_expression: any;
	values: Array<{
		fixture_id: string;
		attribute: string;
		value: { value?: number } | number;
	}>;
	group_values: Record<
		string,
		Record<string, { value: { value?: number } | number }>
	>;
	command_line: string;
}

export const INTENSITY = "intensity";

export async function loadCompactRig(
	api: ApiDriver,
	bench: any,
	name: string,
): Promise<void> {
	await loadCanonicalCopy(api, bench, name);
	await api.command("selection.set", { fixtures: [] });
	await api.command("programmer.clear", {});
	const group4 = (await objects(api, "group")).find(
		(group) => group.id === "4",
	);
	await putObject(
		api,
		"group",
		"4",
		{
			id: "4",
			name: "Center Spot",
			fixtures: [],
			derived_from: null,
			frozen_from: null,
			programming: {},
			master: 1,
			playback_fader: null,
		},
		group4?.revision ?? 0,
	);
}

export async function command(api: ApiDriver, value: string): Promise<void> {
	if (commandLineRequiresLegacyCompatibility(value)) {
		await api.executeLegacyCommandLine(value);
	} else {
		await api.executeCommandLine(value);
	}
}

export async function commandError(
	api: ApiDriver,
	value: string,
): Promise<string> {
	try {
		await command(api, value);
	} catch (error) {
		return error instanceof Error ? error.message : String(error);
	}
	throw new Error(`Expected command to fail: ${value}`);
}

export async function pressCommand(
	page: Page,
	value: string,
	visibleValue = value,
): Promise<void> {
	const commandLine = page.getByLabel("Command line");
	await page.getByRole("button", { name: "ESC", exact: true }).click();
	for (const key of commandKeys(value)) {
		await page.getByRole("button", { name: key, exact: true }).click();
	}
	await expect(commandLine).toHaveValue(visibleValue);
	await page.getByRole("button", { name: "ENT", exact: true }).click();
}

export async function pressCommandAndWait(
	page: Page,
	value: string,
	visibleValue = value,
): Promise<void> {
	await pressCommand(page, value, visibleValue);
	await expect(page.getByLabel("Command line")).toHaveValue(
		/^(FIXTURE|GROUP)$/,
	);
}

export async function enterCommandWithoutEscape(
	page: Page,
	value: string,
	visibleValue = value,
): Promise<void> {
	const commandLine = page.getByLabel("Command line");
	for (const key of commandKeys(value))
		await page.getByRole("button", { name: key, exact: true }).click();
	await expect(commandLine).toHaveValue(visibleValue);
	await page.getByRole("button", { name: "ENT", exact: true }).click();
	await expect(commandLine).toHaveValue("FIXTURE");
}

function commandKeys(value: string): string[] {
	return value
		.trim()
		.split(/\s+/)
		.flatMap((token) => {
			if (token === "GROUP") return ["GRP"];
			if (token === "DEGRP") return ["GRP", "GRP"];
			if (token === "THRU") return ["TRU"];
			if (token === "RECORD") return ["REC"];
			if (token === "DELETE") return ["DEL"];
			if (token === "DIV") return ["DIV"];
			if (/^\d+$/.test(token)) return [...token];
			return [token];
		});
}

export async function programmer(api: ApiDriver): Promise<ProgrammerState> {
	const programmers = await api.request<ProgrammerState[]>(
		"GET",
		"/api/v1/programmers",
		undefined,
		false,
	);
	const current =
		programmers.find(
			(item: any) => item.session_id === api.session?.session_id,
		) ?? programmers[0];
	expect(current).toBeDefined();
	return current;
}

export async function expectProgrammer(
	api: ApiDriver,
	assertion: (programmer: ProgrammerState) => void | Promise<void>,
): Promise<void> {
	await expect
		.poll(
			async () => {
				const programmers = await api.request<ProgrammerState[]>(
					"GET",
					"/api/v1/programmers",
					undefined,
					false,
				);
				let lastError: unknown = null;
				for (const snapshot of programmers) {
					try {
						await assertion(snapshot);
						return true;
					} catch (error) {
						lastError = error;
					}
				}
				if (lastError) throw lastError;
				throw new Error("No programmer matched assertion");
			},
			{ timeout: 2_000 },
		)
		.toBe(true);
}

export async function select(
	api: ApiDriver,
	fixtures: string[],
): Promise<void> {
	await api.command("selection.set", { fixtures });
}

export async function gestureFixture(
	api: ApiDriver,
	fixtureId: string,
	remove = false,
): Promise<void> {
	await api.command("selection.gesture", {
		source: { type: "fixture", fixture_id: fixtureId },
		remove,
	});
}

export async function gestureGroup(
	api: ApiDriver,
	groupId: string,
	remove = false,
): Promise<void> {
	await api.command("selection.gesture", {
		source: { type: "live_group", group_id: groupId },
		remove,
	});
}

export async function objects<T = Record<string, any>>(
	api: ApiDriver,
	kind: string,
): Promise<Array<VersionedObject<T>>> {
	const bootstrap = await api.request<{ active_show: ShowEntry | null }>(
		"GET",
		"/api/v1/bootstrap",
		undefined,
		false,
	);
	expect(bootstrap.active_show).toBeTruthy();
	const result = await api.request<Array<VersionedObject<T>>>(
		"GET",
		`/api/v1/shows/${bootstrap.active_show!.id}/objects/${kind}`,
		undefined,
		false,
	);
	return result.sort((left, right) =>
		left.id.localeCompare(right.id, undefined, { numeric: true }),
	);
}

export async function object<T = Record<string, any>>(
	api: ApiDriver,
	kind: string,
	id: string,
): Promise<VersionedObject<T>> {
	const found = (await objects<T>(api, kind)).find((item) => item.id === id);
	expect(found).toBeDefined();
	return found!;
}

export async function putObject(
	api: ApiDriver,
	kind: string,
	id: string,
	body: unknown,
	revision = 0,
): Promise<void> {
	const bootstrap = await api.request<{ active_show: ShowEntry | null }>(
		"GET",
		"/api/v1/bootstrap",
		undefined,
		false,
	);
	expect(bootstrap.active_show).toBeTruthy();
	await api.request(
		"PUT",
		`/api/v1/shows/${bootstrap.active_show!.id}/objects/${kind}/${id}`,
		body,
		true,
		revision,
	);
}

export async function fixtureIdsByNumber(
	api: ApiDriver,
): Promise<Record<number, string>> {
	const fixtures = await objects(api, "patched_fixture");
	return Object.fromEntries(
		fixtures.map((fixture) => [
			fixture.body.fixture_number,
			fixture.body.fixture_id,
		]),
	);
}

async function fixtureNumberById(
	api: ApiDriver,
): Promise<Record<string, number>> {
	const fixtures = await objects(api, "patched_fixture");
	return Object.fromEntries(
		fixtures.map((fixture) => [
			fixture.body.fixture_id,
			fixture.body.fixture_number,
		]),
	);
}

export async function expectSelectedNumbers(
	api: ApiDriver,
	expected: number[],
): Promise<void> {
	const byId = await fixtureNumberById(api);
	await expectProgrammer(api, (snapshot) => {
		expect(snapshot.selected.map((id) => byId[id])).toEqual(expected);
	});
}

export async function expectGroup(
	api: ApiDriver,
	id: string,
	assertion: (group: VersionedObject) => void,
): Promise<void> {
	await expect
		.poll(
			async () => {
				const group = (await objects(api, "group")).find(
					(item) => item.id === id,
				);
				expect(group).toBeDefined();
				assertion(group!);
				return true;
			},
			{ timeout: 2_000 },
		)
		.toBe(true);
}

export async function expectGroupMissing(
	api: ApiDriver,
	id: string,
): Promise<void> {
	await expect
		.poll(
			async () => (await objects(api, "group")).some((item) => item.id === id),
			{ timeout: 2_000 },
		)
		.toBe(false);
}

export async function expectGroupNumbers(
	api: ApiDriver,
	id: string,
	expected: number[],
): Promise<void> {
	const byId = await fixtureNumberById(api);
	await expectGroup(api, id, (group) =>
		expect(group.body.fixtures.map((fixture: string) => byId[fixture])).toEqual(
			expected,
		),
	);
}

export async function setGroupByNumbers(
	api: ApiDriver,
	id: string,
	name: string,
	numbers: number[],
): Promise<void> {
	const byNumber = await fixtureIdsByNumber(api);
	const existing = (await objects(api, "group")).find(
		(group) => group.id === id,
	);
	await putObject(
		api,
		"group",
		id,
		{
			...(existing?.body ?? {}),
			id,
			name,
			fixtures: numbers.map((number) => byNumber[number]),
			derived_from: null,
			frozen_from: null,
			programming: existing?.body.programming ?? {},
			master: existing?.body.master ?? 1,
			playback_fader: existing?.body.playback_fader ?? null,
		},
		existing?.revision ?? 0,
	);
}

export async function overwriteGroupByNumbers(
	api: ApiDriver,
	id: string,
	numbers: number[],
): Promise<void> {
	const byNumber = await fixtureIdsByNumber(api);
	const existing = await object(api, "group", id);
	await putObject(
		api,
		"group",
		id,
		{
			...existing.body,
			fixtures: numbers.map((number) => byNumber[number]),
			derived_from: null,
			frozen_from: null,
		},
		existing.revision,
	);
}

export async function unpatchFixture(
	api: ApiDriver,
	fixtureId: string,
): Promise<void> {
	const fixture = (await objects(api, "patched_fixture")).find(
		(item) => item.body.fixture_id === fixtureId,
	);
	expect(fixture).toBeDefined();
	await putObject(
		api,
		"patched_fixture",
		fixture!.id,
		{ ...fixture!.body, universe: null, address: null },
		fixture!.revision,
	);
}

export async function expectSlotsAfterTick(
	bench: any,
	millis: number,
	expected: number[],
): Promise<void> {
	const artnetMark = bench.artnet.mark();
	const sacnMark = bench.sacn.mark();
	const tick = await bench.tick(millis);
	const slots =
		tick.universes.find((universe: any) => universe.universe === 1)?.slots ??
		[];
	expect(slots.slice(0, expected.length)).toEqual(expected);
	const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", 1);
	const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", 101);
	expect(Array.from(artnet.slots.slice(0, expected.length))).toEqual(expected);
	expect(Array.from(sacn.slots.slice(0, expected.length))).toEqual(expected);
}

export function slotsFromFrame(
	frame: { universes: Array<{ universe: number; slots: number[] }> },
	count: number,
): number[] {
	return (
		frame.universes.find((universe) => universe.universe === 1)?.slots ?? []
	).slice(0, count);
}

export function normalized(
	value: { value?: number } | number | undefined,
): number | undefined {
	return typeof value === "number" ? value : value?.value;
}

export async function openBuiltIn(page: Page, name: string): Promise<void> {
	const entry = page.locator(".dock-entry").filter({ hasText: name }).first();
	if (!(await entry.isVisible()))
		await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
	await expect(entry).toBeVisible();
	await entry.click();
}

export async function openGroups(page: Page): Promise<void> {
	if (!(await page.locator(".group-pool-window").isVisible())) {
		await page.getByRole("button", { name: "SHIFT", exact: true }).click();
		await page.getByRole("button", { name: "1", exact: true }).click();
	}
	await expect(page.locator(".group-pool-window")).toBeVisible();
}

export async function openFixtures(page: Page): Promise<void> {
	await openBuiltIn(page, "Fixtures");
	await expect(page.locator(".fixture-window")).toBeVisible();
}

export async function openPatch(page: Page): Promise<void> {
	if (await page.locator(".patch-table").isVisible()) return;
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Show Patch", exact: true }).click();
	await expect(page.locator(".patch-table")).toBeVisible();
}

export function patchFixtureRow(page: Page, number: number) {
	return page
		.locator(".patch-table tbody tr")
		.filter({
			has: page
				.locator("td:nth-child(2)")
				.filter({ hasText: new RegExp(`^${number}$`) }),
		})
		.first();
}

export function groupCard(page: Page, number: number) {
	return page.locator(".group-pool-window .group-card").nth(number - 1);
}

export async function recordExistingGroup(
	page: Page,
	number: number,
	mode: "Merge" | "Overwrite",
): Promise<void> {
	await openGroups(page);
	await page.locator(".global-store-button").click();
	await expect(page.locator(".global-store-button")).toHaveText("REC ARMED");
	await groupCard(page, number).click();
	const dialog = page.locator(".record-mode-dialog");
	await expect(dialog).toBeVisible();
	await dialog.getByRole("button", { name: mode, exact: true }).click();
	await expect(dialog).toBeHidden();
}

export async function expectVisibleGroupOrder(
	page: Page,
	number: number,
	fixtures: number[],
): Promise<void> {
	await openGroups(page);
	const card = groupCard(page, number);
	const box = await card.boundingBox();
	expect(box).toBeTruthy();
	await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
	await page.mouse.down();
	await page.waitForTimeout(700);
	await page.mouse.up();
	const order = page.locator(".group-context-menu .group-order");
	await expect(order).toBeVisible();
	for (const [index, fixture] of fixtures.entries())
		await expect(order).toContainText(`${index + 1}. Fixture ${fixture}`);
	await page
		.locator(".group-context-menu")
		.getByRole("button", { name: "Cancel", exact: true })
		.click();
}

export function fixtureRow(page: Page, number: number) {
	return page
		.locator(".fixture-window .ui-data-table-row:not(.header)")
		.filter({
			has: page.getByRole("cell", { name: String(number), exact: true }),
		})
		.first();
}

export async function selectFixtureRows(
	api: ApiDriver,
	page: Page,
	fixtures: number[],
): Promise<void> {
	await openFixtures(page);
	for (const [index, fixture] of fixtures.entries()) {
		await fixtureRow(page, fixture).click();
		await expectSelectedNumbers(api, fixtures.slice(0, index + 1));
	}
}

export function stageFixture(page: Page, fixtureId: string) {
	return page.locator(`.stage-fixture[data-fixture-id="${fixtureId}"]`);
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
