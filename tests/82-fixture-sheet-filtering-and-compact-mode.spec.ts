import type { Page } from "@playwright/test";
import type {
	DynamicInstanceActionOutcome,
	ProgrammingSelectionActionOutcome,
} from "../apps/light-desktop/src/api/generated/light-wire";
import type { PatchedFixture } from "../apps/light-desktop/src/api/types";
import { expect, test } from "./bench/core/fixtures";
import { BrowserDesktops } from "./bench/window-system/desktopScenario";
import { PaneType } from "./bench/window-system/paneTypes";

test("FIXTURE-SHEET-002-001 @ui › independent scenery identities stay out of the programmable sheet", async ({
	api,
	desk,
	page,
	show,
}) => {
	test.setTimeout(90_000);
	const source = await api.showObject<PatchedFixture>(
		show.id,
		"patched_fixture",
		show.fixtureIds[0],
	);
	expect(source).not.toBeNull();
	if (!source) throw new Error("Fixture Sheet scenario needs a source fixture");

	const hidden = [
		{
			id: crypto.randomUUID(),
			name: "Venue-only scenery",
			fixtureNumber: 201,
			address: 501,
			virtualFixtureNumber: null,
			manufacturer: "Venue",
			patchPolicy: "dmx",
		},
		{
			id: crypto.randomUUID(),
			name: "Visual-only profile scenery",
			fixtureNumber: null,
			address: null,
			virtualFixtureNumber: 6,
			manufacturer: "Independent",
			patchPolicy: "visual_only",
		},
		{
			id: crypto.randomUUID(),
			name: "Legacy reserved-ID scenery",
			fixtureNumber: null,
			address: null,
			virtualFixtureNumber: 7,
			manufacturer: "Independent",
			patchPolicy: "visual_only",
		},
	] as const;
	for (const candidate of hidden) {
		const profileId = crypto.randomUUID();
		const modeId = crypto.randomUUID();
		const profile = source.body.definition.profile_snapshot;
		if (!profile) throw new Error("Source fixture has no embedded profile");
		const visualOnly = candidate.patchPolicy === "visual_only";
		await api.seedShowObject(show.id, "patched_fixture", candidate.id, {
			...source.body,
			fixture_id: candidate.id,
			fixture_number: candidate.fixtureNumber,
			virtual_fixture_number: candidate.virtualFixtureNumber,
			name: candidate.name,
			universe: candidate.address == null ? null : 1,
			address: candidate.address,
			split_patches: [
				{
					split: 1,
					universe: candidate.address == null ? null : 1,
					address: candidate.address,
				},
			],
			definition: {
				...source.body.definition,
				footprint: visualOnly ? 0 : source.body.definition.footprint,
				id: profileId,
				profile_id: profileId,
				mode_id: modeId,
				manufacturer: candidate.manufacturer,
				profile_snapshot: {
					...profile,
					id: profileId,
					manufacturer: candidate.manufacturer,
					patch_policy: candidate.patchPolicy,
					modes: profile.modes.map((mode, index) => {
						if (index !== 0) return mode;
						return {
							...mode,
							id: modeId,
							splits: visualOnly
								? mode.splits.map((split) => ({ ...split, footprint: 0 }))
								: mode.splits,
							channels: visualOnly ? [] : mode.channels,
							color_systems: visualOnly ? [] : mode.color_systems,
							control_actions: visualOnly ? [] : mode.control_actions,
						};
					}),
				},
			},
		});
	}

	await desk.open(api.baseUrl);
	const fixtureSheet = page.locator(".fixture-window");
	await expect(fixtureSheet).toBeVisible();
	const fixturePane = page.locator(".desk-pane").filter({ has: fixtureSheet });
	const rows = fixtureSheet.locator(
		".ui-data-table-row:not(.header)[data-fixture-id]",
	);
	await expect(rows).toHaveCount(show.fixtureIds.length);
	for (const candidate of hidden) {
		await expect(rows.filter({ hasText: candidate.name })).toHaveCount(0);
	}

	for (const candidate of hidden) {
		const selectionRequest = crypto.randomUUID();
		const selected = await api.liveAction<ProgrammingSelectionActionOutcome>(
			{
				type: "programming_selection",
				request: {
					request_id: selectionRequest,
					action: "gesture",
					source: { type: "fixture", fixture_id: candidate.id },
					remove: false,
				},
			},
			selectionRequest,
		);
		expect(selected.payload?.selection.selected).toContain(candidate.id);
		await expect(
			fixtureSheet.locator(".ui-data-table-row.selected"),
		).toHaveCount(0);
	}
	await expect(fixturePane).toContainText(`${hidden.length} selected`);

	await fixturePane.getByRole("button", { name: "Settings" }).click();
	const settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings.getByRole("tab", { name: "Fixture Sheet" }).click();
	const compact = settings
		.locator(".ui-form-field")
		.filter({ hasText: "Compact mode" })
		.locator(".ui-select-trigger");
	await compact.click();
	await expect(page.getByRole("option")).toHaveText([
		"Off",
		"Icon only",
		"Text only",
	]);
	await page.getByRole("option", { name: "Icon only" }).click();
	await expect(fixtureSheet).toHaveAttribute(
		"data-fixture-sheet-compact-mode",
		"icon-only",
	);
	await compact.click();
	await page.getByRole("option", { name: "Text only" }).click();
	await expect(fixtureSheet).toHaveAttribute(
		"data-fixture-sheet-compact-mode",
		"text-only",
	);
});

test("FIXTURE-SHEET-002-003 @ui › a Dynamic changes DMX while the Fixture Sheet base stays stable", async ({
	api,
	bench,
	desk,
	page,
	show,
}) => {
	test.setTimeout(90_000);
	await api.executeCommandLine("FIXTURE 1 AT 50");
	await bench.tick(3_000);
	const dynamic = await api.request<{ object: { id: string } }>(
		"POST",
		"/api/v2/dynamics/create",
		{
			request_id: crypto.randomUUID(),
			definition: fixtureSheetDynamicDefinition(),
		},
		true,
		undefined,
		{ showId: show.id },
	);
	const startRequest = crypto.randomUUID();
	const started = await api.liveAction<DynamicInstanceActionOutcome>(
		{
			type: "dynamic_start",
			request: {
				dynamic_id: dynamic.object.id,
				request: {
					request_id: startRequest,
					targets: [show.fixtureIds[0]],
					overrides: {
						size: 1,
						speed_multiplier: { numerator: 1, denominator: 1 },
						phase_offset_degrees: 0,
					},
					timing: {},
					undo_group: "fixture-sheet-002",
				},
			},
		},
		startRequest,
	);
	expect(started.payload?.started).toBe(true);
	if (!started.payload) throw new Error("Dynamic start returned no payload");

	await desk.open(api.baseUrl);
	const desktops = new BrowserDesktops(page, async () => undefined);
	const configuration = desktops.configure("Fixture Sheet and DMX");
	configuration.addPane(PaneType.Fixtures, {
		slug: "fixture-sheet-dynamic",
		column: 1,
		row: 1,
		width: 12,
		height: 18,
	});
	configuration.addPane(PaneType.Dmx, {
		slug: "fixture-sheet-dmx",
		column: 13,
		row: 1,
		width: 12,
		height: 18,
	});
	await configuration.apply();

	const fixtureSheet = page.locator(".fixture-window");
	const dmx = page.locator(".dmx-window");
	await expect(fixtureSheet).toBeVisible();
	await expect(dmx).toBeVisible();
	const row = fixtureRow(page, show.fixtureIds[0]);
	await expect(row).toBeVisible();
	await expect(row).toContainText("50%");
	await expect(row.locator(".fixture-dynamic-stack")).toContainText("77");

	const projected = await api.request<{
		values: Array<{
			fixture_id: string;
			attribute: string;
			value: { kind: string; value: number };
		}>;
		dynamic_stack: Array<Record<string, unknown>>;
	}>(
		"GET",
		`/api/v2/output/visualization?dynamic_stack_only=true&fixture_ids=${show.fixtureIds[0]}`,
		undefined,
		true,
		undefined,
		{ showId: show.id },
	);
	expect(
		projected.values.find(
			(value) =>
				value.fixture_id === show.fixtureIds[0] &&
				value.attribute === "intensity",
		)?.value,
	).toEqual({ kind: "normalized", value: 0.5 });
	const projectedDynamic = projected.dynamic_stack.find(
		(entry) =>
			entry.fixture_id === show.fixtureIds[0] &&
			entry.attribute === "intensity",
	);
	expect(projectedDynamic).toMatchObject({ pool_number: 77 });
	expect(projectedDynamic).not.toHaveProperty("value");
	expect(projectedDynamic).not.toHaveProperty("resolved_value");

	const outputValues: number[] = [];
	for (const advance of [0, 1_000, 1_000]) {
		const frame = await bench.tick(advance);
		const value = fixtureOneDmx(frame);
		outputValues.push(value);
		await expect(
			dmx.getByRole("button", {
				name: `Universe 1, address 1, value ${value}`,
			}),
		).toBeVisible();
		await expect(row).toContainText("50%");
		await expect(row.locator(".fixture-dynamic-stack")).toContainText("77");
		if (outputValues.length > 1)
			expect(value).not.toBe(outputValues[outputValues.length - 2]);
	}
	expect(outputValues).toHaveLength(3);

	const offRequest = crypto.randomUUID();
	await api.liveAction(
		{
			type: "dynamic_off",
			request: {
				controller_id: started.payload.controller_id,
				request: { request_id: offRequest, timing: {} },
			},
		},
		offRequest,
	);
	await expect(row.locator(".fixture-dynamic-stack")).toHaveCount(0);
	await expect(row).toContainText("50%");
});

function fixtureRow(page: Page, fixtureId: string) {
	return page.locator(
		`.fixture-window .ui-data-table-row:not(.header)[data-fixture-id="${fixtureId}"]`,
	);
}

function fixtureOneDmx(frame: {
	universes: Array<{ universe: number; slots: number[] }>;
}) {
	const universe = frame.universes.find(
		(candidate) => candidate.universe === 1,
	);
	if (!universe) throw new Error("Logical universe 1 is absent");
	return universe.slots[0] ?? 0;
}

function fixtureSheetDynamicDefinition() {
	const value = (normalized: number) => ({
		type: "value",
		value: normalized,
	});
	const pwm = {
		attack: 0,
		on: 0.5,
		decay: 0,
		off: 0.5,
		attack_interpolation: "linear",
		decay_interpolation: "linear",
	};
	return {
		id: crypto.randomUUID(),
		pool_number: 77,
		revision: 0,
		name: "Fixture Sheet stable-base Dynamic",
		color: "#4edcff",
		icon: "∿",
		target_binding: { type: "targetless" },
		lanes: [
			{
				id: crypto.randomUUID(),
				attribute: "intensity",
				mode: "max_min",
				keyframes: {
					points: [
						{ position: 0, source: value(0), interpolation: "ease_in_out" },
						{
							position: 0.5,
							source: value(1),
							interpolation: "ease_in_out",
						},
					],
					size: 1,
				},
				max_min: {
					minimum: value(0),
					maximum: value(1),
					function: "sinus",
					size: 1,
					pwm,
				},
				middle_amplitude: {
					middle: value(0.5),
					amplitude: 0.5,
					function: "sinus",
					size: 1,
					pwm,
				},
				speed_multiplier: { numerator: 1, denominator: 1 },
				width: 1,
				random_group_id: null,
				phase: null,
			},
		],
		random_groups: [],
		phase_mode: "uniform",
		phase: {
			ordering: { type: "selection" },
			offset_degrees: 0,
			span_degrees: 360,
			block_size: 1,
			repeats: 1,
			wings: false,
			anchors_degrees: [],
		},
		speed: { type: "fixed", duration_millis: 4_000 },
		overall_speed_multiplier: { numerator: 1, denominator: 1 },
		run_mode: "loop",
		default_activation: "start_now",
		activation_boundary: "beat",
	};
}
