import type { Locator, Page } from "@playwright/test";
import { selectProgrammingGroup } from "./bench/command-selection/programmingSelection";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import { requireSemanticContract } from "./bench/core/semanticContract";
import { poseAfter, readouts } from "./bench/dynamics/intentFrameOutput";
import {
	arrangeRig,
	clearProgrammer,
	fixtureInput,
	type IntentRig,
	POINT,
	patchFixtures,
	SPOT,
} from "./bench/dynamics/intentFrameScenario";
import { reopenFromSavedFile } from "./bench/tracking/psnTracking";

/**
 * docs/testing/34-position-operator-controls.md POSITION-CONTROLS-012 and 013 (TL-651): Points
 * are created and managed from the Position Point encoder, through Show Patch › Points, and the
 * Point encoder steps through patched and unpatched Points alike.
 *
 * GATE: production reports programming contract 1, so the semantic Position pages are published
 * under `npm run test:e2e` as well as `npm run test:e2e-semantic`.
 */

const GATE =
	"semantic programming contract is not enabled on this runtime (a contract-0 server; run npm run test:e2e-semantic)";
const GROUP = "1";

/** Two movers on a truss 6 m upstage, stored as Group 1 and selected; no 3D Point yet. */
async function moverRig(api: ApiDriver, bench: LightBench, label: string) {
	const truss = (x: number) => ({ x, y: 6_000, z: 0 });
	const rig = await arrangeRig(api, `POSITION-POINTS ${label}`, [
		{ number: 1, address: "1.1", location: truss(-2_000), ...SPOT },
		{ number: 2, address: "1.21", location: truss(2_000), ...SPOT },
	]);
	const movers = [rig.ids[1], rig.ids[2]];
	await api.seedShowObject(rig.showId, "group", GROUP, {
		id: GROUP,
		name: "Movers",
		fixtures: movers,
		color: null,
		icon: "◇",
		derived_from: null,
		frozen_from: null,
		programming: {},
	});
	await selectGroup(api, rig);
	await bench.tick(25);
	return { rig, movers };
}

function selectGroup(api: ApiDriver, rig: IntentRig) {
	return selectProgrammingGroup(api, {
		surface: "api",
		showId: rig.showId,
		groupId: GROUP,
		frozen: false,
		rule: { type: "all" },
	});
}

/** The Position family button; a multi-page family names its page, e.g. `Position 1 of 2`. */
function positionFamily(page: Page) {
	return page.getByRole("button", { name: /^Position( \d+ of \d+)?$/ });
}

/** The software Point encoder, on whichever Position page holds it. */
async function pointEncoder(page: Page) {
	const locator = page.getByRole("group", { name: /^Enc \d+ · Point( · .+)?$/ });
	if (!(await locator.isVisible())) await positionFamily(page).click();
	if (!(await locator.isVisible())) await positionFamily(page).click();
	await expect(locator).toBeVisible();
	return locator;
}

async function encoderText(locator: Locator) {
	return (await locator.locator(".touch-encoder-value").innerText()).replace(/\s+/gu, " ").trim();
}

async function stepPoint(page: Page) {
	await (await pointEncoder(page)).getByRole("button", { name: /^Next Enc \d+ · Point value$/ }).click();
}

/** The selected group's programmed Position value. */
async function groupPosition(api: ApiDriver) {
	const snapshot = await api.request<{
		projection: {
			group_values?: Array<{
				group_id: string;
				attribute: string;
				value: { value: { kind: string; reference?: { kind: string; point_id?: string } } };
			}>;
		};
	}>("GET", "/api/v2/programmer/values/snapshot");
	return (snapshot.projection.group_values ?? []).find(
		(entry) => entry.group_id === GROUP && entry.attribute === "position",
	)?.value.value;
}

const atPoint = (pointId: string) => ({ kind: "target", reference: { kind: "point", point_id: pointId } });

async function fixtureByNumber(api: ApiDriver, number: number) {
	return (await api.patch()).fixtures.find((fixture) => fixture.fixture_number === number) as
		| {
				fixture_id: string;
				name: string;
				universe: number | null;
				address: number | null;
				location?: { x: number; y: number; z: number };
		  }
		| undefined;
}

/** Fill a Points table field and commit it with Enter, as typed on a keyboard. */
async function commitField(field: Locator, value: string) {
	await field.fill(value);
	await field.press("Enter");
}

test.describe("Position Points (TL-651)", () => {
	test("POSITION-CONTROLS-012 @ui › Create Point from the Point encoder, name and place it unpatched, step to it and to a patched Point, and keep it across save and reload", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const { rig, movers } = await moverRig(api, bench, "012");
		const pages = await api
			.request<{ semantic: boolean }>(
				"GET",
				`/api/v2/programming/family-encoder-pages?fixture_ids=${movers.join(",")}`,
			)
			.catch(() => null);
		requireSemanticContract(Boolean(pages?.semantic), GATE);
		await desk.open(api.baseUrl);
		await positionFamily(page).click();

		// 1. Without a 3D Point the Point encoder says so instead of an em dash.
		let encoder = await pointEncoder(page);
		await expect.poll(() => encoderText(encoder)).toBe("No Points");

		// 2. Its picker explains why and offers Create Point, which opens Show Patch › Points
		// with one new aim Point, unpatched, at the stage origin.
		await encoder.getByRole("button", { name: /^Set Enc \d+ · Point value$/ }).click();
		const picker = page.getByRole("dialog").filter({ hasText: "Target reference" });
		await expect(picker).toContainText("This show has no Points yet");
		await expect(picker.getByRole("button", { name: "Origin" })).toBeVisible();
		await picker.getByRole("button", { name: "Create Point", exact: true }).click();
		const header = page.locator("header.ui-window-header").filter({ hasText: "Show Patch" }).first();
		await expect(header.getByRole("tab", { name: "Points", exact: true })).toHaveAttribute("aria-selected", "true");
		const table = page.getByRole("table", { name: "Points" });
		const row = table.getByRole("row", { name: "3 · Point 1" });
		await expect(row).toBeVisible();
		await expect(row).toHaveAttribute("aria-current", "true");
		await expect(row).toContainText("Unpatched");
		const created = await fixtureByNumber(api, 3);
		expect(created).toMatchObject({ name: "Point 1", universe: null, address: null, location: { x: 0, y: 0, z: 0 } });
		const singer = created?.fixture_id ?? "";

		// 3. Name and place it without a DMX patch.
		await commitField(page.getByLabel("Point 3 name", { exact: true }), "Singer");
		await expect(table.getByRole("row", { name: "3 · Singer" })).toBeVisible();
		await commitField(page.getByLabel("Point 3 X", { exact: true }), "-1");
		await expect.poll(async () => (await fixtureByNumber(api, 3))?.location?.x).toBe(-1_000);
		await commitField(page.getByLabel("Point 3 Y", { exact: true }), "2");
		await expect.poll(async () => (await fixtureByNumber(api, 3))?.location?.y).toBe(2_000);
		await commitField(page.getByLabel("Point 3 Z", { exact: true }), "1.5");
		await expect
			.poll(() => fixtureByNumber(api, 3))
			.toMatchObject({ fixture_id: singer, name: "Singer", universe: null, location: { x: -1_000, y: 2_000, z: 1_500 } });

		// A fixture-backed Point patched at 2.1 is listed beside it.
		await patchFixtures(api, [
			await fixtureInput(api, crypto.randomUUID(), { number: 901, address: "2.1", location: { x: 2_000, y: 0, z: 2_000 }, ...POINT }),
		]);
		const rigPoint = (await fixtureByNumber(api, 901))?.fixture_id ?? "";
		await expect(table.getByRole("row", { name: /^901 · / })).toContainText("2.1");

		// 4. Back on the desk the Point encoder steps Origin → Singer → the patched Point → Origin,
		// naming each one, and the movers aim at the chosen Point.
		await desk.open(api.baseUrl);
		await positionFamily(page).click();
		encoder = await pointEncoder(page);
		await expect.poll(() => encoderText(encoder)).toBe("—");
		await stepPoint(page);
		await expect.poll(() => groupPosition(api)).toMatchObject({ kind: "target", reference: { kind: "origin" } });
		await expect.poll(() => encoderText(encoder)).toBe("Origin");
		await stepPoint(page);
		await expect.poll(() => groupPosition(api)).toMatchObject(atPoint(singer));
		await expect.poll(() => encoderText(encoder)).toBe("3 · Singer");
		await bench.tick(25);
		const atSinger = await poseAfter(api, bench, movers[0], 25);
		expect(atSinger.available, "the mover aims at the unpatched Point").toBe(true);
		await expect(page.getByRole("group", { name: /^Enc \d+ · Pan · From Point$/u })).toBeVisible();
		await stepPoint(page);
		await expect.poll(() => groupPosition(api)).toMatchObject(atPoint(rigPoint));
		await expect.poll(() => encoderText(encoder)).toMatch(/^901 · /u);
		const atRigPoint = await poseAfter(api, bench, movers[0], 25);
		expect(Math.abs(atRigPoint.pan - atSinger.pan) + Math.abs(atRigPoint.tilt - atSinger.tilt)).toBeGreaterThan(1);
		await stepPoint(page);
		await expect.poll(() => groupPosition(api)).toMatchObject({ kind: "target", reference: { kind: "origin" } });

		// 5. The picker marks the current Point and picks Singer by name.
		await encoder.getByRole("button", { name: /^Set Enc \d+ · Point value$/ }).click();
		await expect(picker.getByRole("button", { name: "Origin" })).toHaveAttribute("aria-pressed", "true");
		await picker.getByRole("button", { name: "3 · Singer" }).click();
		await expect.poll(() => groupPosition(api)).toMatchObject(atPoint(singer));
		await encoder.getByRole("button", { name: /^Set Enc \d+ · Point value$/ }).click();
		await expect(picker.getByRole("button", { name: "3 · Singer" })).toHaveAttribute("aria-pressed", "true");
		await page.keyboard.press("Escape");
		const aimed = await poseAfter(api, bench, movers[0], 25);

		// 6. Save and reload: the Point keeps its identity, name and place, still unpatched, and a
		// cue that stored the Target at it aims there again.
		await api.executeCommandLine("RECORD PBK 1");
		await clearProgrammer(api, rig);
		const reopened = await reopenFromSavedFile(api, rig.showId);
		expect(await fixtureByNumber(api, 3)).toMatchObject({
			fixture_id: singer,
			name: "Singer",
			universe: null,
			location: { x: -1_000, y: 2_000, z: 1_500 },
		});
		await api.executeCommandLine("GO TO PBK 1 CUE 1");
		await expect
			.poll(async () => {
				const pose = await poseAfter(api, bench, movers[0], 25);
				return Math.abs(pose.pan - aimed.pan) + Math.abs(pose.tilt - aimed.tilt);
			})
			.toBeLessThan(0.05);
		expect(reopened).not.toBe(rig.showId);
	});

	test("POSITION-CONTROLS-013 @ui › a deleted Point reads Missing point and the encoder steps on to the Points that exist", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const { rig, movers } = await moverRig(api, bench, "013");
		requireSemanticContract(
			Boolean(
				(
					await api
						.request<{ semantic: boolean }>(
							"GET",
							`/api/v2/programming/family-encoder-pages?fixture_ids=${movers.join(",")}`,
						)
						.catch(() => null)
				)?.semantic,
			),
			GATE,
		);
		await patchFixtures(api, [
			await fixtureInput(api, crypto.randomUUID(), { number: 901, address: "2.1", location: { x: 2_000, y: 0, z: 2_000 }, ...POINT }),
		]);
		const rigPoint = (await fixtureByNumber(api, 901))?.fixture_id ?? "";
		await desk.open(api.baseUrl);
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page.getByRole("button", { name: "Show Patch", exact: true }).click();
		const header = page.locator("header.ui-window-header").filter({ hasText: "Show Patch" }).first();
		await header.getByRole("tab", { name: "Points", exact: true }).click();
		await header.getByRole("button", { name: "+ Create Point" }).click();
		const table = page.getByRole("table", { name: "Points" });
		await expect(table.getByRole("row", { name: "902 · Point 2" })).toContainText("Unpatched");
		const spare = (await fixtureByNumber(api, 902))?.fixture_id ?? "";

		// Aim the group at the unpatched Point through the encoder.
		await desk.open(api.baseUrl);
		await positionFamily(page).click();
		const encoder = await pointEncoder(page);
		for (const reference of [{ kind: "origin" }, { kind: "point", point_id: rigPoint }, { kind: "point", point_id: spare }]) {
			await stepPoint(page);
			await expect.poll(() => groupPosition(api)).toMatchObject({ kind: "target", reference });
		}
		await expect.poll(() => encoderText(encoder)).toBe("902 · Point 2");

		// Delete it in Show Patch › Points (two touches), keeping the Programmer's Target.
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page.getByRole("button", { name: "Show Patch", exact: true }).click();
		await header.getByRole("tab", { name: "Points", exact: true }).click();
		await page.getByRole("button", { name: "Delete Point 902" }).click();
		await page.getByRole("button", { name: "Confirm delete" }).click();
		await expect(table.getByRole("row", { name: /^902 · / })).toHaveCount(0);
		await expect.poll(() => fixtureByNumber(api, 902)).toBeUndefined();

		// The reference is kept and named as missing; the next step goes on to the Points that exist.
		await desk.open(api.baseUrl);
		await positionFamily(page).click();
		const after = await pointEncoder(page);
		await expect.poll(() => groupPosition(api)).toMatchObject(atPoint(spare));
		await expect.poll(() => encoderText(after)).toBe("Missing point");
		await stepPoint(page);
		await expect.poll(() => groupPosition(api)).toMatchObject({ kind: "target", reference: { kind: "origin" } });
		await stepPoint(page);
		await expect.poll(() => groupPosition(api)).toMatchObject(atPoint(rigPoint));
		await bench.tick(25);
		expect((await readouts(api, movers)).poses[movers[0]]?.available).toBe(true);
		expect(rig.showId).toBeTruthy();
	});
});
