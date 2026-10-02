import type { Page } from "@playwright/test";
import { replaceProgrammingSelection } from "./bench/command-selection/programmingSelection";
import type { ApiDriver } from "./bench/core/api";
import type { DeskDriver } from "./bench/core/desk";
import { expect, test } from "./bench/core/fixtures";
import type { LightBench } from "./bench/core/lightBench";
import { requireSemanticContract } from "./bench/core/semanticContract";
import { BrowserPatch } from "./bench/show-setup/patchScenario";

/**
 * docs/testing/35-focus-zoom-operator-controls.md (TL-551): the production Focus Special Dialog and
 * the Focus family encoders under the semantic programming contract.
 *
 * GATE: the production runtime reports programming contract 0
 * (`crates/light/adapters/headless/src/runtime/e2e_semantic_contract.rs`), so the family pages
 * route answers `semantic: false`. Under `npm run test:e2e` these cases skip; under
 * `npm run test:e2e-semantic` they run on the contract-1 E2E test server, where a missing semantic
 * publication fails instead of skipping.
 *
 * The Beam rig is two Cameo AURO SPOT Z300: its profile declares the Beam convention from the
 * user manual (TL-637) with 10–25° selection limits, so FOCUS-ZOOM-002 to 005 run on it. The
 * unknown-convention rig adds a ROBE Robin DLS Profile, whose Zoom has no degree mapping and no
 * convention, to one AURO: the selection then shares no convention (FOCUS-ZOOM-007 and 009).
 *
 * Every case starts from scratch: no Zoom is programmed through the API. The first dialog Zoom
 * step adopts the displayed output's opening in degrees (TL-637 follow-up), and key steps are
 * pressed at operator pace, the next one while the previous step is still settling.
 */

const GATE =
	"semantic programming contract is not enabled on this runtime (production contract 0; run npm run test:e2e-semantic)";

const SPOT = { manufacturer: "Cameo", profile: "AURO SPOT Z300", mode: "20-Channel" } as const;
const DLS = { manufacturer: "ROBE", profile: "Robin DLS Profile", mode: "Mode 2" } as const;
/** Two Beam-convention spots, or one spot plus a profile without any Zoom convention. */
const BEAM_RIG = [SPOT, SPOT] as const;
const MIXED_RIG = [SPOT, DLS] as const;

interface FocusRig {
	showId: string;
	selected: string[];
}

async function arrange(
	{ api, bench, desk, page }: { api: ApiDriver; bench: LightBench; desk: DeskDriver; page: Page },
	label: string,
	rig: readonly (typeof SPOT | typeof DLS)[] = MIXED_RIG,
): Promise<FocusRig> {
	await api.request("PUT", "/api/v2/configuration", { programmer_fade_millis: 0 });
	const show = await api.createShow<{ id: string }>({ name: `FOCUS-ZOOM ${label} ${crypto.randomUUID()}` });
	await api.openShow(show.id, { transition: "hold_current" });
	const patch = new BrowserPatch(api, page, desk);
	for (const [index, profile] of rig.entries())
		await patch.via.api.add({ number: index + 1, name: `Head ${index + 1}`, ...profile, address: `1.${index * 64 + 1}` });
	const selected = (await api.patch()).fixtures.map((fixture) => fixture.fixture_id);
	await replaceProgrammingSelection(api, { surface: "api", showId: show.id, fixtures: selected });
	await bench.tick(25);
	return { showId: show.id, selected };
}

/** Programmed Zoom opening angles in degrees, in fixture order. */
function zoomDegrees(values: unknown[]) {
	return values.map((value) => {
		const zoom = value as { value?: { opening_degrees?: { value?: number } } };
		return Math.round((zoom.value?.opening_degrees?.value ?? Number.NaN) * 1000) / 1000;
	});
}

async function semanticFocus(api: ApiDriver, fixtureIds: readonly string[]) {
	const pages = await api
		.request<{ semantic: boolean; families: Array<{ family: string }> }>(
			"GET",
			`/api/v2/programming/family-encoder-pages?fixture_ids=${fixtureIds.join(",")}`,
		)
		.catch(() => null);
	return Boolean(pages?.semantic && pages.families.some((group) => group.family === "focus"));
}

async function programmerRevision(api: ApiDriver) {
	const snapshot = await api.request<{ projection: { revision: number } }>(
		"GET",
		"/api/v2/programmer/values/snapshot",
	);
	return snapshot.projection.revision;
}

async function familyValues(api: ApiDriver, attribute: string) {
	const snapshot = await api.request<{
		projection: { fixture_values?: Array<{ attribute: string; value: unknown }> };
	}>("GET", "/api/v2/programmer/values/snapshot");
	return (snapshot.projection.fixture_values ?? [])
		.filter((entry) => entry.attribute === attribute)
		.map((entry) => entry.value);
}

function encoder(page: Page, slot: number, name: string) {
	return page.getByRole("group", { name: `Enc ${slot} · ${name}`, exact: true });
}

async function openFocusDialog(page: Page) {
	await page.getByRole("button", { name: "Focus", exact: true }).click();
	await page.getByRole("button", { name: "Special Dialog", exact: true }).click();
	const dialog = page.getByRole("dialog", { name: "Focus Special Dialog" });
	await expect(dialog).toBeVisible();
	// The controls are disabled until the lane and its requested values are loaded.
	await expect(dialog.getByRole("slider", { name: "Focus position" })).not.toHaveAttribute("aria-disabled", "true");
	return dialog;
}

test.describe("docs/testing/35-focus-zoom-operator-controls.md", () => {
	test("FOCUS-ZOOM-001 @ui › the Special Dialog opens directly as a modal and closing it sends nothing", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "001");
		requireSemanticContract(await semanticFocus(api, selected), GATE);
		await desk.open(api.baseUrl);
		const before = await programmerRevision(api);

		for (const close of ["button", "Escape"] as const) {
			const dialog = await openFocusDialog(page);
			await expect(dialog.getByRole("heading", { name: "Focus" })).toBeVisible();
			await expect(dialog.getByRole("slider", { name: "Focus position" })).toBeVisible();
			await expect(dialog.locator("img, canvas")).toHaveCount(0);
			await expect(dialog.getByRole("button", { name: /Expand|Encoders/ })).toHaveCount(0);
			if (close === "button")
				await dialog.getByRole("button", { name: "Close Focus Special Dialog" }).click();
			else await page.keyboard.press("Escape");
			await expect(dialog).toBeHidden();
			await expect(encoder(page, 1, "Focus")).toBeVisible();
		}
		expect(await programmerRevision(api)).toBe(before);
	});

	test("FOCUS-ZOOM-007 @ui › an unknown Zoom convention stays quiet in the dialog while Focus still works", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "007");
		requireSemanticContract(await semanticFocus(api, selected), GATE);
		await desk.open(api.baseUrl);
		const dialog = await openFocusDialog(page);

		const zoom = dialog.getByRole("slider", { name: "Zoom opening angle" });
		await expect(zoom).toBeVisible();
		await expect(dialog.getByTestId("focus-zoom-zoom-status")).toHaveText("Requested · unsupported");
		const before = await programmerRevision(api);
		await zoom.focus();
		await page.keyboard.press("ArrowUp");
		await page.keyboard.press("PageUp");
		await page.waitForTimeout(300);
		expect(await programmerRevision(api)).toBe(before);
		expect(await familyValues(api, "zoom")).toEqual([]);
		await expect(page.getByRole("alert")).toHaveCount(0);
		await expect(page.getByRole("alertdialog")).toHaveCount(0);

		await dialog.getByRole("slider", { name: "Focus position" }).focus();
		await page.keyboard.press("ArrowUp");
		await expect.poll(() => programmerRevision(api)).toBeGreaterThan(before);
		expect(await familyValues(api, "focus")).toHaveLength(selected.length);
		expect(await familyValues(api, "zoom")).toEqual([]);
	});

	test("FOCUS-ZOOM-009 @ui › Focus, Zoom page order; hardware encode/N moves Focus 1% and never sends an unknown-convention Zoom", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "009");
		requireSemanticContract(await semanticFocus(api, selected), GATE);
		// The desk must not even send a Zoom edit it cannot express (not merely see it refused).
		const zoomEditsSent: string[] = [];
		page.on("websocket", (socket) =>
			socket.on("framesent", ({ payload }) => {
				const text = String(payload);
				if (text.includes('"component":{"kind":"zoom"}')) zoomEditsSent.push(text);
			}),
		);
		await desk.open(api.baseUrl);
		await page.getByRole("button", { name: "Focus", exact: true }).click();
		await expect(encoder(page, 1, "Focus")).toBeVisible();
		await expect(encoder(page, 2, "Zoom · Unsupported")).toBeVisible();
		await expect(encoder(page, 2, "Zoom · Unsupported")).toHaveAttribute("aria-disabled", "true");

		const hardware = await bench.osc();
		await hardware.subscribe(`focus-zoom-${crypto.randomUUID()}`, "desk");
		const before = await programmerRevision(api);
		await hardware.send("/light/desk/encode/2", ["up"]);
		await hardware.send("/light/desk/encode/2", ["right"]);
		await page.waitForTimeout(400);
		expect(await programmerRevision(api)).toBe(before);
		expect(await familyValues(api, "zoom")).toEqual([]);
		expect(zoomEditsSent).toEqual([]);

		await hardware.send("/light/desk/encode/1", ["up"]);
		await expect.poll(async () => (await familyValues(api, "focus")).length).toBe(selected.length);
		// One fine detent is one descriptor step (1%) from the unprogrammed lens travel 0%.
		for (const value of await familyValues(api, "focus"))
			expect((value as { kind: string; value: number }).value).toBeCloseTo(0.01, 6);
		expect(await familyValues(api, "zoom")).toEqual([]);
		expect(zoomEditsSent).toEqual([]);
	});

	test("FOCUS-ZOOM-002 @ui › Beam handles: a press without movement sends nothing, a drag widens in degrees within the selection limits", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "002", BEAM_RIG);
		requireSemanticContract(await semanticFocus(api, selected), GATE);
		await desk.open(api.baseUrl);
		const dialog = await openFocusDialog(page);
		const zoom = dialog.getByRole("slider", { name: "Beam opening angle" });
		await expect(zoom).toHaveAttribute("aria-valuemin", "10");
		await expect(zoom).toHaveAttribute("aria-valuemax", "25");
		await expect(dialog.getByText(/48/)).toHaveCount(0);

		// Program a known Zoom from scratch in the dialog: five quick 1° steps up from the 10° limit.
		// The first adopts the displayed output's opening; none waits for the previous to settle.
		await zoom.focus();
		for (let step = 0; step < 5; step += 1) await page.keyboard.press("ArrowUp");
		await bench.tick(25);
		await expect.poll(async () => zoomDegrees(await familyValues(api, "zoom"))).toEqual([15, 15]);

		// The readout shows the convention and the programmed angle; grab the upper handle off-centre.
		await expect(zoom).toHaveAttribute("aria-valuenow", "15");
		await expect(dialog.getByTestId("focus-zoom-zoom-status")).not.toHaveText(/unsupported/);
		const handle = dialog.getByTestId("beam-angle-handle-upper");
		const box = await handle.boundingBox();
		if (!box) throw new Error("upper beam handle has no box");
		const grab = { x: box.x + box.width / 2 + 6, y: box.y + box.height / 2 + 6 };
		const before = await programmerRevision(api);
		await page.mouse.move(grab.x, grab.y);
		await page.mouse.down();
		await page.mouse.up();
		await page.waitForTimeout(250);
		expect(await programmerRevision(api)).toBe(before);

		await page.mouse.move(grab.x, grab.y);
		await page.mouse.down();
		for (let step = 1; step <= 5; step += 1) await page.mouse.move(grab.x, grab.y - step * 8);
		await page.mouse.up();
		await expect.poll(async () => zoomDegrees(await familyValues(api, "zoom"))[0] ?? 0).toBeGreaterThan(15);
		for (const degrees of zoomDegrees(await familyValues(api, "zoom"))) {
			expect(degrees).toBeGreaterThanOrEqual(10);
			expect(degrees).toBeLessThanOrEqual(25);
		}
		expect(await familyValues(api, "focus")).toEqual([]);
		// The status may report that no achieved Zoom is read back, but never a refusal.
		await expect(dialog.getByTestId("focus-zoom-zoom-status")).not.toHaveText(/unsupported/);
	});

	test("FOCUS-ZOOM-003 @ui › the focus plane moves Focus in percent and leaves Zoom alone", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "003", BEAM_RIG);
		requireSemanticContract(await semanticFocus(api, selected), GATE);
		await desk.open(api.baseUrl);
		const dialog = await openFocusDialog(page);
		const plane = dialog.getByTestId("focus-position-drag");
		const box = await plane.boundingBox();
		if (!box) throw new Error("focus plane has no box");
		expect(box.width).toBeGreaterThanOrEqual(44);
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		for (let step = 1; step <= 5; step += 1) await page.mouse.move(box.x + box.width / 2 + step * 20, box.y + box.height / 2);
		await page.mouse.up();
		await expect.poll(async () => (await familyValues(api, "focus")).length).toBe(selected.length);
		for (const value of await familyValues(api, "focus")) {
			const focus = (value as { value: number }).value;
			expect(focus).toBeGreaterThan(0);
			expect(focus).toBeLessThanOrEqual(1);
			expect(Math.abs(focus * 100 - Math.round(focus * 100))).toBeLessThan(1e-6);
		}
		expect(await familyValues(api, "zoom")).toEqual([]);
	});

	test("FOCUS-ZOOM-004 @ui › both angle handles stay reachable at the far focus end and the narrowest Zoom", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "004", BEAM_RIG);
		requireSemanticContract(await semanticFocus(api, selected), GATE);
		await desk.open(api.baseUrl);
		const dialog = await openFocusDialog(page);
		const focusSlider = dialog.getByRole("slider", { name: "Focus position" });
		const zoomSlider = dialog.getByRole("slider", { name: "Beam opening angle" });
		// From scratch: Focus to 100%, then Zoom to the widest and straight back to the narrowest
		// limit, each key pressed while the previous step is still settling.
		await focusSlider.focus();
		await page.keyboard.press("End");
		await zoomSlider.focus();
		await page.keyboard.press("End");
		await page.keyboard.press("Home");
		await bench.tick(25);
		await expect(focusSlider).toHaveAttribute("aria-valuenow", "100");
		await expect(zoomSlider).toHaveAttribute("aria-valuenow", "10");
		await expect.poll(async () => zoomDegrees(await familyValues(api, "zoom"))).toEqual([10, 10]);
		await expect.poll(async () => (await familyValues(api, "focus")).map((v) => (v as { value: number }).value)).toEqual([1, 1]);

		for (const side of ["upper", "lower"] as const) {
			const box = await dialog.getByTestId(`beam-angle-handle-${side}`).boundingBox();
			if (!box) throw new Error(`${side} handle has no box`);
			const centre = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
			const hit = await page.evaluate(
				({ x, y }) => document.elementFromPoint(x, y)?.closest("[data-angle-handle]")?.getAttribute("data-testid") ?? null,
				centre,
			);
			expect(hit).toBe(`beam-angle-handle-${side}`);
		}
		const box = await dialog.getByTestId("beam-angle-handle-lower").boundingBox();
		if (!box) throw new Error("lower handle has no box");
		const before = await programmerRevision(api);
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		for (let step = 1; step <= 5; step += 1) await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2 + step * 8);
		await page.mouse.up();
		await expect.poll(() => programmerRevision(api)).toBeGreaterThan(before);
		expect(zoomDegrees(await familyValues(api, "zoom"))[0]).toBeGreaterThan(10);
		expect((await familyValues(api, "focus")).map((v) => (v as { value: number }).value)).toEqual([1, 1]);
	});

	test("FOCUS-ZOOM-005 @ui › keyboard steps: 1° and 10° Zoom, 1% and 10% Focus, Home/End reach the limits", async ({ api, bench, desk, page }) => {
		const { selected } = await arrange({ api, bench, desk, page }, "005", BEAM_RIG);
		requireSemanticContract(await semanticFocus(api, selected), GATE);
		await desk.open(api.baseUrl);
		const dialog = await openFocusDialog(page);
		const zoom = dialog.getByRole("slider", { name: "Beam opening angle" });
		// Nothing is programmed: the dialog starts at the 10° limit and the first step adopts the
		// displayed output's opening.
		await expect(zoom).toHaveAttribute("aria-valuenow", "10");
		await zoom.focus();
		// Each step waits until the dialog shows the accepted value, as an operator reads it; the
		// next key follows at once, while the previous step's Finish is still settling.
		const zoomSteps: Array<[string, number]> = [["ArrowUp", 11], ["Shift+ArrowUp", 21], ["PageDown", 11], ["ArrowDown", 10], ["End", 25], ["Home", 10]];
		for (const [key, expected] of zoomSteps) {
			const before = await programmerRevision(api);
			await page.keyboard.press(key);
			// The bench clock is manual: let the output accept a frame, as a running desk does.
			await bench.tick(25);
			await expect.poll(async () => zoomDegrees(await familyValues(api, "zoom"))).toEqual([expected, expected]);
			await expect(zoom).toHaveAttribute("aria-valuenow", String(expected));
			expect(await programmerRevision(api)).toBe(before + 1);
		}
		for (const value of await familyValues(api, "zoom"))
			expect(value).toMatchObject({ kind: "zoom", value: { convention: "beam" } });
		// Three keys pressed faster than the desk answers: three steps, three Undo steps, no loss.
		const burst = await programmerRevision(api);
		for (let step = 0; step < 3; step += 1) await page.keyboard.press("ArrowUp");
		await bench.tick(25);
		await expect.poll(async () => zoomDegrees(await familyValues(api, "zoom"))).toEqual([13, 13]);
		await expect(zoom).toHaveAttribute("aria-valuenow", "13");
		expect(await programmerRevision(api)).toBe(burst + 3);
		await expect(dialog.getByTestId("focus-zoom-zoom-status")).not.toHaveText(/unsupported/);

		const focus = dialog.getByRole("slider", { name: "Focus position" });
		await focus.focus();
		const focusSteps: Array<[string, number]> = [["ArrowUp", 0.01], ["PageUp", 0.11], ["Shift+ArrowDown", 0.01], ["End", 1], ["Home", 0]];
		for (const [key, expected] of focusSteps) {
			const before = await programmerRevision(api);
			await page.keyboard.press(key);
			await bench.tick(25);
			await expect
				.poll(async () => (await familyValues(api, "focus")).map((v) => Math.round((v as { value: number }).value * 1000) / 1000))
				.toEqual([expected, expected]);
			await expect(focus).toHaveAttribute("aria-valuenow", String(Math.round(expected * 100)));
			expect(await programmerRevision(api)).toBe(before + 1);
		}
		await expect(page.getByRole("alert")).toHaveCount(0);
	});
});
