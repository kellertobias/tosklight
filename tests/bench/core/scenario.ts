import { BrowserScenarioWorld } from "./browserScenario";
import { test } from "./fixtures";

export type ScenarioCallback = (
	world: BrowserScenarioWorld,
) => Promise<void> | void;

export interface ScenarioOptions {
	tags?: readonly `@${string}`[];
}

/** Registers one browser-backed operator scenario on the isolated Light bench. */
export function scenario(
	id: string,
	title: string,
	callback: ScenarioCallback,
	options: ScenarioOptions = {},
): void {
	if (!/^[A-Z][A-Z0-9-]+$/.test(id))
		throw new Error(`Scenario id "${id}" must be uppercase kebab-case`);
	const tags = options.tags?.length ? ` ${options.tags.join(" ")}` : "";
	test(`${id} @bench @ui${tags} › ${title}`, async ({
		api,
		bench,
		desk,
		page,
		show,
	}, testInfo) => {
		testInfo.setTimeout(90_000);
		page.setDefaultTimeout(10_000);
		const world = new BrowserScenarioWorld(
			page,
			desk,
			bench,
			api,
			show,
			testInfo,
		);
		let failure: unknown;
		try {
			await callback(world);
		} catch (reason) {
			failure = reason;
			throw reason;
		} finally {
			await world.finish(failure);
		}
	});
}
