import { test } from "./fixtures";
import { BrowserScenarioWorld } from "./browserScenario";

export type ScenarioCallback = (world: BrowserScenarioWorld) => Promise<void> | void;

/** Registers one browser-backed operator scenario on the isolated Light bench. */
export function scenario(id: string, title: string, callback: ScenarioCallback): void {
	if (!/^[A-Z][A-Z0-9-]+$/.test(id)) throw new Error(`Scenario id "${id}" must be uppercase kebab-case`);
	test(`${id} @bench @ui › ${title}`, async ({ bench, desk, page }, testInfo) => {
		testInfo.setTimeout(90_000);
		page.setDefaultTimeout(10_000);
		const world = new BrowserScenarioWorld(page, desk, bench, testInfo);
		await callback(world);
	});
}
