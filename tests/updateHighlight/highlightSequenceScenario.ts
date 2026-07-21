import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../apps/control-ui/e2e/bench/pairedScenario";
import { setProgrammerFixtureValue } from "../../apps/control-ui/e2e/bench/programmerValues";
import { replaceProgrammingSelection } from "../../apps/control-ui/e2e/bench/programmingSelection";
import {
	loadCanonicalCopy,
	object,
	pressCommand,
	programmer,
} from "../support/catalog";
import {
	assertFixtureSheetStep,
	clickHighlightKey,
	expectSelection,
	fixtureIds,
	fixtureSheetRowById,
	fixturesByNumber,
	type HighlightFixture,
	highlightAction,
	highlightState,
	openBuiltIn,
	operateProgrammerFade,
	restoreSecondStep,
	selectionsEqual,
	setPanThroughUi,
	verifyProgrammerKeypadGeometry,
} from "../support/updateHighlight/highlight";

interface HighlightSequenceState {
	showId: string;
	fixtures: HighlightFixture[];
	expectedSequence: string[][];
	observedSequence: string[][];
	singletonGroupId: string;
	completeGroupId: string;
	highStayedOff?: boolean;
	wrappedForward?: boolean;
	wrappedBackward?: boolean;
	highSurvivedEmpty?: boolean;
	highFollowedSelection?: boolean;
	removedCaptureRejected?: boolean;
	altCaptureWasNoOp?: boolean;
	geometryVerified?: boolean;
	fixtureSheetVerified?: boolean;
	noCommandBarPanel?: boolean;
}

pairedScenario<HighlightSequenceState>({
	id: "HIGHLIGHT-003",
	title:
		"PREV NEXT ALL mutate the real selection and preserve exact Programmer keypad geometry",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			`highlight-003-${surface}`,
			"default-stage",
		);
		const fixtures = await fixturesByNumber(api, [101, 102, 103, 104]);
		await replaceProgrammingSelection(api, {
			surface: "api",
			showId: show.id,
			fixtures: fixtureIds(fixtures),
		});
		return {
			showId: show.id,
			fixtures,
			expectedSequence: [
				[fixtures[0].id],
				[fixtures[1].id],
				fixtureIds(fixtures),
				[fixtures[3].id],
				[fixtures[2].id],
				[fixtures[1].id],
			],
			observedSequence: [],
			singletonGroupId: "92",
			completeGroupId: "93",
		};
	},
	api: async ({ api }, state) => {
		for (const action of [
			"next",
			"next",
			"all",
			"previous",
			"previous",
			"previous",
		] as const) {
			await highlightAction(api, action);
			state.observedSequence.push([...(await programmer(api)).selected]);
		}
		state.highStayedOff = !(await highlightState(api)).active;

		await highlightAction(api, "next");
		await highlightAction(api, "next");
		await highlightAction(api, "next");
		state.wrappedForward = selectionsEqual((await programmer(api)).selected, [
			state.fixtures[0].id,
		]);
		await highlightAction(api, "previous");
		state.wrappedBackward = selectionsEqual((await programmer(api)).selected, [
			state.fixtures[3].id,
		]);

		await restoreSecondStep(api);
		await setProgrammerFixtureValue(api, {
			surface: "api",
			showId: state.showId,
			fixtureId: state.fixtures[1].id,
			attribute: "pan",
			value: { kind: "normalized", value: 0.72 },
			timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
		});
		await api.executeCommandLine(
			`RECORD GROUP ${state.singletonGroupId}`,
		);
		await highlightAction(api, "all");
		await api.executeCommandLine(`RECORD GROUP ${state.completeGroupId}`);

		await highlightAction(api, "on");
		await replaceProgrammingSelection(api, {
			surface: "api",
			showId: state.showId,
			fixtures: [],
		});
		state.highSurvivedEmpty = (await highlightState(api)).active;
		await replaceProgrammingSelection(api, {
			surface: "api",
			showId: state.showId,
			fixtures: [state.fixtures[2].id, state.fixtures[3].id],
		});
		state.highFollowedSelection = (await highlightState(api)).active;
		await highlightAction(api, "off");
		const removedActions = await Promise.all(
			["capture", "reset"].map(async (action) => {
				try {
					await api.request("POST", "/api/v1/highlight/action", { action });
					return false;
				} catch {
					return true;
				}
			}),
		);
		state.removedCaptureRejected = removedActions.every(Boolean);
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await expectSelection(api, fixtureIds(state.fixtures));
		for (const [index, key] of (
			["NEXT", "NEXT", "ALL", "PREV", "PREV", "PREV"] as const
		).entries()) {
			await clickHighlightKey(page, api, key, state.expectedSequence[index]);
			state.observedSequence.push([...(await programmer(api)).selected]);
		}
		state.highStayedOff = !(await highlightState(api)).active;

		await clickHighlightKey(page, api, "NEXT", [state.fixtures[2].id]);
		await clickHighlightKey(page, api, "NEXT", [state.fixtures[3].id]);
		await clickHighlightKey(page, api, "NEXT", [state.fixtures[0].id]);
		state.wrappedForward = true;
		await clickHighlightKey(page, api, "PREV", [state.fixtures[3].id]);
		state.wrappedBackward = true;

		await clickHighlightKey(page, api, "ALL", fixtureIds(state.fixtures));
		await clickHighlightKey(page, api, "NEXT", [state.fixtures[0].id]);
		await clickHighlightKey(page, api, "NEXT", [state.fixtures[1].id]);
		await setPanThroughUi(page, 72);
		await pressCommand(
			page,
			`RECORD GROUP ${state.singletonGroupId}`,
			`RECORD GROUP ${state.singletonGroupId}`,
		);
		await clickHighlightKey(page, api, "ALL", fixtureIds(state.fixtures));
		await pressCommand(
			page,
			`RECORD GROUP ${state.completeGroupId}`,
			`RECORD GROUP ${state.completeGroupId}`,
		);

		await clickHighlightKey(page, api, "NEXT", [state.fixtures[0].id]);
		await openBuiltIn(page, "Fixtures");
		await assertFixtureSheetStep(
			page,
			state.fixtures,
			state.fixtures[0].number,
		);
		await clickHighlightKey(page, api, "HIGH");
		await assertFixtureSheetStep(
			page,
			state.fixtures,
			state.fixtures[0].number,
		);
		state.fixtureSheetVerified = true;

		await page.locator('[data-keypad-key="CLR"]').click();
		await expectSelection(api, []);
		state.highSurvivedEmpty = (await highlightState(api)).active;
		await fixtureSheetRowById(page, state.fixtures[2].id).click();
		await fixtureSheetRowById(page, state.fixtures[3].id).click();
		await expectSelection(api, [state.fixtures[2].id, state.fixtures[3].id]);
		state.highFollowedSelection = (await highlightState(api)).active;
		await clickHighlightKey(page, api, "HIGH");
		await expectSelection(api, [state.fixtures[2].id, state.fixtures[3].id]);

		await page.keyboard.press("Alt+H");
		await expect
			.poll(async () => (await highlightState(api)).active)
			.toBe(true);
		await page.waitForTimeout(175);
		await page.keyboard.press("Alt+ArrowRight");
		await expectSelection(api, [state.fixtures[2].id]);
		await page.waitForTimeout(175);
		await page.keyboard.press("Alt+ArrowLeft");
		await expectSelection(api, [state.fixtures[3].id]);
		await page.waitForTimeout(175);
		await page.keyboard.press("Alt+a");
		await expectSelection(api, [state.fixtures[2].id, state.fixtures[3].id]);
		const beforeAltCapture = await highlightState(api);
		await page.keyboard.press("Alt+c");
		await page.waitForTimeout(175);
		const afterAltCapture = await highlightState(api);
		state.altCaptureWasNoOp =
			JSON.stringify(afterAltCapture) === JSON.stringify(beforeAltCapture);

		await verifyProgrammerKeypadGeometry(page, api);
		await operateProgrammerFade(page, api);
		state.geometryVerified = true;
		await expect(
			page.locator(
				".command-line-bar .highlight-feedback, .command-line-bar [aria-label='Highlight status']",
			),
		).toHaveCount(0);
		state.noCommandBarPanel = true;
	},
	assert: async ({ api }, state, surface) => {
		expect(state.observedSequence).toEqual(state.expectedSequence);
		expect(state.highStayedOff).toBe(true);
		expect(state.wrappedForward).toBe(true);
		expect(state.wrappedBackward).toBe(true);
		expect(state.highSurvivedEmpty).toBe(true);
		expect(state.highFollowedSelection).toBe(true);
		expect(
			(await object<any>(api, "group", state.singletonGroupId)).body.fixtures,
		).toEqual([state.fixtures[1].id]);
		expect(
			(await object<any>(api, "group", state.completeGroupId)).body.fixtures,
		).toEqual(fixtureIds(state.fixtures));
		const current = await programmer(api);
		expect(
			current.values.some(
				(entry) =>
					entry.fixture_id === state.fixtures[1].id &&
					entry.attribute === "pan",
			),
		).toBe(true);
		if (surface === "api") {
			expect(state.removedCaptureRejected).toBe(true);
		} else {
			expect(state.altCaptureWasNoOp).toBe(true);
			expect(state.geometryVerified).toBe(true);
			expect(state.fixtureSheetVerified).toBe(true);
			expect(state.noCommandBarPanel).toBe(true);
		}
	},
});
