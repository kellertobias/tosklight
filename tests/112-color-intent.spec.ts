import type { Locator, Page } from "@playwright/test";
import { HttpPresetRecordingTransport } from "../apps/light-desktop/src/api/PresetRecordingTransport";
import type { AttributeValue } from "../apps/light-desktop/src/api/types/playback";
import { replaceProgrammingSelection } from "./bench/command-selection/programmingSelection";
import type { ApiDriver } from "./bench/core/api";
import type { DeskDriver } from "./bench/core/desk";
import { expect, test } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import { requireSemanticContract } from "./bench/core/semanticContract";
import { recallPreset } from "./bench/groups-presets/presetRecall";
import {
	batchProgrammerValues,
	clearProgrammerValues,
	type ProgrammerValuesMutation,
} from "./bench/programmer/programmerValues";
import { BrowserPatch } from "./bench/show-setup/patchScenario";

test.use({ viewport: { width: 1600, height: 1000 } });

type ColorModel = "direct" | "intent";
type Xyz = { x: number; y: number; z: number };

/** The rig named by the scenario. Dimmer-first modes keep Intensity on its own channel. */
const RIG = [
	{ number: 1, name: "RGB 1", profile: "RGB LED", mode: "RGB virtual dimmer", address: 1 },
	{ number: 2, name: "RGB 2", profile: "RGB LED", mode: "DRGB 8-bit dimmer first", address: 11 },
	{ number: 3, name: "RGBW 3", profile: "RGBW LED", mode: "DRGBW 8-bit dimmer first", address: 21 },
	{ number: 4, name: "CMY 4", profile: "CMY LED", mode: "DCMY 8-bit dimmer first", address: 31 },
	{ number: 5, name: "Dimmer 5", profile: "Dimmer", mode: "8-bit", address: 41 },
] as const;
type RigNumber = (typeof RIG)[number]["number"];

/**
 * Channel layouts of the chosen Generic modes, read from the shipped fixture packages. The CMY
 * LED's flags are its inverted red, green and blue channels.
 */
const LAYOUT: Record<RigNumber, readonly string[]> = {
	1: ["red", "green", "blue"],
	2: ["dimmer", "red", "green", "blue"],
	3: ["dimmer", "red", "green", "blue", "white"],
	4: ["dimmer", "cyan", "magenta", "yellow"],
	5: ["dimmer"],
};

const NOW = { fade: false, fadeMillis: null, delayMillis: null };
const RED = { hue: 0, saturation: 1 };
const GREEN = { hue: 1 / 3, saturation: 1 };
const BLUE = { hue: 2 / 3, saturation: 1 };
/** sRGB red at half its level: a Direct colour that carries its own brightness. */
const HALF_RED: Xyz = { x: 0.4124564 / 2, y: 0.2126729 / 2, z: 0.0193339 / 2 };

interface ColorShow {
	id: string;
	ids: Record<number, string>;
}

test.describe("docs/testing/26-color-intent.md", () => {
	test("COLORINTENT-001 @ui › existing and Direct-default shows stay Direct; the desk default only seeds new shows", async ({
		api,
		bench,
		desk,
		page,
		show: existing,
	}) => {
		test.setTimeout(120_000);
		// A show that existed before the feature stores no colour model and reads Direct.
		expect(await colorModel(api, existing.id)).toBe("direct");
		expect(await storedColorModel(api, existing.id)).toBeUndefined();

		await desk.open(api.baseUrl);
		await openSetup(page, "Defaults", "New shows");
		await expect(defaultModelField(page)).toContainText("Direct");

		// Created while the desk default reads Direct: Direct and nothing stored. Since programming
		// contract 1 the Color dialog programs the whole colour on `color` in every model, never a
		// separate channel percentage, and the DMX is unchanged.
		const direct = await createRiggedShow(api, page, desk, "Direct");
		expect(await colorModel(api, direct.id)).toBe("direct");
		expect(await storedColorModel(api, direct.id)).toBeUndefined();
		await select(api, direct, [direct.ids[1], direct.ids[2]]);
		await programColor(api, direct, [1, 2], RED);
		await setIntensity(api, direct, [2], 1);
		const directValues = await programmerValues(api);
		expect(attributesOf(directValues, direct.ids[1])).toEqual(["color"]);
		expect(
			directValues.filter((value) => value.attribute.startsWith("color.")),
		).toEqual([]);
		expect(await fixtureDmx(bench, 1)).toEqual({ red: 255, green: 0, blue: 0 });
		expect(await fixtureDmx(bench, 2)).toEqual({
			dimmer: 255,
			red: 255,
			green: 0,
			blue: 0,
		});
		await clearProgrammer(api, direct);

		// Setup → Defaults → New shows → Color programming model → Color Intent.
		await openSetup(page, "Defaults", "New shows");
		await chooseOption(page, defaultModelField(page), "Color Intent");
		await expect
			.poll(
				async () =>
					(await deskConfiguration(api)).color_programming_model_default,
			)
			.toBe("intent");

		const intent = await createRiggedShow(api, page, desk, "Intent");
		expect(await colorModel(api, intent.id)).toBe("intent");
		expect(await storedColorModel(api, intent.id)).toBe("intent");
		await openSetup(page, "Attributes & encoders", "Color model");
		await expect(showModelField(page)).toContainText("Color Intent");

		// Shows that already existed keep reading Direct.
		for (const showId of [existing.id, direct.id]) {
			await openShow(api, showId);
			expect(await colorModel(api, showId)).toBe("direct");
			await openSetup(page, "Attributes & encoders", "Color model");
			await expect(showModelField(page)).toContainText("Direct");
		}

		// Setting the desk default back to Direct leaves the Color Intent show in Color Intent.
		await openSetup(page, "Defaults", "New shows");
		await chooseOption(page, defaultModelField(page), "Direct");
		await expect
			.poll(
				async () =>
					(await deskConfiguration(api)).color_programming_model_default,
			)
			.toBe("direct");
		await openShow(api, intent.id);
		expect(await colorModel(api, intent.id)).toBe("intent");
		expect(await storedColorModel(api, intent.id)).toBe("intent");
		await openSetup(page, "Attributes & encoders", "Color model");
		await expect(showModelField(page)).toContainText("Color Intent");
	});

	test("COLORINTENT-002 @ui › one whole colour on every fixture, level from Intensity", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(120_000);
		const show = await createRiggedShow(api, page, desk, "Intent", "intent");
		const all = RIG.map((rig) => show.ids[rig.number]);
		await select(api, show, all);
		await setIntensity(api, show, [1, 2, 3, 4, 5], 1);

		// The Color dialog has no Brightness and no Tint control; picking red stores whole colours.
		await desk.open(api.baseUrl);
		const dialog = await openColorDialog(page);
		await expect(dialog.getByText("Brightness", { exact: true })).toHaveCount(0);
		await expect(dialog.getByRole("slider", { name: /brightness/i })).toHaveCount(0);
		await expect(dialog.getByRole("button", { name: /brightness/i })).toHaveCount(0);
		await expect(dialog.getByText("Tint", { exact: true })).toHaveCount(0);
		await expect(dialog.getByRole("slider", { name: /tint/i })).toHaveCount(0);
		await pickPureRed(page, dialog);
		// Every colour-capable fixture takes the one whole colour; Dimmer 5 has no colour owner.
		const colored = all.filter((id) => id !== show.ids[5]);
		await expect
			.poll(async () =>
				(await programmerValues(api))
					.filter((value) => value.attribute === "color")
					.map((value) => value.fixture_id)
					.sort(),
			)
			.toEqual([...colored].sort());
		const picked = await programmerValues(api);
		for (const value of picked.filter((entry) => entry.attribute === "color"))
			expect(value.value.kind).toBe("color_program");
		expect(
			picked.filter((value) => value.attribute.startsWith("color.")),
		).toEqual([]);
		await closeDialog(dialog);

		// Exact pure red on DMX.
		await programColor(api, show, [1, 2, 3, 4, 5], RED);
		expect(await fixtureDmx(bench, 1)).toEqual({ red: 255, green: 0, blue: 0 });
		expect(await fixtureDmx(bench, 2)).toEqual({
			dimmer: 255,
			red: 255,
			green: 0,
			blue: 0,
		});
		expect(await fixtureDmx(bench, 4)).toEqual({
			dimmer: 255,
			cyan: 0,
			magenta: 255,
			yellow: 255,
		});
		// Dimmer 5 has no colour channel to touch; only its Intensity plays.
		expect(await fixtureDmx(bench, 5)).toEqual({ dimmer: 255 });
		const full = await universe(bench);

		// Intensity 50% halves the level: the dimmer channel, or all colour channels together.
		await setIntensity(api, show, [1, 2, 3, 4, 5], 0.5);
		const rgbHalf = await fixtureDmx(bench, 1);
		expect(rgbHalf.red).toBeGreaterThanOrEqual(127);
		expect(rgbHalf.red).toBeLessThanOrEqual(128);
		expect([rgbHalf.green, rgbHalf.blue]).toEqual([0, 0]);
		for (const number of [2, 4, 5] as const) {
			const half = await fixtureDmx(bench, number);
			expect(half.dimmer).toBeGreaterThanOrEqual(127);
			expect(half.dimmer).toBeLessThanOrEqual(128);
		}
		expect(await fixtureDmx(bench, 2)).toMatchObject({
			red: 255,
			green: 0,
			blue: 0,
		});
		expect(await fixtureDmx(bench, 4)).toMatchObject({
			cyan: 0,
			magenta: 255,
			yellow: 255,
		});
		await setIntensity(api, show, [1, 2, 3, 4, 5], 1);

		// Picking the same red again changes nothing: the colour carries no level of its own.
		await programColor(api, show, [1, 2, 3, 4, 5], RED);
		expect(await universe(bench)).toEqual(full);

		// Native colour channels are not programmable, and the refusal names Color Intent.
		await expect(
			mutate(api, show, [
				{
					action: "set_fixture",
					fixtureId: show.ids[1],
					attribute: "color.red",
					value: normalized(0.5),
					timing: NOW,
				},
			]),
		).rejects.toThrow(/Color Intent/);
		expect(
			(await programmerValues(api)).filter((value) =>
				value.attribute.startsWith("color."),
			),
		).toEqual([]);
		expect(await universe(bench)).toEqual(full);

		// A media-server layer selection opens Media color, whose White Blend is the greyscale.
		const patch = new BrowserPatch(api, page, desk);
		await patch.via.api.add({
			number: 9,
			name: "Media 9",
			manufacturer: "ToskLight",
			profile: "Media Server",
			mode: "2 layers",
			address: "2.1",
		});
		const media = (await api.patch()).fixtures.find(
			(fixture) => fixture.fixture_number === 9,
		);
		if (!media) throw new Error("Media Server 9 was not patched");
		await select(api, show, media.logical_heads.map((head) => head.fixture_id));
		const mediaDialog = await openColorDialog(page, "Media color");
		await expect(mediaDialog.getByRole("slider", { name: "White Blend" })).toBeVisible();
		await expect(
			mediaDialog.getByText("Brightness", { exact: true }),
		).toHaveCount(0);
		await closeDialog(mediaDialog);
		// A selection mixing lamps and layers opens the lamp dialog.
		await select(api, show, [show.ids[1], ...media.logical_heads.map((head) => head.fixture_id)]);
		await closeDialog(await openColorDialog(page));
	});

	test("COLORINTENT-002 @ui › the encoders offer no fixture-native colour control", async ({
		api,
		desk,
		page,
	}) => {
		const show = await createRiggedShow(api, page, desk, "Intent", "intent");
		await select(api, show, RIG.map((rig) => show.ids[rig.number]));
		await desk.open(api.baseUrl);
		await page.getByRole("button", { name: "Color", exact: true }).first().click();
		await expect(
			page.getByRole("group", {
				name: /^Enc \d+ · color (red|green|blue|white|amber|cyan|magenta|yellow|wheel|hue|saturation|tint)/,
			}),
		).toHaveCount(0);
	});

	test("COLORINTENT-003 @ui › the Color dialog names every fixture that does not show the colour exactly", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(120_000);
		const show = await createRiggedShow(api, page, desk, "Intent", "intent");
		const all = RIG.map((rig) => show.ids[rig.number]);
		await select(api, show, all);
		await setIntensity(api, show, [1, 2, 3, 4, 5], 1);
		await programColor(api, show, [1, 2, 3, 4, 5], RED);

		// The report reads the output frame that was sent. Dimmer 5 has no colour engine and
		// nothing programs a colour for it, so it is not listed.
		const report = await acceptedColorReport(api, bench, show.id, all);
		expect(report.color_model).toBe("intent");
		expect(
			Object.fromEntries(
				report.heads.map((head) => [head.fixture_number, head.quality]),
			),
		).toEqual({
			1: "uncalibrated",
			2: "uncalibrated",
			3: "uncalibrated",
			4: "uncalibrated",
		});

		await desk.open(api.baseUrl);
		let dialog = await openColorDialog(page);
		let results = colorResults(dialog);
		for (const number of [1, 2, 3, 4] as const)
			await expect(
				resultRow(results, show.ids[number]).locator('td[data-quality="uncalibrated"]'),
			).toContainText("Uncalibrated");
		await expect(resultRow(results, show.ids[5])).toHaveCount(0);
		await expect(results.locator("tbody tr")).toHaveCount(4);
		await expect(results.locator('td[data-quality="exact"]')).toHaveCount(0);
		await expect(results).not.toContainText("exactly");
		await closeDialog(dialog);

		// UV is its own column: a UV request on RGB 1 is unavailable there, the visible match stays.
		await colorEdits(api, [show.ids[1]], [scalarColorEdit("uv", 1)]);
		await expect
			.poll(async () => {
				const [rgb] = (await acceptedColorReport(api, bench, show.id, [show.ids[1]])).heads;
				return [rgb?.quality, rgb?.uv?.status];
			})
			.toEqual(["uncalibrated", "unsupported"]);
		dialog = await openColorDialog(page);
		await expect(resultRow(colorResults(dialog), show.ids[1])).toContainText(
			"UV unavailable on this fixture",
		);
		await expect(
			resultRow(colorResults(dialog), show.ids[1]).locator('td[data-quality="uncalibrated"]'),
		).toBeVisible();
		await closeDialog(dialog);

		// A fixture whose profile authors a measured RGB colour system shows pure red exactly.
		const measured = await saveMeasuredRgbProfile(api);
		const patch = new BrowserPatch(api, page, desk);
		await patch.via.api.add({
			number: 6,
			name: "Measured 6",
			manufacturer: measured.manufacturer,
			profile: measured.name,
			mode: "DRGB 8-bit dimmer first",
			address: "1.51",
		});
		const measuredId = (await fixtureIds(api))[6];
		await select(api, show, [measuredId]);
		await setIntensityById(api, show, [measuredId], 1);
		await programColorById(api, show, [measuredId], RED);
		expect((await acceptedColorReport(api, bench, show.id, [measuredId])).heads).toEqual([
			expect.objectContaining({ fixture_id: measuredId, quality: "exact" }),
		]);
		expect((await universe(bench)).slice(50, 54)).toEqual([255, 255, 0, 0]);
		dialog = await openColorDialog(page);
		results = colorResults(dialog);
		await expect(
			results.getByText("Every selected fixture shows this colour exactly."),
		).toBeVisible();
		await expect(results.locator('td[data-quality="exact"]')).toHaveCount(1);
		await expect(results.locator('td:not([data-quality="exact"])[data-quality]')).toHaveCount(0);
		await closeDialog(dialog);

		// Saturated spectral cyan (about 490 nm) lies outside that fixture's gamut.
		await colorEdits(api, [measuredId], [
			{ kind: "coordinates", xyz: chromaticity(0.0454, 0.295, 0.5) },
		]);
		await expect
			.poll(async () => (await acceptedColorReport(api, bench, show.id, [measuredId])).heads[0]?.quality)
			.toBe("out_of_gamut");
		const [cyan] = (await acceptedColorReport(api, bench, show.id, [measuredId])).heads;
		expect(cyan.delta_uv).toBeGreaterThan(0.004);
		dialog = await openColorDialog(page);
		await expect(
			resultRow(colorResults(dialog), measuredId).locator('td[data-quality="out_of_gamut"]'),
		).toHaveText(
			`Out of gamut Shows the nearest colour it can · Δu′v′ ${cyan.delta_uv?.toFixed(4)}`,
		);
		await closeDialog(dialog);

		// A wheel-only fixture with measured slots: an orange between two slots is wheel-limited,
		// and the wheel never parks on a non-steady Rainbow position even though it matches.
		const wheel = await saveMeasuredWheelProfile(api);
		await patch.via.api.add({
			number: 7,
			name: "Wheel 7",
			manufacturer: wheel.manufacturer,
			profile: wheel.name,
			mode: wheel.mode,
			address: "1.61",
		});
		const wheelId = (await fixtureIds(api))[7];
		await select(api, show, [wheelId]);
		await setIntensityById(api, show, [wheelId], 1);
		await programColorById(api, show, [wheelId], {
			hue: 30 / 360,
			saturation: 1,
		});
		const [orange] = (await acceptedColorReport(api, bench, show.id, [wheelId])).heads;
		expect(orange).toMatchObject({ fixture_id: wheelId, quality: "wheel_limited" });
		const [, wheelSlot] = (await universe(bench)).slice(60, 62);
		expect(
			wheel.steadySlots.some(
				([from, to]) => wheelSlot >= from && wheelSlot <= to,
			),
		).toBe(true);
		expect(
			wheelSlot < wheel.rainbow[0] || wheelSlot > wheel.rainbow[1],
		).toBe(true);
		dialog = await openColorDialog(page);
		await expect(
			resultRow(colorResults(dialog), wheelId).locator('td[data-quality="wheel_limited"]'),
		).toContainText(/^Wheel-limited Uses the nearest wheel slot/);
		await closeDialog(dialog);
	});

	test("COLORINTENT-004 @ui › a universal Color preset follows the selection; a preset of different colours never spreads", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(120_000);
		const show = await createRiggedShow(api, page, desk, "Intent", "intent");

		// RGB 1 and RGB 2 in the same blue are stored once, as a universal colour.
		await select(api, show, [show.ids[1], show.ids[2]]);
		await setIntensity(api, show, [1, 2], 1);
		await programColor(api, show, [1, 2], BLUE);
		await recordColorPreset(api, show.id, 1);
		const color1 = await presetBody(api, show.id, 1);
		expect(color1.values).toEqual({});
		expect(color1.universal_values).toEqual({
			color: { kind: "color_program", value: expect.any(Object) },
		});

		await desk.open(api.baseUrl);
		await showColorPresets(page);
		await expect(presetCard(page, 1)).toContainText("Universal · 2");

		// RGBW 3 and CMY 4 were never part of Color 1; recalling it from the pool makes them blue.
		await clearProgrammer(api, show);
		await select(api, show, [show.ids[3], show.ids[4]]);
		await setIntensity(api, show, [3, 4], 1);
		const blueRgbw = { dimmer: 255, red: 0, green: 0, blue: 255, white: 0 };
		const blueCmy = { dimmer: 255, cyan: 255, magenta: 255, yellow: 0 };
		expect(await fixtureDmx(bench, 3)).not.toEqual(blueRgbw);
		await presetCard(page, 1).click();
		await expect.poll(() => fixtureDmx(bench, 3)).toEqual(blueRgbw);
		expect(await fixtureDmx(bench, 4)).toEqual(blueCmy);
		const fromPool = await universe(bench);

		// The service recall and the command line produce the same DMX.
		await clearProgrammer(api, show);
		await setIntensity(api, show, [3, 4], 1);
		expect(await fixtureDmx(bench, 3)).not.toEqual(blueRgbw);
		await recall(api, show.id, 1);
		expect(await universe(bench)).toEqual(fromPool);
		await clearProgrammer(api, show);
		await setIntensity(api, show, [3, 4], 1);
		// `AT 2.1` is the command-line text the keypad builds for AT COLOR PRESET 1.
		expect(await api.executeCommandLineRaw("AT 2.1")).toMatchObject({
			outcome: "accepted",
			applied: 2,
		});
		expect(await universe(bench)).toEqual(fromPool);

		// RGB 1 red and RGB 2 green keep each fixture's own colour.
		await clearProgrammer(api, show);
		await select(api, show, [show.ids[1], show.ids[2]]);
		await programColor(api, show, [1], RED);
		await programColor(api, show, [2], GREEN);
		await recordColorPreset(api, show.id, 2);
		const color2 = await presetBody(api, show.id, 2);
		expect(color2.universal_values ?? {}).toEqual({});
		expect(Object.keys(color2.values).sort()).toEqual(
			[show.ids[1], show.ids[2]].sort(),
		);

		await clearProgrammer(api, show);
		await select(api, show, [show.ids[1], show.ids[2], show.ids[3]]);
		await setIntensity(api, show, [1, 2, 3], 1);
		const rgbwBefore = await fixtureDmx(bench, 3);
		await recall(api, show.id, 2);
		expect(await fixtureDmx(bench, 1)).toEqual({ red: 255, green: 0, blue: 0 });
		expect(await fixtureDmx(bench, 2)).toEqual({
			dimmer: 255,
			red: 0,
			green: 255,
			blue: 0,
		});
		expect(await fixtureDmx(bench, 3)).toEqual(rgbwBefore);
		expect(
			attributesOf(await programmerValues(api), show.ids[3]),
		).not.toContain("color");

		// With nothing selected, a universal preset leaves the desk unchanged and silent.
		await clearProgrammer(api, show);
		await select(api, show, []);
		const empty = await recall(api, show.id, 1);
		expect(empty.appliedFixtures).toBe(0);
		expect(empty.warning).toBeNull();
		expect((await programmer(api)).selected).toEqual([]);
		await presetCard(page, 1).click();
		await expect(page.getByLabel("Desk notice")).toHaveCount(0);
		await expect(page.getByText("Desk needs attention")).toHaveCount(0);
		expect((await programmer(api)).selected).toEqual([]);
		expect(await programmerValues(api)).toEqual([]);
	});

	test("COLORINTENT-004 @api › FixAT COLOR PRESET 1 produces the same DMX as the pool recall", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const show = await createRiggedShow(api, page, desk, "Intent", "intent");
		await select(api, show, [show.ids[1], show.ids[2]]);
		await programColor(api, show, [1, 2], BLUE);
		await recordColorPreset(api, show.id, 1);
		await clearProgrammer(api, show);
		await select(api, show, [show.ids[3], show.ids[4]]);
		await setIntensity(api, show, [3, 4], 1);
		await recall(api, show.id, 1);
		const recalled = await universe(bench);
		await clearProgrammer(api, show);
		await setIntensity(api, show, [3, 4], 1);
		expect(
			await api.executeCommandLineRaw("FIXAT COLOR PRESET 1"),
		).toMatchObject({ outcome: "accepted" });
		expect(await universe(bench)).toEqual(recalled);
	});

	test("COLORINTENT-005 @ui › switching a programmed show lists what changes and rewrites nothing", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		test.setTimeout(120_000);
		const show = await createRiggedShow(api, page, desk, "Direct");
		// A fixture-native colour value that a contract-1 show can hold: a colour-wheel slot on
		// a wheel fixture. (A single colour-channel percentage such as White is legacy
		// programming since programming contract 1 and is refused before it is stored.)
		const wheel = await saveMeasuredWheelProfile(api);
		await new BrowserPatch(api, page, desk).via.api.add({
			number: 7,
			name: "Wheel 7",
			manufacturer: wheel.manufacturer,
			profile: wheel.name,
			mode: wheel.mode,
			address: "1.61",
		});
		const wheelId = (await fixtureIds(api))[7];
		// A Direct Color preset holding a half-level red whole colour and a colour-wheel value.
		await api.seedShowObject(show.id, "preset", "2.1", {
			family: "Color",
			number: 1,
			name: "Color 1",
			group_values: {},
			values: {
				[show.ids[3]]: { color: { kind: "color_xyz", value: HALF_RED } },
				[wheelId]: { "color.wheel.1": { kind: "discrete", value: "deep_green" } },
			},
		});
		const stored = await api.showObject(show.id, "preset", "2.1");
		if (!stored) throw new Error("Color 1 was not stored");
		const wheelSlot = async () => (await universe(bench))[61];
		await select(api, show, [show.ids[3], wheelId]);
		await setIntensityById(api, show, [show.ids[3], wheelId], 1);
		await recall(api, show.id, 1);
		// Direct plays the stored wheel slot.
		const slot = await wheelSlot();
		expect(wheel.steadySlots.some(([from, to]) => slot >= from && slot <= to)).toBe(true);
		await clearProgrammer(api, show);

		await desk.open(api.baseUrl);
		await openSetup(page, "Attributes & encoders", "Color model");
		await expect(showModelField(page)).toContainText("Direct");
		await chooseOption(page, showModelField(page), "Color Intent");
		let impact = page.getByRole("alertdialog", {
			name: "Switch to Color Intent",
		});
		await expect(impact).toBeVisible();
		await expect(impact.locator("li")).toHaveCount(2);
		await expect(impact).toContainText(
			"1 stored fixture-native colour value(s) are kept and still play",
		);
		await expect(impact).toContainText(
			"1 stored colour(s) carry their own brightness. Color Intent shows them at full brightness",
		);
		await impact.getByRole("button", { name: "Keep Direct", exact: true }).click();
		await expect(impact).toBeHidden();
		expect(await colorModel(api, show.id)).toBe("direct");
		await expect(showModelField(page)).toContainText("Direct");

		await chooseOption(page, showModelField(page), "Color Intent");
		impact = page.getByRole("alertdialog", { name: "Switch to Color Intent" });
		await impact
			.getByRole("button", { name: "Switch to Color Intent", exact: true })
			.click();
		await expect(impact).toBeHidden();
		await expect.poll(() => colorModel(api, show.id)).toBe("intent");
		await expect(showModelField(page)).toContainText("Color Intent");
		expect(await api.showObject(show.id, "preset", "2.1")).toMatchObject({
			revision: stored.revision,
			body: stored.body,
		});

		// Color Intent: the half-level red plays at full brightness and the wheel value still plays.
		await select(api, show, [show.ids[3], wheelId]);
		await setIntensityById(api, show, [show.ids[3], wheelId], 1);
		await recall(api, show.id, 1);
		expect(await fixtureDmx(bench, 3)).toMatchObject({
			dimmer: 255,
			red: 255,
			green: 0,
			blue: 0,
		});
		expect(await wheelSlot()).toBe(slot);
		await clearProgrammer(api, show);

		// Back to Direct: the whole colour sits on an uncalibrated fixture and would be lost.
		await chooseOption(page, showModelField(page), "Direct");
		impact = page.getByRole("alertdialog", { name: "Switch to Direct" });
		await expect(impact).toBeVisible();
		await expect(impact).toContainText(
			"Some stored colour will not come back unchanged if you switch back.",
		);
		const loss = impact.locator('li[data-lossy="true"]');
		await expect(loss).toContainText("fixtures without an authored colour system");
		await expect(loss).toContainText("those fixtures lose that colour");
		expect(await colorModel(api, show.id)).toBe("intent");
		await impact
			.getByRole("button", { name: "Switch to Direct", exact: true })
			.click();
		await expect(impact).toBeHidden();
		await expect.poll(() => colorModel(api, show.id)).toBe("direct");
		expect(await api.showObject(show.id, "preset", "2.1")).toMatchObject({
			revision: stored.revision,
			body: stored.body,
		});
	});
});

// ---------------------------------------------------------------------------------------------
// Show and rig

async function createRiggedShow(
	api: ApiDriver,
	page: Page,
	desk: DeskDriver,
	label: string,
	model?: ColorModel,
): Promise<ColorShow> {
	await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
	const show = await api.createShow<{ id: string }>({
		name: `COLORINTENT ${label} ${crypto.randomUUID()}`,
	});
	await openShow(api, show.id);
	if (model && (await colorModel(api, show.id)) !== model)
		await switchColorModel(api, show.id, model);
	const patch = new BrowserPatch(api, page, desk);
	for (const rig of RIG)
		await patch.via.api.add({
			number: rig.number,
			name: rig.name,
			manufacturer: "Generic",
			profile: rig.profile,
			mode: rig.mode,
			address: `1.${rig.address}`,
		});
	return { id: show.id, ids: await fixtureIds(api) };
}

async function openShow(api: ApiDriver, showId: string) {
	await api.openShow(showId, { transition: "hold_current" });
	await expect.poll(() => activeShowId(api)).toBe(showId);
}

async function fixtureIds(api: ApiDriver): Promise<Record<number, string>> {
	return Object.fromEntries(
		(await api.patch()).fixtures.map((fixture) => [
			fixture.fixture_number,
			fixture.fixture_id,
		]),
	);
}

async function activeShowId(api: ApiDriver): Promise<string | null> {
	const bootstrap = await api.request<{ active_show: { id: string } | null }>(
		"GET",
		"/api/v2/bootstrap",
		undefined,
		false,
	);
	return bootstrap.active_show?.id ?? null;
}

async function deskConfiguration(api: ApiDriver) {
	return (
		await api.request<{
			configuration: {
				color_programming_model_default: ColorModel;
				color_presentation?: string;
			};
		}>("GET", "/api/v2/configuration")
	).configuration;
}

interface AttributeConfigurationSnapshot {
	show_revision: number;
	object_revision: number;
	configuration: { color_model?: ColorModel | null };
}

function attributeConfiguration(api: ApiDriver, showId: string) {
	return api.request<AttributeConfigurationSnapshot>(
		"GET",
		"/api/v2/attribute-configuration",
		undefined,
		true,
		undefined,
		{ showId },
	);
}

async function colorModel(api: ApiDriver, showId: string): Promise<ColorModel> {
	return (
		(await attributeConfiguration(api, showId)).configuration.color_model ??
		"direct"
	);
}

/** What the show file itself stores: absent for every Direct show. */
async function storedColorModel(
	api: ApiDriver,
	showId: string,
): Promise<string | undefined> {
	const stored = await api.showObject<{ color_model?: string }>(
		showId,
		"attribute_configuration",
		"default",
	);
	return stored?.body.color_model;
}

async function switchColorModel(
	api: ApiDriver,
	showId: string,
	model: ColorModel,
) {
	const snapshot = await attributeConfiguration(api, showId);
	await api.request(
		"POST",
		"/api/v2/attribute-configuration/update",
		{
			request_id: crypto.randomUUID(),
			expected_show_revision: snapshot.show_revision,
			expected_object_revision: snapshot.object_revision,
			patch: { color_model: model },
		},
		true,
		undefined,
		{ showId },
	);
}

// ---------------------------------------------------------------------------------------------
// Programmer

function normalized(value: number): AttributeValue {
	return { kind: "normalized", value } as AttributeValue;
}

async function select(
	api: ApiDriver,
	show: ColorShow,
	fixtureIds: readonly string[],
) {
	await replaceProgrammingSelection(api, {
		surface: "api",
		showId: show.id,
		fixtures: fixtureIds,
	});
}

async function mutate(
	api: ApiDriver,
	show: ColorShow,
	mutations: ProgrammerValuesMutation[],
) {
	await batchProgrammerValues(api, {
		surface: "api",
		showId: show.id,
		mutations,
	});
}

function setIntensity(
	api: ApiDriver,
	show: ColorShow,
	numbers: number[],
	level: number,
) {
	return setIntensityById(
		api,
		show,
		numbers.map((number) => show.ids[number]),
		level,
	);
}

function setIntensityById(
	api: ApiDriver,
	show: ColorShow,
	fixtureIds: string[],
	level: number,
) {
	return mutate(api, show, [
		{
			action: "set_selection",
			fixtureIds,
			attribute: "intensity",
			value: normalized(level),
			timing: NOW,
		},
	]);
}

/** The Color dialog's own write: one hue and saturation for the given fixtures. */
function programColor(
	api: ApiDriver,
	show: ColorShow,
	numbers: number[],
	color: { hue: number; saturation: number },
) {
	return programColorById(
		api,
		show,
		numbers.map((number) => show.ids[number]),
		color,
	);
}

/** A complete semantic white: the start a fresh pick edits, exactly as the dialog's picker. */
const SEMANTIC_WHITE = {
	kind: "color_program",
	value: {
		kind: "semantic",
		intent: {
			base_xyz: { x: 0.95047, y: 1, z: 1.08883 },
			recipe: { version: 1, rgb: [1, 1, 1], amber: 0, approximate: false },
			white_blend: 0,
			white_target: { kelvin: 6500, duv: 0 },
			uv: { amount: 0 },
			relative_output: 1,
			allocation: "preserve_recipe",
		},
	},
};

/**
 * The semantic Color dialog's write (programming contract 1): the whole colour of `color`,
 * edited by its Hue and Saturation components, from a white start for every fixture.
 */
async function programColorById(
	api: ApiDriver,
	_show: ColorShow,
	fixtureIds: string[],
	color: { hue: number; saturation: number },
) {
	await valuesAction(api, {
		type: "batch",
		mutations: fixtureIds.map((fixture_id) => ({
			type: "set_fixture",
			fixture_id,
			attribute: "color",
			value: SEMANTIC_WHITE,
			timing: { fade: false },
		})),
	});
	await colorEdits(api, fixtureIds, [
		scalarColorEdit("hue", color.hue * 360),
		scalarColorEdit("saturation", color.saturation),
	]);
}

function scalarColorEdit(component: string, value: number) {
	return {
		kind: "scalar",
		component: { kind: "color", component },
		operation: { kind: "set", value: { kind: "value", value } },
	};
}

/** One atomic `component_edits` action on the whole colour of the given fixtures. */
async function colorEdits(api: ApiDriver, fixtureIds: string[], edits: unknown[]) {
	const outcome = await valuesAction(api, {
		type: "apply_intent",
		fixture_ids: fixtureIds,
		attribute: "color",
		operation: { type: "component_edits", edits },
		timing: { fade: false },
	});
	expect(outcome.status, JSON.stringify(outcome)).toBe("changed");
}

/** CIE 1931 xy chromaticity at the given luminance. */
function chromaticity(x: number, y: number, luminance: number): Xyz {
	return {
		x: (x * luminance) / y,
		y: luminance,
		z: ((1 - x - y) * luminance) / y,
	};
}

async function clearProgrammer(api: ApiDriver, show: ColorShow) {
	await clearProgrammerValues(api, { surface: "api", showId: show.id });
}

interface ProgrammerValue {
	fixture_id: string;
	attribute: string;
	value: { kind: string; value: unknown };
}

async function programmerValues(api: ApiDriver): Promise<ProgrammerValue[]> {
	const snapshot = await api.request<{
		projection: { fixture_values: ProgrammerValue[] };
	}>("GET", "/api/v2/programmer/values/snapshot");
	return snapshot.projection.fixture_values;
}

function attributesOf(values: ProgrammerValue[], fixtureId: string) {
	return values
		.filter((value) => value.fixture_id === fixtureId)
		.map((value) => value.attribute);
}

async function programmer(api: ApiDriver) {
	const programmers = await api.request<
		Array<{ session_id?: string; selected: string[] }>
	>("GET", "/api/v2/programmers");
	const current =
		programmers.find(
			(entry) => entry.session_id === api.session?.session_id,
		) ?? programmers[0];
	if (!current) throw new Error("No programmer");
	return current;
}

// ---------------------------------------------------------------------------------------------
// Presets

async function recordColorPreset(
	api: ApiDriver,
	showId: string,
	number: number,
) {
	const before = await api.showObject(showId, "preset", `2.${number}`);
	const session = api.session;
	if (!session) throw new Error("API session is not initialized");
	await new HttpPresetRecordingTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
	}).record(showId, {
		requestId: crypto.randomUUID(),
		address: { family: "Color", number },
		name: `Color ${number}`,
		mode: "overwrite",
		expectedObjectRevision: before?.revision ?? 0,
	});
}

interface StoredColorPreset {
	values: Record<string, Record<string, unknown>>;
	universal_values?: Record<string, unknown>;
}

async function presetBody(api: ApiDriver, showId: string, number: number) {
	const preset = await api.showObject<StoredColorPreset>(
		showId,
		"preset",
		`2.${number}`,
	);
	if (!preset) throw new Error(`Color ${number} is not stored`);
	return preset.body;
}

function recall(api: ApiDriver, showId: string, number: number) {
	return recallPreset(api, {
		surface: "api",
		showId,
		preset: { objectId: `2.${number}`, family: "Color", number },
	});
}

// ---------------------------------------------------------------------------------------------
// DMX and resolution reports

async function universe(bench: LightBench): Promise<number[]> {
	const frame = await bench.tick(0);
	return frame.universes.find((entry) => entry.universe === 1)?.slots ?? [];
}

async function fixtureDmx(
	bench: LightBench,
	number: RigNumber,
): Promise<Record<string, number>> {
	const rig = RIG.find((candidate) => candidate.number === number);
	if (!rig) throw new Error(`Fixture ${number} is not part of the rig`);
	const slots = await universe(bench);
	return Object.fromEntries(
		LAYOUT[number].map((channel, offset) => [
			channel,
			slots[rig.address - 1 + offset],
		]),
	);
}

interface ColorIntentReport {
	color_model: ColorModel;
	heads: Array<{
		fixture_id: string;
		fixture_number: number | null;
		quality: string;
		engine: string | null;
		delta_uv: number | null;
		calibration_revision: number | null;
		uv?: { status: string; clipped: boolean } | null;
	}>;
	accepted_frame?: { state: string } | null;
}

/** The report of the output frame that was sent: tick once and read until it is accepted. */
async function acceptedColorReport(
	api: ApiDriver,
	bench: LightBench,
	showId: string,
	fixtureIds: string[],
): Promise<ColorIntentReport> {
	let report: ColorIntentReport | undefined;
	await expect
		.poll(async () => {
			await bench.tick(0);
			report = await colorReport(api, showId, fixtureIds);
			return report.accepted_frame?.state;
		})
		.toBe("accepted");
	if (!report) throw new Error("no accepted Color report");
	return report;
}

function colorReport(api: ApiDriver, showId: string, fixtureIds: string[]) {
	return api.request<ColorIntentReport>(
		"GET",
		`/api/v2/color-intent/report?fixtures=${fixtureIds.map(encodeURIComponent).join(",")}`,
		undefined,
		true,
		undefined,
		{ showId },
	);
}

// ---------------------------------------------------------------------------------------------
// Calibrated test profiles: no shipped profile carries measured colour data, so COLORINTENT-003
// saves two into the library the way the fixture editor does.

interface LibraryMode {
	id: string;
	name: string;
	heads: Array<{ id: string }>;
	splits: Array<{ number: number; footprint: number }>;
	channels: Array<{ id: string; attribute: string; head_id: string; split: number }>;
	// biome-ignore lint/suspicious/noExplicitAny: the colour-system schema is edited as JSON.
	color_systems?: Array<Record<string, any>>;
}

interface LibraryProfile {
	id: string;
	revision: number;
	manufacturer: string;
	name: string;
	modes: LibraryMode[];
}

async function libraryProfile(
	api: ApiDriver,
	manufacturer: string,
	name: string,
) {
	const library = await api.request<{ profiles: LibraryProfile[] }>(
		"GET",
		"/api/v2/fixture-library/profiles",
	);
	const profile = library.profiles.find(
		(candidate) =>
			candidate.manufacturer === manufacturer && candidate.name === name,
	);
	if (!profile)
		throw new Error(`The fixture library has no ${manufacturer} ${name}`);
	return structuredClone(profile);
}

function srgbXyz(red: number, green: number, blue: number): Xyz {
	return {
		x: 0.4124564 * red + 0.3575761 * green + 0.1804375 * blue,
		y: 0.2126729 * red + 0.7151522 * green + 0.072175 * blue,
		z: 0.0193339 * red + 0.119192 * green + 0.9503041 * blue,
	};
}

const MEASURED = {
	status: "measured",
	revision: 1,
	source: "COLORINTENT-003 colorimeter",
};
const IDENTITY = [
	[1, 0, 0],
	[0, 1, 0],
	[0, 0, 1],
];

/** Generic RGB LED with a measured sRGB-primary additive colour system on every mode. */
async function saveMeasuredRgbProfile(api: ApiDriver) {
	const profile = await libraryProfile(api, "Generic", "RGB LED");
	profile.id = crypto.randomUUID();
	profile.revision = 0;
	profile.manufacturer = "COLORINTENT";
	profile.name = "Measured RGB";
	for (const mode of profile.modes) {
		const emitter = (attribute: string, name: string, xyz: Xyz) => {
			const channel = mode.channels.find(
				(candidate) => candidate.attribute === attribute,
			);
			if (!channel) throw new Error(`${mode.name} has no ${attribute}`);
			return {
				channel_id: channel.id,
				name,
				xyz,
				maximum_level: 1,
				response_curve: 1,
				visible: true,
			};
		};
		mode.color_systems = [
			{
				head_id: mode.heads[0].id,
				correction_matrix: IDENTITY,
				calibration: MEASURED,
				system: {
					type: "additive",
					emitters: [
						emitter("color.red", "Red", srgbXyz(1, 0, 0)),
						emitter("color.green", "Green", srgbXyz(0, 1, 0)),
						emitter("color.blue", "Blue", srgbXyz(0, 0, 1)),
					],
				},
			},
		];
	}
	await api.fixtureLibraryAction({
		type: "save_profile",
		profile,
		expected_revision: 0,
	});
	return profile;
}

/**
 * A wheel-only fixture: Generic Dimmer plus the shipped Cameo AURO SPOT Z300 colour wheel, with
 * measured slots and an extra non-steady Rainbow position whose colour matches orange exactly.
 */
async function saveMeasuredWheelProfile(api: ApiDriver) {
	const auro = await libraryProfile(api, "Cameo", "AURO SPOT Z300");
	const auroMode = auro.modes.find((mode) => mode.name === "17-Channel");
	const auroWheel = auroMode?.channels.find(
		(channel) => channel.attribute === "color.wheel.1",
	);
	const auroSystem = auroMode?.color_systems?.[0];
	if (!auroWheel || !auroSystem)
		throw new Error("AURO SPOT Z300 has no colour wheel");
	const slotColor: Record<string, [number, number, number]> = {
		open: [1, 1, 1],
		deep_red: [1, 0, 0],
		medium_blue: [0, 0, 1],
		deep_green: [0, 1, 0],
		yellow: [1, 1, 0],
		lavender: [0.7, 0.5, 1],
		amber_deep_orange: [1, 0.2, 0],
		cto_3200k: [1, 0.75, 0.5],
		congo_blue: [0.2, 0, 1],
	};
	const rainbow: [number, number] = [54, 60];
	const profile = await libraryProfile(api, "Generic", "Dimmer");
	profile.id = crypto.randomUUID();
	profile.revision = 0;
	profile.manufacturer = "COLORINTENT";
	profile.name = "Measured Wheel Spot";
	const steadySlots: Array<[number, number]> = [];
	profile.modes = profile.modes
		.filter((mode) => mode.name === "8-bit")
		.map((mode) => {
			const wheel = {
				...structuredClone(auroWheel),
				id: crypto.randomUUID(),
				head_id: mode.heads[0].id,
				split: 1,
			};
			const system = structuredClone(auroSystem);
			system.head_id = mode.heads[0].id;
			system.system.channel_id = wheel.id;
			system.calibration = MEASURED;
			for (const slot of system.system.slots) {
				const color = slotColor[slot.semantic_id];
				if (!color)
					throw new Error(`No measurement for wheel slot ${slot.semantic_id}`);
				slot.measured_xyz = srgbXyz(...color);
				steadySlots.push([slot.dmx_from, slot.dmx_to]);
			}
			system.system.slots.push({
				semantic_id: "rainbow",
				label: "Rainbow",
				dmx_from: rainbow[0],
				dmx_to: rainbow[1],
				measured_xyz: srgbXyz(1, 0.5, 0),
				steady: false,
			});
			return {
				...mode,
				name: "Dimmer + wheel",
				splits: [{ number: 1, footprint: 2 }],
				channels: [...mode.channels, wheel],
				color_systems: [system],
			};
		});
	await api.fixtureLibraryAction({
		type: "save_profile",
		profile,
		expected_revision: 0,
	});
	return {
		manufacturer: profile.manufacturer,
		name: profile.name,
		mode: "Dimmer + wheel",
		steadySlots,
		rainbow,
	};
}

// ---------------------------------------------------------------------------------------------
// Operator UI

async function openSetup(page: Page, section: string, tab: string) {
	const setup = page.locator(".setup-window");
	if (!(await setup.isVisible())) {
		await page.getByRole("button", { name: /Open show menu/ }).click();
		await page
			.locator(".show-modal")
			.getByRole("button", { name: "Enter Setup", exact: true })
			.click();
	}
	await setup
		.locator("nav")
		.getByRole("button", { name: section, exact: true })
		.click();
	await page.getByRole("tab", { name: tab, exact: true }).click();
}

function defaultModelField(page: Page) {
	return page.getByRole("button", {
		name: "Default color programming model for new shows",
	});
}

function showModelField(page: Page) {
	return page.getByRole("button", { name: "Show color programming model" });
}

async function chooseOption(
	page: Page,
	field: Locator,
	label: "Direct" | "Color Intent",
) {
	await field.click();
	await page.getByRole("option", { name: new RegExp(`^${label} — `) }).click();
}

/**
 * Opens the semantic Color dialog as the full modal (its approximation lives there): the Color
 * tab, then **Special Dialog**, then **Expand** when it opened compact in the encoder area. The
 * lamp and the Media variant share the dialog's accessible name; the full modal's visible title
 * (**Color** or **Media color**) tells them apart.
 */
async function openColorDialog(page: Page, title = "Color"): Promise<Locator> {
	await page.getByRole("button", { name: "Color", exact: true }).first().click();
	await page
		.getByRole("button", { name: "Special Dialog", exact: true })
		.click();
	const opened = page.getByRole("dialog", { name: "Color Special Dialog" });
	await expect(opened).toBeVisible();
	const expand = opened.getByRole("button", { name: "Expand", exact: true });
	if (await expand.count()) await expand.click();
	const dialog = page.locator(".ui-modal-stack-layer").filter({
		has: page.getByRole("dialog", { name: "Color Special Dialog" }),
	});
	await expect(dialog).toBeVisible();
	await expect(
		dialog.getByRole("heading", { level: 2, name: title, exact: true }),
	).toBeVisible();
	return dialog;
}

/** The per-fixture approximation of the full Color modal (accepted-frame report rows). */
function colorResults(dialog: Locator) {
	return dialog.getByTestId("color-approximation");
}

function resultRow(results: Locator, fixtureId: string) {
	return results.locator(`tbody tr[data-fixture-id="${fixtureId}"]`);
}

async function closeDialog(dialog: Locator) {
	const close = dialog.getByRole("button", { name: "Close modal", exact: true });
	if (await close.count()) await close.click();
	else await dialog.page().keyboard.press("Escape");
	await expect(dialog).toBeHidden();
}

/** Pure red on the full modal's own controls: Hue 0° (Home) and Saturation 100% (End). */
async function pickPureRed(page: Page, dialog: Locator) {
	await dialog.getByRole("slider", { name: "Hue" }).focus();
	await page.keyboard.press("Home");
	await dialog.getByRole("slider", { name: "Saturation" }).focus();
	await page.keyboard.press("End");
}

/** Shows the Color family in the Preset pool, as BrowserPresets does. */
async function showColorPresets(page: Page) {
	const pane = page.locator('[data-pane-type="presets"]:visible');
	const direct = pane.getByRole("button", { name: "Color", exact: true });
	if (await direct.count()) {
		await direct.click();
		return;
	}
	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	const settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings.getByRole("tab", { name: "Pool", exact: true }).click();
	await settings.getByRole("button", { name: "Color", exact: true }).click();
	await settings.getByRole("button", { name: "Close settings" }).click();
}

function presetCard(page: Page, number: number) {
	return page
		.locator('[data-pane-type="presets"] .preset-card:visible')
		.nth(number - 1);
}

/**
 * docs/testing/36-semantic-color-controls.md (TL-550): the production semantic Color dialog.
 *
 * Production reports programming contract 1 since TL-552, so these cases run under
 * `npm run test:e2e` too; the gate below only skips on an older (contract 0) runtime.
 */
const SEMANTIC_GATE =
	"semantic programming contract is not enabled on this runtime (production contract 0; run npm run test:e2e-semantic)";

async function semanticColorPages(api: ApiDriver, fixtureIds: readonly string[]) {
	return api
		.request<{ semantic: boolean; families: Array<{ family: string; pages: unknown[] }> }>(
			"GET",
			`/api/v2/programming/family-encoder-pages?fixture_ids=${fixtureIds.join(",")}`,
		)
		.catch(() => null);
}

async function programmerValuesRevision(api: ApiDriver) {
	const snapshot = await api.request<{ projection: { revision: number } }>(
		"GET",
		"/api/v2/programmer/values/snapshot",
	);
	return snapshot.projection.revision;
}

test.describe("docs/testing/36-semantic-color-controls.md", () => {
	test("SEMANTIC-COLOR-001 @ui › compact in the measured encoder area, the active Color tab returns to the encoders", async ({
		api,
		desk,
		page,
	}) => {
		const show = await createRiggedShow(api, page, desk, "Semantic", "intent");
		const rgb = [show.ids[1], show.ids[2], show.ids[3]];
		const pages = await semanticColorPages(api, rgb);
		requireSemanticContract(Boolean(pages?.semantic), SEMANTIC_GATE);
		await select(api, show, rgb);
		await desk.open(api.baseUrl);
		const revision = await programmerValuesRevision(api);
		await page.getByRole("button", { name: "Color", exact: true }).first().click();
		const area = page.locator(".parameter-surfaces").first();
		const box = await area.boundingBox();
		if (!box) throw new Error("the lower encoder area has no box");
		await page.getByRole("button", { name: "Special Dialog", exact: true }).click();
		const dialog = page.getByRole("dialog", { name: "Color Special Dialog" });
		await expect(dialog).toBeVisible();
		if (box.width >= 680 && box.height >= 210) {
			await expect(area.getByRole("dialog", { name: "Color Special Dialog" })).toBeVisible();
			await expect(dialog.getByRole("slider", { name: "White Blend" })).toBeVisible();
			await expect(dialog.getByRole("button")).toHaveText(["White balance", "Expand"]);
			await page.getByRole("button", { name: "Color", exact: true }).first().click();
			await expect(dialog).toBeHidden();
			await expect(area.getByRole("group").first()).toBeVisible();
		} else {
			await expect(dialog.locator("xpath=ancestor::*[contains(@class,'ui-modal-stack-layer')]")).toHaveCount(1);
			await expect(dialog.getByRole("slider", { name: "Hue" })).toBeVisible();
		}
		expect(await programmerValuesRevision(api)).toBe(revision);
	});

	test("SEMANTIC-COLOR-004 @api › Easy and Advanced belong to the desk and leave programmed colour alone", async ({
		api,
		desk,
		page,
	}) => {
		const show = await createRiggedShow(api, page, desk, "Presentation", "intent");
		const rgb = [show.ids[1], show.ids[2]];
		requireSemanticContract(Boolean((await semanticColorPages(api, rgb))?.semantic), SEMANTIC_GATE);
		await select(api, show, rgb);
		await programColor(api, show, [1, 2], RED);
		const before = await programmerValues(api);
		const revision = await programmerValuesRevision(api);
		const colorPages = async () =>
			(await semanticColorPages(api, rgb))?.families.find((group) => group.family === "color")
				?.pages.length;
		for (const [presentation, count] of [["easy_rgbw", 1], ["advanced", 2], ["easy_rgbw", 1]] as const) {
			await api.request("POST", "/api/v2/configuration/update", {
				request_id: crypto.randomUUID(),
				patch: { color_presentation: presentation },
			});
			expect((await deskConfiguration(api)).color_presentation).toBe(presentation);
			await expect.poll(colorPages).toBe(count);
		}
		expect(await programmerValues(api)).toEqual(before);
		expect(await programmerValuesRevision(api)).toBe(revision);
	});
});

/**
 * docs/testing/37-direct-color-pages.md (TL-554): Direct (native) Color pages 3/4 on the E2E
 * semantic server. Reading pages, choosing a reference head and reading reports are inert; the
 * first native edit seeds the reference head's premaster output once and atomically selects
 * Direct for the whole mixed selection; the report names exact replay versus best-effort match.
 */
interface NativePages {
	semantic: boolean;
	reference?: { fixture_id: string; head_id: string; chosen: boolean } | null;
	candidates: Array<{ fixture_id: string }>;
	pages: Array<{
		number: number;
		controls: Array<{
			channel_id: string;
			raw_max: number;
			functions: Array<{ function_id: string; raw_to: number; continuous: boolean }>;
		} | null>;
	}>;
	values?: { controls: Array<{ channel_id: string; raw: number }> } | null;
	fixtures: Array<{ fixture_id: string; replay: string }>;
}

const SEMANTIC_RED = {
	kind: "color_program",
	value: {
		kind: "semantic",
		intent: {
			base_xyz: { x: 0.4124564, y: 0.2126729, z: 0.0193339 },
			recipe: { version: 1, rgb: [1, 0, 0], amber: 0, approximate: false },
			white_blend: 0,
			white_target: { kelvin: 6500, duv: 0 },
			uv: { amount: 0 },
			relative_output: 1,
			allocation: "preserve_recipe",
		},
	},
};

/** Two verified ROOT PAR 6 heads (A1, A2) and one verified fixture of another type (B). */
const DIRECT_RIG = [
	{ number: 101, name: "Par A1", manufacturer: "Cameo", profile: "ROOT PAR 6", mode: "D7CH — Delay Off, virtual dimmer", address: 101 },
	{ number: 102, name: "Par A2", manufacturer: "Cameo", profile: "ROOT PAR 6", mode: "D7CH — Delay Off, virtual dimmer", address: 111 },
	{ number: 103, name: "Lustr B", manufacturer: "ETC", profile: "Source Four LED Series 2 Lustr", mode: "Direct", address: 121 },
] as const;

async function createDirectShow(api: ApiDriver, page: Page, desk: DeskDriver): Promise<ColorShow> {
	await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
	const show = await api.createShow<{ id: string }>({ name: `DIRECT-COLOR ${crypto.randomUUID()}` });
	await openShow(api, show.id);
	if ((await colorModel(api, show.id)) !== "intent") await switchColorModel(api, show.id, "intent");
	const patch = new BrowserPatch(api, page, desk);
	for (const rig of DIRECT_RIG)
		await patch.via.api.add({
			number: rig.number,
			name: rig.name,
			manufacturer: rig.manufacturer,
			profile: rig.profile,
			mode: rig.mode,
			address: `1.${rig.address}`,
		});
	return { id: show.id, ids: await fixtureIds(api) };
}

async function valuesAction(api: ApiDriver, action: Record<string, unknown>) {
	const capture = await api.request<{ projection: { revision: number } }>(
		"GET",
		"/api/v2/programmer/capture-mode/snapshot",
	);
	return api.request<Record<string, unknown>>("POST", "/api/v2/programmer/values/actions", {
		request_id: crypto.randomUUID(),
		expected_revision: await programmerValuesRevision(api),
		expected_capture_mode_revision: capture.projection.revision,
		action,
	});
}

test.describe("docs/testing/37-direct-color-pages.md", () => {
	test("DIRECT-COLOR-001 @api › native pages are inert; the first native edit seeds the shown premaster output once", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const show = await createDirectShow(api, page, desk);
		const [a1, a2, b] = [show.ids[101], show.ids[102], show.ids[103]];
		const mixed = [a1, a2, b];
		requireSemanticContract(Boolean((await semanticColorPages(api, mixed))?.semantic), SEMANTIC_GATE);
		await select(api, show, mixed);
		for (const fixtureId of mixed)
			await valuesAction(api, {
				type: "set_fixture",
				fixture_id: fixtureId,
				attribute: "color",
				value: SEMANTIC_RED,
				timing: {},
			});
		await bench.tick(25);
		const nativePages = (query = "") =>
			api.request<NativePages>(
				"GET",
				`/api/v2/programming/color/native-pages?fixture_ids=${mixed.join(",")}${query}`,
			);
		const revision = await programmerValuesRevision(api);
		const pages = await nativePages();
		expect(pages.semantic).toBe(true);
		expect(pages.reference?.fixture_id, JSON.stringify(pages)).toBe(a1);
		expect(pages.pages[0]?.number).toBe(3);
		expect(pages.fixtures.map((fixture) => fixture.replay)).toEqual(["exact", "exact", "fallback"]);
		expect((await nativePages(`&reference=${b}`)).reference?.chosen).toBe(true);
		await api.request("GET", `/api/v2/color-intent/report?fixtures=${mixed.join(",")}`);
		expect(await programmerValuesRevision(api), "reading pages is inert").toBe(revision);

		const control = pages.pages[0]?.controls[0];
		const shown = pages.values?.controls.find((value) => value.channel_id === control?.channel_id);
		expect(control && shown, "the reference head's shown premaster value").toBeTruthy();
		if (!control || !shown || !pages.reference) return;
		const edit = (value: number) => ({
			type: "apply_intent",
			fixture_ids: mixed,
			attribute: "color",
			operation: {
				type: "component_edits",
				edits: [
					{
						kind: "native",
						binding: { channel_id: control.channel_id, function_id: control.functions[0].function_id },
						operation: { kind: "relative", value },
					},
				],
			},
			undo_group: "native-e2e",
			timing: {},
			native_reference: { fixture_id: pages.reference?.fixture_id, head_id: pages.reference?.head_id },
		});
		const step = -Math.min(10, shown.raw);
		const first = await valuesAction(api, edit(step));
		expect(first.status, JSON.stringify(first)).toBe("changed");
		await bench.tick(25);
		await valuesAction(api, edit(step));
		await valuesAction(api, { type: "finish_gesture", attribute: "color", undo_group: "native-e2e" });
		const recipes = (await programmerValues(api))
			.filter((value) => value.attribute === "color" && mixed.includes(value.fixture_id))
			.map((value) => (value as unknown as { value: { value: { kind: string; recipe?: { channels: Array<{ channel_id: string; raw: number }> } } } }).value.value);
		expect(recipes).toHaveLength(3);
		for (const program of recipes) {
			expect(program.kind, "the whole selection is Direct").toBe("direct");
			expect(
				program.recipe?.channels.find((channel) => channel.channel_id === control.channel_id)?.raw,
				"seeded once from the shown premaster value, then edited in place",
			).toBe(shown.raw + 2 * step);
		}
		await bench.tick(25);
		const report = await api.request<{
			heads: Array<{ fixture_id: string; direct?: { replay: string } | null }>;
		}>("GET", `/api/v2/color-intent/report?fixtures=${mixed.join(",")}`);
		const replay = (fixtureId: string) =>
			report.heads.find((head) => head.fixture_id === fixtureId)?.direct?.replay;
		expect(replay(a1)).toBe("exact");
		expect(replay(a2)).toBe("exact");
		expect(["fallback", "native_only"]).toContain(replay(b));
	});
});

