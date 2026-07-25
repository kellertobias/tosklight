import { expect } from "../../apps/control-ui/e2e/bench/core/fixtures";
import { pairedScenario } from "../../apps/control-ui/e2e/bench/core/pairedScenario";
import {
	command,
	expectGroupNumbers,
	expectProgrammer,
	expectSlotsAfterTick,
	INTENSITY,
	loadCompactRig,
	normalized,
	pressCommandAndWait,
} from "../support/foundational/helpers";
import { storeGroup } from "../support/operator";
import {
	armSet,
	controls,
	definition,
	expectConfigurationModal,
	openPlaybackMode,
	playbackAt,
	playbackSlider,
	poolAction,
	saveSlot,
	writePage,
} from "./helpers";

export function registerPbk006GroupMasterHtpScenario(): void {
	pairedScenario({
		id: "PBK-006",
		title:
			"independent overlapping Group Masters use the highest assigned level",
		arrange: async ({ api, bench }, surface) => {
			await loadCompactRig(api, bench, `pbk-006-group-master-htp-${surface}`);
			await writePage(api, 1, {});
			return {};
		},
		api: async ({ api, bench }) => {
			await createAlternatingGroups({
				run: (value) => command(api, value),
			});
			await saveSlot(
				api,
				1,
				1,
				definition(0, "Odd master", { type: "group", group_id: "6" }),
			);
			await saveSlot(
				api,
				1,
				2,
				definition(0, "Even master", { type: "group", group_id: "7" }),
			);
			await command(api, "GROUP 7 AT 100");
			await poolAction(
				api,
				(await playbackAt(api, 1, 2)).body.number,
				"master",
				{ value: 0.8 },
			);
			await expectSlotsAfterTick(bench, 3_000, alternatingSlots(204));

			await saveSlot(
				api,
				1,
				3,
				definition(0, "All master", { type: "group", group_id: "5" }),
			);
			await poolAction(
				api,
				(await playbackAt(api, 1, 3)).body.number,
				"master",
				{ value: 0.6 },
			);
			await expectSlotsAfterTick(bench, 0, alternatingSlots(204));

			await saveSlot(
				api,
				1,
				4,
				definition(0, "Grand Master", { type: "grand_master" }),
			);
			await poolAction(
				api,
				(await playbackAt(api, 1, 4)).body.number,
				"master",
				{ value: 0.5 },
			);
		},
		ui: async ({ api, bench, desk, page }) => {
			await desk.open(bench.baseUrl);
			await createAlternatingGroups({
				run: async (value, visible) => {
					await pressCommandAndWait(page, value, visible);
				},
				record: async (group) => {
					await storeGroup({
						via: "programmer",
						surface: { via: "software", page },
						group,
					});
				},
				runOffset: async () => {
					const commandLine = page.getByLabel("Command line");
					await commandLine.fill("GROUP 5 DIV 2 + 1");
					await expect(commandLine).toHaveValue("GROUP 5 DIV 2 + 1");
					await commandLine.press("Enter");
					await expect(commandLine).toHaveValue(/^(FIXTURE|GROUP)$/);
				},
			});
			await assignGroupMaster(page, 1, "Group 6");
			await assignGroupMaster(page, 2, "Group 7");
			await page.locator(".mode-toggle").click();
			await pressCommandAndWait(page, "GROUP 7 AT 100", "G7 AT 100");
			await openPlaybackMode(page);
			await playbackSlider(page, 2).fill("80");
			await expect
				.poll(
					async () =>
						(await controls(api)).groups.find(
							(group: { id: string }) => group.id === "7",
						)?.master,
				)
				.toBeCloseTo(0.8, 5);
			await expectSlotsAfterTick(bench, 3_000, alternatingSlots(204));

			await assignGroupMaster(page, 3, "Group 5");
			await playbackSlider(page, 3).fill("60");
			await expect
				.poll(
					async () =>
						(await controls(api)).groups.find(
							(group: { id: string }) => group.id === "5",
						)?.master,
				)
				.toBeCloseTo(0.6, 5);
			await expectSlotsAfterTick(bench, 0, alternatingSlots(204));

			await assignGrandMaster(page, 4);
			await playbackSlider(page, 4).fill("50");
			await expect
				.poll(async () => (await controls(api)).grand_master.level)
				.toBeCloseTo(0.5, 5);
		},
		assert: async ({ api, bench }) => {
			await expectGroupNumbers(api, "5", [1, 2, 3, 4, 5, 6]);
			await expectGroupNumbers(api, "6", [1, 3, 5]);
			await expectGroupNumbers(api, "7", [2, 4, 6]);
			await expectProgrammer(api, (state) => {
				expect(normalized(state.group_values["7"][INTENSITY].value)).toBe(1);
			});
			await expect((await playbackAt(api, 1, 1)).body.target).toEqual({
				type: "group",
				group_id: "6",
			});
			await expect((await playbackAt(api, 1, 2)).body.target).toEqual({
				type: "group",
				group_id: "7",
			});
			await expect((await playbackAt(api, 1, 3)).body.target).toEqual({
				type: "group",
				group_id: "5",
			});
			await expectSlotsAfterTick(bench, 0, alternatingSlots(102));
		},
	});
}

async function createAlternatingGroups(options: {
	run: (value: string, visible: string) => Promise<void>;
	record?: (group: number) => Promise<void>;
	runOffset?: () => Promise<void>;
}): Promise<void> {
	for (const [value, visible, group] of [
		["1 THRU 6", "F1 THRU 6", 5],
		["GROUP 5 DIV 2", "G5 DIV 2", 6],
		["GROUP 5 DIV 2 + 1", "G5 DIV 2 + 1", 7],
	] as const) {
		if (group === 7 && options.runOffset) await options.runOffset();
		else await options.run(value, visible);
		if (options.record) await options.record(group);
		else await options.run(`RECORD GROUP ${group}`, `RECORD GROUP ${group}`);
	}
}

async function assignGroupMaster(
	page: Parameters<typeof openPlaybackMode>[0],
	slot: number,
	groupName: string,
): Promise<void> {
	await openPlaybackMode(page);
	await armSet(page);
	await page
		.getByRole("button", {
			name: `Playback representation page 1 playback ${slot}`,
		})
		.click();
	const modal = await expectConfigurationModal(page, 1, slot);
	await modal.getByRole("radio", { name: "Group Master", exact: true }).click();
	await modal.getByRole("radio", { name: groupName, exact: true }).click();
	await modal.getByRole("button", { name: "Apply", exact: true }).click();
	await expect(modal).toBeHidden();
}

async function assignGrandMaster(
	page: Parameters<typeof openPlaybackMode>[0],
	slot: number,
): Promise<void> {
	await openPlaybackMode(page);
	await armSet(page);
	await page
		.getByRole("button", {
			name: `Playback representation page 1 playback ${slot}`,
		})
		.click();
	const modal = await expectConfigurationModal(page, 1, slot);
	await modal.getByRole("radio", { name: "Special", exact: true }).click();
	await modal.getByRole("radio", { name: "Grand Master", exact: true }).click();
	await modal.getByRole("button", { name: "Apply", exact: true }).click();
	await expect(modal).toBeHidden();
}

function alternatingSlots(level: number): number[] {
	return [0, level, 0, level, 0, level, 0, 0, 0, 0, 0, 0];
}
