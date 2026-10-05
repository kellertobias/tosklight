import type { Locator, Page } from "@playwright/test";
import { replaceProgrammingSelection } from "./bench/command-selection/programmingSelection";
import type { ApiDriver } from "./bench/core/api";
import type { DeskDriver } from "./bench/core/desk";
import { expect, test } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import { requireSemanticContract } from "./bench/core/semanticContract";
import { batchProgrammerValues } from "./bench/programmer/programmerValues";
import {
	enterPreloadCapture,
	seedPreloadValue,
	goPreload,
	leavePreloadCapture,
	liveSlots,
	normalValues,
	preloadValues,
	recordLaneWrites,
} from "./bench/programmer/semanticPreloadLanes";
import { BrowserPatch } from "./bench/show-setup/patchScenario";
import { continuousFunctions, saveProfileVariant } from "./bench/show-setup/profileVariant";

/**
 * docs/testing/34-position-operator-controls.md (TL-549, TL-637): the production Position encoders
 * and the modal Position Special Dialog under the semantic programming contract.
 *
 * GATE: production reports programming contract 1 (TL-552), so
 * `GET /api/v2/programming/family-encoder-pages` publishes the semantic Position pages and every
 * test below runs under `npm run test:e2e` as well as `npm run test:e2e-semantic`. Only an older
 * contract-0 server, which answers `semantic: false`, skips them; in the `e2e-semantic` project a
 * missing semantic publication fails instead of skipping.
 *
 * Every case starts from scratch: a fresh show patched from the shipped library and an empty
 * Programmer. The rig is two Cameo AURO SPOT Z300, whose profile binds Pan/Tilt to a nominal
 * Position physical graph (TL-637), so the displayed output seeds the first Position edit. The
 * unsupported case uses two GLP JDC1, whose Tilt-only head cannot form a Pan/Tilt pair and
 * therefore gets no Position physical data.
 *
 * POSITION-CONTROLS-006 arms Preload capturing Programmer changes through the Preload lifecycle
 * route and leaves it with Blind off, so the pending Preload values stay intact; the lane of every
 * dialog write is read from the request the desk sends (`semanticPreloadLanes.ts`).
 */

const GATE =
	"semantic programming contract is not enabled on this runtime (a contract-0 server; run npm run test:e2e-semantic)";

const SPOT = { manufacturer: "Cameo", profile: "AURO SPOT Z300", mode: "20-Channel", footprint: 20 } as const;
/**
 * A fixture without Position physical data: a user copy of the GLP JDC1 whose Tilt has no
 * continuous function, so no nominal Position model can be derived and it stays Unsupported.
 * Every shipped mover now gets an authored or derived model, so the gap is authored on a copy.
 */
const NO_POSITION_MODEL = {
	manufacturer: "POSITION-CONTROLS",
	profile: "JDC1 without Position model",
	mode: "Easy 11-channel",
	footprint: 11,
} as const;

async function saveNoPositionModelProfile(api: ApiDriver) {
	await saveProfileVariant(
		api,
		{ manufacturer: "GLP", profile: "JDC1" },
		{ manufacturer: NO_POSITION_MODEL.manufacturer, name: NO_POSITION_MODEL.profile },
		(profile) => {
			for (const mode of profile.modes) delete mode.position_physical;
			for (const axis of ["pan", "tilt"])
				for (const fn of continuousFunctions(profile, axis))
					fn.behavior = { type: "fixed", semantic_id: `${axis}.home`, label: `${axis} home`, raw_value: 0 };
		},
	);
}

type Mover = typeof SPOT | typeof NO_POSITION_MODEL;

/** AURO default Pan/Tilt raw 32767 of 65535 across ±270° / ±135°: the displayed start pose. */
const SPOT_HOME_PAN = -270 + (540 * 32767) / 65535;
const SPOT_HOME_TILT = -135 + (270 * 32767) / 65535;

interface Rig {
	showId: string;
	selected: string[];
}

async function arrange(
	{ api, bench, desk, page }: { api: ApiDriver; bench: LightBench; desk: DeskDriver; page: Page },
	label: string,
	mover: Mover = SPOT,
): Promise<Rig> {
	await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
	const show = await api.createShow<{ id: string }>({ name: `POSITION-CONTROLS ${label} ${crypto.randomUUID()}` });
	await api.openShow(show.id, { transition: "hold_current" });
	const patch = new BrowserPatch(api, page, desk);
	const { footprint, ...profile } = mover;
	for (const number of [1, 2])
		await patch.via.api.add({ number, name: `Mover ${number}`, ...profile, address: `1.${(number - 1) * footprint + 1}` });
	const selected = (await api.patch()).fixtures.map((fixture) => fixture.fixture_id);
	await replaceProgrammingSelection(api, { surface: "api", showId: show.id, fixtures: selected });
	// The bench clock is manual: accept one output frame so displayed-source readouts exist.
	await bench.tick(25);
	return { showId: show.id, selected };
}

/** Whether the runtime publishes semantic Position pages; a runtime without the route is not. */
async function semanticPosition(api: ApiDriver, fixtureIds: string[]) {
	const pages = await api
		.request<{
			semantic: boolean;
			families: Array<{ family: string; pages: unknown[] }>;
		}>(
			"GET",
			`/api/v2/programming/family-encoder-pages?fixture_ids=${fixtureIds.join(",")}`,
		)
		.catch(() => null);
	return Boolean(
		pages?.semantic && pages.families.some((group) => group.family === "position"),
	);
}

async function programmerRevision(api: ApiDriver) {
	const snapshot = await api.request<{ projection: { revision: number } }>(
		"GET",
		"/api/v2/programmer/values/snapshot",
	);
	return snapshot.projection.revision;
}

type PositionValue = {
	kind: "position";
	value:
		| { kind: "angles"; pan_degrees: { kind: string; value: number }; tilt_degrees: { kind: string; value: number } }
		| { kind: "target"; reference: unknown; offset_metres: unknown[] };
};

async function positionValues(api: ApiDriver) {
	const snapshot = await api.request<{
		projection: { fixture_values?: Array<{ attribute: string; value: PositionValue }> };
	}>("GET", "/api/v2/programmer/values/snapshot");
	return (snapshot.projection.fixture_values ?? []).filter(
		(entry) => entry.attribute === "position",
	);
}

/** The programmed Angles of every selected fixture, once each one holds Angles. */
async function programmedAngles(api: ApiDriver, count: number) {
	const values = await positionValues(api);
	const angles = values.flatMap((entry) =>
		entry.value.value.kind === "angles"
			? [{ pan: entry.value.value.pan_degrees.value, tilt: entry.value.value.tilt_degrees.value }]
			: [],
	);
	return angles.length === count ? angles : null;
}

async function openPositionDialog(page: Page): Promise<Locator> {
	await positionFamily(page).click();
	await page.getByRole("button", { name: "Special Dialog", exact: true }).click();
	const dialog = page.getByRole("dialog", { name: "Position Special Dialog" });
	await expect(dialog).toBeVisible();
	return dialog;
}

/** The Position family button; a multi-page family names its page, e.g. `Position 1 of 2`. */
function positionFamily(page: Page) {
	return page.getByRole("button", { name: /^Position( \d+ of \d+)?$/ });
}

/** The encoder slot labelled `name` on the software encoders (`Enc N · name`). */
function encoder(page: Page, slot: number, name: string) {
	return page.getByRole("group", { name: `Enc ${slot} · ${name}`, exact: true });
}

/**
 * Whether the visible encoder count packs Point/X/Y/Z onto page 1 after Pan/Tilt (6-encoder
 * layouts fill pages sequentially; 4-encoder layouts put them on page 2).
 */
async function pointOnFirstPage(page: Page) {
	return page.getByText("Point", { exact: true }).first().isVisible();
}

/** Records every Position `component_edits` frame the desk sends. */
function recordPositionEdits(page: Page) {
	const sent: string[] = [];
	page.on("websocket", (socket) =>
		socket.on("framesent", ({ payload }) => {
			const text = String(payload);
			if (text.includes("component_edits") && text.includes('"attribute":"position"')) sent.push(text);
		}),
	);
	return sent;
}

/** A stored Position preset holding the same Angles for every fixture, as the shipped shows do. */
async function seedPositionPreset(api: ApiDriver, showId: string, number: number, name: string, fixtureIds: string[], tilt: number) {
	const position = {
		kind: "position",
		value: { kind: "angles", pan_degrees: { kind: "value", value: 0 }, tilt_degrees: { kind: "value", value: tilt } },
	};
	await api.seedShowObject(showId, "preset", `3.${number}`, {
		name,
		family: "Position",
		number,
		values: Object.fromEntries(fixtureIds.map((id) => [id, { position }])),
		group_values: {},
	});
}

/** The visible Preset pool, switched to its Position family. */
async function showPositionPresets(page: Page) {
	const pane = page.locator('[data-pane-type="presets"]:visible').first();
	const direct = pane.getByRole("button", { name: "Position", exact: true });
	if (await direct.count()) {
		await direct.click();
		return pane;
	}
	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	const settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings.getByRole("tab", { name: "Pool", exact: true }).click();
	await settings.getByRole("button", { name: "Position", exact: true }).click();
	await settings.getByRole("button", { name: "Close settings" }).click();
	return pane;
}

/** Programs every selected fixture to the same Angles through the Normal Programmer. */
async function programAngles(api: ApiDriver, showId: string, fixtureIds: string[], pan: number, tilt: number) {
	await batchProgrammerValues(api, {
		surface: "api",
		showId,
		mutations: fixtureIds.map((fixtureId) => ({
			action: "set_fixture" as const,
			fixtureId,
			attribute: "position",
			value: {
				kind: "position",
				value: { kind: "angles", pan_degrees: { kind: "value", value: pan }, tilt_degrees: { kind: "value", value: tilt } },
			} as never,
			timing: { fade: false, fadeMillis: null, delayMillis: null },
		})),
	});
}

const ANGLES = (pan: number, tilt: number) => ({
	kind: "position",
	value: { kind: "angles", pan_degrees: { kind: "value", value: pan }, tilt_degrees: { kind: "value", value: tilt } },
});

/** Every selected fixture's programmed Pan, once all of them equal `pan` (else `null`). */
async function uniformPan(api: ApiDriver, count: number) {
	const angles = await programmedAngles(api, count);
	const pan = angles?.[0]?.pan;
	return pan !== undefined && angles?.every((entry) => Math.abs(entry.pan - pan) < 1e-6) ? pan : null;
}

/**
 * Drags the Pan circle from `fromDegrees` around the ring by `turnDegrees` (positive is
 * clockwise) in 15° pointer samples, so the shortest-turn unwrapping never sees an ambiguous
 * half turn. The drag starts on the ring at the handle's angle.
 */
async function dragPanCircle(page: Page, dialog: Locator, fromDegrees: number, turnDegrees: number) {
	const box = await dialog.getByTestId("pan-circle").boundingBox();
	if (!box) throw new Error("Pan circle has no box");
	const radius = (81 / 220) * box.width;
	const at = (degrees: number) => ({
		x: box.x + box.width / 2 + Math.sin((degrees * Math.PI) / 180) * radius,
		y: box.y + box.height / 2 - Math.cos((degrees * Math.PI) / 180) * radius,
	});
	const start = at(fromDegrees);
	await page.mouse.move(start.x, start.y);
	await page.mouse.down();
	const steps = Math.round(Math.abs(turnDegrees) / 15);
	for (let step = 1; step <= steps; step += 1) {
		const point = at(fromDegrees + Math.sign(turnDegrees) * step * 15);
		await page.mouse.move(point.x, point.y);
	}
	await page.mouse.up();
}

/** Pans the selection at full joystick deflection for a moment, then releases. */
async function holdJoystickRight(page: Page, dialog: Locator, holdMillis: number) {
	const box = await dialog.getByTestId("position-joystick").boundingBox();
	if (!box) throw new Error("joystick has no box");
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width - 4, box.y + box.height / 2);
	await page.waitForTimeout(holdMillis);
	return {
		release: async () => {
			await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
			await page.mouse.up();
		},
	};
}

/**
 * POSITION-CONTROLS-006 step 2: Angles 0°/0° in both Programmers, Preload capturing, then a held
 * joystick gesture that has started authoring into Preload when Preload capture is left.
 */
async function holdInPreloadThenLeave(
	{ api, bench, desk, page }: { api: ApiDriver; bench: LightBench; desk: DeskDriver; page: Page },
	showId: string,
	selected: string[],
) {
	await programAngles(api, showId, selected, 0, 0);
	await enterPreloadCapture(api, showId);
	await seedPreloadValue(api, showId, selected, "position", ANGLES(0, 0));
	await bench.tick(25);
	await desk.open(api.baseUrl);
	const dialog = await openPositionDialog(page);
	await expect(dialog.getByTestId("pan-circle")).toHaveAttribute("aria-valuenow", "0");
	const held = await holdJoystickRight(page, dialog, 300);
	await expect.poll(async () => (await preloadPans(api))[0] ?? 0).toBeGreaterThan(0);
	await leavePreloadCapture(api, showId);
	return { dialog, held };
}

/** Pan of every Preload Position entry that holds Angles. */
async function preloadPans(api: ApiDriver) {
	return (await preloadValues(api, "position")).flatMap((entry) => {
		const value = entry.value as PositionValue;
		return value.value.kind === "angles" ? [value.value.pan_degrees.value] : [];
	});
}

/** A tile's `active / defined` label after the manual bench clock renders another frame. */
async function tileCount(bench: LightBench, tile: Locator) {
	await bench.tick(25);
	return (await tile.innerText()).replace(/\s+/gu, " ");
}

test.describe("docs/testing/34-position-operator-controls.md", () => {
	test("POSITION-CONTROLS-001 @ui › pages, modal geometry and inert navigation", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "001");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		await desk.open(api.baseUrl);
		const before = await programmerRevision(api);

		await positionFamily(page).click();
		// From scratch the angles read back from the displayed output, marked Resolved.
		await expect(encoder(page, 1, "Pan · Resolved")).toBeVisible();
		await expect(encoder(page, 2, "Tilt · Resolved")).toBeVisible();
		if (await pointOnFirstPage(page)) {
			await expect(encoder(page, 3, "Point")).toBeVisible();
			await expect(encoder(page, 4, "X")).toBeVisible();
		} else {
			await positionFamily(page).click();
			await expect(encoder(page, 1, "Point")).toBeVisible();
			await expect(encoder(page, 2, "X")).toBeVisible();
		}
		expect(await programmerRevision(api)).toBe(before);

		for (const viewport of [
			{ width: 1496, height: 761 },
			{ width: 1024, height: 768 },
			{ width: 760, height: 900 },
		]) {
			await page.setViewportSize(viewport);
			const dialog = await openPositionDialog(page);
			const pan = await dialog.getByTestId("pan-circle").boundingBox();
			const tilt = await dialog.getByRole("slider", { name: "Tilt angle" }).boundingBox();
			const joystick = await dialog.getByTestId("position-joystick").boundingBox();
			expect(pan && tilt && joystick).toBeTruthy();
			if (!pan || !tilt || !joystick) return;
			expect(tilt.y).toBeGreaterThan(pan.y + pan.height - 1);
			expect(Math.abs(joystick.width - joystick.height)).toBeLessThanOrEqual(1);
			expect(joystick.x).toBeGreaterThanOrEqual(pan.x + pan.width - 1);
			for (const name of ["Decrease pan by 90 degrees", "Reset pan to zero", "Increase pan by 90 degrees"])
				await expect(dialog.getByRole("button", { name })).toBeVisible();
			await expect(dialog.getByText("Aim reference")).toHaveCount(0);
			await expect(dialog.getByText(/^(X|Y|Z|Point)$/)).toHaveCount(0);
			await expect(dialog.locator("img, canvas")).toHaveCount(0);
			// Return Home sits below the joystick (POSITION-HOME-001); opening sends nothing.
			const home = await dialog.getByRole("button", { name: "Return Home" }).boundingBox();
			expect(home).toBeTruthy();
			if (home) expect(home.y).toBeGreaterThanOrEqual(joystick.y + joystick.height - 1);
			await page.keyboard.press("Escape");
			await expect(dialog).toBeHidden();
		}
		expect(await programmerRevision(api)).toBe(before);
	});

	test("POSITION-CONTROLS-004 @ui › the held joystick moves without pointer moves and stops on centre, release, blur and close", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "004");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		await desk.open(api.baseUrl);
		const dialog = await openPositionDialog(page);
		const joystick = dialog.getByTestId("position-joystick");
		const box = await joystick.boundingBox();
		if (!box) throw new Error("joystick has no box");
		const centre = { x: box.x + box.width / 2, y: box.y + box.height / 2 };

		const stopped = async (stop: () => Promise<void>) => {
			await page.mouse.move(centre.x, centre.y);
			await page.mouse.down();
			await page.mouse.move(box.x + box.width - 4, centre.y);
			const start = await programmerRevision(api);
			await expect.poll(() => programmerRevision(api)).toBeGreaterThan(start);
			await stop();
			await page.waitForTimeout(150);
			const settled = await programmerRevision(api);
			await page.waitForTimeout(500);
			expect(await programmerRevision(api)).toBe(settled);
		};
		await stopped(async () => page.mouse.move(centre.x, centre.y));
		await page.mouse.up();
		await stopped(async () => page.mouse.up());
		await stopped(async () => {
			await page.evaluate(() => window.dispatchEvent(new Event("blur")));
			await page.mouse.up();
		});
		await stopped(async () => {
			await page.keyboard.press("Escape");
			await page.mouse.up();
		});
		await expect(dialog).toBeHidden();
	});

	test("POSITION-CONTROLS-003 @ui › hardware encode/N edits Pan in degrees and the first X offset activates Target atomically", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "003");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		await desk.open(api.baseUrl);
		await positionFamily(page).click();
		await expect(encoder(page, 1, "Pan · Resolved")).toBeVisible();
		const hardware = await bench.osc();
		await hardware.subscribe(`position-controls-${crypto.randomUUID()}`, "desk");

		// From scratch: the first detent adopts the displayed pose, then moves Pan by exactly 1°.
		await hardware.send("/light/desk/encode/1", ["up"]);
		await expect.poll(() => programmedAngles(api, selected.length)).not.toBeNull();
		for (const angles of (await programmedAngles(api, selected.length)) ?? []) {
			expect(angles.pan).toBeCloseTo(SPOT_HOME_PAN + 1, 3);
			expect(angles.tilt).toBeCloseTo(SPOT_HOME_TILT, 3);
		}
		await hardware.send("/light/desk/encode/1", ["up"]);
		await expect
			.poll(async () => (await programmedAngles(api, selected.length))?.[0]?.pan ?? null)
			.toBeCloseTo(SPOT_HOME_PAN + 2, 3);

		// X is encoder 4 on a sequential 6-encoder page 1, else encoder 2 on page 2.
		let xEncoder = 4;
		if (!(await pointOnFirstPage(page))) {
			await positionFamily(page).click();
			xEncoder = 2;
		}
		const before = await programmerRevision(api);
		await hardware.send(`/light/desk/encode/${xEncoder}`, ["up"]);
		await expect.poll(() => programmerRevision(api)).toBe(before + 1);
		const values = JSON.stringify(await positionValues(api));
		expect(values).toContain('"target"');
		expect(values).toContain('"origin"');
		expect(values).not.toContain('"angles"');
	});

	test("POSITION-CONTROLS-007 @ui › from scratch, the first typed Pan and the dialog adopt the displayed output instead of a silent no change", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "007");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		await desk.open(api.baseUrl);
		expect(await positionValues(api)).toEqual([]);
		await positionFamily(page).click();
		await expect(encoder(page, 1, "Pan · Resolved")).toBeVisible();
		await expect(encoder(page, 2, "Tilt · Resolved")).toBeVisible();

		// An absolute typed Pan on an empty Programmer is accepted and keeps the displayed Tilt.
		const before = await programmerRevision(api);
		await encoder(page, 1, "Pan · Resolved").getByRole("button", { name: /^Set Enc 1 · Pan/ }).click();
		await expect(page.getByRole("dialog", { name: /^Enc 1 · Pan.* value$/ })).toBeVisible();
		await page.keyboard.type("10");
		await page.keyboard.press("Enter");
		await expect.poll(() => programmerRevision(api)).toBeGreaterThan(before);
		await expect.poll(() => programmedAngles(api, selected.length)).not.toBeNull();
		for (const angles of (await programmedAngles(api, selected.length)) ?? []) {
			expect(angles.pan).toBeCloseTo(10, 6);
			expect(angles.tilt).toBeCloseTo(SPOT_HOME_TILT, 3);
		}

		// The dialog edits the programmed Angles directly: +90° is one exact step.
		const dialog = await openPositionDialog(page);
		await expect(dialog.getByTestId("pan-value-caption")).toHaveCount(0);
		const stepped = await programmerRevision(api);
		await dialog.getByRole("button", { name: "Increase pan by 90 degrees" }).click();
		await expect.poll(() => programmerRevision(api)).toBeGreaterThan(stepped);
		await expect
			.poll(async () => (await programmedAngles(api, selected.length))?.every((angles) => Math.abs(angles.pan - 100) < 1e-6) ?? false)
			.toBe(true);
		await expect(page.getByRole("alert")).toHaveCount(0);
	});

	test("POSITION-CONTROLS-008 @ui › fixtures without Position physical data are quietly Unsupported and send nothing", async ({ api, bench, desk, page }) => {
		await saveNoPositionModelProfile(api);
		const { selected } = await arrange({ api, bench, desk, page }, "008", NO_POSITION_MODEL);
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		const sent = recordPositionEdits(page);
		await desk.open(api.baseUrl);
		await positionFamily(page).click();
		await expect(encoder(page, 1, "Pan · Unsupported")).toBeVisible();
		await expect(encoder(page, 1, "Pan · Unsupported")).toHaveAttribute("aria-disabled", "true");
		await expect(encoder(page, 2, "Tilt · Unsupported")).toHaveAttribute("aria-disabled", "true");

		const hardware = await bench.osc();
		await hardware.subscribe(`position-controls-${crypto.randomUUID()}`, "desk");
		const before = await programmerRevision(api);
		await hardware.send("/light/desk/encode/1", ["up"]);
		await hardware.send("/light/desk/encode/2", ["right"]);
		await page.waitForTimeout(400);

		const dialog = await openPositionDialog(page);
		await expect(dialog.getByTestId("pan-value-caption")).toHaveText("Unsupported");
		await expect(dialog.getByTestId("tilt-value-caption")).toHaveText("Unsupported");
		await expect(dialog.getByTestId("position-joystick")).toHaveAttribute("aria-disabled", "true");
		await expect(dialog.getByRole("button", { name: "Increase pan by 90 degrees" })).toBeDisabled();
		await page.waitForTimeout(200);

		expect(sent).toEqual([]);
		expect(await programmerRevision(api)).toBe(before);
		expect(await positionValues(api)).toEqual([]);
		await expect(page.getByRole("alert")).toHaveCount(0);
		await expect(page.getByRole("alertdialog")).toHaveCount(0);
	});

	test("POSITION-CONTROLS-009 @ui › Position preset tiles count the fixtures whose requested Position is the preset", async ({ api, bench, desk, page }) => {
		const { showId, selected } = await arrange({ api, bench, desk, page }, "009");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		await seedPositionPreset(api, showId, 1, "Down", selected, -67.5);
		await seedPositionPreset(api, showId, 2, "Up", selected, 45);
		await desk.open(api.baseUrl);
		const pane = await showPositionPresets(page);
		const down = pane.locator(".preset-card").nth(0);
		const up = pane.locator(".preset-card").nth(1);
		await expect(down).toContainText("Down");
		await expect(up).toContainText("Up");
		await expect.poll(() => tileCount(bench, down)).toContain("0 / 2");
		await expect.poll(() => tileCount(bench, up)).toContain("0 / 2");

		await down.click();
		await expect.poll(async () => (await programmedAngles(api, selected.length))?.every((angles) => angles.tilt === -67.5) ?? false).toBe(true);
		await expect.poll(() => tileCount(bench, down)).toContain("2 / 2");
		// Negative control: a preset the fixtures do not show stays inactive.
		expect(await tileCount(bench, up)).toContain("0 / 2");

		await up.click();
		await expect.poll(async () => (await programmedAngles(api, selected.length))?.every((angles) => angles.tilt === 45) ?? false).toBe(true);
		await expect.poll(() => tileCount(bench, up)).toContain("2 / 2");
		await expect.poll(() => tileCount(bench, down)).toContain("0 / 2");
		await expect(page.getByRole("alert")).toHaveCount(0);
	});

	test("POSITION-CONTROLS-002 @ui › Pan is unwrapped across turns and ±90°/Reset are exact single steps", async ({ api, bench, desk, page }) => {
		const { showId, selected } = await arrange({ api, bench, desk, page }, "002");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		await programAngles(api, showId, selected, 0, 20);
		await desk.open(api.baseUrl);
		let dialog = await openPositionDialog(page);
		await expect(dialog.getByTestId("pan-circle")).toHaveAttribute("aria-valuenow", "0");

		// Clockwise across 0° again and again, to +450°: the value keeps accumulating, never wraps.
		await dragPanCircle(page, dialog, 0, 450);
		await expect.poll(() => uniformPan(api, selected.length)).toBeCloseTo(450, 6);
		await expect(dialog.getByTestId("pan-circle")).toHaveAttribute("aria-valuenow", "450");
		await expect(dialog.getByLabel("Pan turns")).toHaveText("+1.25 turns");
		for (const angles of (await programmedAngles(api, selected.length)) ?? []) expect(angles.tilt).toBeCloseTo(20, 6);

		// Back the other way to −450°: still signed and unwrapped.
		await dragPanCircle(page, dialog, 450, -900);
		await expect.poll(() => uniformPan(api, selected.length)).toBeCloseTo(-450, 6);
		await expect(dialog.getByTestId("pan-circle")).toHaveAttribute("aria-valuenow", "-450");
		await expect(dialog.getByLabel("Pan turns")).toHaveText("-1.25 turns");

		// +90° and −90° each move Pan by exactly 90° and each is one Undo step. The dialog is
		// reopened after each UND: an open dialog does not follow it (see the BUG case below).
		for (const [name, expected] of [
			["Increase pan by 90 degrees", -360],
			["Decrease pan by 90 degrees", -540],
		] as const) {
			await dialog.getByRole("button", { name }).click();
			await expect.poll(() => uniformPan(api, selected.length)).toBeCloseTo(expected, 6);
			await api.sendCommandKey("UND");
			await expect.poll(() => uniformPan(api, selected.length)).toBeCloseTo(-450, 6);
			await page.keyboard.press("Escape");
			await expect(dialog).toBeHidden();
			dialog = await openPositionDialog(page);
			await expect(dialog.getByTestId("pan-circle")).toHaveAttribute("aria-valuenow", "-450");
		}

		// Reset returns Pan to 0° and keeps Tilt; a second Reset makes no programmer revision.
		await dialog.getByRole("button", { name: "Reset pan to zero" }).click();
		await expect.poll(() => uniformPan(api, selected.length)).toBe(0);
		for (const angles of (await programmedAngles(api, selected.length)) ?? []) expect(angles.tilt).toBeCloseTo(20, 6);
		await expect(dialog.getByTestId("pan-circle")).toHaveAttribute("aria-valuenow", "0");
		const settled = await programmerRevision(api);
		await dialog.getByRole("button", { name: "Reset pan to zero" }).click();
		await page.waitForTimeout(400);
		expect(await programmerRevision(api)).toBe(settled);
		await expect(page.getByRole("alert")).toHaveCount(0);
	});

	test("POSITION-CONTROLS-002 @ui › the open dialog follows UND of its own ±90° step", async ({ api, bench, desk, page }) => {
		const { showId, selected } = await arrange({ api, bench, desk, page }, "002-undo");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		await programAngles(api, showId, selected, -450, 20);
		await desk.open(api.baseUrl);
		const dialog = await openPositionDialog(page);
		const circle = dialog.getByTestId("pan-circle");
		await expect(circle).toHaveAttribute("aria-valuenow", "-450");
		await dialog.getByRole("button", { name: "Increase pan by 90 degrees" }).click();
		await expect.poll(() => uniformPan(api, selected.length)).toBeCloseTo(-360, 6);
		await expect(circle).toHaveAttribute("aria-valuenow", "-360");
		await api.sendCommandKey("UND");
		await expect.poll(() => uniformPan(api, selected.length)).toBeCloseTo(-450, 6);
		await expect(circle).toHaveAttribute("aria-valuenow", "-450", { timeout: 2_000 });
		// The next step must start from the undone value: −450° + 90° = −360°, never −270°.
		await dialog.getByRole("button", { name: "Increase pan by 90 degrees" }).click();
		await expect.poll(() => uniformPan(api, selected.length), { timeout: 2_000 }).toBeCloseTo(-360, 6);
	});

	test("POSITION-CONTROLS-006 @ui › in Preload the held joystick authors only into Preload with the Programmer Fade; live output waits for Preload GO", async ({ api, bench, desk, page }) => {
		const { showId, selected } = await arrange({ api, bench, desk, page }, "006");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 2_000 });
		const lanes = recordLaneWrites(page);
		await enterPreloadCapture(api, showId);
		// Production publishes no Pending (Preload) Position readouts yet, so the Preload lane starts
		// from requested Angles rather than the displayed pose (see the adoption BUG case below).
		await seedPreloadValue(api, showId, selected, "position", ANGLES(0, 0));
		await bench.tick(25);
		const live = await liveSlots(api, 40);
		await desk.open(api.baseUrl);

		// POSITION-CONTROLS-004 step 1 in Preload: the held joystick keeps panning, in Preload only.
		const dialog = await openPositionDialog(page);
		await expect(dialog.getByTestId("pan-circle")).toHaveAttribute("aria-valuenow", "0");
		const held = await holdJoystickRight(page, dialog, 600);
		await held.release();
		await expect.poll(async () => (await preloadPans(api))[0] ?? 0).toBeGreaterThan(10);
		expect(new Set(await preloadPans(api)).size).toBe(1);

		// Only the Preload Programmer changed, every write went to the Preload lane, and each Preload
		// value carries the Programmer Fade. Live output is untouched until Preload GO.
		expect(await normalValues(api, "position")).toEqual([]);
		expect(lanes.edits("normal")).toEqual([]);
		expect(lanes.edits("preload", "position").length).toBeGreaterThan(1);
		expect(lanes.finishes("preload", "position")).toHaveLength(1);
		for (const entry of await preloadValues(api, "position")) {
			expect(entry.fade).toBe(true);
			expect(entry.fade_millis).toBe(2_000);
		}
		await bench.tick(100);
		await bench.tick(3_000);
		expect(await liveSlots(api, 40)).toEqual(live);

		await goPreload(api, showId);
		await bench.tick(100);
		await bench.tick(3_000);
		await expect.poll(() => liveSlots(api, 40)).not.toEqual(live);
	});

	test("POSITION-CONTROLS-006 @ui › in Preload the first Tilt edit adopts the displayed pose", async ({ api, bench, desk, page }) => {
		const { showId, selected } = await arrange({ api, bench, desk, page }, "006-adopt");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		const lanes = recordLaneWrites(page);
		await enterPreloadCapture(api, showId);
		await bench.tick(25);
		await desk.open(api.baseUrl);
		const dialog = await openPositionDialog(page);
		const tilt = dialog.getByRole("slider", { name: "Tilt angle" });
		// The Pending lane publishes on the next output frames; the open dialog reads it again.
		await expect
			.poll(async () => {
				await bench.tick(25);
				return tilt.isEnabled();
			}, { timeout: 5_000 })
			.toBe(true);
		await tilt.focus();
		await page.keyboard.press("ArrowUp");
		await expect.poll(async () => (await preloadValues(api, "position")).length, { timeout: 2_000 }).toBe(selected.length);
		for (const entry of await preloadValues(api, "position")) {
			const value = entry.value as PositionValue;
			if (value.value.kind !== "angles") throw new Error("Preload Position is not Angles");
			expect(value.value.pan_degrees.value).toBeCloseTo(SPOT_HOME_PAN, 3);
			expect(value.value.tilt_degrees.value).toBeGreaterThan(SPOT_HOME_TILT);
		}
		expect(lanes.edits("normal")).toEqual([]);
		expect(await normalValues(api, "position")).toEqual([]);
	});

	test("POSITION-CONTROLS-006 @ui › leaving Preload mid-gesture finishes the Preload part; the rest of the held gesture continues on the Normal Programmer", async ({ api, bench, desk, page }) => {
		const { showId, selected } = await arrange({ api, bench, desk, page }, "006-switch");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		const lanes = recordLaneWrites(page);
		const { dialog, held } = await holdInPreloadThenLeave({ api, bench, desk, page }, showId, selected);
		await page.waitForTimeout(500);
		await held.release();

		// Preload changes are atomic (2026-10-05): the gesture's Preload part ends with one Finish
		// keeping what it sent; the rest of the same held motion is a Normal-lane gesture.
		await expect.poll(() => lanes.finishes("preload", "position").length).toBe(1);
		expect(lanes.edits("preload", "position").length).toBeGreaterThan(0);
		await expect.poll(() => lanes.finishes("normal", "position").length).toBe(1);
		expect(lanes.edits("normal", "position").length).toBeGreaterThan(0);
		await expect.poll(() => uniformPan(api, selected.length)).toBeGreaterThan(5);

		// The next gesture also writes to the Normal Programmer only; Preload stays as it was.
		const preloadAfter = JSON.stringify(await preloadValues(api, "position"));
		const before = (await uniformPan(api, selected.length)) ?? 0;
		await dialog.getByRole("button", { name: "Increase pan by 90 degrees" }).click();
		await expect.poll(() => uniformPan(api, selected.length)).toBeCloseTo(before + 90, 6);
		expect(JSON.stringify(await preloadValues(api, "position"))).toBe(preloadAfter);
	});

	test("POSITION-CONTROLS-006 @ui › a held gesture no longer changes Preload after leaving Preload capture", async ({ api, bench, desk, page }) => {
		const { showId, selected } = await arrange({ api, bench, desk, page }, "006-rest");
		requireSemanticContract(await semanticPosition(api, selected), GATE);
		const { held } = await holdInPreloadThenLeave({ api, bench, desk, page }, showId, selected);
		await page.waitForTimeout(200);
		const atSwitch = (await preloadPans(api))[0] ?? 0;
		await page.waitForTimeout(500);
		await held.release();
		// The joystick pans at about 120°/s: the 500 ms after the switch move the Normal Pan.
		await expect.poll(async () => (await uniformPan(api, selected.length)) ?? 0, { timeout: 2_000 }).toBeGreaterThan(20);
		expect((await preloadPans(api))[0] ?? 0).toBeCloseTo(atSwitch, 0);
	});
});
