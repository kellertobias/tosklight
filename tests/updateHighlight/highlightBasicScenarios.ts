import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../apps/control-ui/e2e/bench/pairedScenario";
import { setProgrammerFixtureValue } from "../../apps/control-ui/e2e/bench/programmerValues";
import {
	replaceProgrammingSelection,
	selectProgrammingGroup,
} from "../../apps/control-ui/e2e/bench/programmingSelection";
import {
	loadCanonicalCopy,
	object,
	programmer,
	putObject,
} from "../support/catalog";
import {
	clickHighlightKey,
	expectSelection,
	fixtureIds,
	fixtureSheetRowById,
	fixturesByNumber,
	groupBody,
	type HighlightFixture,
	highlightAction,
	highlightState,
	openBuiltIn,
	openGroups,
	selectionsEqual,
	storeCurrentProgrammerPreset,
} from "../support/updateHighlight/highlight";

interface HighlightScenarioState {
	showId: string;
	fixtures: HighlightFixture[];
	storedPresetId: string;
	selectionStayedComplete?: boolean;
}

interface HighlightSurfaceState {
	showId: string;
	fixtures: HighlightFixture[];
	liveGroup: {
		id: string;
		name: string;
		initial: string[];
		updated: string[];
	};
	steppedSelection?: string[];
	restoredSelection?: string[];
	highSurvivedEmpty?: boolean;
	highFollowedSelection?: boolean;
	reconnectRetained?: boolean;
}

pairedScenario<HighlightScenarioState>({
	id: "HIGHLIGHT-001",
	title:
		"HIGH follows the actual selection while stepped values remain normal programmer data",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			`highlight-001-${surface}`,
			"default-stage",
		);
		const fixtures = await fixturesByNumber(api, [101, 102, 103]);
		await replaceProgrammingSelection(api, {
			surface: "api",
			showId: show.id,
			fixtures: [fixtures[0].id],
		});
		await setPan(api, show.id, fixtures[0].id, 0.63);
		await replaceProgrammingSelection(api, {
			surface: "api",
			showId: show.id,
			fixtures: fixtureIds(fixtures),
		});
		return { showId: show.id, fixtures, storedPresetId: "197" };
	},
	api: async ({ api }, state) => {
		await highlightAction(api, "on");
		state.selectionStayedComplete = selectionsEqual(
			(await programmer(api)).selected,
			fixtureIds(state.fixtures),
		);
		await highlightAction(api, "next");
		const first = (await programmer(api)).selected[0];
		await setPan(api, state.showId, first, 0.41);
		await highlightAction(api, "next");
		const second = (await programmer(api)).selected[0];
		await setPan(api, state.showId, second, 0.52);
		await highlightAction(api, "off");
		await storeCurrentProgrammerPreset(api, state.showId, state.storedPresetId);
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await expectSelection(api, fixtureIds(state.fixtures));
		await clickHighlightKey(page, api, "HIGH");
		await expect
			.poll(async () => (await highlightState(api)).active)
			.toBe(true);
		state.selectionStayedComplete = selectionsEqual(
			(await programmer(api)).selected,
			fixtureIds(state.fixtures),
		);
		await clickHighlightKey(page, api, "NEXT", [state.fixtures[0].id]);
		await setPan(api, state.showId, state.fixtures[0].id, 0.41);
		await clickHighlightKey(page, api, "NEXT", [state.fixtures[1].id]);
		await setPan(api, state.showId, state.fixtures[1].id, 0.52);
		await clickHighlightKey(page, api, "HIGH");
		await expect
			.poll(async () => (await highlightState(api)).active)
			.toBe(false);
		await storeCurrentProgrammerPreset(api, state.showId, state.storedPresetId);
	},
	assert: async ({ api }, state) => {
		expect(state.selectionStayedComplete).toBe(true);
		const highlight = await highlightState(api);
		expect(highlight).toMatchObject({
			active: false,
			output_enabled: false,
			mode: "step",
			active_index: 1,
			can_previous: true,
			can_next: true,
		});
		expect(
			highlight.remembered.map((fixture: any) => fixture.fixture_id),
		).toEqual(fixtureIds(state.fixtures));
		expect(highlight.active_fixture.fixture_id).toBe(state.fixtures[1].id);
		const current = await programmer(api);
		expect(current.selected).toEqual([state.fixtures[1].id]);
		const values = current.values ?? [];
		expect(
			values.some(
				(entry) =>
					entry.fixture_id === state.fixtures[0].id &&
					entry.attribute === "pan",
			),
		).toBe(true);
		expect(
			values.some(
				(entry) =>
					entry.fixture_id === state.fixtures[1].id &&
					entry.attribute === "pan",
			),
		).toBe(true);
		expect(
			values.every(
				(entry) => !String(entry.attribute).toLowerCase().includes("highlight"),
			),
		).toBe(true);
		const preset = await object<any>(api, "preset", state.storedPresetId);
		const storedAttributes = Object.values(preset.body.values ?? {}).flatMap(
			(attributes: any) => Object.keys(attributes),
		);
		expect(storedAttributes).toContain("pan");
		expect(
			storedAttributes.every(
				(attribute) => !attribute.toLowerCase().includes("highlight"),
			),
		).toBe(true);
	},
});

function setPan(
	api: ApiDriver,
	showId: string,
	fixtureId: string,
	value: number,
) {
	return setProgrammerFixtureValue(api, {
		surface: "api",
		showId,
		fixtureId,
		attribute: "pan",
		value: { kind: "normalized", value },
		timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
	});
}

pairedScenario<HighlightSurfaceState>({
	id: "HIGHLIGHT-002",
	title:
		"live Group ALL restoration, external selection, empty HIGH, and lifecycle stay authoritative",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			`highlight-002-${surface}`,
			"default-stage",
		);
		const fixtures = await fixturesByNumber(
			api,
			[101, 102, 103, 104, 105, 106],
		);
		const initial = fixtureIds(fixtures.slice(0, 4));
		const updated = [
			fixtures[3].id,
			fixtures[1].id,
			fixtures[4].id,
			fixtures[0].id,
		];
		const liveGroup = {
			id: "30",
			name: "Feature 20 Live Group",
			initial,
			updated,
		};
		await putObject(
			api,
			"group",
			liveGroup.id,
			groupBody(liveGroup.name, initial),
		);
		return {
			showId: show.id,
			fixtures,
			liveGroup,
		};
	},
	api: async ({ api }, state) => {
		await selectProgrammingGroup(api, {
			surface: "api",
			showId: state.showId,
			groupId: state.liveGroup.id,
			frozen: false,
			rule: { type: "all" },
		});
		await highlightAction(api, "next");
		await highlightAction(api, "next");
		state.steppedSelection = [...(await programmer(api)).selected];
		const stored = await object<any>(api, "group", state.liveGroup.id);
		await putObject(
			api,
			"group",
			state.liveGroup.id,
			{ ...stored.body, fixtures: state.liveGroup.updated },
			stored.revision,
		);
		await highlightAction(api, "all");
		state.restoredSelection = [...(await programmer(api)).selected];
		await highlightAction(api, "on");
		await replaceProgrammingSelection(api, {
			surface: "api",
			showId: state.showId,
			fixtures: [],
		});
		state.highSurvivedEmpty =
			(await highlightState(api)).active &&
			(await programmer(api)).selected.length === 0;
		await replaceProgrammingSelection(api, {
			surface: "api",
			showId: state.showId,
			fixtures: [state.fixtures[2].id, state.fixtures[3].id],
		});
		state.highFollowedSelection =
			(await highlightState(api)).active &&
			selectionsEqual((await programmer(api)).selected, [
				state.fixtures[2].id,
				state.fixtures[3].id,
			]);
		const deskId = api.session!.desk.id;
		await api.login("Operator", deskId);
		state.reconnectRetained = (await highlightState(api)).active;
		await api.request("POST", `/api/v1/shows/${state.showId}/open`, {
			transition: "hold_current",
		});
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await openGroups(page);
		await page
			.locator(".group-pool-window .group-card")
			.filter({ hasText: state.liveGroup.name })
			.first()
			.click();
		await expectSelection(api, state.liveGroup.initial);
		await clickHighlightKey(page, api, "NEXT", [state.liveGroup.initial[0]]);
		await clickHighlightKey(page, api, "NEXT", [state.liveGroup.initial[1]]);
		state.steppedSelection = [...(await programmer(api)).selected];
		const stored = await object<any>(api, "group", state.liveGroup.id);
		await putObject(
			api,
			"group",
			state.liveGroup.id,
			{ ...stored.body, fixtures: state.liveGroup.updated },
			stored.revision,
		);
		await clickHighlightKey(page, api, "ALL", state.liveGroup.updated);
		state.restoredSelection = [...(await programmer(api)).selected];
		await clickHighlightKey(page, api, "HIGH");
		await page.locator('[data-keypad-key="CLR"]').click();
		await expectSelection(api, []);
		state.highSurvivedEmpty = (await highlightState(api)).active;
		await openBuiltIn(page, "Fixtures");
		await fixtureSheetRowById(page, state.fixtures[2].id).click();
		await fixtureSheetRowById(page, state.fixtures[3].id).click();
		await expectSelection(api, [state.fixtures[2].id, state.fixtures[3].id]);
		state.highFollowedSelection = (await highlightState(api)).active;
		await page.reload();
		await expect(page.locator(".connection-cover")).toBeHidden({
			timeout: 10_000,
		});
		state.reconnectRetained = (await highlightState(api)).active;
		await api.request("POST", `/api/v1/shows/${state.showId}/open`, {
			transition: "hold_current",
		});
	},
	assert: async ({ api }, state) => {
		expect(state.steppedSelection).toEqual([state.liveGroup.initial[1]]);
		expect(state.restoredSelection).toEqual(state.liveGroup.updated);
		expect(state.highSurvivedEmpty).toBe(true);
		expect(state.highFollowedSelection).toBe(true);
		expect(state.reconnectRetained).toBe(true);
		const highlight = await highlightState(api);
		expect(highlight).toMatchObject({
			active: false,
			output_enabled: false,
			mode: "selection",
			active_index: null,
			active_fixture: null,
		});
		expect(
			highlight.remembered.map((fixture: any) => fixture.fixture_id),
		).toEqual([state.fixtures[2].id, state.fixtures[3].id]);
	},
});
