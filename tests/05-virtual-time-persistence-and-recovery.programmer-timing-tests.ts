import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { clearProgrammerValues } from "../apps/control-ui/e2e/bench/programmerValues";
import {
	assertCueReplayBoundaries,
	connectHardware,
	disconnectHardware,
	expectEncoderTarget,
	expectFixtureSheetDimmer,
	fixtureRow,
	groupCard,
	type HardwareState,
	openFixtures,
	recordFirstCuelistThroughUi,
	setDimmerByTouch,
	setProgrammerFadeThroughUi,
} from "./05-virtual-time-persistence-and-recovery.time-helpers";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	programmer,
} from "./support/catalog";

export function registerFixtureTimingTest(): void {
	test("TIME-002 @ui › touch-set fixture timing is stored and replayed as resolved light and DMX", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const hardware: HardwareState = {};
		try {
			const show = await loadCanonicalCopy(
				api,
				bench,
				"time-002-fixture-cue",
			);
			const fixtureId = (await fixtureIdsByNumber(api))[1];
			await connectHardware(api, bench, hardware, "time-002-fixture-cue");
			await desk.open(bench.baseUrl);
			await setProgrammerFadeThroughUi(api, page, 3);
			await openFixtures(page);
			await fixtureRow(page, 1).click();
			await setDimmerByTouch(page, 0);
			await bench.tick(3_000);
			await fixtureRow(page, 1).click();
			await setDimmerByTouch(page, 100);

			await expectEncoderTarget(page, 100);
			await bench.tick(0);
			await expectFixtureSheetDimmer(page, 1, 0);
			await desk.recordStep(
				"PROGRAMMER TARGET 100% · RESOLVED 0%",
				"The touch encoder has jumped immediately to 100%, while Fixture Sheet and actual DMX remain at the start of the three-second Programmer Fade.",
			);
			await expect
				.poll(async () => {
					const value = (await programmer(api)).values.find(
						(candidate: any) =>
							candidate.fixture_id === fixtureId &&
							candidate.attribute === "intensity",
					);
					return value?.fade_millis;
				})
				.toBe(3_000);

			const cue = await recordFirstCuelistThroughUi(api, page);
			const change = cue.changes.find(
				(candidate: any) =>
					candidate.fixture_id === fixtureId &&
					candidate.attribute === "intensity",
			);
			expect(change).toMatchObject({
				value: { kind: "normalized", value: 1 },
				fade_millis: 3_000,
			});

			await clearProgrammerValues(api, {
				surface: "api",
				showId: show.id,
			});
			await expect.poll(async () => (await programmer(api)).values).toEqual([]);
			await openFixtures(page);
			await api.request("POST", "/api/v1/cuelists/1/go", {});
			await assertCueReplayBoundaries(api, bench, page, desk, [
				{ fixtureId, number: 1, slot: 1 },
			]);
		} finally {
			await disconnectHardware(api, hardware);
		}
	});
}

export function registerGroupTimingTest(): void {
	test("TIME-002 @ui › touch-set Group timing is stored and replayed for every member", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const hardware: HardwareState = {};
		try {
			const show = await loadCanonicalCopy(api, bench, "time-002-group-cue");
			const fixtureIds = await fixtureIdsByNumber(api);
			await connectHardware(api, bench, hardware, "time-002-group-cue");
			await desk.open(bench.baseUrl);
			await setProgrammerFadeThroughUi(api, page, 3);
			await page.getByRole("button", { name: "Groups", exact: true }).click();
			await expect(page.locator(".group-pool-window")).toBeVisible();
			await groupCard(page, 3).click();
			await setDimmerByTouch(page, 0);
			await bench.tick(3_000);
			await groupCard(page, 3).click();
			await setDimmerByTouch(page, 100);

			await expectEncoderTarget(page, 100);
			await bench.tick(0);
			await openFixtures(page);
			for (const number of [1, 2, 3, 4])
				await expectFixtureSheetDimmer(page, number, 0);
			await desk.recordStep(
				"GROUP TARGET 100% · MEMBERS RESOLVED 0%",
				"The Group touch encoder has jumped immediately to 100%, while every member and actual DMX remain at the start of the three-second Programmer Fade.",
			);
			await expect
				.poll(
					async () =>
						(await programmer(api)).group_values["3"]?.intensity?.fade_millis,
				)
				.toBe(3_000);

			const cue = await recordFirstCuelistThroughUi(api, page);
			const change = cue.group_changes.find(
				(candidate: any) =>
					candidate.group_id === "3" && candidate.attribute === "intensity",
			);
			expect(change).toMatchObject({
				value: { kind: "normalized", value: 1 },
				fade_millis: 3_000,
			});

			await clearProgrammerValues(api, {
				surface: "api",
				showId: show.id,
			});
			await expect
				.poll(async () => (await programmer(api)).group_values)
				.toEqual({});
			await openFixtures(page);
			await api.request("POST", "/api/v1/cuelists/1/go", {});
			await assertCueReplayBoundaries(
				api,
				bench,
				page,
				desk,
				[1, 2, 3, 4].map((number) => ({
					fixtureId: fixtureIds[number],
					number,
					slot: number,
				})),
			);
		} finally {
			await disconnectHardware(api, hardware);
		}
	});
}
