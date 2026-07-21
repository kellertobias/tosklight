import { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { setProgrammerPriority } from "../../apps/control-ui/e2e/bench/programmerPriority";
import {
	clearProgrammerValues,
	releaseProgrammerFixtureValue,
	setProgrammerFixtureValue,
} from "../../apps/control-ui/e2e/bench/programmerValues";
import { fixtureIdsByNumber, loadCanonicalCopy } from "../support/catalog";
import {
	CUE_SEMANTIC_CONTRACTS,
	fixtureCue,
	installPlaybackSequence,
	registerPairedCueScenario,
	rgbValues,
	setSequenceMasterFade,
	slot,
	visualizationAfterTick,
	visualizationLevel,
} from "./support";

const PROGRAMMER_TIMING = {
	fade: true,
	fadeMillis: 0,
	delayMillis: null,
} as const;

test.describe(CUE_SEMANTIC_CONTRACTS, () => {
	test("MERGE-001 @api › two programmer identities arbitrate by priority, HTP magnitude, and stable LTP edit time", async ({
		api,
		bench,
	}) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			"merge-001-two-programmers",
			"compact-rig",
		);
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		await api.request("POST", "/api/v1/users", {
			name: "Programmer A",
			enabled: true,
		});
		await api.request("POST", "/api/v1/users", {
			name: "Programmer B",
			enabled: true,
		});
		const first = new ApiDriver(api.baseUrl);
		const second = new ApiDriver(api.baseUrl);
		await first.login("Programmer A");
		await second.login("Programmer B");
		await setProgrammerPriority(first, { surface: "api", priority: 0 });
		await setProgrammerPriority(second, { surface: "api", priority: 0 });
		await setFixtureValue(first, show.id, fixtures[1], "intensity", 0.4);
		await bench.tick(1);
		await setFixtureValue(second, show.id, fixtures[1], "intensity", 0.7);
		expect(slot(await bench.tick(0), 1)).toBe(179);

		await setProgrammerPriority(first, { surface: "api", priority: 10 });
		await setProgrammerPriority(second, { surface: "api", priority: 20 });
		await setFixtureValue(first, show.id, fixtures[1], "intensity", 0.9);
		await setFixtureValue(second, show.id, fixtures[1], "intensity", 0.2);
		expect(slot(await bench.tick(0), 1)).toBe(51);

		const rgb = fixtures[21];
		await setProgrammerPriority(first, { surface: "api", priority: 10 });
		await setProgrammerPriority(second, { surface: "api", priority: 10 });
		await setFixtureValue(first, show.id, rgb, "red", 0.4);
		await bench.tick(1);
		await setFixtureValue(second, show.id, rgb, "red", 0.8);
		expect(await visualizationAfterTick(api, bench, rgb, "red", 0)).toBe(0.8);
		expect(await visualizationAfterTick(api, bench, rgb, "red", 0)).toBe(0.8);

		const diagnostics = await api.request<any>("GET", "/api/v1/diagnostics");
		expect(
			diagnostics.active_programmers.filter((programmer: any) =>
				programmer.values.some(
					(value: any) => value.fixture_id === rgb && value.attribute === "red",
				),
			),
		).toHaveLength(2);
		await releaseProgrammerFixtureValue(second, {
			surface: "api",
			showId: show.id,
			fixtureId: rgb,
			attribute: "red",
		});
		expect(await visualizationAfterTick(api, bench, rgb, "red", 0)).toBe(0.4);
	});
});

registerPairedCueScenario<{ completed: boolean; showId: string }>({
	id: "MERGE-002",
	title:
		"independent Sequences coexist and retrigger only their stored addresses",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			surface === "api" ? "merge-002-independent" : "merge-002-independent-ui",
			"compact-rig",
		);
		return { completed: false, showId: show.id };
	},
	api: async ({ api, bench }, state) => {
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		const aFixture = fixtures[21];
		const bFixture = fixtures[22];
		await installPlaybackSequence(
			api,
			1,
			[
				fixtureCue(1, [
					[aFixture, "intensity", 0.6],
					[aFixture, "red", 0],
					[aFixture, "green", 0],
					[aFixture, "blue", 1],
				]),
			],
			{ name: "Sequence A", priority: 100 },
		);
		await installPlaybackSequence(
			api,
			2,
			[
				fixtureCue(1, [
					[bFixture, "intensity", 0.4],
					[bFixture, "red", 1],
					[bFixture, "green", 0.7],
					[bFixture, "blue", 0.4],
				]),
			],
			{ name: "Sequence B", priority: 100 },
		);
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await bench.tick(1);
		await api.request("POST", "/api/v1/cuelists/2/go", {});
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
		expect(await visualizationLevel(api, bFixture, "intensity")).toBe(0.4);

		await setFixtureValue(api, state.showId, aFixture, "intensity", 0.3);
		for (const [attribute, value] of [
			["red", 1],
			["green", 0],
			["blue", 0],
		] as const)
			await setFixtureValue(api, state.showId, aFixture, attribute, value);
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
		expect(await rgbValues(api, aFixture)).toEqual([1, 0, 0]);
		expect(await rgbValues(api, bFixture)).toEqual([1, 0.7, 0.4]);

		await bench.tick(1);
		await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 1 });
		await bench.tick(0);
		expect(await rgbValues(api, aFixture)).toEqual([0, 0, 1]);
		expect(await rgbValues(api, bFixture)).toEqual([1, 0.7, 0.4]);

		await setFixtureValue(api, state.showId, bFixture, "blue", 0.8);
		await bench.tick(1);
		await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 1 });
		await bench.tick(0);
		expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);

		await setProgrammerPriority(api, { surface: "api", priority: 110 });
		await setFixtureValue(api, state.showId, aFixture, "red", 1);
		await bench.tick(1);
		await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 1 });
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "red")).toBe(1);
		await setProgrammerPriority(api, { surface: "api", priority: 90 });
		await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 1 });
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "red")).toBe(0);

		await clearProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
		});
		await api.request("POST", "/api/v1/cuelists/2/off", {});
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
		expect(await visualizationLevel(api, bFixture, "intensity")).toBe(0);
		await api.request("POST", "/api/v1/cuelists/1/off", {});
		await bench.tick(0);
		expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0);
		state.completed = true;
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await setSequenceMasterFade(api, 0);
		const fixtures = await fixtureIdsByNumber(api);
		const aFixture = fixtures[21];
		const bFixture = fixtures[22];
		await installPlaybackSequence(
			api,
			1,
			[
				fixtureCue(1, [
					[aFixture, "intensity", 0.6],
					[aFixture, "red", 0],
					[aFixture, "green", 0],
					[aFixture, "blue", 1],
				]),
			],
			{ name: "Sequence A", priority: 100 },
		);
		await installPlaybackSequence(
			api,
			2,
			[
				fixtureCue(1, [
					[bFixture, "intensity", 0.4],
					[bFixture, "red", 1],
					[bFixture, "green", 0.7],
					[bFixture, "blue", 0.4],
				]),
			],
			{ name: "Sequence B", priority: 100 },
		);
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await api.request("POST", "/api/v1/cuelists/2/go", {});
		await bench.tick(1);
		await setFixtureValue(api, state.showId, aFixture, "red", 1);
		await setFixtureValue(api, state.showId, bFixture, "blue", 0.8);
		await bench.tick(1);
		expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);
		await desk.open(bench.baseUrl);
		api.session = await desk.session();
		expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);
		await page.locator(".mode-toggle").click();
		await page.keyboard.press("Shift+KeyZ");
		const first = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: "Sequence A" });
		await first.getByRole("button", { name: "GO +", exact: true }).click();
		await expect(first).toHaveAttribute("data-selected-playback", "true");
		expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);
		await page.locator(".mode-toggle").click();
		await page.getByRole("button", { name: "CUE", exact: true }).click();
		await page.getByRole("button", { name: "1", exact: true }).click();
		await expect(page.getByLabel("Command line")).toHaveValue("CUE 1");
		await page.getByRole("button", { name: "ENT", exact: true }).click();
		await expect(page.getByLabel("Command line")).toHaveValue("FIXTURE");
		await bench.tick(0);
		expect(await rgbValues(api, aFixture)).toEqual([0, 0, 1]);
		expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);
		expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
		expect(await visualizationLevel(api, bFixture, "intensity")).toBe(0.4);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});

function setFixtureValue(
	api: ApiDriver,
	showId: string,
	fixtureId: string,
	attribute: string,
	value: number,
) {
	return setProgrammerFixtureValue(api, {
		surface: "api",
		showId,
		fixtureId,
		attribute,
		value: { kind: "normalized", value },
		timing: PROGRAMMER_TIMING,
	});
}
