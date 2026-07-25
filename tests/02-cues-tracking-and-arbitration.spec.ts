import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { pairedScenario } from "./bench/core/pairedScenario";
import { applySpeedGroupRuntimeAction } from "./bench/playbacks/speedGroupRuntime";
import type { Page } from "@playwright/test";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	objects,
	putObject,
} from "./support/catalog";

const cueTransfers = [
	{ operation: "COPY", mode: "Plain", moves: false, status: false },
	{ operation: "MOVE", mode: "Plain", moves: true, status: false },
	{ operation: "COPY", mode: "Status", moves: false, status: true },
	{ operation: "MOVE", mode: "Status", moves: true, status: true },
] as const;
type CueTransfer = (typeof cueTransfers)[number];

test.describe("docs/testing/02-cues-tracking-and-arbitration.md", () => {
	pairedScenario<{ completed: boolean }>({
		id: "CUE-009",
		title:
			"explicit Plain/Status Move/Copy choices preserve both independent axes",
		surfaces: ["api"],
		arrange: () => ({ completed: false }),
		api: async ({ api, bench }, state) => {
			for (const transfer of cueTransfers) {
				const setup = await installCueTransferScenario(
					api,
					bench,
					`cue-009-api-${transfer.operation.toLowerCase()}-${transfer.mode.toLowerCase()}`,
				);
				const sourceBefore = await object<any>(api, "cue_list", setup.sourceId);
				await api.executeCommandLine(
					`${transfer.operation} ${transfer.mode.toUpperCase()} SET 1 CUE 2 AT SET 2 CUE 2`,
				);
				await assertCueTransferOutcome(
					api,
					bench,
					setup,
					sourceBefore,
					transfer,
				);
			}
			state.completed = true;
		},
		assert: async (_context, state) => expect(state.completed).toBe(true),
	});

	for (const transfer of cueTransfers) {
		test(`CUE-009 @supplemental-ui › ${transfer.mode} ${transfer.operation === "COPY" ? "Copy" : "Move"} preserves its independent source and status semantics`, async ({
			api,
			bench,
			desk,
			page,
		}) => {
			const setup = await installCueTransferScenario(
				api,
				bench,
				`cue-009-${transfer.operation.toLowerCase()}-${transfer.mode.toLowerCase()}`,
			);
			const sourceBefore = await object<any>(api, "cue_list", setup.sourceId);
			const destinationBefore = await object<any>(
				api,
				"cue_list",
				setup.destinationId,
			);
			await desk.open(bench.baseUrl);

			await enterCueTransfer(page, transfer.operation);
			const title = transfer.operation === "COPY" ? "Copy" : "Move";
			const dialog = page.getByRole("dialog", { name: `Cue ${title} choice` });
			await expect(
				dialog.getByRole("button", { name: `Plain ${title}`, exact: true }),
			).toBeVisible();
			await expect(
				dialog.getByRole("button", { name: `Status ${title}`, exact: true }),
			).toBeVisible();
			await expect(
				dialog.getByRole("button", { name: "Cancel", exact: true }),
			).toBeVisible();
			expect((await object<any>(api, "cue_list", setup.sourceId)).body).toEqual(
				sourceBefore.body,
			);
			expect(
				(await object<any>(api, "cue_list", setup.destinationId)).body,
			).toEqual(destinationBefore.body);

			await dialog
				.getByRole("button", { name: `${transfer.mode} ${title}`, exact: true })
				.click();
			await expect(dialog).toBeHidden();
			await assertCueTransferOutcome(api, bench, setup, sourceBefore, transfer);
		});
	}

	pairedScenario<{ starting: number[]; completed: boolean }>({
		id: "CMD-002",
		title:
			"Speed Group commands address, synchronize, display, and manually unlink all five groups",
		surfaces: ["api"],
		arrange: async ({ api, bench }, surface) => {
			await loadCanonicalCopy(
				api,
				bench,
				`cmd-002-speed-groups-${surface}`,
				"default-stage",
			);
			return { starting: await speedConfiguration(api), completed: false };
		},
		api: async ({ api, bench }, state) => {
			for (const [group, bpm] of [
				["A", 120],
				["B", 127.5],
				["C", 131],
				["D", 142],
				["E", 153],
			] as const) {
				await applySpeedGroupRuntimeAction(api, {
					surface: "api",
					action: { type: "set_bpm", group, bpm },
				});
			}
			expect(await speedConfiguration(api)).toEqual([
				120, 127.5, 131, 142, 153,
			]);
			await applySpeedGroupRuntimeAction(api, {
				surface: "api",
				action: { type: "adjust_bpm", group: "A", deltaBpm: 5 },
			});
			await applySpeedGroupRuntimeAction(api, {
				surface: "api",
				action: { type: "adjust_bpm", group: "A", deltaBpm: -5 },
			});
			expect((await speedConfiguration(api))[0]).toBe(120);
			await applySpeedGroupRuntimeAction(api, {
				surface: "api",
				action: { type: "set_bpm", group: "C", bpm: 90 },
			});
			await applySpeedGroupRuntimeAction(api, {
				surface: "api",
				action: { type: "synchronize", source: "A", target: "C" },
			});
			await assertSpeedGroupsSynchronized(api, bench, 120);
			await applySpeedGroupRuntimeAction(api, {
				surface: "api",
				action: { type: "set_bpm", group: "C", bpm: 90 },
			});
			let [speedA, speedC] = await Promise.all([
				speedGroup(api, "A"),
				speedGroup(api, "C"),
			]);
			expect([
				speedA.snapshot.manual_bpm,
				speedC.snapshot.manual_bpm,
				speedA.snapshot.synchronized_with,
				speedC.snapshot.synchronized_with,
			]).toEqual([120, 90, null, null]);
			await applySpeedGroupRuntimeAction(api, {
				surface: "api",
				action: { type: "synchronize", source: "A", target: "C" },
			});
			await assertSpeedGroupsSynchronized(api, bench, 120);
			for (let tap = 0; tap < 5; tap += 1) {
				if (tap > 0) await bench.tick(750);
				await api.request("POST", "/api/v2/speed-groups/A/actions", {
					action: "learn",
					captured_at_millis: tap * 750,
				});
			}
			[speedA, speedC] = await Promise.all([
				speedGroup(api, "A"),
				speedGroup(api, "C"),
			]);
			expect([
				speedA.snapshot.manual_bpm,
				speedC.snapshot.manual_bpm,
				speedA.snapshot.synchronized_with,
				speedC.snapshot.synchronized_with,
			]).toEqual([80, 120, null, null]);
			state.completed = true;
		},
		assert: async (_context, state) => expect(state.completed).toBe(true),
	});
});

async function installCueTransferScenario(
	api: ApiDriver,
	bench: any,
	name: string,
) {
	await loadCanonicalCopy(api, bench, name, "compact-rig");
	const fixtures = await fixtureIdsByNumber(api);
	const existingGroups = await objects<any>(api, "group");
	for (const [id, numbers] of [
		["1", [1, 2, 3, 4]],
		["2", [5, 6, 7, 8]],
		["3", [9, 10, 11, 12]],
	] as const) {
		const existing = existingGroups.find((group) => group.id === id);
		await putObject(
			api,
			"group",
			id,
			{
				...(existing?.body ?? {}),
				id,
				name: `Group ${id}`,
				fixtures: numbers.map((number) => fixtures[number]),
				derived_from: null,
				frozen_from: null,
				programming: existing?.body.programming ?? {},
				master: 1,
				playback_fader: existing?.body.playback_fader ?? null,
			},
			existing?.revision ?? 0,
		);
	}

	const sourceId = crypto.randomUUID();
	const destinationId = crypto.randomUUID();
	const sourceCueId = crypto.randomUUID();
	await putObject(
		api,
		"cue_list",
		sourceId,
		cueList(sourceId, "Source", [
			groupCue(crypto.randomUUID(), 1, [["1", 1]]),
			groupCue(sourceCueId, 2, [["2", 1]]),
			groupCue(crypto.randomUUID(), 3, [["1", 0]]),
		]),
	);
	await putObject(
		api,
		"cue_list",
		destinationId,
		cueList(destinationId, "Destination", [
			groupCue(crypto.randomUUID(), 1, [
				["1", 0],
				["3", 1],
			]),
		]),
	);

	const existingPlaybacks = await objects<any>(api, "playback");
	for (const [number, cueListId] of [
		[1, sourceId],
		[2, destinationId],
	] as const) {
		const existing = existingPlaybacks.find(
			(playback) => playback.id === String(number),
		);
		await putObject(
			api,
			"playback",
			String(number),
			{
				number,
				name: number === 1 ? "Source Cuelist" : "Destination Cuelist",
				target: { type: "cue_list", cue_list_id: cueListId },
				buttons: ["go", "go_minus", "flash"],
				button_count: 3,
				fader: "master",
				has_fader: true,
				go_activates: true,
				auto_off: false,
				xfade_millis: 0,
				color: number === 1 ? "#20c997" : "#4d8cff",
				flash_release: "release_all",
				protect_from_swap: false,
				presentation_icon: null,
				presentation_image: null,
			},
			existing?.revision ?? 0,
		);
	}
	return { sourceId, destinationId, sourceCueId };
}

function cueList(id: string, name: string, cues: any[]) {
	return {
		id,
		name,
		priority: 0,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1_000,
		speed_group: null,
		cues,
	};
}

function groupCue(
	id: string,
	number: number,
	changes: Array<[string, number]>,
) {
	return {
		id,
		number,
		name: `Cue ${number}`,
		changes: [],
		group_changes: changes.map(([group_id, value]) => ({
			group_id,
			attribute: "intensity",
			value: { kind: "normalized", value },
			fade_millis: 0,
			delay_millis: 0,
		})),
		fade_millis: 0,
		delay_millis: 0,
		trigger: { type: "manual" },
		phasers: [],
	};
}

function groupCueValues(cue: any): Record<string, number> {
	return Object.fromEntries(
		cue.group_changes.map((change: any) => [
			change.group_id,
			change.value.value,
		]),
	);
}

async function assertCueTransferOutcome(
	api: ApiDriver,
	bench: any,
	setup: Awaited<ReturnType<typeof installCueTransferScenario>>,
	sourceBefore: any,
	transfer: CueTransfer,
) {
	await expect
		.poll(
			async () =>
				(await object<any>(api, "cue_list", setup.destinationId)).body.cues
					.length,
		)
		.toBe(2);
	const sourceAfter = await object<any>(api, "cue_list", setup.sourceId);
	const destinationAfter = await object<any>(
		api,
		"cue_list",
		setup.destinationId,
	);
	const transferred = destinationAfter.body.cues.find(
		(cue: any) => cue.number === 2,
	);
	expect(transferred).toBeDefined();
	expect(groupCueValues(transferred)).toEqual(
		transfer.status ? { "1": 1, "2": 1 } : { "2": 1 },
	);
	expect(
		transferred.group_changes.some((change: any) => change.group_id === "3"),
	).toBe(false);
	if (transfer.moves) {
		expect(sourceAfter.body.cues.map((cue: any) => cue.number)).toEqual([1, 3]);
		expect(
			sourceAfter.body.cues.some((cue: any) => cue.id === setup.sourceCueId),
		).toBe(false);
		expect(transferred.id).toBe(setup.sourceCueId);
	} else {
		expect(sourceAfter.body).toEqual(sourceBefore.body);
		expect(transferred.id).not.toBe(setup.sourceCueId);
	}
	await api.playbackNumberAction(2, "go", {});
	await api.playbackNumberAction(2, "go", {});
	let frame = await bench.tick(3_000);
	const slots =
		frame.universes.find((universe: any) => universe.universe === 1)?.slots ??
		[];
	expect(slots.slice(0, 12)).toEqual([
		...Array(4).fill(transfer.status ? 255 : 0),
		...Array(4).fill(255),
		...Array(4).fill(255),
	]);
	if (transfer.moves) {
		await api.cueListPlaybackAction(setup.destinationId, "release", {});
		await api.playbackNumberAction(1, "go", {});
		await api.playbackNumberAction(1, "go", {});
		frame = await bench.tick(3_000);
		const recalculated =
			frame.universes.find((universe: any) => universe.universe === 1)?.slots ??
			[];
		expect(recalculated.slice(0, 12)).toEqual(Array(12).fill(0));
	}
}

async function enterCueTransfer(page: Page, operation: "COPY" | "MOVE") {
	await pressKeypad(page, [
		operation === "COPY" ? "CPY" : "MOV",
		"SET",
		"1",
		"CUE",
		"2",
		"AT",
		"SET",
		"2",
		"CUE",
		"2",
	]);
	await expect(page.getByLabel("Command line")).toHaveValue(
		`${operation} SET 1 CUE 2 AT SET 2 CUE 2`,
	);
	await page
		.locator(".numeric-pad")
		.getByRole("button", { name: "ENT", exact: true })
		.click();
}

async function pressKeypad(page: Page, keys: string[]) {
	const keypad = page.locator(".numeric-pad");
	for (const key of keys)
		await keypad.getByRole("button", { name: key, exact: true }).click();
}

async function speedConfiguration(api: ApiDriver): Promise<number[]> {
	return Promise.all(
		["A", "B", "C", "D", "E"].map(async (group) => {
			const response = await api.request<any>(
				"GET",
				`/api/v2/speed-groups/${group}`,
			);
			return response.snapshot.manual_bpm;
		}),
	);
}

async function speedGroup(api: ApiDriver, group: "A" | "B" | "C") {
	return api.request<any>("GET", `/api/v2/speed-groups/${group}`);
}

async function assertSpeedGroupsSynchronized(
	api: ApiDriver,
	bench: any,
	expectedBpm: number,
) {
	await expect
		.poll(async () => {
			const [speedA, speedC] = await Promise.all([
				speedGroup(api, "A"),
				speedGroup(api, "C"),
			]);
			return [
				speedA.snapshot.manual_bpm,
				speedC.snapshot.manual_bpm,
				speedA.snapshot.synchronized_with,
				speedC.snapshot.synchronized_with,
			];
		})
		.toEqual([expectedBpm, expectedBpm, 3, 1]);
	for (const advance of [0, 375, 750]) {
		if (advance) await bench.tick(advance);
		const [speedA, speedC] = await Promise.all([
			speedGroup(api, "A"),
			speedGroup(api, "C"),
		]);
		expect(
			Math.abs(speedA.snapshot.beat_phase - speedC.snapshot.beat_phase),
		).toBeLessThan(0.000_001);
	}
}
