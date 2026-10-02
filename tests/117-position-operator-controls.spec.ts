import type { Locator, Page } from "@playwright/test";
import { replaceProgrammingSelection } from "./bench/command-selection/programmingSelection";
import type { ApiDriver } from "./bench/core/api";
import type { DeskDriver } from "./bench/core/desk";
import { expect, test } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import { requireSemanticContract } from "./bench/core/semanticContract";
import { BrowserPatch } from "./bench/show-setup/patchScenario";

/**
 * docs/testing/34-position-operator-controls.md (TL-549, TL-637): the production Position encoders
 * and the modal Position Special Dialog under the semantic programming contract.
 *
 * GATE: the production runtime still reports programming contract 0
 * (`crates/light/adapters/headless/src/runtime/e2e_semantic_contract.rs`), so
 * `GET /api/v2/programming/family-encoder-pages` answers `semantic: false` and the desk keeps the
 * legacy normalized Position pages and dialog. Under `npm run test:e2e` every test below skips
 * with this reason. `npm run test:e2e-semantic` runs them on the contract-1 E2E test server,
 * where a missing semantic publication fails instead of skipping.
 *
 * Every case starts from scratch: a fresh show patched from the shipped library and an empty
 * Programmer. The rig is two Cameo AURO SPOT Z300, whose profile binds Pan/Tilt to a nominal
 * Position physical graph (TL-637), so the displayed output seeds the first Position edit. The
 * unsupported case uses two Claypaky Sharpy, whose profile has no lens geometry and therefore no
 * Position physical data.
 */

const GATE =
	"semantic programming contract is not enabled on this runtime (production contract 0; run npm run test:e2e-semantic)";

const SPOT = { manufacturer: "Cameo", profile: "AURO SPOT Z300", mode: "20-Channel", footprint: 20 } as const;
/**
 * A Tilt-only fixture: one axis cannot form the complete Pan/Tilt Position family, so it gets no
 * derived nominal Position model (TL-552) and stays Unsupported. Movers with Pan and Tilt but no
 * authored Position data, such as the Sharpy, are now programmed through that derived model.
 */
const TILT_ONLY = { manufacturer: "GLP", profile: "JDC1", mode: "Easy 11-channel", footprint: 11 } as const;
type Mover = typeof SPOT | typeof TILT_ONLY;

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
		const { selected } = await arrange({ api, bench, desk, page }, "008", TILT_ONLY);
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
});
