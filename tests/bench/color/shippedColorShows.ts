import fs from "node:fs/promises";
import type { Page } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import { expect } from "../core/fixtures";
import { selectFixtures } from "./semanticColorScenario";
import { colorFamily, showColor } from "./directColorRig";

/**
 * TL-653: the shipped and existing shows the Color encoder readouts are verified in.
 *
 * - **Existing**: `tests/fixtures/default-stage.show`, a Default Stage Show saved by an earlier
 *   build, uploaded and opened as it is.
 * - **Default**: the packaged default (`assets/demo.show`), the show a fresh desk starts with.
 * - **Clean Built-in Default**: loaded through the operator's New Show dialog.
 */

export type ShippedShow = "existing" | "default" | "clean";

interface ActiveShow {
	id: string;
	name: string;
}

async function upload(api: ApiDriver, file: URL, name: string) {
	const bytes = await fs.readFile(file);
	const show = await api.createShow<ActiveShow>({
		name: `${name}-${crypto.randomUUID()}`,
		data_base64: bytes.toString("base64"),
		overwrite: false,
	});
	await api.openShow(show.id);
	return show;
}

async function active(api: ApiDriver): Promise<ActiveShow> {
	const bootstrap = await api.request<{ active_show: ActiveShow | null }>("GET", "/api/v2/bootstrap");
	if (!bootstrap.active_show) throw new Error("No show is active");
	return bootstrap.active_show;
}

/** Loads one shipped show; `clean` goes through the open desk's New Show dialog. */
export async function loadShippedShow(api: ApiDriver, page: Page, which: ShippedShow): Promise<ActiveShow> {
	if (which === "existing")
		return upload(api, new URL("../../fixtures/default-stage.show", import.meta.url), "existing-default-stage");
	if (which === "default")
		return upload(api, new URL("../../../assets/demo.show", import.meta.url), "packaged-default");
	const before = (await active(api)).id;
	const menu = page.getByRole("dialog", { name: "Show", exact: true });
	if (!(await menu.isVisible())) await page.getByRole("button", { name: /Open show menu/ }).click();
	await menu.getByRole("button", { name: "New Show", exact: true }).click();
	await page
		.getByRole("dialog", { name: "New show", exact: true })
		.getByRole("button", { name: "Load Clean Built-in Default", exact: true })
		.click();
	await expect.poll(async () => (await active(api)).id).not.toBe(before);
	if (await menu.isVisible()) await page.getByRole("button", { name: "Close Show", exact: true }).click();
	await expect(menu).toBeHidden();
	const shown = await active(api);
	expect(shown.name).toMatch(/^Default Stage Show Clean Copy/);
	return shown;
}

/** Selects the patched fixtures with these numbers, in this order. */
export async function selectNumbers(api: ApiDriver, showId: string, numbers: readonly number[]) {
	const patch = await api.patch();
	const ids = numbers.map((number) => {
		const fixture = patch.fixtures.find((entry) => entry.fixture_number === number);
		if (!fixture) throw new Error(`fixture ${number} is not patched`);
		return fixture.fixture_id;
	});
	await selectFixtures(api, showId, ids);
	return ids;
}

export interface EncoderReading {
	name: string;
	value: string;
}

/** The visible software encoders of the lower area: accessible name and shown value. */
export async function encoderReadings(page: Page): Promise<EncoderReading[]> {
	const groups = page.locator(".parameter-surfaces").getByRole("group", { name: /^Enc \d+ · / });
	return groups.evaluateAll((all) =>
		all.map((group) => ({
			name: group.getAttribute("aria-label") ?? "",
			value: (group.querySelector(".touch-encoder-value")?.textContent ?? "").trim(),
		})),
	);
}

/** Pages the Color family to the first page whose encoders name the reference head `number`. */
export async function directColorPage(page: Page, number: number) {
	await showColor(page);
	const family = colorFamily(page);
	const named = new RegExp(` · ${number}\\b`);
	for (let attempt = 0; attempt < 8; attempt += 1) {
		if ((await encoderReadings(page)).some((reading) => named.test(reading.name))) return;
		await family.click();
	}
	throw new Error(`no Color page names the reference head ${number}`);
}
