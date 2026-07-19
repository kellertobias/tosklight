import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	putObject,
} from "../support/catalog";
import {
	type FixtureValue,
	fixtureCue,
	installPlaybackSequence,
	registerPairedCueScenario,
	rgbValues,
	runtime,
	setSequenceMasterFade,
	visualizationLevel,
} from "./support";

registerPairedCueScenario<{ completed: boolean }>({
	id: "MERGE-003",
	title:
		"full normal overwrite auto-Offs while partial, disabled, Flash, and Temp restore",
	arrange: () => ({ completed: false }),
	api: async ({ api, bench }, state) => {
		const prepare = async (name: string, includeIntensity: boolean) => {
			await loadCanonicalCopy(api, bench, name, "compact-rig");
			await setSequenceMasterFade(api, 0);
			const fixture = (await fixtureIdsByNumber(api))[21];
			const underlying = await installPlaybackSequence(
				api,
				1,
				[
					fixtureCue(1, [
						...(includeIntensity
							? [[fixture, "intensity", 1] as FixtureValue]
							: []),
						[fixture, "red", 0],
						[fixture, "green", 0],
						[fixture, "blue", 1],
					]),
				],
				{ name: "Underlying blue", auto_off: true },
			);
			await installPlaybackSequence(
				api,
				2,
				[
					fixtureCue(1, [
						[fixture, "red", 1],
						[fixture, "green", 0],
						[fixture, "blue", 0],
					]),
				],
				{ name: "Replacing red", auto_off: false },
			);
			return { fixture, underlying };
		};

		let prepared = await prepare("merge-003-full", false);
		await api.request("POST", "/api/v1/cuelists/1/on", {});
		await bench.tick(1);
		await api.request("POST", "/api/v1/cuelists/2/on", {});
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: false });
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);

		let definition = await object<any>(api, "playback", "1");
		await putObject(
			api,
			"playback",
			"1",
			{ ...definition.body, auto_off: false },
			definition.revision,
		);
		await api.request("POST", "/api/v1/cuelists/2/off", {});
		await api.request("POST", "/api/v1/cuelists/1/on", {});
		await bench.tick(1);
		await api.request("POST", "/api/v1/cuelists/2/on", {});
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		await api.request("POST", "/api/v1/cuelists/2/off", {});
		await bench.tick(0);
		expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);

		definition = await object<any>(api, "playback", "1");
		await putObject(
			api,
			"playback",
			"1",
			{ ...definition.body, auto_off: true },
			definition.revision,
		);
		await api.request("POST", "/api/v1/cuelists/1/on", {});
		await api.request("POST", "/api/v1/cuelists/2/flash", { pressed: true });
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
		await api.request("POST", "/api/v1/cuelists/2/flash", { pressed: false });
		await bench.tick(0);
		expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);
		await api.request("POST", "/api/v1/cuelists/2/temp", {});
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
		await api.request("POST", "/api/v1/cuelists/2/temp", {});
		await bench.tick(0);
		expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);

		prepared = await prepare("merge-003-partial", true);
		await api.request("POST", "/api/v1/cuelists/1/on", {});
		await bench.tick(1);
		await api.request("POST", "/api/v1/cuelists/2/on", {});
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		expect(await visualizationLevel(api, prepared.fixture, "intensity")).toBe(
			1,
		);
		expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
		state.completed = true;
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await loadCanonicalCopy(
			api,
			bench,
			"merge-003-visible-actions",
			"compact-rig",
		);
		await setSequenceMasterFade(api, 0);
		const fixture = (await fixtureIdsByNumber(api))[21];
		const underlying = await installPlaybackSequence(
			api,
			1,
			[
				fixtureCue(1, [
					[fixture, "red", 0],
					[fixture, "green", 0],
					[fixture, "blue", 1],
				]),
			],
			{ name: "Underlying blue", auto_off: true },
		);
		await installPlaybackSequence(
			api,
			2,
			[
				fixtureCue(1, [
					[fixture, "red", 1],
					[fixture, "green", 0],
					[fixture, "blue", 0],
				]),
			],
			{ name: "Replacing red", auto_off: false },
		);
		for (const [number, buttons] of [
			[1, ["on", "off", "none"]],
			[2, ["on", "flash", "temp"]],
		] as const) {
			const definition = await object<any>(api, "playback", String(number));
			await putObject(
				api,
				"playback",
				String(number),
				{ ...definition.body, buttons },
				definition.revision,
			);
		}
		await desk.open(bench.baseUrl);
		await page.locator(".mode-toggle").click();
		const first = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: "Underlying blue" });
		const second = page
			.locator(".playback-fader-bank article")
			.filter({ hasText: "Replacing red" });
		await first.getByRole("button", { name: "ON", exact: true }).click();
		await bench.tick(1);
		await second.getByRole("button", { name: "ON", exact: true }).click();
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: false });
		expect(await rgbValues(api, fixture)).toEqual([1, 0, 0]);

		await api.request("POST", "/api/v1/cuelists/2/off", {});
		await first.getByRole("button", { name: "ON", exact: true }).click();
		const flash = second.getByRole("button", { name: "FLASH", exact: true });
		await flash.hover();
		await page.mouse.down();
		await expect.poll(async () => rgbValues(api, fixture)).toEqual([1, 0, 0]);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		await page.mouse.up();
		await expect.poll(async () => rgbValues(api, fixture)).toEqual([0, 0, 1]);
		const temp = second.getByRole("button", { name: "TEMP", exact: true });
		await temp.click();
		await expect.poll(async () => rgbValues(api, fixture)).toEqual([1, 0, 0]);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		await temp.click();
		await expect.poll(async () => rgbValues(api, fixture)).toEqual([0, 0, 1]);

		const stored = await object<any>(api, "cue_list", underlying.id);
		stored.body.cues[0].changes.push({
			fixture_id: fixture,
			attribute: "intensity",
			value: { kind: "normalized", value: 1 },
			automatic_restore: false,
		});
		await putObject(
			api,
			"cue_list",
			underlying.id,
			stored.body,
			stored.revision,
		);
		await api.request("POST", "/api/v1/cuelists/1/off", {});
		await api.request("POST", "/api/v1/cuelists/2/off", {});
		await first.getByRole("button", { name: "ON", exact: true }).click();
		await bench.tick(1);
		await second.getByRole("button", { name: "ON", exact: true }).click();
		await bench.tick(0);
		expect(await runtime(api, 1)).toMatchObject({ enabled: true });
		expect(await visualizationLevel(api, fixture, "intensity")).toBe(1);
		expect(await rgbValues(api, fixture)).toEqual([1, 0, 0]);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});
