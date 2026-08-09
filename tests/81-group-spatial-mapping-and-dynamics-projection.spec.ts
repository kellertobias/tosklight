import { expect, type Page } from "@playwright/test";
import { test } from "./bench/core/fixtures";

interface StoredGroupBody {
	name: string;
	source?: GroupSource;
	mapping?: {
		projection: { preset?: string | null };
		shape: { type: string; direction?: string };
	};
}

interface StoredStageLayoutBody {
	version?: number;
	positions?: Record<string, { x: number; y: number }>;
	positions3d?: Record<string, { x: number; y: number; z: number }>;
}

type GroupSource =
	| { type: "explicit"; fixture_ids: string[] }
	| {
			type: "references";
			references: Array<{ group_id: string; rule: { type: "all" } }>;
	  };

const topGridMapping = {
	projection: {
		anchor: { x: 0, y: 0, z: 0 },
		view_direction: { x: 0, y: 0, z: -1 },
		rotation_degrees: 0,
		preset: "top",
	},
	shape: { type: "grid", angle_degrees: 0, direction: "ascending" },
};

test("GROUP-SPATIAL-010 @ui › Group settings author ranked output without retired authority", async ({
	api,
	bench,
	desk,
	page,
	show,
}) => {
	test.setTimeout(90_000);
	const stage = await api.showObject<StoredStageLayoutBody>(
		show.id,
		"stage_layout",
		"main",
	);
	const positions3d = Object.fromEntries(
		show.fixtureIds.map((fixtureId, index) => [
			fixtureId,
			{ x: index <= 1 ? 500 : (index + 1) * 500, y: 0, z: 0 },
		]),
	);
	await api.seedShowObject(
		show.id,
		"stage_layout",
		"main",
		{
			version: stage?.body.version ?? 2,
			positions: stage?.body.positions ?? {},
			positions3d,
		},
		stage?.revision ?? 0,
	);

	await desk.open(api.baseUrl);
	await openGroups(page);
	const group = page
		.locator(".group-card")
		.filter({ has: page.getByText("All Dimmers", { exact: true }) })
		.first();
	await group.dispatchEvent("pointerdown", {
		pointerId: 1,
		pointerType: "mouse",
		button: 0,
	});
	await page.waitForTimeout(700);
	await group.dispatchEvent("pointerup", {
		pointerId: 1,
		pointerType: "mouse",
		button: 0,
	});

	const dialog = page.getByRole("dialog", { name: "Group 1 settings" });
	await expect(dialog).toBeVisible();
	await expect(dialog.getByRole("tab")).toHaveText([
		"General",
		"Projection",
		"Phase",
	]);
	await expect(
		dialog.getByRole("button", { name: "Close settings" }),
	).toBeVisible();
	for (const retired of [
		"Master",
		"Select live group",
		"Select frozen group",
		"Replace membership with selection",
		"Undo membership/programming change",
	])
		await expect(dialog.getByText(retired, { exact: true })).toHaveCount(0);
	await expect(page.locator(".layout-window")).toHaveCount(0);
	await expect(page.getByText("Grid Settings", { exact: true })).toHaveCount(0);

	await dialog.getByRole("tab", { name: "Projection" }).click();
	// There is no button that only claims ownership: setting the projection is what takes it.
	await dialog.getByRole("button", { name: "Top", exact: true }).click();
	await expect
		.poll(
			async () =>
				(await api.showObject<StoredGroupBody>(show.id, "group", "1"))?.body
					.mapping?.projection.preset,
		)
		.toBe("top");

	await dialog.getByRole("tab", { name: "Phase" }).click();
	await dialog.getByRole("radio", { name: "Radial" }).click();
	await expect
		.poll(
			async () =>
				(await api.showObject<StoredGroupBody>(show.id, "group", "1"))?.body
					.mapping?.shape,
		)
		.toMatchObject({ type: "radial", direction: "outward" });
	await dialog.getByRole("button", { name: "Close settings" }).click();
	await expect(dialog).toBeHidden();

	await desk.command("GROUP 1 AT 0 TRU 100", "G1 AT 0 THRU 100");
	const frame = await bench.tick(3_000);
	const universe = frame.universes.find(
		(candidate) => candidate.universe === 1,
	);
	expect(universe).toBeDefined();
	expect(universe?.slots[0]).toBe(0);
	expect(universe?.slots[1]).toBe(0);
	expect(universe?.slots[11]).toBe(255);

	const saved = await api.showObject<Record<string, unknown>>(
		show.id,
		"group",
		"1",
	);
	expect(saved?.body).not.toHaveProperty("master");
	expect(saved?.body).not.toHaveProperty("playback_fader");
});

test("GROUP-SPATIAL-020 @api › canonical live references drive command output and refresh membership", async ({
	api,
	bench,
	show,
}) => {
	test.setTimeout(90_000);
	await api.seedShowObject(show.id, "group", "4", {
		id: "4",
		name: "Canonical source",
		fixtures: [show.fixtureIds[11]],
		source: {
			type: "explicit",
			fixture_ids: show.fixtureIds.slice(0, 3),
		} satisfies GroupSource,
		mapping: topGridMapping,
		derived_from: null,
		frozen_from: null,
		programming: {},
	});
	await api.seedShowObject(show.id, "group", "5", {
		id: "5",
		name: "Canonical live reference",
		fixtures: [show.fixtureIds[10]],
		source: {
			type: "references",
			references: [{ group_id: "4", rule: { type: "all" } }],
		} satisfies GroupSource,
		derived_from: null,
		frozen_from: null,
		programming: {},
	});

	await api.executeCommandLine("GROUP 5 AT 0 THRU 100");
	let frame = await bench.tick(3_000);
	let universe = frame.universes.find((candidate) => candidate.universe === 1);
	expect(universe?.slots[0]).toBe(0);
	expect(universe?.slots[1]).toBeGreaterThanOrEqual(127);
	expect(universe?.slots[1]).toBeLessThanOrEqual(128);
	expect(universe?.slots[2]).toBe(255);
	expect(universe?.slots[10]).toBe(0);
	expect(universe?.slots[11]).toBe(0);

	const source = await api.showObject<StoredGroupBody>(show.id, "group", "4");
	expect(source).not.toBeNull();
	if (!source) throw new Error("Expected canonical source Group");
	await api.seedShowObject(
		show.id,
		"group",
		"4",
		{
			...source.body,
			source: {
				type: "explicit",
				fixture_ids: [show.fixtureIds[0], show.fixtureIds[3]],
			} satisfies GroupSource,
		},
		source.revision,
	);
	const cleared = await api.sendCommandKey("CLR");
	expect(cleared.outcome).toBe("accepted");
	await api.executeCommandLine("GROUP 5 AT 0 THRU 100");
	frame = await bench.tick(3_000);
	universe = frame.universes.find((candidate) => candidate.universe === 1);
	expect(universe?.slots[0]).toBe(0);
	expect(universe?.slots[1]).toBe(0);
	expect(universe?.slots[2]).toBe(0);
	expect(universe?.slots[3]).toBe(255);

	await expect
		.poll(async () => {
			const snapshot = await api.request<{
				resolved_spatial: { source_order: string[] };
			}>("GET", "/api/v2/groups/5", undefined, true, undefined, {
				showId: show.id,
			});
			return snapshot.resolved_spatial.source_order;
		})
		.toEqual([show.fixtureIds[0], show.fixtureIds[3]]);
});

async function openGroups(page: Page) {
	if (!(await page.locator(".group-pool-window").isVisible())) {
		await page.getByRole("button", { name: "SHIFT", exact: true }).click();
		await page.getByRole("button", { name: "1", exact: true }).click();
	}
	await expect(page.locator(".group-pool-window")).toBeVisible();
}
